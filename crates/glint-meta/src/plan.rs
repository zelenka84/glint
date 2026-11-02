//! Where metadata lives on disk and which files a version needs.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use glint_net::ExpectedHash;

use crate::library::{LibraryKind, library_files};
use crate::mojang::{AssetIndex, VersionInfo};
use crate::rules::Platform;

/// Layout of `meta/`, shared by every instance.
///
/// ```text
/// meta/
/// ├─ version_manifest_v2.json
/// ├─ versions/<id>/<id>.json, <id>.jar
/// ├─ libraries/<maven path>
/// └─ assets/indexes/<id>.json, objects/<xx>/<hash>, log_configs/<file>
/// ```
#[derive(Debug, Clone)]
pub struct MetaLayout {
    root: PathBuf,
}

impl MetaLayout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest_cache(&self) -> PathBuf {
        self.root.join("version_manifest_v2.json")
    }

    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.root.join("versions").join(id)
    }

    pub fn version_json(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.json"))
    }

    pub fn client_jar(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.jar"))
    }

    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }

    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }

    pub fn asset_index(&self, id: &str) -> PathBuf {
        self.assets().join("indexes").join(format!("{id}.json"))
    }

    pub fn asset_objects(&self) -> PathBuf {
        self.assets().join("objects")
    }

    pub fn log_config(&self, file_id: &str) -> PathBuf {
        self.assets().join("log_configs").join(file_id)
    }
}

/// One file to fetch into `meta/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    pub url: String,
    pub dest: PathBuf,
    pub hash: Option<ExpectedHash>,
    pub size: Option<u64>,
}

/// Everything a version needs on disk before it can launch.
#[derive(Debug, Clone, Default)]
pub struct InstallPlan {
    pub files: Vec<PlannedFile>,
    /// Classpath entries in launch order (client jar last).
    pub classpath: Vec<PathBuf>,
    /// Legacy native archives to extract before launch, with their exclude prefixes.
    pub natives: Vec<(PathBuf, Vec<String>)>,
}

impl InstallPlan {
    pub fn total_bytes(&self) -> u64 {
        self.files.iter().filter_map(|f| f.size).sum()
    }

    fn push(&mut self, seen: &mut HashSet<PathBuf>, file: PlannedFile) {
        if seen.insert(file.dest.clone()) {
            self.files.push(file);
        }
    }
}

/// Lists every file `version` needs on `platform`. `asset_index` is the parsed index
/// the version refers to (fetch it first); `resources_base` is the asset CDN.
pub fn plan_install(
    layout: &MetaLayout,
    version: &VersionInfo,
    asset_index: Option<&AssetIndex>,
    platform: &Platform,
    resources_base: &str,
) -> InstallPlan {
    let mut plan = InstallPlan::default();
    let mut seen = HashSet::new();

    for lib in &version.libraries {
        let files = library_files(lib, platform);
        if files.is_empty() && lib.rules.is_empty() {
            tracing::warn!(library = %lib.name, "library has no downloadable file");
        }
        for file in files {
            let dest = layout.libraries().join(&file.path);
            match &file.kind {
                LibraryKind::Classpath => {
                    if !plan.classpath.contains(&dest) {
                        plan.classpath.push(dest.clone());
                    }
                }
                LibraryKind::Natives { exclude } => plan.natives.push((dest.clone(), exclude.clone())),
            }
            let file = PlannedFile {
                url: file.url,
                dest,
                hash: file.sha1.map(ExpectedHash::Sha1),
                size: file.size,
            };
            plan.push(&mut seen, file);
        }
    }

    if let Some(client) = version.downloads.as_ref().map(|d| &d.client) {
        let dest = layout.client_jar(&version.id);
        plan.classpath.push(dest.clone());
        let file = PlannedFile {
            url: client.url.clone(),
            dest,
            hash: Some(ExpectedHash::Sha1(client.sha1.clone())),
            size: Some(client.size),
        };
        plan.push(&mut seen, file);
    }

    if let Some(log) = version.logging.as_ref().and_then(|l| l.client.as_ref()) {
        let file = PlannedFile {
            url: log.file.url.clone(),
            dest: layout.log_config(&log.file.id),
            hash: Some(ExpectedHash::Sha1(log.file.sha1.clone())),
            size: Some(log.file.size),
        };
        plan.push(&mut seen, file);
    }

    if let Some(index) = asset_index {
        let base = resources_base.trim_end_matches('/');
        let mut objects: Vec<_> = index.objects.values().collect();
        objects.sort_by(|a, b| a.hash.cmp(&b.hash));
        for object in objects {
            let rel = object.object_path();
            let file = PlannedFile {
                url: format!("{base}/{rel}"),
                dest: layout.asset_objects().join(&rel),
                hash: Some(ExpectedHash::Sha1(object.hash.clone())),
                size: Some(object.size),
            };
            plan.push(&mut seen, file);
        }
    }

    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Arch, Os};

    #[test]
    fn plans_client_libraries_logging_and_deduplicated_assets() {
        let version: VersionInfo = serde_json::from_str(
            r#"{
                "id": "1.21.4", "type": "release", "mainClass": "net.minecraft.client.main.Main",
                "downloads": {"client": {"sha1": "c1", "size": 100, "url": "https://x/client.jar"}},
                "libraries": [
                    {"name": "com.mojang:brigadier:1.3.10",
                     "downloads": {"artifact": {"path": "com/mojang/brigadier/1.3.10/brigadier-1.3.10.jar", "sha1": "b1", "size": 10, "url": "https://x/b.jar"}}},
                    {"name": "com.mojang:brigadier:1.3.10",
                     "downloads": {"artifact": {"path": "com/mojang/brigadier/1.3.10/brigadier-1.3.10.jar", "sha1": "b1", "size": 10, "url": "https://x/b.jar"}}},
                    {"name": "org.lwjgl:lwjgl:3.3.3:natives-macos",
                     "downloads": {"artifact": {"path": "m.jar", "sha1": "m1", "size": 5, "url": "https://x/m.jar"}},
                     "rules": [{"action": "allow", "os": {"name": "osx"}}]}
                ],
                "logging": {"client": {"argument": "-Dlog4j.configurationFile=${path}", "type": "log4j2-xml",
                    "file": {"id": "client-1.21.2.xml", "sha1": "l1", "size": 1, "url": "https://x/log.xml"}}}
            }"#,
        )
        .unwrap();
        let index: AssetIndex = serde_json::from_str(
            r#"{"objects": {
                "minecraft/sounds/a.ogg": {"hash": "aa11", "size": 3},
                "minecraft/sounds/a_copy.ogg": {"hash": "aa11", "size": 3},
                "icons/icon.png": {"hash": "bb22", "size": 4}
            }}"#,
        )
        .unwrap();

        let layout = MetaLayout::new("/meta");
        let plan = plan_install(&layout, &version, Some(&index), &Platform::new(Os::Windows, Arch::X86_64), "https://res/");

        // brigadier once, client, log config, two unique assets; the macOS native is skipped.
        assert_eq!(plan.files.len(), 5);
        assert_eq!(plan.total_bytes(), 10 + 100 + 1 + 3 + 4);
        assert_eq!(
            plan.classpath,
            vec![
                layout.libraries().join("com/mojang/brigadier/1.3.10/brigadier-1.3.10.jar"),
                layout.client_jar("1.21.4"),
            ]
        );
        assert!(plan.files.iter().any(|f| f.url == "https://res/aa/aa11"));
    }
}
