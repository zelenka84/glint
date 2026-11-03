//! Installs and launches a vanilla version under an offline name, end to end.
//!
//! cargo run -p glint-launch --example play -- [version] [name] [loader[:version]]
//!
//! The version `horizon` plays on the Horizon engine instead of Java.
//!
//! Uses the real Glint data folder and the development offline policy.

use std::io::Write;

use glint_auth::{AccountStore, OfflinePolicy};
use glint_core::{GlintPaths, Loader, LoaderKind};
use glint_launch::{PlayEvent, PlayRequest, play};
use glint_meta::{Endpoints, MetaClient, MetaLayout};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("warn").init();
    let mut args = std::env::args().skip(1);
    let requested = args.next().filter(|v| !v.is_empty());
    let name = args
        .next()
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Player".into());

    let paths = GlintPaths::from_system()?;
    paths.ensure()?;
    let http = glint_net::http_client();
    let meta = MetaClient::new(
        http.clone(),
        MetaLayout::new(paths.meta()),
        Endpoints::default(),
    );
    let engine = if requested.as_deref() == Some("horizon") {
        glint_core::Engine::Horizon
    } else {
        glint_core::Engine::Java
    };
    let version_id = match requested {
        Some(_) if engine == glint_core::Engine::Horizon => glint_horizon::MINECRAFT_VERSION.into(),
        Some(id) => id,
        None => meta.manifest().await?.latest.release,
    };

    // Optional third argument: `fabric`, `quilt`, `forge`, `neoforge`, or `kind:version`.
    let loader = match args.next().filter(|l| !l.is_empty()) {
        None => None,
        Some(spec) => {
            let (kind, version) = spec
                .split_once(':')
                .map_or((spec.as_str(), None), |(k, v)| (k, Some(v.to_owned())));
            let kind = match kind {
                "fabric" => LoaderKind::Fabric,
                "quilt" => LoaderKind::Quilt,
                "forge" => LoaderKind::Forge,
                "neoforge" => LoaderKind::NeoForge,
                other => return Err(format!("unknown loader {other}").into()),
            };
            let version = match version {
                Some(v) => v,
                None => {
                    let builds = glint_loader::loader_versions(&http, kind, &version_id).await?;
                    builds
                        .iter()
                        .find(|b| b.recommended)
                        .unwrap_or(&builds[0])
                        .id
                        .clone()
                }
            };
            println!("loader: {kind:?} {version}");
            Some(Loader { kind, version })
        }
    };

    let accounts_path = paths.root().join("accounts.json");
    let mut accounts = AccountStore::load(&accounts_path)?;
    let account = match accounts.accounts().iter().find(|a| a.name() == name) {
        Some(existing) => existing.clone(),
        None => accounts
            .add_offline(&name, OfflinePolicy::Unrestricted)?
            .clone(),
    };
    accounts.save(&accounts_path)?;

    let request = PlayRequest {
        engine,
        instance_name: match &loader {
            _ if engine == glint_core::Engine::Horizon => "dev-horizon".into(),
            Some(l) => format!("dev-{version_id}-{:?}", l.kind).to_lowercase(),
            None => format!("dev-{version_id}"),
        },
        version_id,
        identity: account.identity(None),
        max_memory_mb: 4096,
        loader,
        extra_jvm_args: Vec::new(),
        min_memory_mb: None,
        java_path: None,
        game: Default::default(),
        quick_play: None,
        hooks: Default::default(),
        env: Vec::new(),
    };
    play(&paths, &meta, http, request, |event| match event {
        PlayEvent::Downloading {
            what,
            done_files,
            total_files,
            done_bytes,
            total_bytes,
        } => {
            print!(
                "\r{what:?}: {done_files}/{total_files} files, {:.1}/{:.1} MB   ",
                done_bytes as f64 / 1e6,
                total_bytes as f64 / 1e6
            );
            let _ = std::io::stdout().flush();
            if done_files == total_files {
                println!();
            }
        }
        PlayEvent::Log { level, message } => println!("[{level}] {message}"),
        other => println!("{other:?}"),
    })
    .await?;
    Ok(())
}
