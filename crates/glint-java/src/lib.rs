//! Java runtimes for Glint.
//!
//! Every version JSON names a runtime component (`jre-legacy`, `java-runtime-delta`, …).
//! Mojang publishes those runtimes per platform; this crate installs them into
//! `java/<component>/` and returns the path to the `java` executable.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use futures_util::{StreamExt, stream};
use glint_meta::{Arch, Os, Platform, Progress};
use glint_net::{Downloader, ExpectedHash};
use serde::{Deserialize, Serialize};

/// Component used by versions whose JSON predates `javaVersion` (Java 8).
pub const LEGACY_COMPONENT: &str = "jre-legacy";

pub const DEFAULT_INDEX_URL: &str =
    "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Net(#[from] glint_net::Error),

    #[error("Mojang publishes no {component} runtime for {platform}")]
    Unsupported { component: String, platform: String },

    #[error("invalid JSON in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Mojang's name for a platform in the runtime index.
pub fn platform_key(platform: &Platform) -> &'static str {
    match (platform.os, platform.arch) {
        (Os::Windows, Arch::X86_64) => "windows-x64",
        (Os::Windows, Arch::X86) => "windows-x86",
        (Os::Windows, Arch::Arm64) => "windows-arm64",
        (Os::Linux, Arch::X86) => "linux-i386",
        (Os::Linux, _) => "linux",
        (Os::MacOs, Arch::Arm64) => "mac-os-arm64",
        (Os::MacOs, _) => "mac-os",
    }
}

/// Path of the `java` executable inside an installed runtime folder.
pub fn java_executable(runtime_dir: &Path, os: Os) -> PathBuf {
    match os {
        Os::Windows => runtime_dir.join("bin").join("java.exe"),
        Os::MacOs => runtime_dir.join("jre.bundle/Contents/Home/bin/java"),
        Os::Linux => runtime_dir.join("bin").join("java"),
    }
}

// all.json: platform → component → [entry]
type RuntimeIndex = HashMap<String, HashMap<String, Vec<RuntimeEntry>>>;

#[derive(Debug, Deserialize)]
struct RuntimeEntry {
    manifest: ManifestRef,
    version: RuntimeVersion,
}

