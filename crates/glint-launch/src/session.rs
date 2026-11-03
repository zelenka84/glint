//! The whole "Play" flow: resolve, download, install Java, prepare and launch.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use glint_auth::LaunchIdentity;
use glint_core::GlintPaths;
use glint_java::{DEFAULT_INDEX_URL, JavaRuntimes, LEGACY_COMPONENT};
use glint_meta::{MetaClient, Platform, Progress};
use serde::Serialize;

use crate::injector;
use crate::log::LogParser;
use crate::{LaunchCommand, LaunchOptions, build_command, prepare, spawn};

#[derive(Debug, Clone)]
pub struct PlayRequest {
    /// Java runs `version_id`; Horizon always plays its own Minecraft version.
    pub engine: glint_core::Engine,
    pub version_id: String,
    pub instance_name: String,
    pub identity: LaunchIdentity,
    pub max_memory_mb: u32,
    /// Fabric, Quilt, Forge or NeoForge build to run the game with.
    pub loader: Option<glint_core::Loader>,
    pub extra_jvm_args: Vec<String>,
    pub min_memory_mb: Option<u32>,
    /// Java executable to use instead of the Mojang runtime.
    pub java_path: Option<PathBuf>,
    /// Window size and fullscreen; the auto-join fields are read by the caller.
    pub game: glint_core::GameSettings,
    pub quick_play: Option<QuickPlay>,
    pub hooks: glint_core::LaunchHooks,
    pub env: Vec<(String, String)>,
}

/// Where the game goes straight after starting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickPlay {
    Server(String),
    World(String),
}

