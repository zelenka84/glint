//! Forge and NeoForge: run the official installer's client steps without its UI.
//!
//! The installer jar holds `install_profile.json` (libraries, data and "processors",
//! small Java tools that deobfuscate and patch the vanilla client) and `version.json`
//! (the launch profile). Only the modern format (Minecraft 1.13+) is supported.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use glint_core::LoaderKind;
use glint_meta::library::{library_files, maven_path};
use glint_meta::mojang::{Library, VersionInfo};
use glint_meta::{InstallPlan, MetaClient, Os, PlannedFile, Platform};
use glint_net::ExpectedHash;
use serde::Deserialize;

use crate::{Error, Result, io, loaders_dir};

const MARKER: &str = ".installed";

#[derive(Deserialize)]
struct InstallProfile {
    #[serde(default)]
    data: HashMap<String, Sided>,
    #[serde(default)]
    processors: Vec<Processor>,
    #[serde(default)]
    libraries: Vec<Library>,
    /// Present only in the pre-1.13 installer format.
    #[serde(rename = "versionInfo")]
    version_info: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct Sided {
    client: String,
}

#[derive(Deserialize)]
struct Processor {
    #[serde(default)]
    sides: Option<Vec<String>>,
    jar: String,
    #[serde(default)]
    classpath: Vec<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    outputs: HashMap<String, String>,
}

fn coordinates(kind: LoaderKind, game: &str, loader: &str) -> (String, String) {
    match kind {
        LoaderKind::NeoForge => (
            loader.to_owned(),
            format!(
                "https://maven.neoforged.net/releases/net/neoforged/neoforge/{loader}/neoforge-{loader}-installer.jar"
            ),
        ),
        _ => {
            let full = format!("{game}-{loader}");
            let url = format!(
                "https://maven.minecraftforge.net/net/minecraftforge/forge/{full}/forge-{full}-installer.jar"
            );
            (full, url)
        }
    }
}

pub(crate) async fn install(
    http: &reqwest::Client,
    meta: &MetaClient,
    kind: LoaderKind,
    vanilla: &VersionInfo,
    loader: &str,
    java: &Path,
    platform: &Platform,
) -> Result<VersionInfo> {
    let (full, url) = coordinates(kind, &vanilla.id, loader);
    let name = if kind == LoaderKind::NeoForge {
        "neoforge"
    } else {
        "forge"
    };
    let dir = loaders_dir(meta).join(name).join(&full);
    let profile_path = dir.join("version.json");

    if dir.join(MARKER).is_file() {
        return read_profile(&profile_path).await;
    }

    let installer = dir.join("installer.jar");
    if !installer.is_file() {
        let downloader = glint_net::Downloader::new(http.clone(), 1);
        downloader
            .download(&url, &installer, None)
            .await
            .map_err(|e| match e {
                glint_net::Error::Status { status: 404, .. } => Error::UnknownVersion {
                    loader: crate::display_name(kind),
                    version: loader.into(),
                    game: vanilla.id.clone(),
                },
                other => other.into(),
            })?;
    }

    let mut archive = open_zip(&installer)?;
    let install: InstallProfile = parse_entry(&mut archive, &installer, "install_profile.json")?;
    if install.version_info.is_some() {
        return Err(Error::Unsupported(format!(
            "{} for Minecraft {} uses the old installer format; Glint supports 1.13 and newer",
            crate::display_name(kind),
            vanilla.id
        )));
    }
    let version_text = read_entry(&mut archive, &installer, "version.json")?;
    let profile: VersionInfo =
        serde_json::from_slice(&version_text).map_err(|source| Error::Json {
            what: format!("{name} version.json"),
            source,
        })?;

    let libraries = meta.layout().libraries();
    extract_maven(&mut archive, &installer, &libraries)?;

    // Libraries for the installer steps and for the game itself.
    // Both lists name some of the same jars; download each once.
    let mut plan = InstallPlan::default();
    let mut seen = std::collections::HashSet::new();
    for lib in install.libraries.iter().chain(&profile.libraries) {
        for file in library_files(lib, platform) {
            let dest = libraries.join(&file.path);
            if file.url.is_empty() || dest.is_file() || !seen.insert(dest.clone()) {
                continue;
            }
            plan.files.push(PlannedFile {
                url: file.url,
                dest,
                hash: file.sha1.map(ExpectedHash::Sha1),
                size: file.size,
            });
        }
    }
    meta.install(&plan, |_| {}).await?;

    let data = resolve_data(&install, &mut archive, &installer, &dir, meta, vanilla)?;
    for processor in &install.processors {
        let for_client = processor
            .sides
            .as_ref()
            .is_none_or(|s| s.iter().any(|side| side == "client"));
        if for_client {
            run_processor(processor, &data, &libraries, java, platform.os).await?;
        }
    }

    tokio::fs::write(&profile_path, &version_text)
        .await
        .map_err(|e| io(&profile_path, e))?;
    tokio::fs::write(dir.join(MARKER), b"")
        .await
        .map_err(|e| io(&dir, e))?;
    Ok(profile)
}

async fn read_profile(path: &Path) -> Result<VersionInfo> {
    let text = tokio::fs::read(path).await.map_err(|e| io(path, e))?;
    serde_json::from_slice(&text).map_err(|source| Error::Json {
        what: path.display().to_string(),
        source,
    })
}

fn open_zip(path: &Path) -> Result<zip::ZipArchive<std::fs::File>> {
    let file = std::fs::File::open(path).map_err(|e| io(path, e))?;
    zip::ZipArchive::new(file).map_err(|source| Error::Zip {
        path: path.to_owned(),
        source,
    })
}

fn read_entry(
    archive: &mut zip::ZipArchive<std::fs::File>,
    path: &Path,
    name: &str,
) -> Result<Vec<u8>> {
    let mut entry = archive.by_name(name).map_err(|source| Error::Zip {
        path: path.to_owned(),
        source,
    })?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).map_err(|e| io(path, e))?;
    Ok(bytes)
}

