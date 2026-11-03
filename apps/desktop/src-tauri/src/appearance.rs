//! The launcher's look, kept in the data folder so Dev, Beta and Public share it,
//! and the images people add for it (Home backgrounds, instance covers).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Manager, State};

use crate::{AppState, CmdResult, err};

const FILE: &str = "appearance.json";
/// Folders under `media/`, one per kind of image. The asset protocol may read all of them.
const FOLDERS: [&str; 2] = ["backgrounds", "covers"];
const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredAppearance {
    /// What the frontend saved last, or nothing on a first launch.
    data: Option<serde_json::Value>,
    /// Absolute folder that image references like `backgrounds/1.png` are relative to.
    media_dir: String,
}

fn media_dir(root: &Path) -> PathBuf {
    root.join("media")
}

#[tauri::command]
pub fn load_appearance(state: State<'_, AppState>) -> CmdResult<StoredAppearance> {
    let root = state.paths.root();
    let data = match std::fs::read_to_string(root.join(FILE)) {
        // A damaged file falls back to defaults rather than blocking the launcher.
        Ok(text) => serde_json::from_str(&text).ok(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(err(e)),
    };
    Ok(StoredAppearance {
        data,
        media_dir: media_dir(root).display().to_string(),
    })
}

#[tauri::command]
pub fn save_appearance(state: State<'_, AppState>, data: serde_json::Value) -> CmdResult<()> {
    let path = state.paths.root().join(FILE);
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(&data).map_err(err)?;
    std::fs::write(&tmp, text).map_err(err)?;
    std::fs::rename(&tmp, &path).map_err(err)
}

/// File extension for the image formats WebView2 shows, judged by the file's first bytes.
fn image_ext(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => Some("png"),
        [0xFF, 0xD8, 0xFF, ..] => Some("jpg"),
        [b'G', b'I', b'F', b'8', ..] => Some("gif"),
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => Some("webp"),
        [b'B', b'M', ..] => Some("bmp"),
        _ => None,
    }
}

/// Resolves a reference like `covers/17.png`, refusing anything that could leave `media/`.
fn media_path(root: &Path, image: &str) -> Option<PathBuf> {
    let (folder, file) = image.split_once('/')?;
    let valid = FOLDERS.contains(&folder)
        && !file.is_empty()
        && !file.starts_with('.')
        && !file.contains(['/', '\\', ':']);
    valid.then(|| media_dir(root).join(folder).join(file))
}

/// Stores an image sent as the raw request body; the `x-glint-kind` header says what it is for.
/// Returns the reference to save, e.g. `backgrounds/1730000000000.png`.
#[tauri::command]
pub fn import_image(state: State<'_, AppState>, request: Request<'_>) -> CmdResult<String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected the image file".into());
    };
    let folder = match request
        .headers()
        .get("x-glint-kind")
        .and_then(|v| v.to_str().ok())
    {
        Some("background") => "backgrounds",
        Some("cover") => "covers",
        _ => return Err("unknown image kind".into()),
    };
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("this image is larger than 32 MB".into());
    }
    let ext = image_ext(bytes).ok_or("this is not a PNG, JPEG, GIF, WebP or BMP image")?;
    let dir = media_dir(state.paths.root()).join(folder);
    std::fs::create_dir_all(&dir).map_err(err)?;
    let mut stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(err)?
        .as_millis();
    while dir.join(format!("{stamp}.{ext}")).exists() {
        stamp += 1;
    }
    let name = format!("{stamp}.{ext}");
    std::fs::write(dir.join(&name), bytes).map_err(err)?;
    Ok(format!("{folder}/{name}"))
}

/// Removes an image Glint stored earlier; a missing file is not an error.
#[tauri::command]
pub fn delete_media(state: State<'_, AppState>, image: String) -> CmdResult<()> {
    let path = media_path(state.paths.root(), &image).ok_or("not a Glint image")?;
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(err(e)),
        _ => Ok(()),
    }
}

fn theme_file_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                ' '
            }
        })
        .collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join("-");
    let stem = if clean.is_empty() {
        "theme".to_owned()
    } else {
        clean
    };
    format!("{stem}.glint-theme.json")
}

/// Writes a theme to the Downloads folder (or the data folder without one); returns the file path.
#[tauri::command]
pub fn export_theme(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    contents: String,
) -> CmdResult<String> {
    let dir = app
        .path()
        .download_dir()
        .unwrap_or_else(|_| state.paths.root().to_owned());
    let path = dir.join(theme_file_name(&name));
    std::fs::write(&path, contents).map_err(err)?;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_image_formats() {
        assert_eq!(image_ext(b"\x89PNG\r\n\x1a\n...."), Some("png"));
        assert_eq!(image_ext(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(image_ext(b"GIF89a"), Some("gif"));
        assert_eq!(image_ext(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(image_ext(b"BM...."), Some("bmp"));
        assert_eq!(image_ext(b"<svg xmlns"), None);
        assert_eq!(image_ext(b""), None);
    }

    #[test]
    fn media_paths_stay_inside_media() {
        let root = Path::new("/data/Glint");
        assert_eq!(
            media_path(root, "covers/1.png"),
            Some(root.join("media").join("covers").join("1.png"))
        );
        assert_eq!(media_path(root, "covers/../../accounts.json"), None);
        assert_eq!(media_path(root, "covers/..\\x"), None);
        assert_eq!(media_path(root, "instances/1.png"), None);
        assert_eq!(media_path(root, "covers/"), None);
        assert_eq!(media_path(root, "covers/C:x"), None);
        assert_eq!(media_path(root, "1.png"), None);
    }

    #[test]
    fn theme_file_names_are_safe() {
        assert_eq!(theme_file_name("Deep Ocean"), "Deep-Ocean.glint-theme.json");
        assert_eq!(theme_file_name("../evil"), "evil.glint-theme.json");
        assert_eq!(theme_file_name("  "), "theme.glint-theme.json");
    }
}
