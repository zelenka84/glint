//! Thin Tauri layer: commands translate between the UI and the `glint-*` crates.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use glint_auth::{Account, AccountStore, OfflinePolicy};
use glint_core::{GlintPaths, InstanceManifest};
use glint_instance::InstanceStore;
use glint_launch::{PlayEvent, PlayRequest};
use glint_meta::mojang::VersionType;
use glint_meta::{Endpoints, MetaClient, MetaLayout};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

/// Offline names need a verified game owner in release builds only.
const OFFLINE_POLICY: OfflinePolicy =
    if cfg!(debug_assertions) { OfflinePolicy::Unrestricted } else { OfflinePolicy::RequireOwnership };

/// Event name the UI listens to while games are installing or running.
const PLAY_EVENT: &str = "play";

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

struct AppState {
    paths: GlintPaths,
    http: reqwest::Client,
    meta: MetaClient,
    instances: InstanceStore,
    accounts: Mutex<AccountStore>,
    /// Instances currently installing or running. Different instances may run side by side.
    running: Mutex<HashSet<String>>,
    /// Game process ids of running instances, for Kill Instance.
    pids: Mutex<HashMap<String, u32>>,
    /// Instances the player force-closed, so their exit is reported as a kill, not a crash.
    killed: Mutex<HashSet<String>>,
}

impl AppState {
    fn save_accounts(&self, store: &AccountStore) -> CmdResult<()> {
        store.save(&self.paths.root().join("accounts.json")).map_err(err)
    }
}

