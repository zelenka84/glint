//! Instances: isolated game folders, each described by its `instance.toml`.
//!
//! ```text
//! instances/<name>/
//! ├─ instance.toml   what the instance is (version, loader, Java, memory)
//! ├─ game/           the game directory (saves, mods, options.txt, …)
//! └─ natives/        extracted natives, recreated on every launch
//! ```

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use glint_core::instance::MANIFEST_FILE;
use glint_core::{GlintPaths, InstanceManifest};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Core(#[from] glint_core::Error),

    #[error("an instance named {0:?} already exists")]
    Exists(String),

    #[error("no instance named {0:?}")]
    NotFound(String),

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

fn io(path: &std::path::Path, source: std::io::Error) -> Error {
    Error::Io { path: path.to_owned(), source }
}

/// Instances under Glint's `instances/` folder.
#[derive(Debug, Clone)]
pub struct InstanceStore {
    paths: GlintPaths,
}

impl InstanceStore {
    pub fn new(paths: GlintPaths) -> Self {
        Self { paths }
    }

    pub fn dir(&self, name: &str) -> Result<PathBuf> {
        Ok(self.paths.instance(name)?)
    }

    pub fn game_dir(&self, name: &str) -> Result<PathBuf> {
        Ok(self.dir(name)?.join("game"))
    }

    /// Every folder with a readable `instance.toml`, most recently played first.
    /// Unreadable manifests are logged and skipped so one broken folder cannot hide the rest.
    pub fn list(&self) -> Result<Vec<InstanceManifest>> {
        let root = self.paths.instances();
        let entries = match std::fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io(&root, e)),
        };

        let mut instances = Vec::new();
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.join(MANIFEST_FILE).is_file() {
                continue;
            }
            match InstanceManifest::load(&dir) {
                Ok(mut manifest) => {
                    // The folder name is the identity; keep the manifest in step with it.
                    if let Some(folder) = dir.file_name().and_then(|n| n.to_str()) {
                        manifest.name = folder.to_owned();
                    }
                    instances.push(manifest);
                }
                Err(err) => tracing::warn!(dir = %dir.display(), error = %err, "skipping unreadable instance"),
            }
        }
        instances.sort_by(|a, b| {
            b.last_played.cmp(&a.last_played).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(instances)
    }

    pub fn get(&self, name: &str) -> Result<InstanceManifest> {
        let dir = self.dir(name)?;
        if !dir.join(MANIFEST_FILE).is_file() {
            return Err(Error::NotFound(name.to_owned()));
        }
        let mut manifest = InstanceManifest::load(&dir)?;
        manifest.name = name.to_owned();
        Ok(manifest)
    }

    pub fn create(&self, manifest: &InstanceManifest) -> Result<()> {
        let dir = self.dir(&manifest.name)?;
        if dir.exists() {
            return Err(Error::Exists(manifest.name.clone()));
        }
        manifest.save(&dir)?;
        let game = dir.join("game");
        std::fs::create_dir_all(&game).map_err(|e| io(&game, e))
    }

    /// Writes back changed settings for an existing instance.
    pub fn save(&self, manifest: &InstanceManifest) -> Result<()> {
        let dir = self.dir(&manifest.name)?;
        if !dir.join(MANIFEST_FILE).is_file() {
            return Err(Error::NotFound(manifest.name.clone()));
        }
        Ok(manifest.save(&dir)?)
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<()> {
        let mut manifest = self.get(from)?;
        if from == to {
            return Ok(());
        }
        let target = self.dir(to)?;
        // Case-only renames are the same folder on Windows.
        if target.exists() && !from.eq_ignore_ascii_case(to) {
            return Err(Error::Exists(to.to_owned()));
        }
        let source = self.dir(from)?;
        std::fs::rename(&source, &target).map_err(|e| io(&source, e))?;
        manifest.name = to.to_owned();
        Ok(manifest.save(&target)?)
    }

    /// Deletes the instance folder, including its worlds. The UI must confirm first.
    pub fn delete(&self, name: &str) -> Result<()> {
        let dir = self.dir(name)?;
        if !dir.join(MANIFEST_FILE).is_file() {
            return Err(Error::NotFound(name.to_owned()));
        }
        std::fs::remove_dir_all(&dir).map_err(|e| io(&dir, e))
    }

    pub fn mark_played(&self, name: &str) -> Result<()> {
        let mut manifest = self.get(name)?;
        manifest.last_played = Some(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()));
        self.save(&manifest)
    }

    /// A free name based on `base`: `base`, `base (2)`, `base (3)`, …
    pub fn unique_name(&self, base: &str) -> String {
        let taken = |n: &str| self.paths.instance(n).map(|d| d.exists()).unwrap_or(true);
        if !taken(base) {
            return base.to_owned();
        }
        (2..).map(|i| format!("{base} ({i})")).find(|n| !taken(n)).expect("some name is free")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, InstanceStore) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = GlintPaths::from_root(tmp.path());
        paths.ensure().unwrap();
        (tmp, InstanceStore::new(paths))
    }

    #[test]
    fn create_list_and_get() {
        let (_tmp, store) = store();
        store.create(&InstanceManifest::vanilla("Survival", "1.21.4")).unwrap();
        store.create(&InstanceManifest::vanilla("Creative", "26.3")).unwrap();
        assert!(store.game_dir("Survival").unwrap().is_dir());

        let names: Vec<_> = store.list().unwrap().into_iter().map(|m| m.name).collect();
        assert_eq!(names, ["Creative", "Survival"]);
        assert_eq!(store.get("Survival").unwrap().game_version, "1.21.4");
        assert!(matches!(store.get("Nope"), Err(Error::NotFound(_))));
        assert!(matches!(
            store.create(&InstanceManifest::vanilla("Survival", "1.20")),
            Err(Error::Exists(_))
        ));
    }

    #[test]
    fn played_instances_come_first() {
        let (_tmp, store) = store();
        store.create(&InstanceManifest::vanilla("A", "1.21")).unwrap();
        store.create(&InstanceManifest::vanilla("B", "1.21")).unwrap();
        store.mark_played("B").unwrap();
        assert_eq!(store.list().unwrap()[0].name, "B");
    }

    #[test]
    fn rename_moves_the_folder_and_updates_the_manifest() {
        let (_tmp, store) = store();
        store.create(&InstanceManifest::vanilla("Old", "1.21")).unwrap();
        store.create(&InstanceManifest::vanilla("Other", "1.21")).unwrap();
        assert!(matches!(store.rename("Old", "Other"), Err(Error::Exists(_))));

        store.rename("Old", "New").unwrap();
        assert!(!store.dir("Old").unwrap().exists());
        assert_eq!(InstanceManifest::load(&store.dir("New").unwrap()).unwrap().name, "New");
    }

    #[test]
    fn delete_removes_everything() {
        let (_tmp, store) = store();
        store.create(&InstanceManifest::vanilla("Temp", "1.21")).unwrap();
        std::fs::write(store.game_dir("Temp").unwrap().join("options.txt"), "x").unwrap();
        store.delete("Temp").unwrap();
        assert!(store.list().unwrap().is_empty());
        assert!(matches!(store.delete("Temp"), Err(Error::NotFound(_))));
    }

    #[test]
    fn folders_without_manifest_are_ignored_and_names_stay_unique() {
        let (tmp, store) = store();
        std::fs::create_dir_all(tmp.path().join("instances/stray")).unwrap();
        store.create(&InstanceManifest::vanilla("Vanilla 26.3", "26.3")).unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
        assert_eq!(store.unique_name("Vanilla 26.3"), "Vanilla 26.3 (2)");
        assert_eq!(store.unique_name("Fresh"), "Fresh");
    }
}
