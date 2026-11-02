//! Single-player worlds in `saves/`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use fastnbt::Value;
use serde::Serialize;

use crate::{Error, Result, checked_name, io, nbt, stamp, tree_size};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct World {
    /// Folder name inside `saves/`; what the other functions take.
    pub folder: String,
    /// Name shown in game (`LevelName`), falling back to the folder.
    pub name: String,
    pub game_mode: &'static str,
    pub hardcore: bool,
    pub cheats: bool,
    /// Unix milliseconds, as the game stores it.
    pub last_played: Option<i64>,
    pub version: Option<String>,
    pub seed: Option<i64>,
    pub size: u64,
    pub icon: Option<PathBuf>,
}

fn game_mode(id: Option<i64>) -> &'static str {
    match id {
        Some(1) => "Creative",
        Some(2) => "Adventure",
        Some(3) => "Spectator",
        _ => "Survival",
    }
}

fn saves(game_dir: &Path) -> PathBuf {
    game_dir.join("saves")
}

/// 26.1 moved world generation settings out of level.dat into their own file.
fn seed_26(dir: &Path) -> Option<i64> {
    let settings = nbt::read(&dir.join("data/minecraft/world_gen_settings.dat")).ok()?;
    nbt::as_i64(nbt::get(&settings, &["data", "seed"]))
        .or(nbt::as_i64(nbt::get(&settings, &["seed"])))
}

fn read_world(dir: &Path) -> Option<World> {
    let level = nbt::read(&dir.join("level.dat")).ok()?;
    let data = nbt::get(&level, &["Data"])?;
    let folder = dir.file_name()?.to_string_lossy().into_owned();
    let seed = nbt::as_i64(nbt::get(data, &["WorldGenSettings", "seed"]))
        .or(nbt::as_i64(nbt::get(data, &["RandomSeed"])))
        .or_else(|| seed_26(dir));
    let icon = dir.join("icon.png");
    Some(World {
        name: nbt::as_str(nbt::get(data, &["LevelName"]))
            .filter(|n| !n.is_empty())
            .unwrap_or(&folder)
            .to_owned(),
        game_mode: game_mode(nbt::as_i64(nbt::get(data, &["GameType"]))),
        hardcore: nbt::as_i64(nbt::get(data, &["hardcore"])) == Some(1),
        cheats: nbt::as_i64(nbt::get(data, &["allowCommands"])) == Some(1),
        last_played: nbt::as_i64(nbt::get(data, &["LastPlayed"])),
        version: nbt::as_str(nbt::get(data, &["Version", "Name"])).map(str::to_owned),
        seed,
        size: tree_size(dir),
        icon: icon.is_file().then_some(icon),
        folder,
    })
}

/// Worlds with a readable `level.dat`, most recently played first.
pub fn list(game_dir: &Path) -> Result<Vec<World>> {
    let dir = saves(game_dir);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(&dir, e)),
    };
    let mut worlds: Vec<World> = entries
        .flatten()
        .filter_map(|e| read_world(&e.path()))
        .collect();
    worlds.sort_by_key(|w| std::cmp::Reverse(w.last_played));
    Ok(worlds)
}

fn world_dir(game_dir: &Path, folder: &str) -> Result<PathBuf> {
    let dir = saves(game_dir).join(checked_name(folder)?);
    if dir.join("level.dat").is_file() {
        Ok(dir)
    } else {
        Err(Error::BadName(folder.to_owned()))
    }
}

/// Changes the name shown in game; the folder stays as it is.
pub fn rename(game_dir: &Path, folder: &str, new_name: &str) -> Result<()> {
    let new_name = new_name.trim();
    if new_name.is_empty() {
        return Err(Error::BadName(new_name.to_owned()));
    }
    let path = world_dir(game_dir, folder)?.join("level.dat");
    let mut level = nbt::read(&path)?;
    match nbt::get_mut(&mut level, &["Data"]) {
        Some(Value::Compound(data)) => {
            data.insert("LevelName".into(), Value::String(new_name.to_owned()));
        }
        _ => {
            return Err(Error::Nbt {
                path,
                message: "no Data compound".into(),
            });
        }
    }
    nbt::write(&path, &level, true)
}

/// Copies a world to a new folder named "<name> (copy)"; returns the new folder.
pub fn duplicate(game_dir: &Path, folder: &str) -> Result<String> {
    let source = world_dir(game_dir, folder)?;
    let mut target_name = format!("{folder} (copy)");
    let mut n = 2;
    while saves(game_dir).join(&target_name).exists() {
        target_name = format!("{folder} (copy {n})");
        n += 1;
    }
    let target = saves(game_dir).join(&target_name);
    copy_tree(&source, &target)?;
    // session.lock belongs to a running game; the copy must not carry it.
    let _ = std::fs::remove_file(target.join("session.lock"));
    if let Some(world) = read_world(&target) {
        rename(game_dir, &target_name, &format!("{} (copy)", world.name))?;
    }
    Ok(target_name)
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to).map_err(|e| io(to, e))?;
    for entry in std::fs::read_dir(from).map_err(|e| io(from, e))?.flatten() {
        let dest = to.join(entry.file_name());
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            copy_tree(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest).map_err(|e| io(&dest, e))?;
        }
    }
    Ok(())
}

