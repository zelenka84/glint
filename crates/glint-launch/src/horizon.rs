//! The "Play" flow for Horizon instances: Mojang's 26.3 files, Java 25 for the
//! preparer, the Horizon build, its prepared resources, then the native game.

use std::time::{Duration, Instant};

use glint_core::GlintPaths;
use glint_horizon::{Build, Horizon, LaunchApi, MINECRAFT_VERSION, PrepareEvent, PrepareInputs};
use glint_java::{DEFAULT_INDEX_URL, JavaRuntimes, LEGACY_COMPONENT};
use glint_meta::mojang::VersionInfo;
use glint_meta::{MetaClient, Platform};
use glint_net::{Downloader, ExpectedHash};

use crate::LaunchCommand;
use crate::session::{Download, PlayError, PlayEvent, PlayRequest, QuickPlay, run_game, throttled};

pub(crate) async fn play(
    paths: &GlintPaths,
    meta: &MetaClient,
    http: reqwest::Client,
    request: PlayRequest,
    emit: impl Fn(PlayEvent),
) -> Result<(), PlayError> {
    let platform = Platform::current();
    emit(PlayEvent::Resolving {
        version: format!("Horizon ({MINECRAFT_VERSION})"),
    });

    let horizon = Horizon::new(http.clone(), paths.horizon(), glint_horizon::RELEASE_URL);
    let build = horizon
        .ensure_build(|p| {
            emit(PlayEvent::Downloading {
                what: Download::Horizon,
                done_files: usize::from(p.done_bytes == p.total_bytes),
                total_files: 1,
                done_bytes: p.done_bytes,
                total_bytes: p.total_bytes,
            })
        })
        .await?;
    emit(log("INFO", format!("Horizon {}", build.name)));
    let api = build.launch_api().await?;

    emit(PlayEvent::Preparing { fraction: None });
    if !build.is_prepared().await? {
        let inputs = mojang_files(paths, meta, &http, &platform, &emit).await?;
        prepare(&build, &inputs, &emit).await?;
    }

    let instance = paths.instance(&request.instance_name)?;
    let game_dir = instance.join("game");
    tokio::fs::create_dir_all(&game_dir)
        .await
        .map_err(|e| crate::io(&game_dir, e))?;
    let mut command = LaunchCommand {
        program: build.client(),
        args: game_args(&api, &build, &game_dir, &request),
        cwd: game_dir.clone(),
        env: request.env.clone(),
        stdin: None,
    };
    if api.supports("--session-stdin") {
        command.args.push("--session-stdin".into());
        command.stdin = Some(session_json(&request.identity));
    }

    let hook_env = {
        let mut env = request.env.clone();
        env.extend([
            ("INST_NAME".into(), request.instance_name.clone()),
            ("INST_ID".into(), request.instance_name.clone()),
            ("INST_DIR".into(), instance.display().to_string()),
            ("INST_MC_DIR".into(), game_dir.display().to_string()),
            ("INST_ENGINE".into(), "horizon".into()),
        ]);
        env
    };
    run_game(
        command,
        crate::LogParser::native(),
        &request.hooks,
        &hook_env,
        &instance,
        &emit,
    )
    .await
}

/// Installs Mojang's files for the version Horizon plays, its server jar and the Java
/// the preparer runs on. Done only when the resources need preparing.
async fn mojang_files(
    paths: &GlintPaths,
    meta: &MetaClient,
    http: &reqwest::Client,
    platform: &Platform,
    emit: &impl Fn(PlayEvent),
) -> Result<PrepareInputs, PlayError> {
    let manifest = meta.manifest().await?;
    let entry = manifest
        .find(MINECRAFT_VERSION)
        .ok_or_else(|| PlayError::UnknownVersion(MINECRAFT_VERSION.into()))?;
    let version = meta.version(entry).await?;
    let index = meta.asset_index(&version).await?;
    let plan = meta.plan(&version, index.as_ref(), platform);
    meta.install(&plan, throttled(Download::Game, emit)).await?;
    let server_jar = server_jar(meta, http, &version).await?;

    let component = version
        .java_version
        .as_ref()
        .map_or(LEGACY_COMPONENT, |j| j.component.as_str());
    let java = JavaRuntimes::new(http.clone(), paths.java(), DEFAULT_INDEX_URL)
        .ensure(component, platform, throttled(Download::Java, emit))
        .await?;
    // `<home>/bin/java`; the preparer wants the home.
    let java_home = java
        .parent()
        .and_then(|bin| bin.parent())
        .ok_or_else(|| PlayError::JavaMissing(java.clone()))?
        .to_owned();

    let layout = meta.layout();
    Ok(PrepareInputs {
        version_json: layout.version_json(&version.id),
        client_jar: layout.client_jar(&version.id),
        libraries_dir: layout.libraries(),
        assets_dir: layout.assets(),
        server_jar,
        java_home,
    })
}

