//! Commands for the instance page: worlds, servers, screenshots, logs, notes.

use std::path::PathBuf;

use glint_gamedir::logs::{self, LogFile};
use glint_gamedir::servers::{self, Server, Status};
use glint_gamedir::shots::{self, Screenshot};
use glint_gamedir::worlds::{self, World};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::{AppState, CmdResult, err};

fn game_dir(state: &AppState, instance: &str) -> CmdResult<PathBuf> {
    state.instances.game_dir(instance).map_err(err)
}

/// Changing files the running game has open would race with it.
fn ensure_stopped(state: &AppState, instance: &str) -> CmdResult<()> {
    if state
        .running
        .lock()
        .expect("running lock")
        .contains(instance)
    {
        Err("close the game first".into())
    } else {
        Ok(())
    }
}

fn reveal(app: &AppHandle, path: &std::path::Path) -> CmdResult<()> {
    app.opener().reveal_item_in_dir(path).map_err(err)
}

// ---- worlds ----

#[tauri::command]
pub fn list_worlds(state: State<'_, AppState>, instance: String) -> CmdResult<Vec<World>> {
    worlds::list(&game_dir(&state, &instance)?).map_err(err)
}

#[tauri::command]
pub fn rename_world(
    state: State<'_, AppState>,
    instance: String,
    folder: String,
    name: String,
) -> CmdResult<()> {
    ensure_stopped(&state, &instance)?;
    worlds::rename(&game_dir(&state, &instance)?, &folder, &name).map_err(err)
}

#[tauri::command]
pub async fn duplicate_world(
    state: State<'_, AppState>,
    instance: String,
    folder: String,
) -> CmdResult<String> {
    ensure_stopped(&state, &instance)?;
    let dir = game_dir(&state, &instance)?;
    tauri::async_runtime::spawn_blocking(move || worlds::duplicate(&dir, &folder))
        .await
        .map_err(err)?
        .map_err(err)
}

/// Zips the world into the instance's `backups/` folder; returns the archive path.
#[tauri::command]
pub async fn backup_world(
    state: State<'_, AppState>,
    instance: String,
    folder: String,
) -> CmdResult<String> {
    let dir = game_dir(&state, &instance)?;
    let backups = state.instances.dir(&instance).map_err(err)?.join("backups");
    tauri::async_runtime::spawn_blocking(move || worlds::backup(&dir, &folder, &backups))
        .await
        .map_err(err)?
        .map(|p| p.display().to_string())
        .map_err(err)
}

#[tauri::command]
pub fn delete_world(state: State<'_, AppState>, instance: String, folder: String) -> CmdResult<()> {
    ensure_stopped(&state, &instance)?;
    worlds::delete(&game_dir(&state, &instance)?, &folder).map_err(err)
}

#[tauri::command]
pub fn open_world_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    instance: String,
    folder: Option<String>,
) -> CmdResult<()> {
    let saves = game_dir(&state, &instance)?.join("saves");
    match folder {
        Some(folder) => reveal(&app, &saves.join(folder)),
        None => {
            std::fs::create_dir_all(&saves).map_err(err)?;
            app.opener()
                .open_path(saves.display().to_string(), None::<&str>)
                .map_err(err)
        }
    }
}

#[tauri::command]
pub fn reveal_path(app: AppHandle, path: String) -> CmdResult<()> {
    reveal(&app, std::path::Path::new(&path))
}

// ---- servers ----

#[tauri::command]
pub fn list_servers(state: State<'_, AppState>, instance: String) -> CmdResult<Vec<Server>> {
    servers::list(&game_dir(&state, &instance)?).map_err(err)
}

/// Writes the whole list; the game rewrites servers.dat itself, so not while it runs.
#[tauri::command]
pub fn save_servers(
    state: State<'_, AppState>,
    instance: String,
    servers: Vec<Server>,
) -> CmdResult<()> {
    ensure_stopped(&state, &instance)?;
    servers::save(&game_dir(&state, &instance)?, &servers).map_err(err)
}

#[tauri::command]
pub async fn ping_server(address: String) -> CmdResult<Status> {
    servers::ping(&address).await.map_err(err)
}

// ---- screenshots ----

#[tauri::command]
pub fn list_screenshots(
    state: State<'_, AppState>,
    instance: String,
) -> CmdResult<Vec<Screenshot>> {
    shots::list(&game_dir(&state, &instance)?).map_err(err)
}

#[tauri::command]
pub fn rename_screenshot(
    state: State<'_, AppState>,
    instance: String,
    name: String,
    new_name: String,
) -> CmdResult<String> {
    shots::rename(&game_dir(&state, &instance)?, &name, &new_name).map_err(err)
}

#[tauri::command]
pub fn delete_screenshot(
    state: State<'_, AppState>,
    instance: String,
    name: String,
) -> CmdResult<()> {
    shots::delete(&game_dir(&state, &instance)?, &name).map_err(err)
}

#[tauri::command]
pub fn open_screenshot(
    app: AppHandle,
    state: State<'_, AppState>,
    instance: String,
    name: String,
) -> CmdResult<()> {
    let name = glint_gamedir::checked_folder(&name).map_err(err)?;
    let path = game_dir(&state, &instance)?.join("screenshots").join(name);
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(err)
}

#[tauri::command]
pub fn open_game_subfolder(
    app: AppHandle,
    state: State<'_, AppState>,
    instance: String,
    folder: String,
) -> CmdResult<()> {
    let dir =
        game_dir(&state, &instance)?.join(glint_gamedir::checked_folder(&folder).map_err(err)?);
    std::fs::create_dir_all(&dir).map_err(err)?;
    app.opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(err)
}

// ---- log files ----

#[tauri::command]
pub fn list_log_files(state: State<'_, AppState>, instance: String) -> CmdResult<Vec<LogFile>> {
    logs::list(&game_dir(&state, &instance)?).map_err(err)
}

#[tauri::command]
pub async fn read_log_file(
    state: State<'_, AppState>,
    instance: String,
    id: String,
) -> CmdResult<String> {
    let dir = game_dir(&state, &instance)?;
    tauri::async_runtime::spawn_blocking(move || logs::read(&dir, &id))
        .await
        .map_err(err)?
        .map_err(err)
}

#[tauri::command]
pub fn delete_log_file(state: State<'_, AppState>, instance: String, id: String) -> CmdResult<()> {
    logs::delete(&game_dir(&state, &instance)?, &id).map_err(err)
}

#[tauri::command]
pub fn delete_old_logs(state: State<'_, AppState>, instance: String) -> CmdResult<usize> {
    logs::delete_old(&game_dir(&state, &instance)?).map_err(err)
}

// ---- notes ----

const NOTES_FILE: &str = "notes.md";

#[tauri::command]
pub fn read_notes(state: State<'_, AppState>, instance: String) -> CmdResult<String> {
    let path = state
        .instances
        .dir(&instance)
        .map_err(err)?
        .join(NOTES_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(err(e)),
    }
}

#[tauri::command]
pub fn save_notes(state: State<'_, AppState>, instance: String, text: String) -> CmdResult<()> {
    let path = state
        .instances
        .dir(&instance)
        .map_err(err)?
        .join(NOTES_FILE);
    if text.is_empty() {
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }
    std::fs::write(&path, text).map_err(err)
}
