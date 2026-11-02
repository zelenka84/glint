//! Thin Tauri layer: commands translate between the UI and the `glint-*` crates.

mod gamedir;
mod settings;

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use glint_auth::ely::ElyClient;
use glint_auth::{Account, AccountStore, OfflinePolicy, keychain};
use glint_core::{GlintPaths, InstanceManifest, Loader, LoaderKind};
use glint_instance::InstanceStore;
use glint_launch::{PlayEvent, PlayRequest, QuickPlay};
use glint_meta::mojang::VersionType;
use glint_meta::{Endpoints, MetaClient, MetaLayout};
use glint_modrinth::{
    ContentKind, InstalledMod, ModManager, ModrinthApi, SearchQuery, SearchResults, Target, Update,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

/// Offline names need a verified game owner in public release builds only.
const OFFLINE_POLICY: OfflinePolicy = if cfg!(any(debug_assertions, feature = "local")) {
    OfflinePolicy::Unrestricted
} else {
    OfflinePolicy::RequireOwnership
};

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
    modrinth: ModrinthApi,
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
        store
            .save(&self.paths.root().join("accounts.json"))
            .map_err(err)
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
    Ok(VersionList {
        latest_release: manifest.latest.release,
        versions,
    })
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
                    Account::Ely { .. } => "ely",
                },
            })
            .collect(),
        active: store.active().map(|a| a.uuid().to_owned()),
        offline_allowed: OFFLINE_POLICY == OfflinePolicy::Unrestricted
            || store.ownership_verified(),
    }
}

#[tauri::command]
fn accounts(state: State<'_, AppState>) -> AccountsView {
    accounts_view(&state.accounts.lock().expect("accounts lock"))
}

