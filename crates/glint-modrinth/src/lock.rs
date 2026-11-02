//! `glint.lock`: what an instance actually has installed, so it can be shared,
//! updated or rolled back without guessing from file names.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Error, Result, io};

pub const LOCK_FILE: &str = "glint.lock";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Lock {
    #[serde(default)]
    pub mods: Vec<LockedMod>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModSource {
    Modrinth,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LockedMod {
    pub source: ModSource,
    pub project_id: String,
    pub version_id: String,
    pub version_number: String,
    pub title: String,
    pub filename: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha512: Option<String>,
    /// Installed because another mod requires it.
    #[serde(default)]
    pub dependency: bool,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
}

fn enabled_default() -> bool {
    true
}

impl Lock {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| Error::Lock { path: path.to_owned(), message: e.to_string() }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(io(path, e)),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self).map_err(|e| Error::Lock { path: path.to_owned(), message: e.to_string() })?;
        let tmp = path.with_extension("lock.tmp");
        std::fs::write(&tmp, text).map_err(|e| io(&tmp, e))?;
        std::fs::rename(&tmp, path).map_err(|e| io(path, e))
    }

    pub fn find_project(&self, project_id: &str) -> Option<&LockedMod> {
        self.mods.iter().find(|m| m.project_id == project_id)
    }

    pub fn remove_project(&mut self, project_id: &str) {
        self.mods.retain(|m| m.project_id != project_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(LOCK_FILE);
        assert_eq!(Lock::load(&path).unwrap(), Lock::default());

        let lock = Lock {
            mods: vec![LockedMod {
                source: ModSource::Modrinth,
                project_id: "AANobbMI".into(),
                version_id: "c3YkZvne".into(),
                version_number: "mc1.21.4-0.6.13-fabric".into(),
                title: "Sodium".into(),
                filename: "sodium-fabric-0.6.13+mc1.21.4.jar".into(),
                sha512: None,
                dependency: false,
                enabled: true,
            }],
        };
        lock.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[[mods]]") && text.contains("source = \"modrinth\""), "{text}");
        assert_eq!(Lock::load(&path).unwrap(), lock);
    }
}
