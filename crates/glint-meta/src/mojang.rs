//! Serde models for Mojang's launcher metadata.
//!
//! Only the fields Glint uses are modelled; unknown fields are ignored so new
//! Mojang additions do not break parsing.

use std::collections::HashMap;

use serde::Deserialize;

use crate::rules::Rule;

/// `version_manifest_v2.json`: every published version.
#[derive(Debug, Clone, Deserialize)]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: VersionType,
    pub url: String,
    pub sha1: String,
    pub release_time: String,
}

impl VersionManifest {
    pub fn find(&self, id: &str) -> Option<&ManifestEntry> {
        self.versions.iter().find(|v| v.id == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionType {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
    #[serde(other)]
    Other,
}

/// `<version>.json`: everything needed to install and launch one version.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: VersionType,
    pub main_class: String,
    pub asset_index: Option<AssetIndexRef>,
    pub assets: Option<String>,
    pub downloads: Option<VersionDownloads>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    /// 1.13+ structured arguments.
    pub arguments: Option<Arguments>,
    /// Pre-1.13 single argument string.
    pub minecraft_arguments: Option<String>,
    pub java_version: Option<JavaVersion>,
    pub logging: Option<Logging>,
    /// Set by mod loader profiles that extend a vanilla version.
    pub inherits_from: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub total_size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionDownloads {
    pub client: DownloadInfo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadInfo {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Library {
    /// Maven coordinate, e.g. `org.lwjgl:lwjgl:3.3.3:natives-windows`.
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// Pre-1.19 natives: OS name → classifier (may contain `${arch}`).
    pub natives: Option<HashMap<String, String>>,
    pub extract: Option<Extract>,
    /// Maven repository base used by loader profiles that omit `downloads`.
    pub url: Option<String>,
    /// Optional checksum some loader profiles provide alongside `url`.
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: HashMap<String, Artifact>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Artifact {
    pub path: Option<String>,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: ArgumentValue },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ArgumentValue {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
    pub major_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Logging {
    pub client: Option<LoggingConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    /// JVM argument template, e.g. `-Dlog4j.configurationFile=${path}`.
    pub argument: String,
    pub file: LogFile,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// `assets/indexes/<id>.json`: logical asset path → content hash.
#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    /// Very old versions read assets from `assets/virtual/<id>/`.
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    /// Pre-1.6 versions read assets from `<game dir>/resources/`.
    #[serde(default)]
    pub map_to_resources: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

impl AssetObject {
    /// Path under `assets/objects/` and under the resources CDN.
    pub fn object_path(&self) -> String {
        format!("{}/{}", &self.hash[..2.min(self.hash.len())], self.hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modern_and_legacy_argument_shapes() {
        let v: VersionInfo = serde_json::from_str(
            r#"{
                "id": "1.21.4", "type": "release", "mainClass": "net.minecraft.client.main.Main",
                "arguments": {
                    "game": ["--username", "${auth_player_name}",
                             {"rules": [{"action": "allow", "features": {"is_demo_user": true}}], "value": "--demo"}],
                    "jvm": [{"rules": [{"action": "allow", "os": {"name": "osx"}}], "value": ["-XstartOnFirstThread"]}]
                },
                "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21},
                "someFutureField": {"ignored": true}
            }"#,
        )
        .unwrap();
        let args = v.arguments.unwrap();
        assert!(matches!(&args.game[0], Argument::Plain(s) if s == "--username"));
        assert!(matches!(&args.game[2], Argument::Conditional { value: ArgumentValue::One(s), .. } if s == "--demo"));
        assert!(matches!(&args.jvm[0], Argument::Conditional { value: ArgumentValue::Many(v), .. } if v.len() == 1));
        assert_eq!(v.java_version.unwrap().major_version, 21);

        let old: VersionInfo = serde_json::from_str(
            r#"{"id": "1.8.9", "type": "release", "mainClass": "net.minecraft.client.main.Main",
                "minecraftArguments": "--username ${auth_player_name}"}"#,
        )
        .unwrap();
        assert!(old.arguments.is_none());
        assert!(old.minecraft_arguments.is_some());
    }

    #[test]
    fn unknown_version_type_does_not_fail() {
        let e: ManifestEntry = serde_json::from_str(
            r#"{"id": "x", "type": "experiment", "url": "u", "sha1": "s", "releaseTime": "t"}"#,
        )
        .unwrap();
        assert_eq!(e.kind, VersionType::Other);
    }

    #[test]
    fn asset_object_path_uses_hash_prefix() {
        let o = AssetObject { hash: "bdf48ef6b5d0d23bbb02e17d04865216179f510a".into(), size: 1 };
        assert_eq!(o.object_path(), "bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a");
    }
}