#[tauri::command]
fn add_offline_account(state: State<'_, AppState>, name: String) -> CmdResult<AccountsView> {
    let mut store = state.accounts.lock().expect("accounts lock");
    let uuid = store
        .add_offline(name.trim(), OFFLINE_POLICY)
        .map_err(err)?
        .uuid()
        .to_owned();
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
async fn remove_account(state: State<'_, AppState>, uuid: String) -> CmdResult<AccountsView> {
    let was_ely = {
        let store = state.accounts.lock().expect("accounts lock");
        store
            .accounts()
            .iter()
            .any(|a| a.uuid() == uuid && a.needs_session())
    };
    if was_ely {
        // Sign out on Ely.by too; the account is removed locally even if that fails.
        if let Ok(Some(session)) = keychain::load_ely(&uuid) {
            let _ = ElyClient::new(state.http.clone())
                .invalidate(&session)
                .await;
        }
        keychain::delete_ely(&uuid).map_err(err)?;
    }
    let mut store = state.accounts.lock().expect("accounts lock");
    store.remove(&uuid);
    state.save_accounts(&store)?;
    Ok(accounts_view(&store))
}

/// Signs in to Ely.by. Returns the error text `TWO_FACTOR_REQUIRED` when the
/// account needs an authenticator code; call again with `totp` set.
#[tauri::command]
async fn ely_sign_in(
    state: State<'_, AppState>,
    login: String,
    password: String,
    totp: Option<String>,
) -> CmdResult<AccountsView> {
    if OFFLINE_POLICY == OfflinePolicy::RequireOwnership
        && !state
            .accounts
            .lock()
            .expect("accounts lock")
            .ownership_verified()
    {
        return Err(glint_auth::Error::OwnershipRequired.to_string());
    }
    let client_token = {
        let mut store = state.accounts.lock().expect("accounts lock");
        let token = store.client_token();
        state.save_accounts(&store)?;
        token
    };
    let session = ElyClient::new(state.http.clone())
        .authenticate(&login, &password, totp.as_deref(), &client_token)
        .await
        .map_err(|e| match e {
            glint_auth::Error::TwoFactorRequired => "TWO_FACTOR_REQUIRED".to_owned(),
            other => other.to_string(),
        })?;
    keychain::save_ely(&session).map_err(err)?;

    let mut store = state.accounts.lock().expect("accounts lock");
    store
        .add_ely(&session.name, &session.uuid, OFFLINE_POLICY)
        .map_err(err)?;
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
    loader_version: Option<String>,
    extra_jvm_args: String,
    max_memory_mb: u32,
    cover: Option<String>,
    last_played: Option<u64>,
    running: bool,
}

fn instance_view(m: &InstanceManifest, running: &HashSet<String>) -> InstanceView {
    InstanceView {
        name: m.name.clone(),
        game_version: m.game_version.clone(),
        loader: m
            .loader
            .as_ref()
            .map(|l| glint_loader::display_name(l.kind).to_owned()),
        loader_version: m.loader.as_ref().map(|l| l.version.clone()),
        extra_jvm_args: m.java.extra_args.join(" "),
        max_memory_mb: m.java.max_memory_mb,
        cover: m.cover.clone(),
        last_played: m.last_played,
        running: running.contains(&m.name),
    }
}

#[tauri::command]
fn instances(state: State<'_, AppState>) -> CmdResult<Vec<InstanceView>> {
    let running = state.running.lock().expect("running lock");
    Ok(state
        .instances
        .list()
        .map_err(err)?
        .iter()
        .map(|m| instance_view(m, &running))
        .collect())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewInstance {
    name: String,
    game_version: String,
    cover: Option<String>,
    max_memory_mb: Option<u32>,
    loader: Option<LoaderChoice>,
    extra_jvm_args: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoaderChoice {
    kind: String,
    version: String,
}

fn parse_loader_kind(kind: &str) -> CmdResult<LoaderKind> {
    match kind {
        "fabric" => Ok(LoaderKind::Fabric),
        "quilt" => Ok(LoaderKind::Quilt),
        "forge" => Ok(LoaderKind::Forge),
        "neoforge" => Ok(LoaderKind::NeoForge),
        other => Err(format!("unknown loader {other}")),
    }
}

/// JVM arguments typed as one line; quotes are not interpreted, spaces separate.
fn split_jvm_args(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_owned).collect()
}

#[tauri::command]
async fn loader_versions(
    state: State<'_, AppState>,
    kind: String,
    game_version: String,
) -> CmdResult<Vec<glint_loader::LoaderVersion>> {
    glint_loader::loader_versions(&state.http, parse_loader_kind(&kind)?, &game_version)
        .await
        .map_err(err)
}

#[tauri::command]
fn create_instance(state: State<'_, AppState>, instance: NewInstance) -> CmdResult<String> {
    let name = instance.name.trim();
    let loader = match &instance.loader {
        Some(choice) => Some(Loader {
            kind: parse_loader_kind(&choice.kind)?,
            version: choice.version.clone(),
        }),
        None => None,
    };
    let name = if name.is_empty() {
        let flavour = loader
            .as_ref()
            .map_or("Vanilla", |l| glint_loader::display_name(l.kind));
        state
            .instances
            .unique_name(&format!("{flavour} {}", instance.game_version))
    } else {
        name.to_owned()
    };
    let mut manifest = InstanceManifest::vanilla(&name, instance.game_version);
    manifest.loader = loader;
    manifest.cover = instance.cover;
    if let Some(args) = &instance.extra_jvm_args {
        manifest.java.extra_args = split_jvm_args(args);
    }
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
fn update_instance(
    state: State<'_, AppState>,
    name: String,
    changes: InstanceChanges,
) -> CmdResult<String> {
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

    match changes
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty() && *n != name)
    {
        Some(new_name) => {
            state.instances.rename(&name, new_name).map_err(err)?;
            Ok(new_name.to_owned())
        }
        None => Ok(name),
    }
}

/// Switches the Minecraft version and/or mod loader. Mods stay in place; the UI
/// warns that they may not fit the new version.
#[tauri::command]
fn change_version(
    state: State<'_, AppState>,
    name: String,
    game_version: String,
    loader: Option<LoaderChoice>,
) -> CmdResult<()> {
    if state.running.lock().expect("running lock").contains(&name) {
        return Err("close the game before changing this instance".into());
    }
    let mut manifest = state.instances.get(&name).map_err(err)?;
    manifest.game_version = game_version;
    manifest.loader = match loader {
        Some(choice) => Some(Loader {
            kind: parse_loader_kind(&choice.kind)?,
            version: choice.version,
        }),
        None => None,
    };
    state.instances.save(&manifest).map_err(err)
}

#[tauri::command]
fn open_libraries_folder(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let dir = state.paths.meta().join("libraries");
    std::fs::create_dir_all(&dir).map_err(err)?;
    app.opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(err)
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
    app.opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(err)
}

// ---- playing ----

#[derive(Serialize, Clone)]
struct PlayEnvelope {
    instance: String,
    #[serde(flatten)]
    event: PlayEvent,
}

#[derive(Deserialize)]
struct PlayTarget {
    world: Option<String>,
    server: Option<String>,
}

/// Installs and launches `instance` in the background as the active (or given) account.
/// Progress and game output arrive as `play` events tagged with the instance name;
/// the last one is `exited` or `failed`.
#[tauri::command]
async fn play(
    app: AppHandle,
    state: State<'_, AppState>,
    instance: String,
    account: Option<String>,
    target: Option<PlayTarget>,
) -> CmdResult<()> {
    let manifest = state.instances.get(&instance).map_err(err)?;
    // An explicit world or server wins over the instance's auto-join setting.
    let quick_play = match target {
        Some(PlayTarget { world: Some(w), .. }) => Some(QuickPlay::World(w)),
        Some(PlayTarget {
            server: Some(s), ..
        }) => Some(QuickPlay::Server(s)),
        _ => manifest
            .game
            .auto_join_server
            .clone()
            .map(QuickPlay::Server)
            .or(manifest.game.auto_join_world.clone().map(QuickPlay::World)),
    };
    let chosen = {
        let store = state.accounts.lock().expect("accounts lock");
        let chosen = match &account {
            Some(uuid) => store.accounts().iter().find(|a| a.uuid() == uuid),
            None => store.active(),
        };
        chosen.cloned().ok_or("add a player name first")?
    };
    let identity = if chosen.needs_session() {
        let session = keychain::load_ely(chosen.uuid())
            .map_err(err)?
            .ok_or(glint_auth::Error::SessionExpired.to_string())?;
        let session = ElyClient::new(state.http.clone())
            .ensure_valid(session)
            .await
            .map_err(err)?;
        keychain::save_ely(&session).map_err(err)?;
        chosen.identity(Some(&session.access_token))
    } else {
        chosen.identity(None)
    };
    if !state
        .running
        .lock()
        .expect("running lock")
        .insert(instance.clone())
    {
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
            loader: manifest.loader.clone(),
            extra_jvm_args: manifest.java.extra_args.clone(),
            min_memory_mb: manifest.java.min_memory_mb,
            java_path: manifest.java.path.clone(),
            game: manifest.game.clone(),
            quick_play,
            hooks: manifest.hooks.clone(),
            env: manifest.env.clone().into_iter().collect(),
        };
        let emit = |event: PlayEvent| {
            match &event {
                PlayEvent::Started { pid: Some(pid) } => {
                    state
                        .pids
                        .lock()
                        .expect("pids lock")
                        .insert(instance.clone(), *pid);
                }
                // Clear the running state before the UI hears about the exit, or its
                // refresh would still see the instance as running.
                PlayEvent::Exited { .. } => {
                    state
                        .running
                        .lock()
                        .expect("running lock")
                        .remove(&instance);
                    state.pids.lock().expect("pids lock").remove(&instance);
                }
                _ => {}
            }
            let _ = app.emit(
                PLAY_EVENT,
                PlayEnvelope {
                    instance: instance.clone(),
                    event,
                },
            );
        };
        let result =
            glint_launch::play(&state.paths, &state.meta, state.http.clone(), request, emit).await;
        state
            .running
            .lock()
            .expect("running lock")
            .remove(&instance);
        state.pids.lock().expect("pids lock").remove(&instance);
        if state.killed.lock().expect("killed lock").remove(&instance) {
            let _ = app.emit(
                PLAY_EVENT,
                serde_json::json!({"instance": instance, "type": "killed"}),
            );
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
    let pid = state
        .pids
        .lock()
        .expect("pids lock")
        .get(&name)
        .copied()
        .ok_or("this instance is not running")?;
    state
        .killed
        .lock()
        .expect("killed lock")
        .insert(name.clone());
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
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "taskkill exited with {status}"
        )))
    }
}

