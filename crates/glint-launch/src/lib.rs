//! Launching Minecraft: Java Edition.
//!
//! [`build_command`] turns a version, its install plan and the player's identity into
//! a `java` command line; [`prepare`] extracts natives and legacy assets; [`spawn`]
//! runs the game and streams its log.

mod args;
mod assets;
mod log;
mod natives;
mod process;
mod session;

pub use args::{LaunchCommand, LaunchOptions, build_command};
pub use log::{LogEntry, LogParser};
pub use process::{GameProcess, LogLine, LogStream, spawn};
pub use session::{Download, PlayError, PlayEvent, PlayRequest, play};

use std::path::{Path, PathBuf};

use glint_meta::mojang::{AssetIndex, VersionInfo};
use glint_meta::{InstallPlan, MetaLayout};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not read native archive {path}: {source}")]
    Zip {
        path: PathBuf,
        #[source]
        source: zip::result::ZipError,
    },

    #[error("could not start Java at {path}: {source}")]
    Spawn {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub(crate) fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io { path: path.to_owned(), source }
}

/// Gets an installed version ready to start: extracts legacy natives into
/// `natives_dir` and lays out assets for versions that read them from old locations.
/// Returns the directory to pass as `${game_assets}`.
pub async fn prepare(
    layout: &MetaLayout,
    version: &VersionInfo,
    plan: &InstallPlan,
    index: Option<&AssetIndex>,
    natives_dir: &Path,
    game_dir: &Path,
) -> Result<PathBuf> {
    natives::extract(&plan.natives, natives_dir).await?;
    tokio::fs::create_dir_all(game_dir).await.map_err(|e| io(game_dir, e))?;
    match index {
        Some(index) => {
            let index_id = version.asset_index.as_ref().map_or("legacy", |a| a.id.as_str());
            assets::prepare_legacy(layout, index_id, index, game_dir).await
        }
        None => Ok(layout.assets()),
    }
}
