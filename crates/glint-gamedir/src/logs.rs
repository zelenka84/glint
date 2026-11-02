//! Log files the game leaves behind: `logs/*.log(.gz)` and `crash-reports/*.txt`.

use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use serde::Serialize;

use crate::{Error, Result, checked_name, io, unix_seconds};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogKind {
    Log,
    Crash,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LogFile {
    /// Path relative to the game folder with forward slashes, e.g. `logs/latest.log`.
    pub id: String,
    pub name: String,
    pub kind: LogKind,
    pub size: u64,
    /// Unix seconds.
    pub modified: u64,
}

const FOLDERS: [(&str, LogKind); 2] = [("logs", LogKind::Log), ("crash-reports", LogKind::Crash)];

/// Largest log text sent to the UI; longer files keep their end, where the errors are.
const MAX_TEXT: usize = 4 << 20;

/// Every log and crash report, newest first.
pub fn list(game_dir: &Path) -> Result<Vec<LogFile>> {
    let mut files = Vec::new();
    for (folder, kind) in FOLDERS {
        let Ok(entries) = std::fs::read_dir(game_dir.join(folder)) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let wanted = match kind {
                LogKind::Log => name.ends_with(".log") || name.ends_with(".log.gz"),
                LogKind::Crash => name.ends_with(".txt"),
            };
            let Ok(meta) = entry.metadata() else { continue };
            if wanted && meta.is_file() {
                files.push(LogFile {
                    id: format!("{folder}/{name}"),
                    name,
                    kind,
                    size: meta.len(),
                    modified: meta.modified().map(unix_seconds).unwrap_or(0),
                });
            }
        }
    }
    files.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| b.name.cmp(&a.name))
    });
    Ok(files)
}

fn resolve(game_dir: &Path, id: &str) -> Result<PathBuf> {
    let (folder, name) = id
        .split_once('/')
        .ok_or_else(|| Error::BadName(id.to_owned()))?;
    if !FOLDERS.iter().any(|(f, _)| *f == folder) {
        return Err(Error::BadName(id.to_owned()));
    }
    Ok(game_dir.join(folder).join(checked_name(name)?))
}

/// The text of a log, decompressed; very long logs are cut to their last few MB.
pub fn read(game_dir: &Path, id: &str) -> Result<String> {
    let path = resolve(game_dir, id)?;
    let bytes = std::fs::read(&path).map_err(|e| io(&path, e))?;
    let bytes = if id.ends_with(".gz") {
        let mut out = Vec::new();
        GzDecoder::new(&bytes[..])
            .read_to_end(&mut out)
            .map_err(|e| io(&path, e))?;
        out
    } else {
        bytes
    };
    let start = bytes.len().saturating_sub(MAX_TEXT);
    let mut text = String::from_utf8_lossy(&bytes[start..]).into_owned();
    if start > 0 {
        text.insert_str(
            0,
            "[Glint] … earlier lines cut, showing the end of the file …\n",
        );
    }
    Ok(text)
}

pub fn delete(game_dir: &Path, id: &str) -> Result<()> {
    let path = resolve(game_dir, id)?;
    std::fs::remove_file(&path).map_err(|e| io(&path, e))
}

/// Deletes every log except `latest.log`; returns how many went.
pub fn delete_old(game_dir: &Path) -> Result<usize> {
    let old: Vec<LogFile> = list(game_dir)?
        .into_iter()
        .filter(|f| f.id != "logs/latest.log")
        .collect();
    for file in &old {
        delete(game_dir, &file.id)?;
    }
    Ok(old.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    #[test]
    fn lists_reads_and_cleans_up() {
        let tmp = tempfile::tempdir().unwrap();
        let game = tmp.path();
        std::fs::create_dir_all(game.join("logs")).unwrap();
        std::fs::create_dir_all(game.join("crash-reports")).unwrap();
        std::fs::write(game.join("logs/latest.log"), "[12:00:00] [main/INFO]: hi\n").unwrap();
        let mut gz = GzEncoder::new(Vec::new(), Compression::default());
        gz.write_all(b"old run\n").unwrap();
        std::fs::write(game.join("logs/2026-11-01-1.log.gz"), gz.finish().unwrap()).unwrap();
        std::fs::write(
            game.join("crash-reports/crash-2026.txt"),
            "---- Minecraft Crash Report ----",
        )
        .unwrap();
        std::fs::write(game.join("logs/readme.md"), "not a log").unwrap();

        let files = list(game).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|f| f.kind == LogKind::Crash));

        assert_eq!(read(game, "logs/2026-11-01-1.log.gz").unwrap(), "old run\n");
        assert!(read(game, "logs/../instance.toml").is_err());
        assert!(read(game, "saves/level.dat").is_err());

        assert_eq!(delete_old(game).unwrap(), 2);
        assert_eq!(list(game).unwrap().len(), 1);
    }
}
