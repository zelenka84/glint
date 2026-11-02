//! Installs and launches a vanilla version under an offline name, end to end.
//!
//! cargo run -p glint-launch --example play -- [version] [name]
//!
//! Uses the real Glint data folder and the development offline policy.

use std::io::Write;

use glint_auth::{AccountStore, OfflinePolicy};
use glint_core::GlintPaths;
use glint_java::{DEFAULT_INDEX_URL, JavaRuntimes, LEGACY_COMPONENT};
use glint_launch::{LaunchOptions, LogStream, build_command, prepare, spawn};
use glint_meta::{Endpoints, MetaClient, MetaLayout, Platform, Progress};

fn progress(label: &'static str) -> impl Fn(Progress) {
    move |p| {
        if p.done_files == p.total_files || p.done_files % 200 == 0 {
            print!(
                "\r{label}: {}/{} files, {:.1}/{:.1} MB   ",
                p.done_files,
                p.total_files,
                p.done_bytes as f64 / 1e6,
                p.total_bytes as f64 / 1e6
            );
            let _ = std::io::stdout().flush();
            if p.done_files == p.total_files {
                println!();
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("warn").init();
    let mut args = std::env::args().skip(1);
    let requested = args.next().filter(|v| !v.is_empty());
    let name = args.next().filter(|n| !n.is_empty()).unwrap_or_else(|| "Player".into());

    let paths = GlintPaths::from_system()?;
    paths.ensure()?;
    let http = glint_net::http_client();
    let platform = Platform::current();

    let meta = MetaClient::new(http.clone(), MetaLayout::new(paths.meta()), Endpoints::default());
    let manifest = meta.manifest().await?;
    let id = requested.unwrap_or_else(|| manifest.latest.release.clone());
    let entry = manifest.find(&id).ok_or_else(|| format!("unknown version {id}"))?;
    let version = meta.version(entry).await?;
    let index = meta.asset_index(&version).await?;
    let plan = meta.plan(&version, index.as_ref(), &platform);
    println!("{id}: {} files, {:.0} MB", plan.files.len(), plan.total_bytes() as f64 / 1e6);
    meta.install(&plan, progress("game")).await?;

    let component = version.java_version.as_ref().map_or(LEGACY_COMPONENT, |j| j.component.as_str());
    let java = JavaRuntimes::new(http, paths.java(), DEFAULT_INDEX_URL)
        .ensure(component, &platform, progress("java"))
        .await?;

    let accounts_path = paths.root().join("accounts.json");
    let mut accounts = AccountStore::load(&accounts_path)?;
    let account = match accounts.accounts().iter().find(|a| a.name() == name) {
        Some(existing) => existing.clone(),
        None => accounts.add_offline(&name, OfflinePolicy::Unrestricted)?.clone(),
    };
    accounts.save(&accounts_path)?;

    let instance = paths.instance(&format!("dev-{id}"))?;
    let game_dir = instance.join("game");
    let natives_dir = instance.join("natives");
    let game_assets = prepare(meta.layout(), &version, &plan, index.as_ref(), &natives_dir, &game_dir).await?;

    let options = LaunchOptions {
        java,
        game_dir,
        natives_dir,
        game_assets,
        identity: account.identity(),
        max_memory_mb: 4096,
        extra_jvm_args: Vec::new(),
        resolution: None,
        quick_play_server: None,
        launcher_version: env!("CARGO_PKG_VERSION").into(),
    };
    let command = build_command(meta.layout(), &version, &plan, &options, &platform);
    println!("starting {} as {name} with {}", id, command.program.display());

    let game = spawn(&command)?;
    let status = game
        .wait(|line| match line.stream {
            LogStream::Stdout => println!("[game] {}", line.text),
            LogStream::Stderr => eprintln!("[game] {}", line.text),
        })
        .await?;
    println!("game exited with {status}");
    Ok(())
}
