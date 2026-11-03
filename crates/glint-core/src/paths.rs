use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// The on-disk layout of Glint's data directory.
///
/// ```text
/// <root>/
/// ├─ meta/        libraries, assets and versions shared by every instance
/// ├─ java/        downloaded Java runtimes
/// ├─ cache/       content-addressed downloads and API responses
/// ├─ instances/   one folder per instance
/// └─ glint.db     index and cache database
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlintPaths {
    root: PathBuf,
}

impl GlintPaths {
    /// `%APPDATA%\Glint` on Windows, `~/.local/share/Glint` on Linux.
    pub fn from_system() -> Result<Self> {
        let data = dirs::data_dir().ok_or(Error::NoDataDir)?;
        Ok(Self::from_root(data.join("Glint")))
    }

    pub fn from_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn meta(&self) -> PathBuf {
        self.root.join("meta")
    }

    /// Horizon builds and the resources prepared for them.
    pub fn horizon(&self) -> PathBuf {
        self.root.join("horizon")
    }

    pub fn java(&self) -> PathBuf {
        self.root.join("java")
    }

    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }

    pub fn instances(&self) -> PathBuf {
        self.root.join("instances")
    }

    pub fn database(&self) -> PathBuf {
        self.root.join("glint.db")
    }

    /// Folder of a single instance. Rejects names that could escape `instances/`.
    pub fn instance(&self, name: &str) -> Result<PathBuf> {
        let valid = !name.trim().is_empty()
            && name != "."
            && name != ".."
            && !name.contains(['/', '\\', ':'])
            && name.trim() == name;
        if !valid {
            return Err(Error::InvalidInstanceName(name.to_owned()));
        }
        Ok(self.instances().join(name))
    }

    /// Creates the top-level folders if they are missing.
    pub fn ensure(&self) -> Result<()> {
        for dir in [self.meta(), self.java(), self.cache(), self.instances()] {
            std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_relative_to_root() {
        let paths = GlintPaths::from_root("/data/Glint");
        assert_eq!(paths.meta(), Path::new("/data/Glint/meta"));
        assert_eq!(paths.database(), Path::new("/data/Glint/glint.db"));
        assert_eq!(
            paths.instance("Survival").unwrap(),
            Path::new("/data/Glint/instances/Survival")
        );
    }

    #[test]
    fn rejects_names_that_escape_the_instances_folder() {
        let paths = GlintPaths::from_root("/data/Glint");
        for bad in ["", " ", ".", "..", "../x", "a/b", "a\\b", "C:", " padded "] {
            assert!(paths.instance(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn ensure_creates_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = GlintPaths::from_root(tmp.path().join("Glint"));
        paths.ensure().unwrap();
        assert!(paths.meta().is_dir());
        assert!(paths.instances().is_dir());
    }
}
