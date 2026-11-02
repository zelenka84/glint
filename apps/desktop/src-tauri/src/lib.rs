//! Thin Tauri layer: commands translate between the UI and the `glint-*` crates.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use glint_auth::{Account, AccountStore, OfflinePolicy};
use glint_core::GlintPaths;
use glint_launch::{PlayEvent, PlayRequest};
use glint_meta::mojang::VersionType;
use glint_meta::{Endpoints, MetaClient, MetaLayout};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

/// Offline names need a verified game owner in release builds only.
const OFFLINE_POLICY: OfflinePolicy =
    if cfg!(debug_assertions) { OfflinePolicy::Unrestricted } else { OfflinePolicy::RequireOwnership };

/// Event name the UI listens to while a game is installing or running.
const PLAY_EVENT: &str = "play";

struct AppState {
    paths: GlintPaths,
    http: reqwest::Client,
    meta: MetaClient,
    accounts: Mutex<AccountStore>,
    playing: AtomicBool,
}

impl AppState {
    fn accounts_path(&self) -> std::path::PathBuf {
        self.paths.root().join("accounts.json")
    }

    fn save_accounts(&self, store: &AccountStore) -> Result<(), String> {
        store.save(&self.accounts_path()).map_err(|e| e.to_string())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: &'static str,
    data_dir: String,
}

#[tauri::command]
fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo { version: env!("CARGO_PKG_VERSION"), data_dir: state.paths.root().display().to_string() }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VersionSummary {
    id: String,
    release: bool,
    release_time: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VersionList {
    latest_release: String,
    versions: Vec<VersionSummary>,
}

#[tauri::command]
async fn versions(state: State<'_, AppState>) -> Result<VersionList, String> {
    let manifest = state.meta.manifest().await.map_err(|e| e.to_string())?;
    let versions = manifest
        .versions
        .iter()
        .filter(|v| matches!(v.kind, VersionType::Release | VersionType::Snapshot))
        .map(|v| VersionSummary {
            id: v.id.clone(),
            release: v.kind == VersionType::Release,
            release_time: v.release_time.clone(),
        })
        .collect();
    Ok(VersionList { latest_release: manifest.latest.release, versions })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountView {
    name: String,
    uuid: String,
    kind: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountsView {
    accounts: Vec<AccountView>,
    active: Option<String>,
    offline_allowed: bool,
}

fn accounts_view(store: &AccountStore) -> AccountsView {
    AccountsView {
        accounts: store
            .accounts()
            .iter()
            .map(|a| AccountView {
                name: a.name().to_owned(),
                uuid: a.uuid().to_owned(),
                kind: match a {
                    Account::Offline { .. } => "offline",
                },
            })
            .collect(),
        active: store.active().map(|a| a.uuid().to_owned()),
        offline_allowed: OFFLINE_POLICY == OfflinePolicy::Unrestricted || store.ownership_verified(),
    }
}

#[tauri::command]
fn accounts(state: State<'_, AppState>) -> AccountsView {
    accounts_view(&state.accounts.lock().expect("accounts lock"))
}

#[tauri::command]
fn add_offline_account(state: State<'_, AppState>, name: String) -> Result<AccountsView, String> {
    let mut store = state.accounts.lock().expect("accounts lock");
    let uuid = store.add_offline(name.trim(), OFFLINE_POLICY).map_err(|e| e.to_string())?.uuid().to_owned();
    store.set_active(&uuid);
    state.save_accounts(&store)?;
    Ok(accounts_view(&store))
}

#[tauri::command]
fn set_active_account(state: State<'_, AppState>, uuid: String) -> Result<AccountsView, String> {
    let mut store = state.accounts.lock().expect("accounts lock");
    if !store.set_active(&uuid) {
        return Err("unknown account".into());
    }
    state.save_accounts(&store)?;
    Ok(accounts_view(&store))
}

#[tauri::command]
fn remove_account(state: State<'_, AppState>, uuid: String) -> Result<AccountsView, String> {
    let mut store = state.accounts.lock().expect("accounts lock");
    store.remove(&uuid);
    state.save_accounts(&store)?;
    Ok(accounts_view(&store))
}

/// Starts installing and launching `version` in the background. Progress and
/// game output arrive as `play` events; the last one is `exited` or `failed`.
#[tauri::command]
fn play(app: AppHandle, state: State<'_, AppState>, version: String) -> Result<(), String> {
    let identity = {
        let store = state.accounts.lock().expect("accounts lock");
        store.active().map(Account::identity).ok_or("add a player name first")?
    };
    if state.playing.swap(true, Ordering::SeqCst) {
        return Err("a game is already starting or running".into());
    }

    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let request = PlayRequest {
            instance_name: version.clone(),
            version_id: version,
            identity,
            max_memory_mb: 4096,
        };
        let emit = |event: PlayEvent| {
            let _ = app.emit(PLAY_EVENT, event);
        };
        let result = glint_launch::play(&state.paths, &state.meta, state.http.clone(), request, emit).await;
        if let Err(err) = result {
            tracing::error!(error = %err, "play failed");
            let _ = app.emit(PLAY_EVENT, serde_json::json!({"type": "failed", "message": err.to_string()}));
        }
        state.playing.store(false, Ordering::SeqCst);
    });
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let paths = GlintPaths::from_system().expect("user data directory");
    paths.ensure().expect("create Glint data folders");
    let http = glint_net::http_client();
    let meta = MetaClient::new(http.clone(), MetaLayout::new(paths.meta()), Endpoints::default());
    let store = AccountStore::load(&paths.root().join("accounts.json")).unwrap_or_else(|e| {
        tracing::error!(error = %e, "could not read accounts, starting empty");
        AccountStore::default()
    });
    let state = AppState { paths, http, meta, accounts: Mutex::new(store), playing: AtomicBool::new(false) };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            app_info,
            versions,
            accounts,
            add_offline_account,
            set_active_account,
            remove_account,
            play
        ])
        .run(tauri::generate_context!())
        .expect("error while running Glint");
}
