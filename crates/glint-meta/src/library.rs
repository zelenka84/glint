//! Turning `libraries` entries into concrete files for one platform.

use crate::mojang::{Artifact, Library};
use crate::rules::{Arch, Platform, rules_allow};

/// One file a library contributes on the current platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFile {
    /// Path relative to `meta/libraries/`, always with forward slashes.
    pub path: String,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub kind: LibraryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryKind {
    /// Goes on the Java classpath.
    Classpath,
    /// Legacy native archive, extracted into the natives folder before launch.
    Natives { exclude: Vec<String> },
}

/// Maven coordinate `group:artifact:version[:classifier][@ext]` → repository path.
pub fn maven_path(coordinate: &str) -> Option<String> {
    let (coords, ext) = match coordinate.split_once('@') {
        Some((c, e)) => (c, e),
        None => (coordinate, "jar"),
    };
    let mut parts = coords.split(':');
    let group = parts.next()?;
    let artifact = parts.next()?;
    let version = parts.next()?;
    let classifier = parts.next();
    if parts.next().is_some() || [group, artifact, version].iter().any(|p| p.is_empty()) {
        return None;
    }
    let file = match classifier {
        Some(c) => format!("{artifact}-{version}-{c}.{ext}"),
        None => format!("{artifact}-{version}.{ext}"),
    };
    Some(format!("{}/{artifact}/{version}/{file}", group.replace('.', "/")))
}

fn from_artifact(lib: &Library, artifact: &Artifact, classifier: Option<&str>, kind: LibraryKind) -> Option<LibraryFile> {
    let path = match &artifact.path {
        Some(p) => p.clone(),
        None => {
            let coordinate = match classifier {
                Some(c) => format!("{}:{c}", lib.name),
                None => lib.name.clone(),
            };
            maven_path(&coordinate)?
        }
    };
    Some(LibraryFile {
        path,
        url: artifact.url.clone(),
        sha1: Some(artifact.sha1.clone()),
        size: Some(artifact.size),
        kind,
    })
}

/// Recent versions list `natives-windows-arm64` and `natives-windows-x86` jars whose
/// rules only check the OS; keep only the ones for this CPU.
fn native_matches_arch(name: &str, platform: &Platform) -> bool {
    let Some(classifier) = name.split(':').nth(3).filter(|c| c.starts_with("natives-")) else {
        return true;
    };
    if classifier.ends_with("-arm64") {
        platform.arch == Arch::Arm64
    } else if classifier.ends_with("-x86") {
        platform.arch == Arch::X86
    } else {
        true
    }
}

