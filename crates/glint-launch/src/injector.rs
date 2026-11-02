//! authlib-injector: the Java agent that points the game's authentication and
//! skins at an alternative server such as Ely.by.

use std::path::{Path, PathBuf};

use glint_net::{Downloader, ExpectedHash};
use serde::Deserialize;

use crate::{Error, Result, io};

pub const LATEST_URL: &str = "https://authlib-injector.yushi.moe/artifact/latest.json";

#[derive(Debug, Deserialize)]
struct Latest {
    version: String,
    download_url: String,
    checksums: Checksums,
}

#[derive(Debug, Deserialize)]
struct Checksums {
    sha256: String,
}

/// Downloads the latest authlib-injector into `dir` (verified by SHA-256) and
/// returns the jar path. Offline, falls back to the newest jar already there.
pub async fn ensure(
    http: &reqwest::Client,
    downloader: &Downloader,
    dir: &Path,
    latest_url: &str,
) -> Result<PathBuf> {
    match fetch_latest(http, latest_url).await {
        Ok(latest) => {
            let jar = dir.join(format!("authlib-injector-{}.jar", latest.version));
            downloader
                .download(
                    &latest.download_url,
                    &jar,
                    Some(&ExpectedHash::Sha256(latest.checksums.sha256)),
                )
                .await
                .map_err(|e| Error::Injector(e.to_string()))?;
            Ok(jar)
        }
        Err(err) => {
            tracing::warn!(error = %err, "authlib-injector release info unavailable, using a cached jar");
            newest_cached(dir).await?.ok_or(err)
        }
    }
}

async fn fetch_latest(http: &reqwest::Client, url: &str) -> Result<Latest> {
    let fail =
        |e: reqwest::Error| Error::Injector(format!("could not check for authlib-injector: {e}"));
    let response = http
        .get(url)
        .send()
        .await
        .map_err(fail)?
        .error_for_status()
        .map_err(fail)?;
    response.json().await.map_err(fail)
}

async fn newest_cached(dir: &Path) -> Result<Option<PathBuf>> {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(dir, e)),
    };
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    while let Some(entry) = entries.next_entry().await.map_err(|e| io(dir, e))? {
        let path = entry.path();
        let is_jar = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("authlib-injector-") && n.ends_with(".jar"));
        if !is_jar {
            continue;
        }
        let modified = entry
            .metadata()
            .await
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        if newest.as_ref().is_none_or(|(t, _)| modified > *t) {
            newest = Some((modified, path));
        }
    }
    Ok(newest.map(|(_, p)| p))
}

/// The JVM argument that loads the agent for `target` (for example `ely.by`).
pub fn javaagent_arg(jar: &Path, target: &str) -> String {
    format!("-javaagent:{}={target}", jar.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn downloads_verified_jar_and_reuses_it_offline() {
        let server = MockServer::start().await;
        // sha256("agent")
        let sha = "1e4b4f8b42a7c4f8d48d8f6d9d6a1a52fb7a7f0b0d8b6a1e1c5e3b5f0d1e2a3b";
        let body = serde_json::json!({
            "version": "1.2.8",
            "download_url": format!("{}/agent.jar", server.uri()),
            "checksums": {"sha256": sha}
        });
        Mock::given(method("GET"))
            .and(path("/latest.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let http = reqwest::Client::new();
        let downloader = Downloader::new(http.clone(), 2).with_retries(1, Duration::from_millis(1));
        let actual = {
            use sha2::Digest;
            hex::encode(sha2::Sha256::digest(b"agent"))
        };
        assert_ne!(
            actual, sha,
            "the fake checksum above is intentionally wrong"
        );

        // Wrong checksum: the download is rejected and nothing is cached.
        Mock::given(method("GET"))
            .and(path("/agent.jar"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes("agent"))
            .mount(&server)
            .await;
        let err = ensure(
            &http,
            &downloader,
            tmp.path(),
            &format!("{}/latest.json", server.uri()),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("hash mismatch"), "{err}");

        // Correct checksum: the jar lands in the folder.
        server.reset().await;
        let body = serde_json::json!({
            "version": "1.2.8",
            "download_url": format!("{}/agent.jar", server.uri()),
            "checksums": {"sha256": actual}
        });
        Mock::given(method("GET"))
            .and(path("/latest.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/agent.jar"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes("agent"))
            .mount(&server)
            .await;
        let jar = ensure(
            &http,
            &downloader,
            tmp.path(),
            &format!("{}/latest.json", server.uri()),
        )
        .await
        .unwrap();
        assert_eq!(jar.file_name().unwrap(), "authlib-injector-1.2.8.jar");

        // Offline: the cached jar is used.
        server.reset().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let cached = ensure(
            &http,
            &downloader,
            tmp.path(),
            &format!("{}/latest.json", server.uri()),
        )
        .await
        .unwrap();
        assert_eq!(cached, jar);
        assert_eq!(
            javaagent_arg(&jar, "ely.by"),
            format!("-javaagent:{}=ely.by", jar.display())
        );
    }
}