#[derive(Debug, Deserialize)]
struct ManifestRef {
    sha1: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeVersion {
    name: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeManifest {
    files: HashMap<String, RuntimeFile>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum RuntimeFile {
    Directory,
    File {
        downloads: FileDownloads,
        #[serde(default)]
        executable: bool,
    },
    Link {
        target: String,
    },
}

#[derive(Debug, Deserialize)]
struct FileDownloads {
    raw: RawDownload,
}

#[derive(Debug, Deserialize)]
struct RawDownload {
    sha1: String,
    size: u64,
    url: String,
}

/// Written after a complete install so later launches skip re-hashing every file.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct InstalledMarker {
    manifest_sha1: String,
    version: String,
}

const MARKER_FILE: &str = ".glint-runtime.json";

pub struct JavaRuntimes {
    http: reqwest::Client,
    downloader: Downloader,
    root: PathBuf,
    index_url: String,
}

impl JavaRuntimes {
    /// `root` is Glint's `java/` folder.
    pub fn new(http: reqwest::Client, root: impl Into<PathBuf>, index_url: impl Into<String>) -> Self {
        let downloader = Downloader::new(http.clone(), 8);
        Self { http, downloader, root: root.into(), index_url: index_url.into() }
    }

    /// Makes sure `component` is installed and returns its `java` executable.
    pub async fn ensure(
        &self,
        component: &str,
        platform: &Platform,
        on_progress: impl Fn(Progress),
    ) -> Result<PathBuf> {
        let key = platform_key(platform);
        let index: RuntimeIndex = self.index().await?;
        let entry = index
            .get(key)
            .and_then(|c| c.get(component))
            .and_then(|entries| entries.first())
            .ok_or_else(|| Error::Unsupported { component: component.to_owned(), platform: key.to_owned() })?;

        let dir = self.root.join(component);
        let java = java_executable(&dir, platform.os);
        let marker = InstalledMarker { manifest_sha1: entry.manifest.sha1.clone(), version: entry.version.name.clone() };
        if java.is_file() && read_marker(&dir).await.as_ref() == Some(&marker) {
            return Ok(java);
        }

        tracing::info!(component, version = %entry.version.name, "installing Java runtime");
        let manifest_path = self.root.join(format!("{component}.manifest.json"));
        self.downloader
            .download(&entry.manifest.url, &manifest_path, Some(&ExpectedHash::Sha1(entry.manifest.sha1.clone())))
            .await?;
        let manifest: RuntimeManifest = read_json(&manifest_path).await?;
        self.install_files(&dir, &manifest, on_progress).await?;

        let marker_path = dir.join(MARKER_FILE);
        let bytes = serde_json::to_vec(&marker).expect("marker serializes");
        tokio::fs::write(&marker_path, bytes).await.map_err(|e| io(&marker_path, e))?;
        Ok(java)
    }

    async fn install_files(
        &self,
        dir: &Path,
        manifest: &RuntimeManifest,
        on_progress: impl Fn(Progress),
    ) -> Result<()> {
        for (rel, file) in &manifest.files {
            if matches!(file, RuntimeFile::Directory) {
                let path = dir.join(rel);
                tokio::fs::create_dir_all(&path).await.map_err(|e| io(&path, e))?;
            }
        }

        // (destination, url, sha1, size, executable), owned so the futures stay `Send`.
        let files: Vec<(PathBuf, String, String, u64, bool)> = manifest
            .files
            .iter()
            .filter_map(|(rel, f)| match f {
                RuntimeFile::File { downloads, executable } => Some((
                    dir.join(rel),
                    downloads.raw.url.clone(),
                    downloads.raw.sha1.clone(),
                    downloads.raw.size,
                    *executable,
                )),
                _ => None,
            })
            .collect();
        let total_files = files.len();
        let total_bytes = files.iter().map(|f| f.3).sum();
        let done = AtomicUsize::new(0);
        let done_bytes = std::sync::atomic::AtomicU64::new(0);

        let downloader = &self.downloader;
        let done_bytes_ref = &done_bytes;
        let mut results = stream::iter(files)
            .map(move |(dest, url, sha1, size, executable)| async move {
                downloader.download(&url, &dest, Some(&ExpectedHash::Sha1(sha1))).await?;
                if executable {
                    make_executable(&dest).await?;
                }
                done_bytes_ref.fetch_add(size, Ordering::Relaxed);
                Ok::<_, Error>(())
            })
            .buffer_unordered(8);
        while let Some(result) = results.next().await {
            result?;
            on_progress(Progress {
                done_files: done.fetch_add(1, Ordering::Relaxed) + 1,
                total_files,
                done_bytes: done_bytes.load(Ordering::Relaxed),
                total_bytes,
            });
        }

        for (rel, file) in &manifest.files {
            if let RuntimeFile::Link { target } = file {
                link(&dir.join(rel), target).await?;
            }
        }
        Ok(())
    }

    /// The runtime index, falling back to the cached copy when offline.
    async fn index(&self) -> Result<RuntimeIndex> {
        let cache = self.root.join("all.json");
        let fetched = async {
            let response = self.http.get(&self.index_url).send().await;
            let response = response.map_err(|source| glint_net::Error::Http { url: self.index_url.clone(), source })?;
            if !response.status().is_success() {
                return Err(glint_net::Error::Status { url: self.index_url.clone(), status: response.status().as_u16() }.into());
            }
            let text = response.text().await;
            text.map_err(|source| Error::Net(glint_net::Error::Http { url: self.index_url.clone(), source }))
        }
        .await;

        match fetched {
            Ok(text) => {
                let index = parse(&text, &cache)?;
                tokio::fs::create_dir_all(&self.root).await.map_err(|e| io(&self.root, e))?;
                tokio::fs::write(&cache, &text).await.map_err(|e| io(&cache, e))?;
                Ok(index)
            }
            Err(err) => {
                tracing::warn!(error = %err, "Java runtime index unavailable, using cache");
                match tokio::fs::read_to_string(&cache).await {
                    Ok(text) => parse(&text, &cache),
                    Err(_) => Err(err),
                }
            }
        }
    }
}

async fn read_marker(dir: &Path) -> Option<InstalledMarker> {
    let text = tokio::fs::read_to_string(dir.join(MARKER_FILE)).await.ok()?;
    serde_json::from_str(&text).ok()
}

fn parse<T: serde::de::DeserializeOwned>(text: &str, path: &Path) -> Result<T> {
    serde_json::from_str(text).map_err(|source| Error::Json { path: path.to_owned(), source })
}

async fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let text = tokio::fs::read_to_string(path).await.map_err(|e| io(path, e))?;
    parse(&text, path)
}

fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io { path: path.to_owned(), source }
}

