//! Fetching and caching Mojang metadata, and running install plans.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::{StreamExt, stream};
use glint_net::{Downloader, ExpectedHash};
use serde::de::DeserializeOwned;

use crate::mojang::{AssetIndex, ManifestEntry, VersionInfo, VersionManifest};
use crate::plan::{InstallPlan, MetaLayout, plan_install};
use crate::rules::Platform;
use crate::{Error, Result};

/// Where metadata comes from. Overridable so tests can point at a local server.
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub manifest: String,
    pub resources: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            manifest: "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json".into(),
            resources: "https://resources.download.minecraft.net".into(),
        }
    }
}

/// Install progress, reported after every finished file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub done_files: usize,
    pub total_files: usize,
    pub done_bytes: u64,
    pub total_bytes: u64,
}

pub struct MetaClient {
    http: reqwest::Client,
    downloader: Downloader,
    layout: MetaLayout,
    endpoints: Endpoints,
}

/// Parallel downloads during installs. Assets are thousands of small files, but
/// local proxies and VPN clients start dropping connections well before 32.
const INSTALL_CONCURRENCY: usize = 12;

impl MetaClient {
    pub fn new(http: reqwest::Client, layout: MetaLayout, endpoints: Endpoints) -> Self {
        let downloader = Downloader::new(http.clone(), INSTALL_CONCURRENCY);
        Self { http, downloader, layout, endpoints }
    }

    pub fn layout(&self) -> &MetaLayout {
        &self.layout
    }

    /// Fresh version list, or the cached copy when the network is unavailable.
    pub async fn manifest(&self) -> Result<VersionManifest> {
        let cache = self.layout.manifest_cache();
        match self.fetch_text(&self.endpoints.manifest).await {
            Ok(text) => {
                let manifest = parse::<VersionManifest>(&text, &cache)?;
                write_atomic(&cache, text.as_bytes()).await?;
                Ok(manifest)
            }
            Err(err) => {
                tracing::warn!(error = %err, "version manifest unavailable, using cache");
                match tokio::fs::read_to_string(&cache).await {
                    Ok(text) => parse(&text, &cache),
                    Err(_) => Err(err),
                }
            }
        }
    }

    /// The version JSON for a manifest entry, verified against the manifest's SHA-1.
    pub async fn version(&self, entry: &ManifestEntry) -> Result<VersionInfo> {
        let dest = self.layout.version_json(&entry.id);
        self.downloader
            .download(&entry.url, &dest, Some(&ExpectedHash::Sha1(entry.sha1.clone())))
            .await?;
        read_json(&dest).await
    }

    /// The asset index a version refers to, if it has one.
    pub async fn asset_index(&self, version: &VersionInfo) -> Result<Option<AssetIndex>> {
        let Some(index) = &version.asset_index else {
            return Ok(None);
        };
        let dest = self.layout.asset_index(&index.id);
        self.downloader
            .download(&index.url, &dest, Some(&ExpectedHash::Sha1(index.sha1.clone())))
            .await?;
        read_json(&dest).await.map(Some)
    }

    pub fn plan(&self, version: &VersionInfo, index: Option<&AssetIndex>, platform: &Platform) -> InstallPlan {
        plan_install(&self.layout, version, index, platform, &self.endpoints.resources)
    }

    /// Downloads every file in `plan`, skipping ones already present and verified.
    pub async fn install(&self, plan: &InstallPlan, on_progress: impl Fn(Progress)) -> Result<()> {
        let total_files = plan.files.len();
        let total_bytes = plan.total_bytes();
        let done_bytes = AtomicU64::new(0);

        // Owned items keep the resulting future `Send` for any caller lifetime.
        let downloader = &self.downloader;
        let done_bytes_ref = &done_bytes;
        let mut results = stream::iter(plan.files.iter().cloned())
            .map(move |file| async move {
                downloader.download(&file.url, &file.dest, file.hash.as_ref()).await?;
                done_bytes_ref.fetch_add(file.size.unwrap_or(0), Ordering::Relaxed);
                Ok::<_, Error>(())
            })
            .buffer_unordered(INSTALL_CONCURRENCY);

        let mut done_files = 0;
        while let Some(result) = results.next().await {
            result?;
            done_files += 1;
            on_progress(Progress {
                done_files,
                total_files,
                done_bytes: done_bytes.load(Ordering::Relaxed),
                total_bytes,
            });
        }
        Ok(())
    }

    async fn fetch_text(&self, url: &str) -> Result<String> {
        let http_err = |source| Error::Net(glint_net::Error::Http { url: url.to_owned(), source });
        let response = self.http.get(url).send().await.map_err(http_err)?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::Net(glint_net::Error::Status { url: url.to_owned(), status: status.as_u16() }));
        }
        response.text().await.map_err(http_err)
    }
}

fn parse<T: DeserializeOwned>(text: &str, path: &Path) -> Result<T> {
    serde_json::from_str(text).map_err(|source| Error::Json { path: path.to_owned(), source })
}

async fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let text = tokio::fs::read_to_string(path)
        .await
        .map_err(|source| Error::Io { path: path.to_owned(), source })?;
    parse(&text, path)
}

async fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let io = |source| Error::Io { path: path.to_owned(), source };
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(io)?;
    }
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, bytes).await.map_err(io)?;
    tokio::fs::rename(&tmp, path).await.map_err(io)
}