// ---- versions ----

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
async fn versions(state: State<'_, AppState>) -> CmdResult<VersionList> {
    let manifest = state.meta.manifest().await.map_err(err)?;
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

// ---- accounts ----

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
fn add_offline_account(state: State<'_, AppState>, name: String) -> CmdResult<AccountsView> {
    let mut store = state.accounts.lock().expect("accounts lock");
    let uuid = store.add_offline(name.trim(), OFFLINE_POLICY).map_err(err)?.uuid().to_owned();
    store.set_active(&uuid);
    state.save_accounts(&store)?;
    Ok(accounts_view(&store))
}

#[tauri::command]
fn set_active_account(state: State<'_, AppState>, uuid: String) -> CmdResult<AccountsView> {
    let mut store = state.accounts.lock().expect("accounts lock");
    if !store.set_active(&uuid) {
        return Err("unknown account".into());
    }
    state.save_accounts(&store)?;
    Ok(accounts_view(&store))
}

#[tauri::command]
fn remove_account(state: State<'_, AppState>, uuid: String) -> CmdResult<AccountsView> {
    let mut store = state.accounts.lock().expect("accounts lock");
    store.remove(&uuid);
    state.save_accounts(&store)?;
    Ok(accounts_view(&store))
}

// ---- instances ----

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstanceView {
    name: String,
    game_version: String,
    loader: Option<String>,
    max_memory_mb: u32,
    cover: Option<String>,
    last_played: Option<u64>,
    running: bool,
}

fn instance_view(m: &InstanceManifest, running: &HashSet<String>) -> InstanceView {
    InstanceView {
        name: m.name.clone(),
        game_version: m.game_version.clone(),
        loader: m.loader.as_ref().map(|l| format!("{:?}", l.kind)),
        max_memory_mb: m.java.max_memory_mb,
        cover: m.cover.clone(),
        last_played: m.last_played,
        running: running.contains(&m.name),
    }
}

#[tauri::command]
fn instances(state: State<'_, AppState>) -> CmdResult<Vec<InstanceView>> {
    let running = state.running.lock().expect("running lock");
    Ok(state.instances.list().map_err(err)?.iter().map(|m| instance_view(m, &running)).collect())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewInstance {
    name: String,
    game_version: String,
    cover: Option<String>,
    max_memory_mb: Option<u32>,
}

#[tauri::command]
fn create_instance(state: State<'_, AppState>, instance: NewInstance) -> CmdResult<String> {
    let name = instance.name.trim();
    let name = if name.is_empty() {
        state.instances.unique_name(&format!("Vanilla {}", instance.game_version))
    } else {
        name.to_owned()
    };
    let mut manifest = InstanceManifest::vanilla(&name, instance.game_version);
    manifest.cover = instance.cover;
    if let Some(mb) = instance.max_memory_mb {
        manifest.java.max_memory_mb = mb.clamp(1024, 65536);
    }
    state.instances.create(&manifest).map_err(err)?;
    Ok(name)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceChanges {
    name: Option<String>,
    max_memory_mb: Option<u32>,
    cover: Option<String>,
}

#[tauri::command]
fn update_instance(state: State<'_, AppState>, name: String, changes: InstanceChanges) -> CmdResult<String> {
    if state.running.lock().expect("running lock").contains(&name) {
        return Err("close the game before changing this instance".into());
    }
    let mut manifest = state.instances.get(&name).map_err(err)?;
    if let Some(mb) = changes.max_memory_mb {
        manifest.java.max_memory_mb = mb.clamp(1024, 65536);
    }
    if changes.cover.is_some() {
        manifest.cover = changes.cover;
    }
    state.instances.save(&manifest).map_err(err)?;

    match changes.name.as_deref().map(str::trim).filter(|n| !n.is_empty() && *n != name) {
        Some(new_name) => {
            state.instances.rename(&name, new_name).map_err(err)?;
            Ok(new_name.to_owned())
        }
        None => Ok(name),
    }
}

#[tauri::command]
fn delete_instance(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    if state.running.lock().expect("running lock").contains(&name) {
        return Err("close the game before deleting this instance".into());
    }
    state.instances.delete(&name).map_err(err)
}

#[tauri::command]
fn open_instance_folder(app: AppHandle, state: State<'_, AppState>, name: String) -> CmdResult<()> {
    let dir = state.instances.game_dir(&name).map_err(err)?;
    std::fs::create_dir_all(&dir).map_err(err)?;
    app.opener().open_path(dir.display().to_string(), None::<&str>).map_err(err)
}

// ---- playing ----

#[derive(Serialize, Clone)]
struct PlayEnvelope {
    instance: String,
    #[serde(flatten)]
    event: PlayEvent,
}

/// Installs and launches `instance` in the background as the active (or given) account.
/// Progress and game output arrive as `play` events tagged with the instance name;
/// the last one is `exited` or `failed`.
#[tauri::command]
fn play(app: AppHandle, state: State<'_, AppState>, instance: String, account: Option<String>) -> CmdResult<()> {
    let manifest = state.instances.get(&instance).map_err(err)?;
    let identity = {
        let store = state.accounts.lock().expect("accounts lock");
        let chosen = match &account {
            Some(uuid) => store.accounts().iter().find(|a| a.uuid() == uuid),
            None => store.active(),
        };
        chosen.map(Account::identity).ok_or("add a player name first")?
    };
    if !state.running.lock().expect("running lock").insert(instance.clone()) {
        return Err(format!("{instance} is already running"));
    }
    if let Err(e) = state.instances.mark_played(&instance) {
        tracing::warn!(error = %e, "could not record last played time");
    }

    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let request = PlayRequest {
            version_id: manifest.game_version.clone(),
            instance_name: instance.clone(),
            identity,
            max_memory_mb: manifest.java.max_memory_mb,
        };
        let emit = |event: PlayEvent| {
            if let PlayEvent::Started { pid: Some(pid) } = &event {
                state.pids.lock().expect("pids lock").insert(instance.clone(), *pid);
            }
            let _ = app.emit(PLAY_EVENT, PlayEnvelope { instance: instance.clone(), event });
        };
        let result = glint_launch::play(&state.paths, &state.meta, state.http.clone(), request, emit).await;
        state.running.lock().expect("running lock").remove(&instance);
        state.pids.lock().expect("pids lock").remove(&instance);
        if state.killed.lock().expect("killed lock").remove(&instance) {
            let _ = app.emit(PLAY_EVENT, serde_json::json!({"instance": instance, "type": "killed"}));
        } else if let Err(e) = result {
            tracing::error!(error = %e, instance, "play failed");
            let failed = serde_json::json!({"instance": instance, "type": "failed", "message": e.to_string()});
            let _ = app.emit(PLAY_EVENT, failed);
        }
    });
    Ok(())
}

/// Force-closes a running game and everything it started. Unsaved progress is lost;
/// the UI asks the player to confirm first.
#[tauri::command]
fn kill_instance(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    let pid = state.pids.lock().expect("pids lock").get(&name).copied().ok_or("this instance is not running")?;
    state.killed.lock().expect("killed lock").insert(name.clone());
    kill_process_tree(pid).map_err(|e| {
        state.killed.lock().expect("killed lock").remove(&name);
        format!("could not stop the game: {e}")
    })
}

#[cfg(windows)]
fn kill_process_tree(pid: u32) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let status = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .status()?;
    if status.success() { Ok(()) } else { Err(std::io::Error::other(format!("taskkill exited with {status}"))) }
}

#[cfg(not(windows))]
fn kill_process_tree(pid: u32) -> std::io::Result<()> {
    let status = std::process::Command::new("kill").args(["-9", &pid.to_string()]).status()?;
    if status.success() { Ok(()) } else { Err(std::io::Error::other(format!("kill exited with {status}"))) }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: &'static str,
    data_dir: String,
    dev_build: bool,
}

#[tauri::command]
fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        data_dir: state.paths.root().display().to_string(),
        dev_build: cfg!(debug_assertions),
    }
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
    let state = AppState {
        instances: InstanceStore::new(paths.clone()),
        paths,
        http,
        meta,
        accounts: Mutex::new(store),
        running: Mutex::new(HashSet::new()),
        pids: Mutex::new(HashMap::new()),
        killed: Mutex::new(HashSet::new()),
    };

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
            instances,
            create_instance,
            update_instance,
            delete_instance,
            open_instance_folder,
            play,
            kill_instance
        ])
        .run(tauri::generate_context!())
        .expect("error while running Glint");
}
