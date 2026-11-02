use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;

use crate::{Error, ExpectedHash, Result, file_matches};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadOutcome {
    /// The file was fetched and written.
    Downloaded,
    /// A file with the expected hash was already on disk; nothing was fetched.
    AlreadyPresent,
}

/// Downloads files with a global concurrency limit, retries and hash checks.
///
/// Files are written to `<dest>.part` and renamed into place only after the hash
/// matches, so a crash or a bad mirror never leaves a corrupt file at `dest`.
#[derive(Clone)]
pub struct Downloader {
    client: reqwest::Client,
    permits: Arc<Semaphore>,
    attempts: u32,
    retry_delay: Duration,
}

impl Downloader {
    pub fn new(client: reqwest::Client, max_concurrent: usize) -> Self {
        Self {
            client,
            permits: Arc::new(Semaphore::new(max_concurrent.max(1))),
            // Generous on purpose: many players go through VPNs and local proxies
            // that drop connections when a lot of them open at once.
            attempts: 5,
            retry_delay: Duration::from_secs(1),
        }
    }

    /// Overrides the retry policy (mostly useful in tests).
    pub fn with_retries(mut self, attempts: u32, delay: Duration) -> Self {
        self.attempts = attempts.max(1);
        self.retry_delay = delay;
        self
    }

    pub async fn download(
        &self,
        url: &str,
        dest: &Path,
        expected: Option<&ExpectedHash>,
    ) -> Result<DownloadOutcome> {
        if let Some(hash) = expected
            && file_matches(dest, hash).await
        {
            return Ok(DownloadOutcome::AlreadyPresent);
        }

        let _permit = self.permits.acquire().await.expect("semaphore is never closed");

        let mut attempt = 1;
        loop {
            match self.fetch_once(url, dest, expected).await {
                Ok(()) => return Ok(DownloadOutcome::Downloaded),
                Err(err) if attempt < self.attempts && is_retryable(&err) => {
                    tracing::warn!(%url, attempt, error = %err, "download failed, retrying");
                    tokio::time::sleep(self.retry_delay * attempt).await;
                    attempt += 1;
                }
                Err(err) => return Err(err),
            }
        }
    }

    async fn fetch_once(&self, url: &str, dest: &Path, expected: Option<&ExpectedHash>) -> Result<()> {
        let http = |source| Error::Http { url: url.to_owned(), source };

        let response = self.client.get(url).send().await.map_err(http)?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::Status { url: url.to_owned(), status: status.as_u16() });
        }

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| io(parent, e))?;
        }
        let part = part_path(dest);
        let mut file = tokio::fs::File::create(&part).await.map_err(|e| io(&part, e))?;
        let mut hasher = expected.map(ExpectedHash::hasher);

        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(source) => {
                    drop(file);
                    let _ = tokio::fs::remove_file(&part).await;
                    return Err(http(source));
                }
            };
            if let Some(h) = hasher.as_mut() {
                h.update(&chunk);
            }
            file.write_all(&chunk).await.map_err(|e| io(&part, e))?;
        }
        file.flush().await.map_err(|e| io(&part, e))?;
        drop(file);

        if let (Some(h), Some(expected)) = (hasher, expected) {
            let actual = h.finish_hex();
            if !actual.eq_ignore_ascii_case(expected.hex()) {
                let _ = tokio::fs::remove_file(&part).await;
                return Err(Error::HashMismatch {
                    url: url.to_owned(),
                    expected: expected.hex().to_owned(),
                    actual,
                });
            }
        }

        tokio::fs::rename(&part, dest).await.map_err(|e| io(dest, e))
    }
}

fn is_retryable(err: &Error) -> bool {
    match err {
        Error::Http { .. } | Error::HashMismatch { .. } => true,
        Error::Status { status, .. } => *status == 429 || *status >= 500,
        Error::Io { .. } => false,
    }
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io { path: path.to_owned(), source }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const HELLO_SHA1: &str = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d";

    fn downloader() -> Downloader {
        Downloader::new(crate::http_client(), 4).with_retries(3, Duration::from_millis(1))
    }

    #[tokio::test]
    async fn downloads_and_verifies() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/hello.txt"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes("hello"))
            .expect(1)
            .mount(&server)
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("nested/dir/hello.txt");
        let url = format!("{}/hello.txt", server.uri());
        let hash = ExpectedHash::Sha1(HELLO_SHA1.into());

        let outcome = downloader().download(&url, &dest, Some(&hash)).await.unwrap();
        assert_eq!(outcome, DownloadOutcome::Downloaded);
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello");
        assert!(!part_path(&dest).exists());

        // Second call finds the verified file and makes no request (expect(1) above).
        let outcome = downloader().download(&url, &dest, Some(&hash)).await.unwrap();
        assert_eq!(outcome, DownloadOutcome::AlreadyPresent);
    }

    #[tokio::test]
    async fn hash_mismatch_leaves_no_file() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes("tampered"))
            .mount(&server)
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("hello.txt");
        let err = downloader()
            .download(&server.uri(), &dest, Some(&ExpectedHash::Sha1(HELLO_SHA1.into())))
            .await
            .unwrap_err();

        assert!(matches!(err, Error::HashMismatch { .. }), "{err}");
        assert!(!dest.exists());
        assert!(!part_path(&dest).exists());
    }

    #[tokio::test]
    async fn retries_server_errors_then_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(2)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes("hello"))
            .mount(&server)
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("hello.txt");
        let outcome = downloader().download(&server.uri(), &dest, None).await.unwrap();
        assert_eq!(outcome, DownloadOutcome::Downloaded);
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello");
    }

    #[tokio::test]
    async fn does_not_retry_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&server)
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let err = downloader().download(&server.uri(), &tmp.path().join("x"), None).await.unwrap_err();
        assert!(matches!(err, Error::Status { status: 404, .. }), "{err}");
    }
}
