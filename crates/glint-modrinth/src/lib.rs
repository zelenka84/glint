//! Modrinth for Glint: search the catalog and install mods, resource packs and
//! shader packs into an instance, pulling in required dependencies and recording
//! everything in `glint.lock`.

mod api;
mod lock;

pub use api::{Hit, ModrinthApi, Project, ProjectType, SearchQuery, SearchResults, Version};
pub use lock::{LOCK_FILE, Lock, LockedMod, ModSource};

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use glint_core::LoaderKind;
use glint_net::{Downloader, ExpectedHash};
use serde::{Deserialize, Serialize};

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

    #[error("mods need a mod loader; create a Fabric, Quilt, Forge or NeoForge instance")]
    NeedsLoader,

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
    Error::Io {
        path: path.to_owned(),
        source,
    }
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

/// What an instance can run: decides which versions are compatible.
#[derive(Debug, Clone)]
pub struct Target {
    pub game_version: String,
    pub loader: Option<LoaderKind>,
}

/// The kinds of content an instance folder holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
    #[default]
    Mod,
    ResourcePack,
    Shader,
}

impl ContentKind {
    /// Folder inside `game/`.
    pub fn folder(self) -> &'static str {
        match self {
            Self::Mod => "mods",
            Self::ResourcePack => "resourcepacks",
            Self::Shader => "shaderpacks",
        }
    }

    pub fn project_type(self) -> ProjectType {
        match self {
            Self::Mod => ProjectType::Mod,
            Self::ResourcePack => ProjectType::Resourcepack,
            Self::Shader => ProjectType::Shader,
        }
    }

    /// Loader filter for Modrinth version queries; packs work regardless of loader.
    fn loaders(self, target: &Target) -> Option<Vec<&'static str>> {
        match (self, target.loader) {
            (Self::Mod, Some(loader)) => Some(modrinth_loaders(loader).to_vec()),
            (Self::ResourcePack, _) => Some(vec!["minecraft"]),
            _ => None,
        }
    }

    /// Whether a file or folder name in this kind's folder is content the game loads.
    fn accepts(self, name: &str, is_dir: bool) -> bool {
        match self {
            Self::Mod => name.ends_with(".jar"),
            Self::ResourcePack | Self::Shader => is_dir || name.ends_with(".zip"),
        }
    }
}

/// Shader packs need a shader loader mod; these are the ones Glint installs for them.
fn shader_mod(loader: LoaderKind) -> &'static str {
    match loader {
        LoaderKind::Forge => "oculus",
        LoaderKind::Fabric | LoaderKind::Quilt | LoaderKind::NeoForge => "iris",
    }
}

/// An item as the UI shows it: from the lock file, or a file dropped in by hand.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledMod {
    pub filename: String,
    pub title: String,
    pub version: Option<String>,
    pub project_id: Option<String>,
    pub icon_url: Option<String>,
    pub enabled: bool,
    pub dependency: bool,
    pub managed: bool,
    pub size: u64,
}

/// A newer compatible version of something installed from Modrinth.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    pub project_id: String,
    pub filename: String,
    pub title: String,
    pub current: String,
    pub latest: String,
}

/// Content management for one instance folder and one kind of content.
pub struct ModManager<'a> {
    api: &'a ModrinthApi,
    http: reqwest::Client,
    downloader: Downloader,
    instance_dir: PathBuf,
    kind: ContentKind,
}

const DISABLED_SUFFIX: &str = ".disabled";

impl<'a> ModManager<'a> {
    pub fn new(
        api: &'a ModrinthApi,
        http: reqwest::Client,
        instance_dir: impl Into<PathBuf>,
        kind: ContentKind,
    ) -> Self {
        Self {
            api,
            downloader: Downloader::new(http.clone(), 6),
            http,
            instance_dir: instance_dir.into(),
            kind,
        }
    }

    fn dir(&self) -> PathBuf {
        self.instance_dir.join("game").join(self.kind.folder())
    }

    fn lock_path(&self) -> PathBuf {
        self.instance_dir.join(LOCK_FILE)
    }