fn parse_entry<T: serde::de::DeserializeOwned>(
    archive: &mut zip::ZipArchive<std::fs::File>,
    path: &Path,
    name: &str,
) -> Result<T> {
    let bytes = read_entry(archive, path, name)?;
    serde_json::from_slice(&bytes).map_err(|source| Error::Json {
        what: name.to_owned(),
        source,
    })
}

/// Installers embed some jars (the Forge universal jar) under `maven/`.
fn extract_maven(
    archive: &mut zip::ZipArchive<std::fs::File>,
    path: &Path,
    libraries: &Path,
) -> Result<()> {
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|source| Error::Zip {
            path: path.to_owned(),
            source,
        })?;
        let Some(rel) = entry
            .enclosed_name()
            .and_then(|n| n.strip_prefix("maven").ok().map(Path::to_path_buf))
        else {
            continue;
        };
        if entry.is_dir() || rel.as_os_str().is_empty() {
            continue;
        }
        let dest = libraries.join(rel);
        if dest.is_file() {
            continue;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
        }
        let mut out = std::fs::File::create(&dest).map_err(|e| io(&dest, e))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| io(&dest, e))?;
    }
    Ok(())
}

fn library_path(libraries: &Path, coordinate: &str) -> Result<String> {
    let rel = maven_path(coordinate)
        .ok_or_else(|| Error::Unsupported(format!("bad library coordinate {coordinate}")))?;
    Ok(libraries.join(rel).display().to_string())
}

/// The installer's `data` table plus the built-in keys, all turned into final strings.
fn resolve_data(
    install: &InstallProfile,
    archive: &mut zip::ZipArchive<std::fs::File>,
    installer: &Path,
    dir: &Path,
    meta: &MetaClient,
    vanilla: &VersionInfo,
) -> Result<HashMap<String, String>> {
    let libraries = meta.layout().libraries();
    let mut data = HashMap::from([
        ("SIDE".to_owned(), "client".to_owned()),
        (
            "MINECRAFT_JAR".to_owned(),
            meta.layout().client_jar(&vanilla.id).display().to_string(),
        ),
        ("MINECRAFT_VERSION".to_owned(), vanilla.id.clone()),
        ("ROOT".to_owned(), dir.display().to_string()),
        ("INSTALLER".to_owned(), installer.display().to_string()),
        ("LIBRARY_DIR".to_owned(), libraries.display().to_string()),
    ]);
    for (key, value) in &install.data {
        let value = &value.client;
        let resolved = if let Some(coord) =
            value.strip_prefix('[').and_then(|v| v.strip_suffix(']'))
        {
            library_path(&libraries, coord)?
        } else if let Some(literal) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
            literal.to_owned()
        } else if let Some(inner) = value.strip_prefix('/') {
            let bytes = read_entry(archive, installer, inner)?;
            let dest = dir.join("extracted").join(inner);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
            }
            std::fs::write(&dest, bytes).map_err(|e| io(&dest, e))?;
            dest.display().to_string()
        } else {
            value.clone()
        };
        data.insert(key.clone(), resolved);
    }
    Ok(data)
}

