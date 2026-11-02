//! The parts of the Modrinth v2 API Glint uses.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::{Error, Result, Target, modrinth_loaders};

pub const DEFAULT_BASE: &str = "https://api.modrinth.com/v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    Mod,
    Modpack,
    Resourcepack,
    Shader,
    Datapack,
}

impl ProjectType {
    fn facet(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::Modpack => "modpack",
            Self::Resourcepack => "resourcepack",
            Self::Shader => "shader",
            Self::Datapack => "datapack",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub text: String,
    pub project_type: ProjectType,
    /// Restricts results to this Minecraft version and loader when set.
    pub target: Option<Target>,
    pub category: Option<String>,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub hits: Vec<Hit>,
    #[serde(alias = "total_hits")]
    pub total_hits: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    #[serde(alias = "project_id")]
    pub project_id: String,
    pub slug: Option<String>,
    pub title: String,
    pub description: String,
    pub author: String,
    #[serde(alias = "icon_url")]
    pub icon_url: Option<String>,
    pub downloads: u64,
    #[serde(default)]
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Version {
    pub id: String,
    pub name: String,
    pub version_number: String,
    #[serde(default)]
    pub version_type: String,
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    pub hashes: FileHashes,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileHashes {
    pub sha512: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Dependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub dependency_type: String,
}

impl Version {
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files.iter().find(|f| f.primary).or_else(|| self.files.first())
    }
}

#[derive(Deserialize)]
struct Project {
    title: String,
}

pub struct ModrinthApi {
    http: reqwest::Client,
    base: String,
    titles: Mutex<HashMap<String, String>>,
}

impl ModrinthApi {
    pub fn new(http: reqwest::Client) -> Self {
        Self::with_base(http, DEFAULT_BASE)
    }

    pub fn with_base(http: reqwest::Client, base: impl Into<String>) -> Self {
        Self { http, base: base.into(), titles: Mutex::new(HashMap::new()) }
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path_and_query: &str) -> Result<T> {
        let url = format!("{}{}", self.base, path_and_query);
        let response = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|source| glint_net::Error::Http { url: url.clone(), source })?;
        if !response.status().is_success() {
            return Err(glint_net::Error::Status { url, status: response.status().as_u16() }.into());
        }
        let text = response.text().await.map_err(|source| glint_net::Error::Http { url: url.clone(), source })?;
        serde_json::from_str(&text).map_err(|source| Error::Json { url, source })
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<SearchResults> {
        let mut facets = vec![vec![format!("project_type:{}", query.project_type.facet())]];
        if let Some(target) = &query.target {
            facets.push(vec![format!("versions:{}", target.game_version)]);
            if matches!(query.project_type, ProjectType::Mod | ProjectType::Modpack) {
                facets.push(modrinth_loaders(target.loader).iter().map(|l| format!("categories:{l}")).collect());
            }
        }
        if let Some(category) = &query.category {
            facets.push(vec![format!("categories:{category}")]);
        }
        let facets = serde_json::to_string(&facets).expect("facets serialize");
        let index = if query.text.trim().is_empty() { "downloads" } else { "relevance" };
        let path = format!(
            "/search?query={}&facets={}&index={index}&offset={}&limit={}",
            encode(query.text.trim()),
            encode(&facets),
            query.offset,
            query.limit.clamp(1, 100)
        );
        self.get(&path).await
    }

    pub async fn version(&self, version_id: &str) -> Result<Version> {
        self.get(&format!("/version/{}", encode(version_id))).await
    }

    /// The newest release compatible with `target`, falling back to betas when there is no release.
    pub async fn best_version(&self, project_id: &str, target: &Target) -> Result<Version> {
        let loaders = serde_json::to_string(modrinth_loaders(target.loader)).expect("loaders serialize");
        let games = serde_json::to_string(&[&target.game_version]).expect("versions serialize");
        let versions: Vec<Version> = self
            .get(&format!(
                "/project/{}/version?loaders={}&game_versions={}",
                encode(project_id),
                encode(&loaders),
                encode(&games)
            ))
            .await?;
        let release = versions.iter().position(|v| v.version_type == "release");
        let index = release.or(if versions.is_empty() { None } else { Some(0) });
        index
            .map(|i| versions[i].clone())
            .ok_or_else(|| Error::NoCompatibleVersion(self.cached_title(project_id).unwrap_or_else(|| project_id.to_owned())))
    }

    pub async fn project_title(&self, project_id: &str) -> Result<String> {
        if let Some(title) = self.cached_title(project_id) {
            return Ok(title);
        }
        let project: Project = self.get(&format!("/project/{}", encode(project_id))).await?;
        self.titles.lock().expect("titles lock").insert(project_id.to_owned(), project.title.clone());
        Ok(project.title)
    }

    fn cached_title(&self, project_id: &str) -> Option<String> {
        self.titles.lock().expect("titles lock").get(project_id).cloned()
    }
}

/// Percent-encodes a query value (RFC 3986 unreserved characters pass through).
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_query_values() {
        assert_eq!(encode("sodium extra"), "sodium%20extra");
        assert_eq!(encode(r#"[["versions:1.21.4"]]"#), "%5B%5B%22versions%3A1.21.4%22%5D%5D");
        assert_eq!(encode("кот"), "%D0%BA%D0%BE%D1%82");
    }
}
