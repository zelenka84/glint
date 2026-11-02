//! The whole "Play" flow: resolve, download, install Java, prepare and launch.

use std::time::{Duration, Instant};

use glint_auth::LaunchIdentity;
use glint_core::GlintPaths;
use glint_java::{DEFAULT_INDEX_URL, JavaRuntimes, LEGACY_COMPONENT};
use glint_meta::{MetaClient, Platform, Progress};
use serde::Serialize;

use crate::injector;
use crate::log::LogParser;
use crate::{LaunchOptions, build_command, prepare, spawn};

#[derive(Debug, Clone)]
pub struct PlayRequest {
    pub version_id: String,
    pub instance_name: String,
    pub identity: LaunchIdentity,
    pub max_memory_mb: u32,
    /// Fabric, Quilt, Forge or NeoForge build to run the game with.
    pub loader: Option<glint_core::Loader>,
    pub extra_jvm_args: Vec<String>,
}

/// Progress of a play request, in the order it happens. Serialized for the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PlayEvent {
    Resolving { version: String },
    Downloading { what: Download, done_files: usize, total_files: usize, done_bytes: u64, total_bytes: u64 },
    /// Installing a mod loader; Forge and NeoForge patch the game here, which takes a while.
    InstallingLoader { name: String },
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
    #[error(transparent)]
    Loader(#[from] glint_loader::Error),
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
    let vanilla = meta.version(entry).await?;
    let index = meta.asset_index(&vanilla).await?;
    let mut plan = meta.plan(&vanilla, index.as_ref(), &platform);

    meta.install(&plan, throttled(Download::Game, &emit)).await?;

    let component = vanilla.java_version.as_ref().map_or(LEGACY_COMPONENT, |j| j.component.as_str());
    let java = JavaRuntimes::new(http.clone(), paths.java(), DEFAULT_INDEX_URL)
        .ensure(component, &platform, throttled(Download::Java, &emit))
        .await?;

    // Loaders build on the installed vanilla files, then add their own libraries.
    let version = match &request.loader {
        Some(loader) => {
            emit(PlayEvent::InstallingLoader { name: glint_loader::display_name(loader.kind).to_owned() });
            let version =
                glint_loader::install(&http, meta, &vanilla, loader.kind, &loader.version, &java, &platform).await?;
            plan = meta.plan(&version, index.as_ref(), &platform);
            meta.install(&plan, throttled(Download::Game, &emit)).await?;
            version
        }
        None => vanilla,
    };

    emit(PlayEvent::Preparing);
    let mut extra_jvm_args = request.extra_jvm_args.clone();
    if let Some(target) = &request.identity.auth_server {
        let downloader = glint_net::Downloader::new(http.clone(), 2);
        let jar = injector::ensure(&http, &downloader, &paths.meta().join("authlib-injector"), injector::LATEST_URL).await?;
        extra_jvm_args.push(injector::javaagent_arg(&jar, target));
    }
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
        extra_jvm_args,
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
