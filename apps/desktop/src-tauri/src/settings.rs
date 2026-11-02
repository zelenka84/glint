//! Per-instance settings for the instance page, and finding Java installs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use glint_core::{GameSettings, LaunchHooks};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{AppState, CmdResult, err, split_jvm_args};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSettings {
    name: String,
    cover: Option<String>,
    max_memory_mb: u32,
    min_memory_mb: Option<u32>,
    jvm_args: String,
    java_path: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    fullscreen: bool,
    auto_join_server: Option<String>,
    auto_join_world: Option<String>,
    pre_launch: Option<String>,
    wrapper: Option<String>,
    post_exit: Option<String>,
    env: Vec<(String, String)>,
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

#[tauri::command]
pub fn instance_settings(state: State<'_, AppState>, name: String) -> CmdResult<InstanceSettings> {
    let m = state.instances.get(&name).map_err(err)?;
    Ok(InstanceSettings {
        name: m.name,
        cover: m.cover,
        max_memory_mb: m.java.max_memory_mb,
        min_memory_mb: m.java.min_memory_mb,
        jvm_args: m.java.extra_args.join(" "),
        java_path: m.java.path.map(|p| p.display().to_string()),
        width: m.game.width,
        height: m.game.height,
        fullscreen: m.game.fullscreen,
        auto_join_server: m.game.auto_join_server,
        auto_join_world: m.game.auto_join_world,
        pre_launch: m.hooks.pre_launch,
        wrapper: m.hooks.wrapper,
        post_exit: m.hooks.post_exit,
        env: m.env.into_iter().collect(),
    })
}

/// Saves everything on the Settings tab; returns the (possibly new) instance name.
#[tauri::command]
pub fn save_instance_settings(
    state: State<'_, AppState>,
    name: String,
    settings: InstanceSettings,
) -> CmdResult<String> {
    if state.running.lock().expect("running lock").contains(&name) {
        return Err("close the game before changing this instance".into());
    }
    let mut m = state.instances.get(&name).map_err(err)?;
    m.cover = settings.cover;
    m.java.max_memory_mb = settings.max_memory_mb.clamp(512, 65536);
    m.java.min_memory_mb = settings
        .min_memory_mb
        .map(|mb| mb.clamp(256, m.java.max_memory_mb));
    m.java.extra_args = split_jvm_args(&settings.jvm_args);
    m.java.path = blank_to_none(settings.java_path).map(PathBuf::from);
    m.game = GameSettings {
        width: settings.width.filter(|w| *w > 0),
        height: settings.height.filter(|h| *h > 0),
        fullscreen: settings.fullscreen,
        auto_join_server: blank_to_none(settings.auto_join_server),
        auto_join_world: blank_to_none(settings.auto_join_world),
    };
    m.hooks = LaunchHooks {
        pre_launch: blank_to_none(settings.pre_launch),
        wrapper: blank_to_none(settings.wrapper),
        post_exit: blank_to_none(settings.post_exit),
    };
    m.env = settings
        .env
        .into_iter()
        .map(|(k, v)| (k.trim().to_owned(), v))
        .filter(|(k, _)| !k.is_empty())
        .collect::<BTreeMap<_, _>>();
    state.instances.save(&m).map_err(err)?;

    let new_name = settings.name.trim();
    if !new_name.is_empty() && new_name != name {
        state.instances.rename(&name, new_name).map_err(err)?;
        Ok(new_name.to_owned())
    } else {
        Ok(name)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaInstall {
    path: String,
    version: Option<String>,
    /// Where it was found: "Glint", "JAVA_HOME", "System".
    source: &'static str,
}

fn java_exe(dir: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) { "javaw.exe" } else { "java" };
    [dir.join("bin").join(name), dir.join(name)]
        .into_iter()
        .find(|p| p.is_file())
}

/// `java -version` prints e.g. `openjdk version "21.0.5" 2024-10-15` to stderr.
pub fn java_version(path: &Path) -> Option<String> {
    let exe = if cfg!(windows) {
        path.with_file_name("java.exe")
    } else {
        path.to_owned()
    };
    let exe = if exe.is_file() { exe } else { path.to_owned() };
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("-version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let output = cmd.output().ok()?;
    let text = String::from_utf8_lossy(&output.stderr);
    let start = text.find('"')? + 1;
    let end = start + text[start..].find('"')?;
    Some(text[start..end].to_owned())
}

/// Java runtimes Glint downloaded, `JAVA_HOME`, and common install folders.
#[tauri::command]
pub async fn detect_java(state: State<'_, AppState>) -> CmdResult<Vec<JavaInstall>> {
    let glint_java = state.paths.java();
    tauri::async_runtime::spawn_blocking(move || {
        let mut found: Vec<(PathBuf, &'static str)> = Vec::new();
        let mut push = |dir: &Path, source: &'static str| {
            if let Some(exe) = java_exe(dir)
                && !found.iter().any(|(p, _)| *p == exe)
            {
                found.push((exe, source));
            }
        };
        let children = |dir: &Path| -> Vec<PathBuf> {
            std::fs::read_dir(dir)
                .map(|e| e.flatten().map(|e| e.path()).collect())
                .unwrap_or_default()
        };
        for runtime in children(&glint_java) {
            push(&runtime, "Glint");
        }
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            push(Path::new(&home), "JAVA_HOME");
        }
        let roots: Vec<PathBuf> = if cfg!(windows) {
            ["ProgramFiles", "ProgramFiles(x86)"]
                .iter()
                .filter_map(std::env::var_os)
                .flat_map(|pf| {
                    [
                        "Java",
                        "Eclipse Adoptium",
                        "Zulu",
                        "Microsoft",
                        "BellSoft",
                        "Amazon Corretto",
                    ]
                    .iter()
                    .map(move |vendor| Path::new(&pf).join(vendor))
                    .collect::<Vec<_>>()
                })
                .collect()
        } else {
            vec!["/usr/lib/jvm".into(), "/opt".into()]
        };
        for root in roots {
            for dir in children(&root) {
                push(&dir, "System");
            }
        }
        found
            .into_iter()
            .map(|(path, source)| JavaInstall {
                version: java_version(&path),
                path: path.display().to_string(),
                source,
            })
            .collect()
    })
    .await
    .map_err(err)
}

#[tauri::command]
pub async fn check_java(path: String) -> CmdResult<String> {
    let path = PathBuf::from(path.trim());
    if !path.is_file() {
        return Err("no file at that path".into());
    }
    tauri::async_runtime::spawn_blocking(move || java_version(&path))
        .await
        .map_err(err)?
        .ok_or_else(|| "this does not look like a Java executable".into())
}