#[cfg(not(windows))]
fn kill_process_tree(pid: u32) -> std::io::Result<()> {
    let status = std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("kill exited with {status}")))
    }
}

// ---- content: mods, resource packs, shader packs ----

fn content_target(state: &AppState, instance: &str) -> CmdResult<(std::path::PathBuf, Target)> {
    let manifest = state.instances.get(instance).map_err(err)?;
    let dir = state.instances.dir(instance).map_err(err)?;
    Ok((
        dir,
        Target {
            game_version: manifest.game_version,
            loader: manifest.loader.map(|l| l.kind),
        },
    ))
}

fn content_manager<'a>(
    state: &'a AppState,
    dir: std::path::PathBuf,
    kind: ContentKind,
) -> ModManager<'a> {
    ModManager::new(&state.modrinth, state.http.clone(), dir, kind)
}

#[tauri::command]
async fn search_content(
    state: State<'_, AppState>,
    instance: Option<String>,
    query: String,
    kind: ContentKind,
    offset: Option<u32>,
) -> CmdResult<SearchResults> {
    let target = match instance {
        Some(name) => Some(content_target(&state, &name)?.1),
        None => None,
    };
    let query = SearchQuery {
        text: query,
        project_type: kind.project_type(),
        target,
        category: None,
        offset: offset.unwrap_or(0),
        limit: 30,
    };
    state.modrinth.search(&query).await.map_err(err)
}

#[tauri::command]
async fn install_content(
    state: State<'_, AppState>,
    instance: String,
    kind: ContentKind,
    project_id: String,
) -> CmdResult<Vec<String>> {
    let (dir, target) = content_target(&state, &instance)?;
    content_manager(&state, dir, kind)
        .install(&project_id, &target)
        .await
        .map_err(err)
}

