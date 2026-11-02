//! Folding a loader profile (`inheritsFrom` a vanilla version) into one version.

use std::collections::HashSet;

use glint_meta::mojang::{Arguments, Library, VersionInfo};

/// `group:artifact[:classifier]`, the part of a coordinate that identifies a library
/// regardless of its version.
fn library_key(name: &str) -> String {
    let coords = name.split('@').next().unwrap_or(name);
    let parts: Vec<&str> = coords.split(':').collect();
    match parts.as_slice() {
        [group, artifact, _version, classifier, ..] => format!("{group}:{artifact}:{classifier}"),
        [group, artifact, ..] => format!("{group}:{artifact}"),
        _ => coords.to_owned(),
    }
}

/// The loader's libraries come first and replace vanilla ones with the same key
/// (loaders ship newer ASM, Log4j, …). Arguments are appended; the loader's main
/// class wins. The id stays the vanilla one so the client jar keeps its name,
/// which Forge's module scanning relies on (`${version_name}.jar`).
pub fn merge(vanilla: &VersionInfo, loader: &VersionInfo) -> VersionInfo {
    let overridden: HashSet<String> = loader
        .libraries
        .iter()
        .map(|l| library_key(&l.name))
        .collect();
    let libraries: Vec<Library> = loader
        .libraries
        .iter()
        .cloned()
        .chain(
            vanilla
                .libraries
                .iter()
                .filter(|l| !overridden.contains(&library_key(&l.name)))
                .cloned(),
        )
        .collect();

    let arguments = match (&vanilla.arguments, &loader.arguments) {
        (Some(v), Some(l)) => Some(Arguments {
            game: v.game.iter().chain(&l.game).cloned().collect(),
            jvm: v.jvm.iter().chain(&l.jvm).cloned().collect(),
        }),
        (v, l) => v.clone().or_else(|| l.clone()),
    };

    VersionInfo {
        id: vanilla.id.clone(),
        kind: vanilla.kind,
        main_class: loader.main_class.clone(),
        asset_index: vanilla.asset_index.clone(),
        assets: vanilla.assets.clone(),
        downloads: vanilla.downloads.clone(),
        libraries,
        arguments,
        minecraft_arguments: loader
            .minecraft_arguments
            .clone()
            .or_else(|| vanilla.minecraft_arguments.clone()),
        java_version: loader
            .java_version
            .clone()
            .or_else(|| vanilla.java_version.clone()),
        logging: vanilla.logging.clone(),
        inherits_from: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glint_meta::mojang::Argument;

    #[test]
    fn loader_libraries_replace_vanilla_ones_and_arguments_append() {
        let vanilla: VersionInfo = serde_json::from_str(
            r#"{"id": "1.21.4", "type": "release", "mainClass": "net.minecraft.client.main.Main",
                "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21},
                "libraries": [{"name": "org.ow2.asm:asm:9.3"}, {"name": "com.mojang:brigadier:1.3.10"},
                              {"name": "org.lwjgl:lwjgl:3.3.3:natives-windows"}],
                "arguments": {"game": ["--username", "${auth_player_name}"], "jvm": ["-cp", "${classpath}"]}}"#,
        )
        .unwrap();
        let fabric: VersionInfo = serde_json::from_str(
            r#"{"id": "fabric-loader-0.16.9-1.21.4", "inheritsFrom": "1.21.4", "type": "release",
                "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
                "libraries": [{"name": "org.ow2.asm:asm:9.7.1", "url": "https://maven.fabricmc.net/"},
                              {"name": "net.fabricmc:fabric-loader:0.16.9", "url": "https://maven.fabricmc.net/"}],
                "arguments": {"game": [], "jvm": ["-DFabricMcEmu= net.minecraft.client.main.Main "]}}"#,
        )
        .unwrap();

        let merged = merge(&vanilla, &fabric);
        let names: Vec<_> = merged.libraries.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "org.ow2.asm:asm:9.7.1",
                "net.fabricmc:fabric-loader:0.16.9",
                "com.mojang:brigadier:1.3.10",
                "org.lwjgl:lwjgl:3.3.3:natives-windows"
            ]
        );
        assert_eq!(merged.id, "1.21.4");
        assert_eq!(
            merged.main_class,
            "net.fabricmc.loader.impl.launch.knot.KnotClient"
        );
        assert_eq!(merged.java_version.unwrap().major_version, 21);
        let jvm = merged.arguments.unwrap().jvm;
        assert!(matches!(jvm.last(), Some(Argument::Plain(s)) if s.starts_with("-DFabricMcEmu")));
        assert!(merged.inherits_from.is_none());
    }

    #[test]
    fn classifiers_are_separate_libraries() {
        assert_ne!(
            library_key("org.lwjgl:lwjgl:3.3.3"),
            library_key("org.lwjgl:lwjgl:3.3.3:natives-windows")
        );
        assert_eq!(
            library_key("org.lwjgl:lwjgl:3.3.3"),
            library_key("org.lwjgl:lwjgl:3.3.4")
        );
        assert_eq!(library_key("a:b:1@zip"), "a:b");
    }
}
