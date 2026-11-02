//! Mod loaders for Glint.
//!
//! Fabric and Quilt publish ready-made version profiles. Forge and NeoForge ship an
//! installer whose "processors" patch the game jar locally. Either way the result is
//! a loader profile that inherits from the vanilla version, which [`merge`] folds
//! into one launchable [`VersionInfo`].

mod forge;
mod merge;
mod versions;

pub use merge::merge;
pub use versions::{LoaderVersion, loader_versions};

use std::path::{Path, PathBuf};

use glint_core::LoaderKind;
use glint_meta::mojang::VersionInfo;
use glint_meta::{MetaClient, Platform};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Net(#[from] glint_net::Error),

    #[error(transparent)]
    Meta(#[from] glint_meta::Error),

    #[error("{loader} has no build {version} for Minecraft {game}")]
    UnknownVersion { loader: &'static str, version: String, game: String },

    #[error("{0}")]
    Unsupported(String),

    #[error("invalid {what}: {source}")]
    Json {
        what: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not read {path}: {source}")]
    Zip {
        path: PathBuf,
        #[source]
        source: zip::result::ZipError,
    },

    #[error("installer step {step} failed ({status}): {output}")]
    Processor { step: String, status: String, output: String },
}

pub(crate) fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io { path: path.to_owned(), source }
}

pub fn display_name(kind: LoaderKind) -> &'static str {
    match kind {
        LoaderKind::Fabric => "Fabric",
        LoaderKind::Quilt => "Quilt",
        LoaderKind::Forge => "Forge",
        LoaderKind::NeoForge => "NeoForge",
    }
}

/// Where loaders install their own files, next to the shared vanilla metadata.
fn loaders_dir(meta: &MetaClient) -> PathBuf {
    meta.layout().root().join("loaders")
}

/// Installs (if needed) and returns the launchable version for `vanilla` with `kind`
/// at `loader_version`. The vanilla files must already be installed, because Forge
/// and NeoForge patch the vanilla client jar; `java` runs their installer steps.
pub async fn install(
    http: &reqwest::Client,
    meta: &MetaClient,
    vanilla: &VersionInfo,
    kind: LoaderKind,
    loader_version: &str,
    java: &Path,
    platform: &Platform,
) -> Result<VersionInfo> {
    let profile = match kind {
        LoaderKind::Fabric | LoaderKind::Quilt => {
            fetch_profile(http, meta, kind, &vanilla.id, loader_version).await?
        }
        LoaderKind::Forge | LoaderKind::NeoForge => {
            forge::install(http, meta, kind, vanilla, loader_version, java, platform).await?
        }
    };
    Ok(merge(vanilla, &profile))
}

/// Fabric and Quilt serve a profile per (game, loader) pair; cached after the first fetch.
async fn fetch_profile(
    http: &reqwest::Client,
    meta: &MetaClient,
    kind: LoaderKind,
    game: &str,
    loader: &str,
) -> Result<VersionInfo> {
    let (base, name) = match kind {
        LoaderKind::Fabric => ("https://meta.fabricmc.net/v2", "fabric"),
        _ => ("https://meta.quiltmc.org/v3", "quilt"),
    };
    let cache = loaders_dir(meta).join(name).join(format!("{game}-{loader}.json"));
    let text = match tokio::fs::read_to_string(&cache).await {
        Ok(text) => text,
        Err(_) => {
            let url = format!("{base}/versions/loader/{game}/{loader}/profile/json");
            let response = http.get(&url).send().await.map_err(|source| glint_net::Error::Http { url: url.clone(), source })?;
            if response.status().as_u16() == 400 || response.status().as_u16() == 404 {
                return Err(Error::UnknownVersion { loader: display_name(kind), version: loader.into(), game: game.into() });
            }
            if !response.status().is_success() {
                return Err(glint_net::Error::Status { url, status: response.status().as_u16() }.into());
            }
            let text = response.text().await.map_err(|source| glint_net::Error::Http { url, source })?;
            if let Some(parent) = cache.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| io(parent, e))?;
            }
            tokio::fs::write(&cache, &text).await.map_err(|e| io(&cache, e))?;
            text
        }
    };
    serde_json::from_str(&text).map_err(|source| Error::Json { what: format!("{name} profile"), source })
}