#[tauri::command]
async fn update_content(
    state: State<'_, AppState>,
    instance: String,
    kind: ContentKind,
    project_id: String,
) -> CmdResult<Vec<String>> {
    let (dir, target) = content_target(&state, &instance)?;
    content_manager(&state, dir, kind)
        .update(&project_id, &target)
        .await
        .map_err(err)
}

#[tauri::command]
async fn content_updates(
    state: State<'_, AppState>,
    instance: String,
    kind: ContentKind,
) -> CmdResult<Vec<Update>> {
    let (dir, target) = content_target(&state, &instance)?;
    content_manager(&state, dir, kind)
        .check_updates(&target)
        .await
        .map_err(err)
}

#[tauri::command]
fn installed_content(
    state: State<'_, AppState>,
    instance: String,
    kind: ContentKind,
) -> CmdResult<Vec<InstalledMod>> {
    let dir = state.instances.dir(&instance).map_err(err)?;
    content_manager(&state, dir, kind).list().map_err(err)
}

#[tauri::command]
async fn add_content_files(
    state: State<'_, AppState>,
    instance: String,
    kind: ContentKind,
    paths: Vec<String>,
) -> CmdResult<Vec<String>> {
    let dir = state.instances.dir(&instance).map_err(err)?;
    let manager = content_manager(&state, dir, kind);
    let mut added = Vec::new();
    for path in paths {
        added.push(
            manager
                .add_file(std::path::Path::new(&path))
                .await
                .map_err(err)?,
        );
    }
    Ok(added)
}

#[tauri::command]
async fn remove_content(
    state: State<'_, AppState>,
    instance: String,
    kind: ContentKind,
    filename: String,
) -> CmdResult<()> {
    let dir = state.instances.dir(&instance).map_err(err)?;
    content_manager(&state, dir, kind)
        .remove(&filename)
        .await
        .map_err(err)
}

#[tauri::command]
async fn set_content_enabled(
    state: State<'_, AppState>,
    instance: String,
    kind: ContentKind,
    filename: String,
    enabled: bool,
) -> CmdResult<()> {
    let dir = state.instances.dir(&instance).map_err(err)?;
    content_manager(&state, dir, kind)
        .set_enabled(&filename, enabled)
        .await
        .map_err(err)
}

#[tauri::command]
fn open_content_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    instance: String,
    kind: ContentKind,
) -> CmdResult<()> {
    let dir = state
        .instances
        .game_dir(&instance)
        .map_err(err)?
        .join(kind.folder());
    std::fs::create_dir_all(&dir).map_err(err)?;
    app.opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(err)
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
        dev_build: cfg!(any(debug_assertions, feature = "local")),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let paths = GlintPaths::from_system().expect("user data directory");
    paths.ensure().expect("create Glint data folders");
    let http = glint_net::http_client();
    let meta = MetaClient::new(
        http.clone(),
        MetaLayout::new(paths.meta()),
        Endpoints::default(),
    );
    let store = AccountStore::load(&paths.root().join("accounts.json")).unwrap_or_else(|e| {
        tracing::error!(error = %e, "could not read accounts, starting empty");
        AccountStore::default()
    });
    let state = AppState {
        instances: InstanceStore::new(paths.clone()),
        paths,
        modrinth: ModrinthApi::new(http.clone()),
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
            ely_sign_in,
            instances,
            create_instance,
            update_instance,
            delete_instance,
            change_version,
            open_libraries_folder,
            open_instance_folder,
            play,
            kill_instance,
            loader_versions,
            search_content,
            install_content,
            update_content,
            content_updates,
            installed_content,
            add_content_files,
            remove_content,
            set_content_enabled,
            open_content_folder,
            gamedir::list_worlds,
            gamedir::rename_world,
            gamedir::duplicate_world,
            gamedir::backup_world,
            gamedir::delete_world,
            gamedir::open_world_folder,
            gamedir::reveal_path,
            gamedir::list_servers,
            gamedir::save_servers,
            gamedir::ping_server,
            gamedir::list_screenshots,
            gamedir::rename_screenshot,
            gamedir::delete_screenshot,
            gamedir::open_screenshot,
            gamedir::open_game_subfolder,
            gamedir::list_log_files,
            gamedir::read_log_file,
            gamedir::delete_log_file,
            gamedir::delete_old_logs,
            gamedir::read_notes,
            gamedir::save_notes,
            settings::instance_settings,
            settings::save_instance_settings,
            settings::detect_java,
            settings::check_java
        ])
        .run(tauri::generate_context!())
        .expect("error while running Glint");
}