#[cfg(unix)]
async fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o755);
    tokio::fs::set_permissions(path, perms).await.map_err(|e| io(path, e))
}

#[cfg(not(unix))]
async fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
async fn link(path: &Path, target: &str) -> Result<()> {
    let _ = tokio::fs::remove_file(path).await;
    tokio::fs::symlink(target, path).await.map_err(|e| io(path, e))
}

// Windows runtimes contain no links; skip rather than require symlink privileges.
#[cfg(not(unix))]
async fn link(path: &Path, target: &str) -> Result<()> {
    tracing::debug!(path = %path.display(), target, "skipping runtime symlink on this platform");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sha1::{Digest, Sha1};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sha1_hex(bytes: &[u8]) -> String {
        hex::encode(Sha1::digest(bytes))
    }

    async fn serve(server: &MockServer, route: &str, body: Vec<u8>, times: u64) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
            .expect(times)
            .mount(server)
            .await;
    }

    #[test]
    fn platform_keys_match_mojang_names() {
        assert_eq!(platform_key(&Platform::new(Os::Windows, Arch::X86_64)), "windows-x64");
        assert_eq!(platform_key(&Platform::new(Os::Linux, Arch::X86_64)), "linux");
        assert_eq!(platform_key(&Platform::new(Os::MacOs, Arch::Arm64)), "mac-os-arm64");
    }

    #[tokio::test]
    async fn installs_runtime_once_and_reuses_it() {
        let server = MockServer::start().await;
        let base = server.uri();
        let java = b"fake java".to_vec();
        let lib = b"fake lib".to_vec();
        let manifest = serde_json::to_vec(&json!({"files": {
            "bin": {"type": "directory"},
            "bin/java.exe": {"type": "file", "executable": true, "downloads": {"raw": {"sha1": sha1_hex(&java), "size": java.len(), "url": format!("{base}/java")}}},
            "bin/java": {"type": "file", "executable": true, "downloads": {"raw": {"sha1": sha1_hex(&java), "size": java.len(), "url": format!("{base}/java")}}},
            "lib/modules": {"type": "file", "downloads": {"raw": {"sha1": sha1_hex(&lib), "size": lib.len(), "url": format!("{base}/lib")}}}
        }}))
        .unwrap();
        let index = serde_json::to_vec(&json!({
            "windows-x64": {"java-runtime-delta": [{"manifest": {"sha1": sha1_hex(&manifest), "url": format!("{base}/manifest.json")}, "version": {"name": "21.0.7"}}]},
            "linux": {"java-runtime-delta": [{"manifest": {"sha1": sha1_hex(&manifest), "url": format!("{base}/manifest.json")}, "version": {"name": "21.0.7"}}]}
        }))
        .unwrap();
        serve(&server, "/all.json", index, 3).await;
        serve(&server, "/manifest.json", manifest, 1).await;
        serve(&server, "/java", java, 2).await;
        serve(&server, "/lib", lib, 1).await;

        let tmp = tempfile::tempdir().unwrap();
        let runtimes = JavaRuntimes::new(glint_net::http_client(), tmp.path(), format!("{base}/all.json"));
        let platform = Platform::new(if cfg!(windows) { Os::Windows } else { Os::Linux }, Arch::X86_64);

        let first = runtimes.ensure("java-runtime-delta", &platform, |_| {}).await.unwrap();
        assert!(first.is_file());
        assert!(tmp.path().join("java-runtime-delta/lib/modules").is_file());

        // Second call trusts the marker: only the index is fetched again (see `expect` counts).
        let second = runtimes.ensure("java-runtime-delta", &platform, |_| {}).await.unwrap();
        assert_eq!(first, second);

        let err = runtimes.ensure("java-runtime-omega", &platform, |_| {}).await.unwrap_err();
        assert!(matches!(err, Error::Unsupported { .. }), "{err}");
    }
}
