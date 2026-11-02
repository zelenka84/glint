//! The whole "Play" flow: resolve, download, install Java, prepare and launch.

use std::time::{Duration, Instant};

use glint_auth::LaunchIdentity;
use glint_core::GlintPaths;
use glint_java::{DEFAULT_INDEX_URL, JavaRuntimes, LEGACY_COMPONENT};
use glint_meta::{MetaClient, Platform, Progress};
use serde::Serialize;

use crate::log::LogParser;
use crate::{LaunchOptions, build_command, prepare, spawn};

#[derive(Debug, Clone)]
pub struct PlayRequest {
    pub version_id: String,
    pub instance_name: String,
    pub identity: LaunchIdentity,
    pub max_memory_mb: u32,
}

/// Progress of a play request, in the order it happens. Serialized for the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PlayEvent {
    Resolving { version: String },
    Downloading { what: Download, done_files: usize, total_files: usize, done_bytes: u64, total_bytes: u64 },
    Preparing,
    Started { pid: Option<u32> },
    Log { level: String, message: String },
    Exited { code: Option<i32> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Download {
    Game,
    Java,
}

#[derive(Debug, thiserror::Error)]
pub enum PlayError {
    #[error("unknown Minecraft version {0}")]
    UnknownVersion(String),
    #[error(transparent)]
    Meta(#[from] glint_meta::Error),
    #[error(transparent)]
    Java(#[from] glint_java::Error),
    #[error(transparent)]
    Core(#[from] glint_core::Error),
    #[error(transparent)]
    Launch(#[from] crate::Error),
}

/// Runs a version from nothing to an exited game, reporting every step to `emit`.
/// Returns once the game process has exited.
pub async fn play(
    paths: &GlintPaths,
    meta: &MetaClient,
    http: reqwest::Client,
    request: PlayRequest,
    emit: impl Fn(PlayEvent),
) -> Result<(), PlayError> {
    let platform = Platform::current();
    emit(PlayEvent::Resolving { version: request.version_id.clone() });

    let manifest = meta.manifest().await?;
    let entry = manifest
        .find(&request.version_id)
        .ok_or_else(|| PlayError::UnknownVersion(request.version_id.clone()))?;
    let version = meta.version(entry).await?;
    let index = meta.asset_index(&version).await?;
    let plan = meta.plan(&version, index.as_ref(), &platform);

    meta.install(&plan, throttled(Download::Game, &emit)).await?;

    let component = version.java_version.as_ref().map_or(LEGACY_COMPONENT, |j| j.component.as_str());
    let java = JavaRuntimes::new(http, paths.java(), DEFAULT_INDEX_URL)
        .ensure(component, &platform, throttled(Download::Java, &emit))
        .await?;

    emit(PlayEvent::Preparing);
    let instance = paths.instance(&request.instance_name)?;
    let game_dir = instance.join("game");
    let natives_dir = instance.join("natives");
    let game_assets = prepare(meta.layout(), &version, &plan, index.as_ref(), &natives_dir, &game_dir).await?;

    let options = LaunchOptions {
        java,
        game_dir,
        natives_dir,
        game_assets,
        identity: request.identity,
        max_memory_mb: request.max_memory_mb,
        extra_jvm_args: Vec::new(),
        resolution: None,
        quick_play_server: None,
        launcher_version: env!("CARGO_PKG_VERSION").into(),
    };
    let command = build_command(meta.layout(), &version, &plan, &options, &platform);
    let game = spawn(&command)?;
    emit(PlayEvent::Started { pid: game.id() });

    let mut parser = LogParser::default();
    let status = game
        .wait(|line| {
            if let Some(entry) = parser.push(&line) {
                emit(PlayEvent::Log { level: entry.level, message: entry.message });
            }
        })
        .await?;
    emit(PlayEvent::Exited { code: status.code() });
    Ok(())
}

/// Reports download progress at most ~10 times a second, plus the final update.
fn throttled(what: Download, emit: &impl Fn(PlayEvent)) -> impl Fn(Progress) + '_ {
    let last = std::sync::Mutex::new(Instant::now() - Duration::from_secs(1));
    move |p: Progress| {
        let mut last = last.lock().expect("progress lock");
        let finished = p.done_files == p.total_files;
        if finished || last.elapsed() >= Duration::from_millis(100) {
            *last = Instant::now();
            emit(PlayEvent::Downloading {
                what,
                done_files: p.done_files,
                total_files: p.total_files,
                done_bytes: p.done_bytes,
                total_bytes: p.total_bytes,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_serialize_for_the_ui() {
        let json = serde_json::to_value(PlayEvent::Downloading {
            what: Download::Java,
            done_files: 1,
            total_files: 2,
            done_bytes: 3,
            total_bytes: 4,
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({"type": "downloading", "what": "java", "doneFiles": 1, "totalFiles": 2, "doneBytes": 3, "totalBytes": 4})
        );
        let json = serde_json::to_value(PlayEvent::Exited { code: Some(0) }).unwrap();
        assert_eq!(json, serde_json::json!({"type": "exited", "code": 0}));
    }
}