/// Resolves a processor argument: `[maven:coord]` → library path, `{KEY}` → data value.
fn resolve_arg(arg: &str, data: &HashMap<String, String>, libraries: &Path) -> Result<String> {
    if let Some(coord) = arg.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        return library_path(libraries, coord);
    }
    let mut out = String::with_capacity(arg.len());
    let mut rest = arg;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        match rest[start..].find('}') {
            Some(end) => {
                let key = &rest[start + 1..start + end];
                match data.get(key) {
                    Some(value) => out.push_str(value),
                    None => out.push_str(&rest[start..=start + end]),
                }
                rest = &rest[start + end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    Ok(out.trim_matches('\'').to_owned())
}

fn main_class(jar: &Path) -> Result<String> {
    let mut archive = open_zip(jar)?;
    let manifest = read_entry(&mut archive, jar, "META-INF/MANIFEST.MF")?;
    String::from_utf8_lossy(&manifest)
        .lines()
        .find_map(|line| {
            line.strip_prefix("Main-Class:")
                .map(|v| v.trim().to_owned())
        })
        .ok_or_else(|| Error::Unsupported(format!("{} has no Main-Class", jar.display())))
}

async fn outputs_ready(outputs: &[(PathBuf, String)]) -> bool {
    for (path, sha1) in outputs {
        if !glint_net::file_matches(path, &ExpectedHash::Sha1(sha1.clone())).await {
            return false;
        }
    }
    true
}

async fn run_processor(
    processor: &Processor,
    data: &HashMap<String, String>,
    libraries: &Path,
    java: &Path,
    os: Os,
) -> Result<()> {
    let outputs = processor
        .outputs
        .iter()
        .map(|(file, sha)| {
            Ok((
                PathBuf::from(resolve_arg(file, data, libraries)?),
                resolve_arg(sha, data, libraries)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    if !outputs.is_empty() && outputs_ready(&outputs).await {
        return Ok(());
    }

    let jar = PathBuf::from(library_path(libraries, &processor.jar)?);
    let separator = if os == Os::Windows { ";" } else { ":" };
    let mut classpath = vec![jar.display().to_string()];
    for coord in &processor.classpath {
        classpath.push(library_path(libraries, coord)?);
    }
    let args = processor
        .args
        .iter()
        .map(|a| resolve_arg(a, data, libraries))
        .collect::<Result<Vec<_>>>()?;
    let step = format!(
        "{} {}",
        processor.jar,
        args.first().map(String::as_str).unwrap_or_default()
    );
    tracing::info!(step, "running loader installer step");

    let mut cmd = tokio::process::Command::new(java);
    // Mojang mapping downloads happen inside the step; let Java honour the system proxy.
    cmd.arg("-Djava.net.useSystemProxies=true")
        .arg("-cp")
        .arg(classpath.join(separator))
        .arg(main_class(&jar)?)
        .args(&args);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let output = cmd.output().await.map_err(|e| io(java, e))?;
    if !output.status.success() {
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let tail: Vec<&str> = text.lines().rev().take(12).collect();
        return Err(Error::Processor {
            step,
            status: output.status.to_string(),
            output: tail.into_iter().rev().collect::<Vec<_>>().join("\n"),
        });
    }
    if !outputs.is_empty() && !outputs_ready(&outputs).await {
        return Err(Error::Processor {
            step,
            status: "finished".into(),
            output: "produced files do not match their checksums".into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_resolve_libraries_and_data() {
        let libs = Path::new("/libs");
        let data = HashMap::from([
            (
                "MC_OFF".to_owned(),
                "/libs/net/minecraft/client/1.21.4/client-1.21.4-official.jar".to_owned(),
            ),
            (
                "MC_OFF_SHA".to_owned(),
                "1848e6275e667d4196ec13b7785a4d13cc6b7a82".to_owned(),
            ),
        ]);
        assert_eq!(
            resolve_arg("{MC_OFF}", &data, libs).unwrap(),
            data["MC_OFF"]
        );
        assert_eq!(
            resolve_arg("{MC_OFF_SHA}", &data, libs).unwrap(),
            data["MC_OFF_SHA"]
        );
        assert_eq!(
            resolve_arg("--side={SIDE}", &data, libs).unwrap(),
            "--side={SIDE}"
        );
        assert_eq!(
            PathBuf::from(
                resolve_arg("[net.minecraft:client:1.21.4:mappings@tsrg]", &data, libs).unwrap()
            ),
            libs.join("net/minecraft/client/1.21.4/client-1.21.4-mappings.tsrg")
        );
    }

    #[test]
    fn installer_urls() {
        let (full, url) = coordinates(LoaderKind::Forge, "1.21.4", "54.1.22");
        assert_eq!(full, "1.21.4-54.1.22");
        assert!(url.ends_with("/forge/1.21.4-54.1.22/forge-1.21.4-54.1.22-installer.jar"));
        let (full, url) = coordinates(LoaderKind::NeoForge, "1.21.4", "21.4.161");
        assert_eq!(full, "21.4.161");
        assert!(url.ends_with("/neoforge/21.4.161/neoforge-21.4.161-installer.jar"));
    }
}