/// Files `lib` needs on `platform`. Empty when its rules exclude this platform.
pub fn library_files(lib: &Library, platform: &Platform) -> Vec<LibraryFile> {
    if !rules_allow(&lib.rules, platform) || !native_matches_arch(&lib.name, platform) {
        return Vec::new();
    }
    let mut files = Vec::new();

    match &lib.downloads {
        Some(downloads) => {
            if let Some(artifact) = &downloads.artifact
                && let Some(file) = from_artifact(lib, artifact, None, LibraryKind::Classpath)
            {
                files.push(file);
            }

            let classifier = lib
                .natives
                .as_ref()
                .and_then(|n| n.get(platform.os.mojang_name()))
                .map(|c| c.replace("${arch}", platform.arch.bits()));
            if let Some(classifier) = classifier
                && let Some(artifact) = downloads.classifiers.get(&classifier)
            {
                let exclude = lib.extract.clone().unwrap_or_default().exclude;
                if let Some(file) = from_artifact(lib, artifact, Some(&classifier), LibraryKind::Natives { exclude }) {
                    files.push(file);
                }
            }
        }
        // Loader profiles (Fabric, Quilt) give a Maven repository instead of download info.
        None => {
            if let Some(path) = maven_path(&lib.name) {
                let base = lib.url.as_deref().unwrap_or("https://libraries.minecraft.net/");
                let url = format!("{}/{path}", base.trim_end_matches('/'));
                files.push(LibraryFile { path, url, sha1: lib.sha1.clone(), size: None, kind: LibraryKind::Classpath });
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Arch, Os};

    fn lib(json: &str) -> Library {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn maven_paths() {
        assert_eq!(
            maven_path("com.mojang:brigadier:1.3.10").as_deref(),
            Some("com/mojang/brigadier/1.3.10/brigadier-1.3.10.jar")
        );
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-windows").as_deref(),
            Some("org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar")
        );
        assert_eq!(
            maven_path("de.oceanlabs.mcp:mcp_config:1.20.1@zip").as_deref(),
            Some("de/oceanlabs/mcp/mcp_config/1.20.1/mcp_config-1.20.1.zip")
        );
        assert_eq!(maven_path("broken"), None);
        assert_eq!(maven_path("a::1"), None);
    }

    #[test]
    fn modern_native_library_is_filtered_by_rules() {
        let l = lib(r#"{
            "name": "org.lwjgl:lwjgl:3.3.3:natives-windows",
            "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar",
                                        "sha1": "aa", "size": 10, "url": "https://libraries.minecraft.net/x.jar"}},
            "rules": [{"action": "allow", "os": {"name": "windows"}}]
        }"#);
        let win = library_files(&l, &Platform::new(Os::Windows, Arch::X86_64));
        assert_eq!(win.len(), 1);
        assert_eq!(win[0].kind, LibraryKind::Classpath);
        assert!(library_files(&l, &Platform::new(Os::Linux, Arch::X86_64)).is_empty());
    }

    #[test]
    fn natives_for_other_cpus_are_skipped() {
        let make = |classifier: &str| {
            lib(&format!(
                r#"{{"name": "org.lwjgl:lwjgl:3.3.3:{classifier}",
                    "downloads": {{"artifact": {{"path": "x.jar", "sha1": "a", "size": 1, "url": "https://x/x.jar"}}}},
                    "rules": [{{"action": "allow", "os": {{"name": "windows"}}}}]}}"#
            ))
        };
        let x64 = Platform::new(Os::Windows, Arch::X86_64);
        assert_eq!(library_files(&make("natives-windows"), &x64).len(), 1);
        assert!(library_files(&make("natives-windows-arm64"), &x64).is_empty());
        assert!(library_files(&make("natives-windows-x86"), &x64).is_empty());
        assert_eq!(library_files(&make("natives-windows-arm64"), &Platform::new(Os::Windows, Arch::Arm64)).len(), 1);
    }

    #[test]
    fn legacy_natives_use_classifier_and_arch() {
        let l = lib(r#"{
            "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209",
            "downloads": {"classifiers": {
                "natives-windows-64": {"path": "p/win64.jar", "sha1": "w64", "size": 1, "url": "https://x/win64.jar"},
                "natives-windows-32": {"path": "p/win32.jar", "sha1": "w32", "size": 1, "url": "https://x/win32.jar"},
                "natives-linux": {"path": "p/linux.jar", "sha1": "lnx", "size": 1, "url": "https://x/linux.jar"}
            }},
            "natives": {"windows": "natives-windows-${arch}", "linux": "natives-linux"},
            "extract": {"exclude": ["META-INF/"]}
        }"#);
        let files = library_files(&l, &Platform::new(Os::Windows, Arch::X86_64));
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].sha1.as_deref(), Some("w64"));
        assert_eq!(files[0].kind, LibraryKind::Natives { exclude: vec!["META-INF/".into()] });
        assert!(library_files(&l, &Platform::new(Os::MacOs, Arch::Arm64)).is_empty());
    }

    #[test]
    fn loader_style_library_uses_maven_repo() {
        let l = lib(r#"{"name": "net.fabricmc:fabric-loader:0.16.9", "url": "https://maven.fabricmc.net/"}"#);
        let files = library_files(&l, &Platform::new(Os::Windows, Arch::X86_64));
        assert_eq!(
            files[0].url,
            "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar"
        );
        assert_eq!(files[0].sha1, None);
    }
}
