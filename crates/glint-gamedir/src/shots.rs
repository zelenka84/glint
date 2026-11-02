//! Screenshots in `screenshots/`.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{Error, Result, checked_name, io, unix_seconds};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Screenshot {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    /// Unix seconds.
    pub taken: u64,
}

fn dir(game_dir: &Path) -> PathBuf {
    game_dir.join("screenshots")
}

/// PNG screenshots, newest first.
pub fn list(game_dir: &Path) -> Result<Vec<Screenshot>> {
    let dir = dir(game_dir);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(&dir, e)),
    };
    let mut shots: Vec<Screenshot> = entries
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("png"))
        })
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some(Screenshot {
                name: e.file_name().to_string_lossy().into_owned(),
                path: e.path(),
                size: meta.len(),
                taken: meta.modified().map(unix_seconds).unwrap_or(0),
            })
        })
        .collect();
    shots.sort_by(|a, b| b.taken.cmp(&a.taken).then_with(|| b.name.cmp(&a.name)));
    Ok(shots)
}

fn existing(game_dir: &Path, name: &str) -> Result<PathBuf> {
    let path = dir(game_dir).join(checked_name(name)?);
    if path.is_file() {
        Ok(path)
    } else {
        Err(Error::BadName(name.to_owned()))
    }
}

/// Renames a screenshot; `.png` is added when missing. Returns the new name.
pub fn rename(game_dir: &Path, name: &str, new_name: &str) -> Result<String> {
    let from = existing(game_dir, name)?;
    let mut new_name = new_name.trim().to_owned();
    if !new_name.to_lowercase().ends_with(".png") {
        new_name.push_str(".png");
    }
    let to = dir(game_dir).join(checked_name(&new_name)?);
    if to.exists() {
        return Err(Error::BadName(new_name));
    }
    std::fs::rename(&from, &to).map_err(|e| io(&from, e))?;
    Ok(new_name)
}

pub fn delete(game_dir: &Path, name: &str) -> Result<()> {
    let path = existing(game_dir, name)?;
    std::fs::remove_file(&path).map_err(|e| io(&path, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_renames_and_deletes() {
        let tmp = tempfile::tempdir().unwrap();
        let shots = tmp.path().join("screenshots");
        std::fs::create_dir_all(&shots).unwrap();
        std::fs::write(shots.join("2026-11-02_12.00.00.png"), b"png").unwrap();
        std::fs::write(shots.join("notes.txt"), b"x").unwrap();

        let list = list(tmp.path()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].size, 3);

        assert_eq!(
            rename(tmp.path(), "2026-11-02_12.00.00.png", "castle").unwrap(),
            "castle.png"
        );
        assert!(shots.join("castle.png").is_file());
        assert!(rename(tmp.path(), "castle.png", "../escape").is_err());

        delete(tmp.path(), "castle.png").unwrap();
        assert!(super::list(tmp.path()).unwrap().is_empty());
    }
}
