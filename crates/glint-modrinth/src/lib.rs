//! Modrinth for Glint: search the catalog and install mods into an instance,
//! pulling in required dependencies and recording everything in `glint.lock`.

mod api;
mod lock;

pub use api::{Hit, ModrinthApi, ProjectType, SearchQuery, SearchResults, Version};
pub use lock::{LOCK_FILE, Lock, LockedMod, ModSource};

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use glint_core::LoaderKind;
use glint_net::{Downloader, ExpectedHash};
use serde::Serialize;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Net(#[from] glint_net::Error),

    #[error("Modrinth returned something unexpected for {url}: {source}")]
    Json {
        url: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("{0} has no version for this Minecraft version and loader")]
    NoCompatibleVersion(String),

    #[error("invalid {path}: {message}")]
    Lock { path: PathBuf, message: String },

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub(crate) fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io { path: path.to_owned(), source }
}

/// Modrinth's loader names. Quilt also runs Fabric mods.
pub fn modrinth_loaders(kind: LoaderKind) -> &'static [&'static str] {
    match kind {
        LoaderKind::Fabric => &["fabric"],
        LoaderKind::Quilt => &["quilt", "fabric"],
        LoaderKind::Forge => &["forge"],
        LoaderKind::NeoForge => &["neoforge"],
    }
}

/// What an instance can run: decides which mod versions are compatible.
#[derive(Debug, Clone)]
pub struct Target {
    pub game_version: String,
    pub loader: LoaderKind,
}

/// A mod as the UI shows it: from the lock file, or a jar dropped in by hand.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledMod {
    pub filename: String,
    pub title: String,
    pub version: Option<String>,
    pub project_id: Option<String>,
    pub enabled: bool,
    pub dependency: bool,
    pub managed: bool,
}

/// Mod management for one instance's `game/mods` folder.
pub struct ModManager<'a> {
    api: &'a ModrinthApi,
    downloader: Downloader,
    instance_dir: PathBuf,
}

const DISABLED_SUFFIX: &str = ".disabled";

impl<'a> ModManager<'a> {
    pub fn new(api: &'a ModrinthApi, http: reqwest::Client, instance_dir: impl Into<PathBuf>) -> Self {
        Self { api, downloader: Downloader::new(http, 6), instance_dir: instance_dir.into() }
    }

    fn mods_dir(&self) -> PathBuf {
        self.instance_dir.join("game").join("mods")
    }

    fn lock_path(&self) -> PathBuf {
        self.instance_dir.join(LOCK_FILE)
    }

    /// Installs the newest compatible version of `project_id` and its required
    /// dependencies. Returns the titles that were newly installed.
    pub async fn install(&self, project_id: &str, target: &Target) -> Result<Vec<String>> {
        let mut lock = Lock::load(&self.lock_path())?;
        let mut installed = Vec::new();
        let mut queue = vec![(project_id.to_owned(), None::<String>, false)];
        let mut seen = HashSet::new();

        while let Some((project, version_id, dependency)) = queue.pop() {
            if !seen.insert(project.clone()) {
                continue;
            }
            // Dependencies already present are kept as they are.
            if dependency && lock.find_project(&project).is_some() {
                continue;
            }
            let version = match version_id {
                Some(id) => self.api.version(&id).await?,
                None => self.api.best_version(&project, target).await?,
            };
            let file = version.primary_file().ok_or_else(|| Error::NoCompatibleVersion(project.clone()))?;

            if let Some(existing) = lock.find_project(&project).cloned() {
                if existing.version_id == version.id {
                    continue;
                }
                self.delete_file(&existing.filename).await?;
                lock.remove_project(&project);
            }

            let dest = self.mods_dir().join(&file.filename);
            let hash = file.hashes.sha512.clone().map(ExpectedHash::Sha512);
            self.downloader.download(&file.url, &dest, hash.as_ref()).await?;

            let title = match self.api.project_title(&project).await {
                Ok(title) => title,
                Err(_) => version.name.clone(),
            };
            lock.mods.push(LockedMod {
                source: ModSource::Modrinth,
                project_id: project.clone(),
                version_id: version.id.clone(),
                version_number: version.version_number.clone(),
                title: title.clone(),
                filename: file.filename.clone(),
                sha512: file.hashes.sha512.clone(),
                dependency,
                enabled: true,
            });
            installed.push(title);

            for dep in version.dependencies.iter().filter(|d| d.dependency_type == "required") {
                if let Some(dep_project) = &dep.project_id {
                    queue.push((dep_project.clone(), dep.version_id.clone(), true));
                }
            }
            lock.save(&self.lock_path())?;
        }
        Ok(installed)
    }

    /// Everything in `mods/`: managed mods from the lock, plus jars added by hand.
    pub fn list(&self) -> Result<Vec<InstalledMod>> {
        let lock = Lock::load(&self.lock_path())?;
        let mut mods: Vec<InstalledMod> = lock
            .mods
            .iter()
            .map(|m| InstalledMod {
                filename: m.filename.clone(),
                title: m.title.clone(),
                version: Some(m.version_number.clone()),
                project_id: Some(m.project_id.clone()),
                enabled: m.enabled,
                dependency: m.dependency,
                managed: true,
            })
            .collect();

        if let Ok(entries) = std::fs::read_dir(self.mods_dir()) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let (base, enabled) = match name.strip_suffix(DISABLED_SUFFIX) {
                    Some(base) => (base.to_owned(), false),
                    None => (name.clone(), true),
                };
                if !base.ends_with(".jar") || lock.mods.iter().any(|m| m.filename == base) {
                    continue;
                }
                mods.push(InstalledMod {
                    title: base.trim_end_matches(".jar").to_owned(),
                    filename: base,
                    version: None,
                    project_id: None,
                    enabled,
                    dependency: false,
                    managed: false,
                });
            }
        }
        mods.sort_by(|a, b| a.dependency.cmp(&b.dependency).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase())));
        Ok(mods)
    }

    pub async fn remove(&self, filename: &str) -> Result<()> {
        self.delete_file(filename).await?;
        let mut lock = Lock::load(&self.lock_path())?;
        lock.mods.retain(|m| m.filename != filename);
        lock.save(&self.lock_path())
    }

    /// Disabled mods stay on disk as `<file>.jar.disabled`, which the game ignores.
    pub async fn set_enabled(&self, filename: &str, enabled: bool) -> Result<()> {
        let on = self.mods_dir().join(filename);
        let off = self.mods_dir().join(format!("{filename}{DISABLED_SUFFIX}"));
        let (from, to) = if enabled { (&off, &on) } else { (&on, &off) };
        if from.is_file() {
            tokio::fs::rename(from, to).await.map_err(|e| io(from, e))?;
        }
        let mut lock = Lock::load(&self.lock_path())?;
        if let Some(m) = lock.mods.iter_mut().find(|m| m.filename == filename) {
            m.enabled = enabled;
            lock.save(&self.lock_path())?;
        }
        Ok(())
    }

    async fn delete_file(&self, filename: &str) -> Result<()> {
        for name in [filename.to_owned(), format!("{filename}{DISABLED_SUFFIX}")] {
            let path = self.mods_dir().join(name);
            match tokio::fs::remove_file(&path).await {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(io(&path, e)),
            }
        }
        Ok(())
    }
}