/// Progress of a play request, in the order it happens. Serialized for the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PlayEvent {
    Resolving {
        version: String,
    },
    Downloading {
        what: Download,
        done_files: usize,
        total_files: usize,
        done_bytes: u64,
        total_bytes: u64,
    },
    /// Installing a mod loader; Forge and NeoForge patch the game here, which takes a while.
    InstallingLoader {
        name: String,
    },
    /// Getting the game ready to start; Horizon reports how far it is.
    Preparing {
        #[serde(skip_serializing_if = "Option::is_none")]
        fraction: Option<f64>,
    },
    Started {
        pid: Option<u32>,
    },
    Log {
        level: String,
        message: String,
    },
    Exited {
        code: Option<i32>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Download {
    Game,
    Java,
    Horizon,
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
    #[error(transparent)]
    Horizon(#[from] glint_horizon::Error),
    #[error("Minecraft {0} has no server download, which Horizon needs")]
    NoServerJar(String),
    #[error("Java was not found at {0}")]
    JavaMissing(PathBuf),
    #[error("the pre-launch command failed ({0}); the game was not started")]
    PreLaunch(String),
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
    if request.engine == glint_core::Engine::Horizon {
        return crate::horizon::play(paths, meta, http, request, emit).await;
    }
    let platform = Platform::current();
    emit(PlayEvent::Resolving {
        version: request.version_id.clone(),
    });

    let manifest = meta.manifest().await?;
    let entry = manifest
        .find(&request.version_id)
        .ok_or_else(|| PlayError::UnknownVersion(request.version_id.clone()))?;
    let vanilla = meta.version(entry).await?;
    let index = meta.asset_index(&vanilla).await?;
    let mut plan = meta.plan(&vanilla, index.as_ref(), &platform);

    meta.install(&plan, throttled(Download::Game, &emit))
        .await?;

    let java = match &request.java_path {
        Some(path) if path.is_file() => path.clone(),
        Some(path) => return Err(PlayError::JavaMissing(path.clone())),
        None => {
            let component = vanilla
                .java_version
                .as_ref()
                .map_or(LEGACY_COMPONENT, |j| j.component.as_str());
            JavaRuntimes::new(http.clone(), paths.java(), DEFAULT_INDEX_URL)
                .ensure(component, &platform, throttled(Download::Java, &emit))
                .await?
        }
    };

    // Loaders build on the installed vanilla files, then add their own libraries.
    let version = match &request.loader {
        Some(loader) => {
            emit(PlayEvent::InstallingLoader {
                name: glint_loader::display_name(loader.kind).to_owned(),
            });
            let version = glint_loader::install(
                &http,
                meta,
                &vanilla,
                loader.kind,
                &loader.version,
                &java,
                &platform,
            )
            .await?;
            plan = meta.plan(&version, index.as_ref(), &platform);
            meta.install(&plan, throttled(Download::Game, &emit))
                .await?;
            version
        }
        None => vanilla,
    };

    emit(PlayEvent::Preparing { fraction: None });
    let mut extra_jvm_args = request.extra_jvm_args.clone();
    if let Some(target) = &request.identity.auth_server {
        let downloader = glint_net::Downloader::new(http.clone(), 2);
        let jar = injector::ensure(
            &http,
            &downloader,
            &paths.meta().join("authlib-injector"),
            injector::LATEST_URL,
        )
        .await?;
        extra_jvm_args.push(injector::javaagent_arg(&jar, target));
    }
    let instance = paths.instance(&request.instance_name)?;
    let game_dir = instance.join("game");
    let natives_dir = instance.join("natives");
    let game_assets = prepare(
        meta.layout(),
        &version,
        &plan,
        index.as_ref(),
        &natives_dir,
        &game_dir,
    )
    .await?;

    let game_settings = &request.game;
    let options = LaunchOptions {
        java,
        game_dir,
        natives_dir,
        game_assets,
        identity: request.identity,
        max_memory_mb: request.max_memory_mb,
        min_memory_mb: request.min_memory_mb,
        extra_jvm_args,
        resolution: game_settings.width.zip(game_settings.height),
        fullscreen: game_settings.fullscreen,
        quick_play_server: match &request.quick_play {
            Some(QuickPlay::Server(s)) => Some(s.clone()),
            _ => None,
        },
        quick_play_world: match &request.quick_play {
            Some(QuickPlay::World(w)) => Some(w.clone()),
            _ => None,
        },
        launcher_version: env!("CARGO_PKG_VERSION").into(),
    };
    let mut command = build_command(meta.layout(), &version, &plan, &options, &platform);
    command.env = request.env.clone();

    let hook_env = {
        let mut env = request.env.clone();
        env.extend([
            ("INST_NAME".into(), request.instance_name.clone()),
            ("INST_ID".into(), request.instance_name.clone()),
            ("INST_DIR".into(), instance.display().to_string()),
            ("INST_MC_DIR".into(), options.game_dir.display().to_string()),
            ("INST_JAVA".into(), options.java.display().to_string()),
            ("INST_JAVA_ARGS".into(), command.args.join(" ")),
        ]);
        env
    };
    let parser = LogParser::default();
    run_game(command, parser, &request.hooks, &hook_env, &instance, &emit).await
}

/// Runs the hooks around the game, starts it and streams its log until it exits.
pub(crate) async fn run_game(
    mut command: LaunchCommand,
    mut parser: LogParser,
    hooks: &glint_core::LaunchHooks,
    hook_env: &[(String, String)],
    instance: &Path,
    emit: &impl Fn(PlayEvent),
) -> Result<(), PlayError> {
    if let Some(pre) = hooks.pre_launch.as_deref().filter(|c| !c.trim().is_empty()) {
        let code = run_hook("pre-launch", pre, hook_env, instance, emit).await;
        if code != Some(0) {
            return Err(PlayError::PreLaunch(
                code.map_or("could not run".into(), |c| format!("exit code {c}")),
            ));
        }
    }
    if let Some(wrapper) = hooks.wrapper.as_deref().filter(|c| !c.trim().is_empty()) {
        let mut parts = wrapper.split_whitespace().map(str::to_owned);
        if let Some(program) = parts.next() {
            let mut args: Vec<String> = parts.collect();
            args.push(command.program.display().to_string());
            args.append(&mut command.args);
            command.program = program.into();
            command.args = args;
        }
    }

    let game = spawn(&command)?;
    emit(PlayEvent::Started { pid: game.id() });

    let status = game
        .wait(|line| {
            if let Some(entry) = parser.push(&line) {
                emit(PlayEvent::Log {
                    level: entry.level,
                    message: entry.message,
                });
            }
        })
        .await?;
    if let Some(post) = hooks.post_exit.as_deref().filter(|c| !c.trim().is_empty()) {
        run_hook("post-exit", post, hook_env, instance, emit).await;
    }
    emit(PlayEvent::Exited {
        code: status.code(),
    });
    Ok(())
}

/// Runs a player-supplied shell command, streaming its output into the game log.
/// Returns its exit code, or `None` when it could not be started.
async fn run_hook(
    label: &str,
    command: &str,
    env: &[(String, String)],
    cwd: &Path,
    emit: &impl Fn(PlayEvent),
) -> Option<i32> {
    emit(PlayEvent::Log {
        level: "INFO".into(),
        message: format!("[{label}] {command}"),
    });
    let mut cmd = if cfg!(windows) {
        let mut c = tokio::process::Command::new("cmd");
        c.arg("/C").arg(command);
        c
    } else {
        let mut c = tokio::process::Command::new("sh");
        c.arg("-c").arg(command);
        c
    };
    cmd.envs(env.iter().map(|(k, v)| (k, v)))
        .current_dir(cwd)
        .stdin(std::process::Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    match cmd.output().await {
        Ok(output) => {
            for (bytes, level) in [(&output.stdout, "INFO"), (&output.stderr, "WARN")] {
                for line in String::from_utf8_lossy(bytes)
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                {
                    emit(PlayEvent::Log {
                        level: level.into(),
                        message: format!("[{label}] {line}"),
                    });
                }
            }
            output.status.code()
        }
        Err(e) => {
            emit(PlayEvent::Log {
                level: "ERROR".into(),
                message: format!("[{label}] could not run: {e}"),
            });
            None
        }
    }
}

/// Reports download progress at most ~10 times a second, plus the final update.
pub(crate) fn throttled(what: Download, emit: &impl Fn(PlayEvent)) -> impl Fn(Progress) + '_ {
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