/// Downloads the dedicated server jar next to the client jar; no launcher keeps it otherwise.
async fn server_jar(
    meta: &MetaClient,
    http: &reqwest::Client,
    version: &VersionInfo,
) -> Result<std::path::PathBuf, PlayError> {
    let server = version
        .downloads
        .as_ref()
        .and_then(|d| d.server.as_ref())
        .ok_or_else(|| PlayError::NoServerJar(version.id.clone()))?;
    let dest = meta
        .layout()
        .version_dir(&version.id)
        .join(format!("{}-server.jar", version.id));
    Downloader::new(http.clone(), 1)
        .download(
            &server.url,
            &dest,
            Some(&ExpectedHash::Sha1(server.sha1.clone())),
        )
        .await
        .map_err(glint_meta::Error::from)?;
    Ok(dest)
}

async fn prepare(
    build: &Build,
    inputs: &PrepareInputs,
    emit: &impl Fn(PlayEvent),
) -> Result<(), PlayError> {
    emit(log(
        "INFO",
        "Preparing Horizon from your Minecraft files. The first time takes a few minutes.".into(),
    ));
    let last = std::sync::Mutex::new(Instant::now() - Duration::from_secs(1));
    build
        .prepare(inputs, |event| match event {
            PrepareEvent::Message(message) => emit(log("INFO", message)),
            PrepareEvent::Fraction(fraction) => {
                let mut last = last.lock().expect("progress lock");
                if fraction >= 1.0 || last.elapsed() >= Duration::from_millis(100) {
                    *last = Instant::now();
                    emit(PlayEvent::Preparing {
                        fraction: Some(fraction),
                    });
                }
            }
        })
        .await?;
    Ok(())
}

/// The game's command line, with only the options this build lists.
fn game_args(
    api: &LaunchApi,
    build: &Build,
    game_dir: &std::path::Path,
    request: &PlayRequest,
) -> Vec<String> {
    let mut args = Vec::new();
    let mut push = |option: &str, value: Option<String>| {
        if api.supports(option) {
            args.push(option.to_owned());
            args.extend(value);
        }
    };
    push("--resources", Some(build.resources().display().to_string()));
    push("--game-dir", Some(game_dir.display().to_string()));
    // Logs and crash reports land in the game folder, as Java's do.
    push("--home", Some(game_dir.display().to_string()));
    let game = &request.game;
    if let (Some(width), Some(height)) = (game.width, game.height) {
        push("--width", Some(width.to_string()));
        push("--height", Some(height.to_string()));
    }
    if game.fullscreen {
        push("--fullscreen", None);
    }
    match &request.quick_play {
        Some(QuickPlay::World(world)) => push("--quickPlaySingleplayer", Some(world.clone())),
        Some(QuickPlay::Server(server)) => push("--quickPlayMultiplayer", Some(server.clone())),
        None => {}
    }
    args
}

/// The account handed to the game on standard input, never on the command line.
fn session_json(identity: &glint_auth::LaunchIdentity) -> String {
    let mut session = serde_json::json!({
        "name": identity.name,
        "uuid": identity.uuid,
        "access_token": identity.access_token,
    });
    if identity.user_type == "msa" {
        session["user_type"] = "msa".into();
        if !identity.xuid.is_empty() {
            session["xuid"] = identity.xuid.clone().into();
        }
    }
    session.to_string()
}

fn log(level: &str, message: String) -> PlayEvent {
    PlayEvent::Log {
        level: level.into(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api(options: &[&str]) -> LaunchApi {
        serde_json::from_value(serde_json::json!({
            "launch_api": "0.1.0",
            "options": options.iter().map(|o| serde_json::json!({"name": o})).collect::<Vec<_>>(),
        }))
        .unwrap()
    }

    fn request() -> PlayRequest {
        PlayRequest {
            engine: glint_core::Engine::Horizon,
            version_id: MINECRAFT_VERSION.into(),
            instance_name: "h".into(),
            identity: glint_auth::LaunchIdentity {
                name: "Steve".into(),
                uuid: "abc".into(),
                access_token: "0".into(),
                user_type: "legacy".into(),
                xuid: String::new(),
                auth_server: None,
            },
            max_memory_mb: 4096,
            loader: None,
            extra_jvm_args: Vec::new(),
            min_memory_mb: None,
            java_path: None,
            game: glint_core::GameSettings {
                width: Some(1280),
                height: Some(720),
                fullscreen: true,
                ..Default::default()
            },
            quick_play: Some(QuickPlay::Server("mc.example.org".into())),
            hooks: Default::default(),
            env: Vec::new(),
        }
    }

    #[test]
    fn passes_only_options_the_build_lists() {
        let build = Build::new("b", "b", "/h/builds/b".into(), "/h/prepared/b".into());
        let args = game_args(
            &api(&[
                "--resources",
                "--game-dir",
                "--fullscreen",
                "--quickPlayMultiplayer",
            ]),
            &build,
            std::path::Path::new("/i/game"),
            &request(),
        );
        let resources = build.resources().display().to_string();
        assert_eq!(
            args,
            [
                "--resources",
                resources.as_str(),
                "--game-dir",
                "/i/game",
                "--fullscreen",
                "--quickPlayMultiplayer",
                "mc.example.org"
            ]
        );
    }

    #[test]
    fn offline_session_has_no_user_type() {
        let json: serde_json::Value =
            serde_json::from_str(&session_json(&request().identity)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"name": "Steve", "uuid": "abc", "access_token": "0"})
        );
    }
}
