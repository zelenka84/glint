//! What lives in an instance's `game/` folder, read and edited for the instance page:
//! worlds (`saves/`), the server list (`servers.dat`), screenshots and log files.

pub mod logs;
pub mod servers;
pub mod shots;
pub mod worlds;

mod nbt;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not read {path}: {message}")]
    Nbt { path: PathBuf, message: String },

    #[error("could not write the backup {path}: {source}")]
    Zip {
        path: PathBuf,
        #[source]
        source: zip::result::ZipError,
    },

    #[error("{0:?} is not a valid name here")]
    BadName(String),

    #[error("server did not answer: {0}")]
    Ping(String),
}

pub(crate) fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}

/// A single path component chosen by the player or the UI: no separators, no `..`.
pub(crate) fn checked_name(name: &str) -> Result<&str> {
    let ok = !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        && name.trim() == name;
    if ok {
        Ok(name)
    } else {
        Err(Error::BadName(name.to_owned()))
    }
}

/// A folder name the UI may open inside the game directory.
pub fn checked_folder(name: &str) -> Result<&str> {
    checked_name(name)
}

pub(crate) fn unix_seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Total size of a file or folder tree in bytes.
pub(crate) fn tree_size(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if !meta.is_dir() {
        return meta.len();
    }
    std::fs::read_dir(path)
        .map(|entries| entries.flatten().map(|e| tree_size(&e.path())).sum())
        .unwrap_or(0)
}

/// `YYYY-MM-DD_HH-MM-SS` in UTC, for file names that sort by time.
pub(crate) fn stamp(time: SystemTime) -> String {
    let secs = unix_seconds(time);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}_{:02}-{:02}-{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn stamps_are_utc_and_sortable() {
        assert_eq!(stamp(UNIX_EPOCH), "1970-01-01_00-00-00");
        assert_eq!(
            stamp(UNIX_EPOCH + Duration::from_secs(1_793_620_800)),
            "2026-11-02_12-00-00"
        );
    }

    #[test]
    fn names_cannot_escape_their_folder() {
        assert!(checked_name("New World").is_ok());
        for bad in ["", "..", "a/b", "a\\b", " x", "con:"] {
            assert!(checked_name(bad).is_err(), "{bad:?}");
        }
    }
}
