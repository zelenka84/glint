//! Thin Tauri layer: commands translate between the UI and the `glint-*` crates.

use glint_core::GlintPaths;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: &'static str,
    data_dir: String,
}

/// Basic facts about this install, shown by the UI to prove the bridge works.
#[tauri::command]
fn app_info() -> Result<AppInfo, String> {
    let paths = GlintPaths::from_system().map_err(|e| e.to_string())?;
    paths.ensure().map_err(|e| e.to_string())?;
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        data_dir: paths.root().display().to_string(),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![app_info])
        .run(tauri::generate_context!())
        .expect("error while running Glint");
}
