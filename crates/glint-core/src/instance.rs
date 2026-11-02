use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// File name of the manifest inside an instance folder.
pub const MANIFEST_FILE: &str = "instance.toml";

/// `instance.toml`: what an instance is. What is actually installed lives in `glint.lock`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceManifest {
    pub name: String,
    pub engine: Engine,
    pub game_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader: Option<Loader>,
    #[serde(default)]
    pub java: JavaSettings,
    /// Cover art preset shown in the library.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    /// Unix seconds of the last launch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_played: Option<u64>,
    #[serde(default, skip_serializing_if = "GameSettings::is_default")]
    pub game: GameSettings,
    #[serde(default, skip_serializing_if = "LaunchHooks::is_default")]
    pub hooks: LaunchHooks,
    /// Extra environment variables for the game process and hooks.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
}

/// How the game window starts and where the game goes first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fullscreen: bool,
    /// Joins this server on every launch (Quick Play on 1.20+, `--server` before).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_join_server: Option<String>,
    /// Opens this world (folder in `saves/`) on every launch; 1.20+ only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_join_world: Option<String>,
}

impl GameSettings {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Shell commands run around the game, Prism-style.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchHooks {
    /// Runs before the game starts; a failure cancels the launch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_launch: Option<String>,
    /// Program the Java command is passed to, e.g. `prime-run` or `gamemoderun`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrapper: Option<String>,
    /// Runs after the game exits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_exit: Option<String>,
}

impl LaunchHooks {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Java,
    Horizon,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loader {
    pub kind: LoaderKind,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoaderKind {
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaSettings {
    /// Major Java version to use; `None` picks the one the game version requires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub major: Option<u32>,
    #[serde(default = "default_max_memory_mb")]
    pub max_memory_mb: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_memory_mb: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_args: Vec<String>,
    /// A Java executable to use instead of the runtime Glint downloads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

fn default_max_memory_mb() -> u32 {
    4096
}

impl Default for JavaSettings {
    fn default() -> Self {
        Self {
            major: None,
            max_memory_mb: default_max_memory_mb(),
            min_memory_mb: None,
            extra_args: Vec::new(),
            path: None,
        }
    }
}

impl InstanceManifest {
    pub fn vanilla(name: impl Into<String>, game_version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            engine: Engine::Java,
            game_version: game_version.into(),
            loader: None,
            java: JavaSettings::default(),
            cover: None,
            last_played: None,
            game: GameSettings::default(),
            hooks: LaunchHooks::default(),
            env: BTreeMap::new(),
        }
    }

    pub fn from_toml(text: &str) -> Result<Self> {
        Ok(toml::from_str(text)?)
    }

    pub fn to_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Reads `instance.toml` from an instance folder.
    pub fn load(instance_dir: &Path) -> Result<Self> {
        let path = instance_dir.join(MANIFEST_FILE);
        let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        Self::from_toml(&text)
    }

    /// Writes `instance.toml` into an instance folder, creating the folder if needed.
    pub fn save(&self, instance_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(instance_dir).map_err(|e| Error::io(instance_dir, e))?;
        let path = instance_dir.join(MANIFEST_FILE);
        std::fs::write(&path, self.to_toml()?).map_err(|e| Error::io(&path, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_manifest_gets_defaults() {
        let m = InstanceManifest::from_toml(
            r#"
            name = "Vanilla"
            engine = "java"
            game_version = "1.21.4"
            "#,
        )
        .unwrap();
        assert_eq!(m, InstanceManifest::vanilla("Vanilla", "1.21.4"));
        assert_eq!(m.java.max_memory_mb, 4096);
    }

    #[test]
    fn full_manifest_round_trips() {
        let mut m = InstanceManifest::vanilla("Survival with friends", "1.21.4");
        m.loader = Some(Loader {
            kind: LoaderKind::NeoForge,
            version: "21.4.10".into(),
        });
        m.java = JavaSettings {
            major: Some(21),
            max_memory_mb: 6144,
            min_memory_mb: Some(1024),
            extra_args: vec!["-XX:+UseZGC".into()],
            path: Some("C:/Java/bin/javaw.exe".into()),
        };
        m.game = GameSettings {
            width: Some(1280),
            height: Some(720),
            fullscreen: true,
            ..GameSettings::default()
        };
        m.hooks.wrapper = Some("gamemoderun".into());
        m.env
            .insert("MESA_GL_VERSION_OVERRIDE".into(), "4.5".into());

        let text = m.to_toml().unwrap();
        assert!(text.contains(r#"kind = "neoforge""#), "{text}");
        assert_eq!(InstanceManifest::from_toml(&text).unwrap(), m);
    }

    #[test]
    fn save_and_load() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("Skyblock");
        let m = InstanceManifest::vanilla("Skyblock", "1.21.4");
        m.save(&dir).unwrap();
        assert_eq!(InstanceManifest::load(&dir).unwrap(), m);
    }

    #[test]
    fn unknown_engine_is_an_error() {
        let err = InstanceManifest::from_toml(
            r#"
            name = "x"
            engine = "bedrock"
            game_version = "1.21"
            "#,
        )
        .unwrap_err();
        assert!(matches!(err, Error::ManifestParse(_)));
    }
}
