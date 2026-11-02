//! Which loader builds exist for a Minecraft version.

use std::cmp::Ordering;

use glint_core::LoaderKind;
use serde::{Deserialize, Serialize};

use crate::{Error, Result, display_name};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersion {
    pub id: String,
    /// The build the loader's authors point players at (stable / recommended).
    pub recommended: bool,
}

/// Available builds of `kind` for Minecraft `game`, newest first.
pub async fn loader_versions(
    http: &reqwest::Client,
    kind: LoaderKind,
    game: &str,
) -> Result<Vec<LoaderVersion>> {
    let mut versions = match kind {
        LoaderKind::Fabric => {
            fabric_like(
                http,
                &format!("https://meta.fabricmc.net/v2/versions/loader/{game}"),
            )
            .await?
        }
        LoaderKind::Quilt => {
            fabric_like(
                http,
                &format!("https://meta.quiltmc.org/v3/versions/loader/{game}"),
            )
            .await?
        }
        LoaderKind::NeoForge => neoforge(http, game).await?,
        LoaderKind::Forge => forge(http, game).await?,
    };
    if versions.is_empty() {
        return Err(Error::Unsupported(format!(
            "{} is not available for Minecraft {game}",
            display_name(kind)
        )));
    }
    if !versions.iter().any(|v| v.recommended) {
        versions[0].recommended = true;
    }
    Ok(versions)
}

async fn get_text(http: &reqwest::Client, url: &str) -> Result<String> {
    let response = http
        .get(url)
        .send()
        .await
        .map_err(|source| glint_net::Error::Http {
            url: url.into(),
            source,
        })?;
    if !response.status().is_success() {
        return Err(glint_net::Error::Status {
            url: url.into(),
            status: response.status().as_u16(),
        }
        .into());
    }
    Ok(response
        .text()
        .await
        .map_err(|source| glint_net::Error::Http {
            url: url.into(),
            source,
        })?)
}

async fn get_json<T: serde::de::DeserializeOwned>(http: &reqwest::Client, url: &str) -> Result<T> {
    let text = get_text(http, url).await?;
    serde_json::from_str(&text).map_err(|source| Error::Json {
        what: url.to_owned(),
        source,
    })
}

#[derive(Deserialize)]
struct FabricEntry {
    loader: FabricLoader,
}

#[derive(Deserialize)]
struct FabricLoader {
    version: String,
    #[serde(default)]
    stable: Option<bool>,
}

/// Fabric and Quilt share the meta format; Quilt's list is unordered and has no
/// `stable` flag, so both are sorted and stability falls back to "not a pre-release".
async fn fabric_like(http: &reqwest::Client, url: &str) -> Result<Vec<LoaderVersion>> {
    let entries: Vec<FabricEntry> = get_json(http, url).await?;
    let mut versions: Vec<_> = entries
        .into_iter()
        .map(|e| {
            let stable = e
                .loader
                .stable
                .unwrap_or_else(|| !is_prerelease(&e.loader.version));
            (e.loader.version, stable)
        })
        .collect();
    versions.sort_by(|a, b| compare_versions(&b.0, &a.0));
    Ok(mark_first_stable(versions))
}

#[derive(Deserialize)]
struct MavenVersions {
    versions: Vec<String>,
}

async fn neoforge(http: &reqwest::Client, game: &str) -> Result<Vec<LoaderVersion>> {
    let list: MavenVersions = get_json(
        http,
        "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge",
    )
    .await?;
    let mut versions: Vec<_> = list
        .versions
        .into_iter()
        .filter(|v| neoforge_game_version(v).as_deref() == Some(game))
        .map(|v| {
            let stable = !is_prerelease(&v);
            (v, stable)
        })
        .collect();
    versions.sort_by(|a, b| compare_versions(&b.0, &a.0));
    Ok(mark_first_stable(versions))
}

/// NeoForge numbers builds after the game: `21.4.161` is for 1.21.4, `21.0.5` for 1.21,
/// and from the year-based releases on `26.3.0.101` is for 26.3 (`26.3.1.x` for 26.3.1).
pub(crate) fn neoforge_game_version(version: &str) -> Option<String> {
    let numbers: Vec<u32> = version
        .split(['.', '-'])
        .map_while(|p| p.parse().ok())
        .collect();
    match numbers.as_slice() {
        [major, minor, patch, _build, ..] if *major >= 25 => Some(if *patch == 0 {
            format!("{major}.{minor}")
        } else {
            format!("{major}.{minor}.{patch}")
        }),
        [major, minor, _build, ..] if *major < 25 => Some(if *minor == 0 {
            format!("1.{major}")
        } else {
            format!("1.{major}.{minor}")
        }),
        _ => None,
    }
}