    /// Installs the newest compatible version of `project_id` and its required
    /// dependencies. Shader packs also get a shader loader mod when the instance
    /// has none. Returns the titles that were newly installed.
    pub async fn install(&self, project_id: &str, target: &Target) -> Result<Vec<String>> {
        if self.kind == ContentKind::Mod && target.loader.is_none() {
            return Err(Error::NeedsLoader);
        }
        let mut installed = self.install_from(project_id, target, true, false).await?;
        if self.kind == ContentKind::Shader
            && let Some(loader) = target.loader
        {
            let mods = ModManager::new(
                self.api,
                self.http.clone(),
                &self.instance_dir,
                ContentKind::Mod,
            );
            match mods
                .install_from(shader_mod(loader), target, false, true)
                .await
            {
                Ok(extra) => installed.extend(extra),
                Err(Error::NoCompatibleVersion(_)) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(installed)
    }

    /// Moves something installed from Modrinth to its newest compatible version.
    pub async fn update(&self, project_id: &str, target: &Target) -> Result<Vec<String>> {
        self.install_from(project_id, target, false, false).await
    }

    /// `explicit` installs mark the root project as chosen by the player rather than
    /// a dependency; updates keep whatever the entry was. `as_dependency` installs the
    /// root like a dependency: skipped when present, flagged as one otherwise.
    async fn install_from(
        &self,
        root: &str,
        target: &Target,
        explicit: bool,
        as_dependency: bool,
    ) -> Result<Vec<String>> {
        let mut lock = Lock::load(&self.lock_path())?;
        let mut installed = Vec::new();
        let mut queue = vec![(root.to_owned(), None::<String>, as_dependency)];
        let mut seen = HashSet::new();

        while let Some((project, version_id, dependency)) = queue.pop() {
            if !seen.insert(project.clone()) {
                continue;
            }
            let version = match version_id {
                Some(id) => self.api.version(&id).await?,
                None => {
                    self.api
                        .best_version(&project, target, self.kind_for(dependency))
                        .await?
                }
            };
            // Slugs resolve to ids here, so the lock always records the id.
            let project = if version.project_id.is_empty() {
                project
            } else {
                version.project_id.clone()
            };
            let existing = lock.find_project(&project).cloned();
            // Dependencies already present are kept as they are.
            if dependency && existing.is_some() {
                continue;
            }
            let file = version
                .primary_file()
                .ok_or_else(|| Error::NoCompatibleVersion(project.clone()))?;
            let kind = self.kind_for(dependency);
            let mut flag = dependency;
            if let Some(existing) = &existing {
                flag = if explicit && !dependency {
                    false
                } else {
                    existing.dependency
                };
                if existing.version_id == version.id {
                    if existing.dependency != flag {
                        lock.mods
                            .iter_mut()
                            .filter(|m| m.project_id == project)
                            .for_each(|m| m.dependency = flag);
                        lock.save(&self.lock_path())?;
                    }
                    continue;
                }
                self.delete_file(existing.kind, &existing.filename).await?;
                lock.remove_project(&project);
            }

            let dest = self
                .instance_dir
                .join("game")
                .join(kind.folder())
                .join(&file.filename);
            let hash = file.hashes.sha512.clone().map(ExpectedHash::Sha512);
            self.downloader
                .download(&file.url, &dest, hash.as_ref())
                .await?;
            if existing.as_ref().is_some_and(|e| !e.enabled) {
                let off = dest.with_file_name(format!("{}{DISABLED_SUFFIX}", file.filename));
                tokio::fs::rename(&dest, &off)
                    .await
                    .map_err(|e| io(&dest, e))?;
            }

            let (title, icon_url) = match self.api.project(&project).await {
                Ok(p) => (p.title, p.icon_url),
                Err(_) => (version.name.clone(), None),
            };
            lock.mods.push(LockedMod {
                source: ModSource::Modrinth,
                kind,
                project_id: project.clone(),
                version_id: version.id.clone(),
                version_number: version.version_number.clone(),
                title: title.clone(),
                filename: file.filename.clone(),
                sha512: file.hashes.sha512.clone(),
                icon_url,
                dependency: flag,
                enabled: existing.as_ref().is_none_or(|e| e.enabled),
            });
            installed.push(title);

            for dep in version
                .dependencies
                .iter()
                .filter(|d| d.dependency_type == "required")
            {
                if let Some(dep_project) = &dep.project_id {
                    queue.push((dep_project.clone(), dep.version_id.clone(), true));
                }
            }
            lock.save(&self.lock_path())?;
        }
        Ok(installed)
    }

    /// Dependencies are always mods (a resource pack never requires another pack).
    fn kind_for(&self, dependency: bool) -> ContentKind {
        if dependency {
            ContentKind::Mod
        } else {
            self.kind
        }
    }

    /// Everything in this kind's folder: managed entries from the lock, plus files added by hand.
    pub fn list(&self) -> Result<Vec<InstalledMod>> {
        let lock = Lock::load(&self.lock_path())?;
        let dir = self.dir();
        let size_of = |name: &str| {
            [name.to_owned(), format!("{name}{DISABLED_SUFFIX}")]
                .iter()
                .find_map(|n| std::fs::metadata(dir.join(n)).ok())
                .map_or(0, |m| m.len())
        };
        let mut items: Vec<InstalledMod> = lock
            .mods
            .iter()
            .filter(|m| m.kind == self.kind)
            .map(|m| InstalledMod {
                filename: m.filename.clone(),
                title: m.title.clone(),
                version: Some(m.version_number.clone()),
                project_id: Some(m.project_id.clone()),
                icon_url: m.icon_url.clone(),
                enabled: m.enabled,
                dependency: m.dependency,
                managed: true,
                size: size_of(&m.filename),
            })
            .collect();

        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
                let (base, enabled) = match name.strip_suffix(DISABLED_SUFFIX) {
                    Some(base) => (base.to_owned(), false),
                    None => (name.clone(), true),
                };
                if !self.kind.accepts(&base, is_dir) || lock.mods.iter().any(|m| m.filename == base)
                {
                    continue;
                }
                let title = base
                    .trim_end_matches(".jar")
                    .trim_end_matches(".zip")
                    .to_owned();
                items.push(InstalledMod {
                    title,
                    size: if is_dir {
                        0
                    } else {
                        entry.metadata().map_or(0, |m| m.len())
                    },
                    filename: base,
                    version: None,
                    project_id: None,
                    icon_url: None,
                    enabled,
                    dependency: false,
                    managed: false,
                });
            }
        }
        items.sort_by(|a, b| {
            a.dependency
                .cmp(&b.dependency)
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });
        Ok(items)
    }