/// Zips a world into `backups_dir`; returns the archive path.
pub fn backup(game_dir: &Path, folder: &str, backups_dir: &Path) -> Result<PathBuf> {
    let source = world_dir(game_dir, folder)?;
    std::fs::create_dir_all(backups_dir).map_err(|e| io(backups_dir, e))?;
    let path = backups_dir.join(format!("{folder}_{}.zip", stamp(SystemTime::now())));
    let zip_err = |source| Error::Zip {
        path: path.clone(),
        source,
    };
    let file = std::fs::File::create(&path).map_err(|e| io(&path, e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut stack = vec![source.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).map_err(|e| io(&dir, e))?.flatten() {
            let entry_path = entry.path();
            let relative = entry_path
                .strip_prefix(&source)
                .expect("inside the world")
                .to_string_lossy()
                .replace('\\', "/");
            let name = format!("{folder}/{relative}");
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                zip.add_directory(name, options).map_err(zip_err)?;
                stack.push(entry_path);
            } else if entry.file_name() != "session.lock" {
                let bytes = std::fs::read(&entry_path).map_err(|e| io(&entry_path, e))?;
                zip.start_file(name, options).map_err(zip_err)?;
                zip.write_all(&bytes).map_err(|e| io(&path, e))?;
            }
        }
    }
    zip.finish().map_err(zip_err)?;
    Ok(path)
}

/// Deletes a world folder for good. The UI asks first.
pub fn delete(game_dir: &Path, folder: &str) -> Result<()> {
    let dir = world_dir(game_dir, folder)?;
    std::fs::remove_dir_all(&dir).map_err(|e| io(&dir, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    pub(crate) fn make_world(game_dir: &Path, folder: &str, name: &str, last_played: i64) {
        let dir = game_dir.join("saves").join(folder);
        std::fs::create_dir_all(dir.join("region")).unwrap();
        std::fs::write(dir.join("region/r.0.0.mca"), vec![7u8; 4096]).unwrap();
        let data = HashMap::from([
            ("LevelName".to_owned(), Value::String(name.into())),
            ("GameType".to_owned(), Value::Int(1)),
            ("hardcore".to_owned(), Value::Byte(0)),
            ("allowCommands".to_owned(), Value::Byte(1)),
            ("LastPlayed".to_owned(), Value::Long(last_played)),
            (
                "Version".to_owned(),
                Value::Compound(HashMap::from([(
                    "Name".to_owned(),
                    Value::String("1.21.4".into()),
                )])),
            ),
            (
                "WorldGenSettings".to_owned(),
                Value::Compound(HashMap::from([("seed".to_owned(), Value::Long(-42))])),
            ),
        ]);
        let root = Value::Compound(HashMap::from([("Data".to_owned(), Value::Compound(data))]));
        nbt::write(&dir.join("level.dat"), &root, true).unwrap();
    }

    #[test]
    fn lists_renames_duplicates_backs_up_and_deletes() {
        let tmp = tempfile::tempdir().unwrap();
        let game = tmp.path();
        make_world(game, "New World", "Base", 1000);
        make_world(game, "Older", "Old one", 10);
        std::fs::create_dir_all(game.join("saves/not a world")).unwrap();

        let worlds = list(game).unwrap();
        assert_eq!(
            worlds.iter().map(|w| w.folder.as_str()).collect::<Vec<_>>(),
            ["New World", "Older"]
        );
        let w = &worlds[0];
        assert_eq!(
            (w.name.as_str(), w.game_mode, w.cheats, w.seed),
            ("Base", "Creative", true, Some(-42))
        );
        assert_eq!(w.version.as_deref(), Some("1.21.4"));
        assert!(w.size >= 4096);

        rename(game, "New World", "Castle").unwrap();
        assert_eq!(list(game).unwrap()[0].name, "Castle");

        let copy = duplicate(game, "New World").unwrap();
        assert_eq!(copy, "New World (copy)");
        let copied = list(game)
            .unwrap()
            .into_iter()
            .find(|w| w.folder == copy)
            .unwrap();
        assert_eq!(copied.name, "Castle (copy)");

        let zip_path = backup(game, "New World", &tmp.path().join("backups")).unwrap();
        let archive = zip::ZipArchive::new(std::fs::File::open(&zip_path).unwrap()).unwrap();
        assert!(
            archive.file_names().any(|n| n == "New World/level.dat"),
            "{:?}",
            archive.file_names().collect::<Vec<_>>()
        );

        delete(game, "Older").unwrap();
        assert!(!game.join("saves/Older").exists());
        assert!(delete(game, "../New World").is_err());
    }
}
