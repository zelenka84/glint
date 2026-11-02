//! Installs and launches a vanilla version under an offline name, end to end.
//!
//! cargo run -p glint-launch --example play -- [version] [name]
//!
//! Uses the real Glint data folder and the development offline policy.

use std::io::Write;

use glint_auth::{AccountStore, OfflinePolicy};
use glint_core::GlintPaths;
use glint_launch::{PlayEvent, PlayRequest, play};
use glint_meta::{Endpoints, MetaClient, MetaLayout};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("warn").init();
    let mut args = std::env::args().skip(1);
    let requested = args.next().filter(|v| !v.is_empty());
    let name = args.next().filter(|n| !n.is_empty()).unwrap_or_else(|| "Player".into());

    let paths = GlintPaths::from_system()?;
    paths.ensure()?;
    let http = glint_net::http_client();
    let meta = MetaClient::new(http.clone(), MetaLayout::new(paths.meta()), Endpoints::default());
    let version_id = match requested {
        Some(id) => id,
        None => meta.manifest().await?.latest.release,
    };

    let accounts_path = paths.root().join("accounts.json");
    let mut accounts = AccountStore::load(&accounts_path)?;
    let account = match accounts.accounts().iter().find(|a| a.name() == name) {
        Some(existing) => existing.clone(),
        None => accounts.add_offline(&name, OfflinePolicy::Unrestricted)?.clone(),
    };
    accounts.save(&accounts_path)?;

    let request = PlayRequest {
        instance_name: format!("dev-{version_id}"),
        version_id,
        identity: account.identity(),
        max_memory_mb: 4096,
    };
    play(&paths, &meta, http, request, |event| match event {
        PlayEvent::Downloading { what, done_files, total_files, done_bytes, total_bytes } => {
            print!("\r{what:?}: {done_files}/{total_files} files, {:.1}/{:.1} MB   ", done_bytes as f64 / 1e6, total_bytes as f64 / 1e6);
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