#[derive(Deserialize)]
struct Promotions {
    promos: std::collections::HashMap<String, String>,
}

async fn forge(http: &reqwest::Client, game: &str) -> Result<Vec<LoaderVersion>> {
    let xml = get_text(
        http,
        "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml",
    )
    .await?;
    let prefix = format!("{game}-");
    let re = regex::Regex::new(r"<version>([^<]+)</version>").expect("valid regex");
    let mut versions: Vec<String> = re
        .captures_iter(&xml)
        .filter_map(|c| c[1].strip_prefix(&prefix).map(str::to_owned))
        .filter(|v| !v.contains('-'))
        .collect();
    versions.sort_by(|a, b| compare_versions(b, a));

    let promos: Option<Promotions> = get_json(
        http,
        "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json",
    )
    .await
    .ok();
    let recommended = promos.and_then(|p| {
        p.promos
            .get(&format!("{game}-recommended"))
            .or_else(|| p.promos.get(&format!("{game}-latest")))
            .cloned()
    });
    Ok(versions
        .into_iter()
        .map(|id| LoaderVersion {
            recommended: recommended.as_deref() == Some(id.as_str()),
            id,
        })
        .collect())
}

fn mark_first_stable(versions: Vec<(String, bool)>) -> Vec<LoaderVersion> {
    let first_stable = versions.iter().position(|(_, stable)| *stable);
    versions
        .into_iter()
        .enumerate()
        .map(|(i, (id, _))| LoaderVersion {
            id,
            recommended: Some(i) == first_stable,
        })
        .collect()
}

fn is_prerelease(version: &str) -> bool {
    let lower = version.to_ascii_lowercase();
    ["alpha", "beta", "rc", "pre"]
        .iter()
        .any(|tag| lower.contains(tag))
}

/// Numeric comparison of dotted versions; a pre-release sorts below the release it precedes.
pub(crate) fn compare_versions(a: &str, b: &str) -> Ordering {
    let numbers = |v: &str| -> Vec<u64> {
        v.split(|c: char| !c.is_ascii_digit())
            .filter_map(|p| p.parse().ok())
            .collect()
    };
    let core = |v: &str| v.split(['-', '+']).next().unwrap_or(v).to_owned();
    numbers(&core(a))
        .cmp(&numbers(&core(b)))
        .then_with(|| is_prerelease(b).cmp(&is_prerelease(a)))
        .then_with(|| numbers(a).cmp(&numbers(b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neoforge_versions_map_to_game_versions() {
        assert_eq!(neoforge_game_version("21.4.161").as_deref(), Some("1.21.4"));
        assert_eq!(neoforge_game_version("21.0.167").as_deref(), Some("1.21"));
        assert_eq!(
            neoforge_game_version("20.2.88-beta").as_deref(),
            Some("1.20.2")
        );
        assert_eq!(neoforge_game_version("26.3.0.101").as_deref(), Some("26.3"));
        assert_eq!(neoforge_game_version("26.3.1.4").as_deref(), Some("26.3.1"));
        assert_eq!(neoforge_game_version("nonsense"), None);
    }

    #[test]
    fn versions_sort_numerically_with_prereleases_first_below() {
        let mut v = vec![
            "0.20.0-beta.9",
            "0.19.2",
            "0.20.0",
            "0.20.0-beta.10",
            "0.9.1",
        ];
        v.sort_by(|a, b| compare_versions(b, a));
        assert_eq!(
            v,
            [
                "0.20.0",
                "0.20.0-beta.10",
                "0.20.0-beta.9",
                "0.19.2",
                "0.9.1"
            ]
        );
    }

    #[test]
    fn first_stable_is_recommended() {
        let marked = mark_first_stable(vec![
            ("2.0-beta".into(), false),
            ("1.9".into(), true),
            ("1.8".into(), true),
        ]);
        assert_eq!(
            marked
                .iter()
                .filter(|v| v.recommended)
                .map(|v| v.id.as_str())
                .collect::<Vec<_>>(),
            ["1.9"]
        );
    }
}