    /// Newer compatible versions for everything of this kind installed from Modrinth.
    pub async fn check_updates(&self, target: &Target) -> Result<Vec<Update>> {
        let lock = Lock::load(&self.lock_path())?;
        let entries: Vec<&LockedMod> = lock
            .mods
            .iter()
            .filter(|m| m.kind == self.kind && m.sha512.is_some())
            .collect();
        let hashes: Vec<String> = entries.iter().filter_map(|m| m.sha512.clone()).collect();
        let latest = self
            .api
            .latest_for_hashes(&hashes, target, self.kind)
            .await?;
        Ok(entries
            .into_iter()
            .filter_map(|m| {
                let newest = latest.get(m.sha512.as_ref()?)?;
                (newest.id != m.version_id).then(|| Update {
                    project_id: m.project_id.clone(),
                    filename: m.filename.clone(),
                    title: m.title.clone(),
                    current: m.version_number.clone(),
                    latest: newest.version_number.clone(),
                })
            })
            .collect())
    }

    /// Copies a file the player dropped onto the window into this kind's folder.
    pub async fn add_file(&self, source: &Path) -> Result<String> {
        let name = source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !self.kind.accepts(&name, false) {
            let wanted = if self.kind == ContentKind::Mod {
                ".jar"
            } else {
                ".zip"
            };
            return Err(io(
                source,
                std::io::Error::other(format!("expected a {wanted} file")),
            ));
        }
        let dir = self.dir();
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| io(&dir, e))?;
        let dest = dir.join(&name);
        tokio::fs::copy(source, &dest)
            .await
            .map_err(|e| io(&dest, e))?;
        Ok(name)
    }

    pub async fn remove(&self, filename: &str) -> Result<()> {
        self.delete_file(self.kind, filename).await?;
        let mut lock = Lock::load(&self.lock_path())?;
        lock.mods.retain(|m| m.filename != filename);
        lock.save(&self.lock_path())
    }

    /// Disabled items stay on disk as `<name>.disabled`, which the game ignores.
    pub async fn set_enabled(&self, filename: &str, enabled: bool) -> Result<()> {
        let on = self.dir().join(filename);
        let off = self.dir().join(format!("{filename}{DISABLED_SUFFIX}"));
        let (from, to) = if enabled { (&off, &on) } else { (&on, &off) };
        if from.exists() {
            tokio::fs::rename(from, to).await.map_err(|e| io(from, e))?;
        }
        let mut lock = Lock::load(&self.lock_path())?;
        if let Some(m) = lock.mods.iter_mut().find(|m| m.filename == filename) {
            m.enabled = enabled;
            lock.save(&self.lock_path())?;
        }
        Ok(())
    }

    async fn delete_file(&self, kind: ContentKind, filename: &str) -> Result<()> {
        let dir = self.instance_dir.join("game").join(kind.folder());
        for name in [filename.to_owned(), format!("{filename}{DISABLED_SUFFIX}")] {
            let path = dir.join(name);
            let result = match tokio::fs::metadata(&path).await {
                Ok(meta) if meta.is_dir() => tokio::fs::remove_dir_all(&path).await,
                Ok(_) => tokio::fs::remove_file(&path).await,
                Err(e) => Err(e),
            };
            match result {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(io(&path, e)),
            }
        }
        Ok(())
    }
}
