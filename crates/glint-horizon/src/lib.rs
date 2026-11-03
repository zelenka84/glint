//! Horizon: an open-source Rust reimplementation of Minecraft: Java Edition 26.3.
//!
//! [`Horizon::ensure_build`] keeps the newest Nightly installed under Glint's
//! `horizon/` folder. A [`Build`] says what it supports ([`Build::launch_api`]) and
//! prepares, from the player's own Minecraft files, the resources the game reads.
//! See Horizon's `docs/api/launch-guide.md` for the contract this follows.

mod archive;
mod build;

pub use build::{Build, LaunchApi, LaunchOption, PrepareEvent, PrepareInputs};

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use glint_net::{Downloader, ExpectedHash};
use serde::{Deserialize, Serialize};

/// The only Minecraft version Horizon plays; its files are what preparing reads.
pub const MINECRAFT_VERSION: &str = "26.3";

/// The rolling Nightly release, the only game channel Horizon ships.
pub const RELEASE_URL: &str =
    "https://api.github.com/repos/horizon-rust/horizon/releases/tags/nightly";

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Net(#[from] glint_net::Error),

    #[error("could not reach the Horizon releases: {0}")]
    Release(String),

    #[error("Horizon has no build for this platform yet")]
    Unsupported,

    #[error("the Horizon release has no {0}")]
    MissingAsset(String),

    #[error("{0} is not in the Horizon release's SHA256SUMS")]
    MissingChecksum(String),

    #[error("the Horizon archive {0} has no horizon-client in it")]
    BadArchive(PathBuf),

    #[error("could not unpack {path}: {message}")]
    Unpack { path: PathBuf, message: String },

    #[error("could not run {path}: {source}")]
    Run {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{program} answered something Glint does not understand: {message}")]
    Protocol { program: String, message: String },

    #[error("this Horizon build speaks Launch API {0}, Glint knows 0.1")]
    LaunchApiVersion(String),

    #[error("preparing Horizon failed: {0}")]
    Prepare(String),

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub(crate) fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}

/// The archive Horizon publishes for an OS and CPU, as `std::env::consts` names them.
pub fn asset_for(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("windows", "x86_64") => Some("horizon-windows-x64.zip"),
        ("linux", "x86_64") => Some("horizon-linux-x86_64.tar.gz"),
        ("macos", "aarch64") => Some("horizon-macos-arm64.zip"),
        ("macos", "x86_64") => Some("horizon-macos-x64.zip"),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct Release {
    name: Option<String>,
    tag_name: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

/// `current.json`: the build Glint plays, and the one before it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Current {
    id: String,
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous: Option<String>,
}

/// Bytes of the build archive fetched so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub done_bytes: u64,
    pub total_bytes: u64,
}

/// Horizon builds under one folder: `builds/<id>/`, `prepared/<id>/` and `current.json`.
pub struct Horizon {
    http: reqwest::Client,
    downloader: Downloader,
    root: PathBuf,
    release_url: String,
}

impl Horizon {
    /// `root` is Glint's `horizon/` folder.
    pub fn new(
        http: reqwest::Client,
        root: impl Into<PathBuf>,
        release_url: impl Into<String>,
    ) -> Self {
        Self {
            downloader: Downloader::new(http.clone(), 2),
            http,
            root: root.into(),
            release_url: release_url.into(),
        }
    }

    /// The build installed last, if it is still whole.
    pub async fn installed(&self) -> Option<Build> {
        let current = self.read_current().await?;
        let build = self.build(&current.id, &current.name);
        build.client().is_file().then_some(build)
    }

    /// Installs the newest Nightly when it is not installed yet and returns it. Without
    /// a network the installed build is used.
    pub async fn ensure_build(&self, on_progress: impl Fn(Progress)) -> Result<Build> {
        let release = match self.latest().await {
            Ok(release) => release,
            Err(err) => {
                return match self.installed().await {
                    Some(build) => {
                        tracing::warn!(error = %err, "Horizon releases unavailable, using the installed build");
                        Ok(build)
                    }
                    None => Err(err),
                };
            }
        };
        let name = release
            .name
            .clone()
            .unwrap_or_else(|| release.tag_name.clone());
        let id = build_id(&name);
        let current = self.read_current().await;
        if let Some(current) = &current
            && current.id == id
            && let Some(build) = self.installed().await
        {
            return Ok(build);
        }

        let asset_name =
            asset_for(std::env::consts::OS, std::env::consts::ARCH).ok_or(Error::Unsupported)?;
        let find = |wanted: &str| {
            release
                .assets
                .iter()
                .find(|a| a.name == wanted)
                .ok_or_else(|| Error::MissingAsset(wanted.to_owned()))
        };
        let asset = find(asset_name)?;
        let sums = self.text(&find("SHA256SUMS")?.browser_download_url).await?;
        let sha256 = parse_sums(&sums)
            .remove(asset_name)
            .ok_or_else(|| Error::MissingChecksum(asset_name.to_owned()))?;

        tracing::info!(build = %name, "installing Horizon");
        on_progress(Progress {
            done_bytes: 0,
            total_bytes: asset.size,
        });
        let archive = self.root.join("downloads").join(asset_name);
        self.downloader
            .download(
                &asset.browser_download_url,
                &archive,
                Some(&ExpectedHash::Sha256(sha256)),
            )
            .await?;
        on_progress(Progress {
            done_bytes: asset.size,
            total_bytes: asset.size,
        });

        let dir = self.root.join("builds").join(&id);
        if !dir.is_dir() {
            archive::unpack_build(&archive, &dir).await?;
        }
        let _ = tokio::fs::remove_file(&archive).await;

        let previous = current.map(|c| c.id).filter(|c| *c != id);
        self.write_current(&Current {
            id: id.clone(),
            name: name.clone(),
            previous: previous.clone(),
        })
        .await?;
        // The previous build stays so a broken Nightly can be stepped back from.
        let mut keep = vec![id.clone()];
        keep.extend(previous);
        self.prune(&keep).await;
        Ok(self.build(&id, &name))
    }

    fn build(&self, id: &str, name: &str) -> Build {
        Build::new(
            id,
            name,
            self.root.join("builds").join(id),
            self.root.join("prepared").join(id),
        )
    }

    async fn latest(&self) -> Result<Release> {
        let text = self.text(&self.release_url).await?;
        serde_json::from_str(&text).map_err(|e| Error::Release(e.to_string()))
    }

    async fn text(&self, url: &str) -> Result<String> {
        let response = self
            .http
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| Error::Release(e.to_string()))?;
        response
            .text()
            .await
            .map_err(|e| Error::Release(e.to_string()))
    }

    async fn read_current(&self) -> Option<Current> {
        let bytes = tokio::fs::read(self.root.join("current.json")).await.ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    async fn write_current(&self, current: &Current) -> Result<()> {
        let path = self.root.join("current.json");
        tokio::fs::create_dir_all(&self.root)
            .await
            .map_err(|e| io(&self.root, e))?;
        let bytes = serde_json::to_vec_pretty(current).expect("current.json serializes");
        tokio::fs::write(&path, bytes)
            .await
            .map_err(|e| io(&path, e))
    }

    /// Removes builds and prepared resources other than `keep`. Failures only cost disk space.
    async fn prune(&self, keep: &[String]) {
        for folder in ["builds", "prepared"] {
            let Ok(mut entries) = tokio::fs::read_dir(self.root.join(folder)).await else {
                continue;
            };
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !keep.contains(&name)
                    && let Err(e) = tokio::fs::remove_dir_all(entry.path()).await
                {
                    tracing::warn!(path = %entry.path().display(), error = %e, "could not remove an old Horizon build");
                }
            }
        }
    }
}

/// A folder name for a release title like `Nightly 2026-11-03 · f42ef42`.
fn build_id(name: &str) -> String {
    let mut id = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c.to_ascii_lowercase());
        } else if !id.is_empty() && !id.ends_with('-') {
            id.push('-');
        }
    }
    id.trim_end_matches('-').to_owned()
}

/// `SHA256SUMS` in `sha256sum` format: file name to lowercase hex digest.
fn parse_sums(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let (hash, name) = line.trim().split_once(char::is_whitespace)?;
            let name = name.trim_start().trim_start_matches('*');
            Some((name.to_owned(), hash.to_ascii_lowercase()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;
    use std::io::Write;
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn build_ids_are_folder_names() {
        assert_eq!(
            build_id("Nightly 2026-11-03 · f42ef42"),
            "nightly-2026-11-03-f42ef42"
        );
        assert_eq!(build_id("nightly"), "nightly");
    }

    #[test]
    fn sums_are_parsed() {
        let sums =
            parse_sums("27c8  horizon-linux-x86_64.tar.gz\n7103 *horizon-windows-x64.zip\n\n");
        assert_eq!(sums["horizon-windows-x64.zip"], "7103");
        assert_eq!(sums["horizon-linux-x86_64.tar.gz"], "27c8");
        assert_eq!(sums.len(), 2);
    }

    #[test]
    fn every_supported_platform_has_an_archive() {
        assert_eq!(
            asset_for("windows", "x86_64"),
            Some("horizon-windows-x64.zip")
        );
        assert_eq!(asset_for("windows", "aarch64"), None);
    }

    fn fake_build_zip() -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        for file in [
            build::exe("horizon-client"),
            build::exe("horizon-prepare"),
            "research-lock.json".into(),
        ] {
            zip.start_file(format!("horizon/{file}"), options).unwrap();
            zip.write_all(b"fake").unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[tokio::test]
    async fn installs_the_nightly_once_and_keeps_it() {
        let Some(asset) = asset_for(std::env::consts::OS, std::env::consts::ARCH) else {
            return;
        };
        if asset.ends_with(".tar.gz") {
            return;
        }
        let server = MockServer::start().await;
        let archive = fake_build_zip();
        let sha = hex::encode(sha2::Sha256::digest(&archive));
        let release = serde_json::json!({
            "name": "Nightly 2026-11-03 · f42ef42",
            "tag_name": "nightly",
            "assets": [
                {"name": asset, "browser_download_url": format!("{}/a", server.uri()), "size": archive.len()},
                {"name": "SHA256SUMS", "browser_download_url": format!("{}/sums", server.uri()), "size": 1},
            ],
        });
        Mock::given(path("/release"))
            .respond_with(ResponseTemplate::new(200).set_body_json(release))
            .mount(&server)
            .await;
        Mock::given(path("/sums"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!("{sha}  {asset}\n")))
            .mount(&server)
            .await;
        Mock::given(path("/a"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(archive))
            .expect(1)
            .mount(&server)
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let horizon = Horizon::new(
            reqwest::Client::new(),
            tmp.path(),
            format!("{}/release", server.uri()),
        );
        let build = horizon.ensure_build(|_| {}).await.unwrap();
        assert_eq!(build.id, "nightly-2026-11-03-f42ef42");
        assert!(build.client().is_file(), "{}", build.client().display());
        assert!(build.dir.join("research-lock.json").is_file());

        // Same release again: nothing is downloaded (the mock expects one hit).
        let again = horizon.ensure_build(|_| {}).await.unwrap();
        assert_eq!(again, build);
        assert_eq!(horizon.installed().await, Some(build));
    }
}
