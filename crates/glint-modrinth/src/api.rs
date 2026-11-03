//! The parts of the Modrinth v2 API Glint uses.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::{ContentKind, Error, Result, Target, modrinth_loaders};

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
    #[serde(default)]
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    #[serde(default)]
    pub version_type: String,
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub date_published: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub changelog: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub size: u64,
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
        self.files
            .iter()
            .find(|f| f.primary)
            .or_else(|| self.files.first())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub icon_url: Option<String>,
}

/// Everything Modrinth says about a project, for its page in Discover.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetails {
    pub id: String,
    pub slug: Option<String>,
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// Long description in Markdown (may contain HTML).
    #[serde(default)]
    pub body: String,
    #[serde(default, alias = "icon_url")]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub followers: u64,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default, alias = "additional_categories")]
    pub additional_categories: Vec<String>,
    #[serde(default)]
    pub license: Option<License>,
    #[serde(default, alias = "source_url")]
    pub source_url: Option<String>,
    #[serde(default, alias = "issues_url")]
    pub issues_url: Option<String>,
    #[serde(default, alias = "wiki_url")]
    pub wiki_url: Option<String>,
    #[serde(default, alias = "discord_url")]
    pub discord_url: Option<String>,
    #[serde(default)]
    pub gallery: Vec<GalleryImage>,
    #[serde(default)]
    pub published: String,
    #[serde(default)]
    pub updated: String,
    #[serde(default, alias = "client_side")]
    pub client_side: String,
    #[serde(default, alias = "server_side")]
    pub server_side: String,
    #[serde(default, alias = "project_type")]
    pub project_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct License {
    pub id: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryImage {
    pub url: String,
    #[serde(default, alias = "raw_url")]
    pub raw_url: Option<String>,
    #[serde(default)]
    pub featured: bool,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

pub struct ModrinthApi {
    http: reqwest::Client,
    base: String,
    projects: Mutex<HashMap<String, Project>>,
}

impl ModrinthApi {
    pub fn new(http: reqwest::Client) -> Self {
        Self::with_base(http, DEFAULT_BASE)
    }

    pub fn with_base(http: reqwest::Client, base: impl Into<String>) -> Self {
        Self {
            http,
            base: base.into(),
            projects: Mutex::new(HashMap::new()),
        }
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path_and_query: &str) -> Result<T> {
        let url = format!("{}{}", self.base, path_and_query);
        let request = self.http.get(&url);
        self.send(url, request).await
    }

    async fn post<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let url = format!("{}{}", self.base, path);
        let request = self.http.post(&url).json(body);
        self.send(url, request).await
    }

    async fn send<T: serde::de::DeserializeOwned>(
        &self,
        url: String,
        request: reqwest::RequestBuilder,
    ) -> Result<T> {
        let response = request
            .send()
            .await
            .map_err(|source| glint_net::Error::Http {
                url: url.clone(),
                source,
            })?;
        if !response.status().is_success() {
            return Err(glint_net::Error::Status {
                url,
                status: response.status().as_u16(),
            }
            .into());
        }
        let text = response
            .text()
            .await
            .map_err(|source| glint_net::Error::Http {
                url: url.clone(),
                source,
            })?;
        serde_json::from_str(&text).map_err(|source| Error::Json { url, source })
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<SearchResults> {
        let mut facets = vec![vec![format!("project_type:{}", query.project_type.facet())]];
        if let Some(target) = &query.target {
            facets.push(vec![format!("versions:{}", target.game_version)]);
            if let (ProjectType::Mod | ProjectType::Modpack, Some(loader)) =
                (query.project_type, target.loader)
            {
                facets.push(
                    modrinth_loaders(loader)
                        .iter()
                        .map(|l| format!("categories:{l}"))
                        .collect(),
                );
            }
        }
        if let Some(category) = &query.category {
            facets.push(vec![format!("categories:{category}")]);
        }
        let facets = serde_json::to_string(&facets).expect("facets serialize");
        let index = if query.text.trim().is_empty() {
            "downloads"
        } else {
            "relevance"
        };
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

    /// Versions of `project_id` (an id or slug) compatible with `target`, newest first.
    pub async fn versions(
        &self,
        project_id: &str,
        target: &Target,
        kind: ContentKind,
    ) -> Result<Vec<Version>> {
        let games = serde_json::to_string(&[&target.game_version]).expect("versions serialize");
        let mut path = format!(
            "/project/{}/version?game_versions={}",
            encode(project_id),
            encode(&games)
        );
        if let Some(loaders) = kind.loaders(target) {
            let loaders = serde_json::to_string(&loaders).expect("loaders serialize");
            path.push_str(&format!("&loaders={}", encode(&loaders)));
        }
        self.get(&path).await
    }

    /// The full project, uncached: its page shows the live description and counts.
    pub async fn project_details(&self, project_id: &str) -> Result<ProjectDetails> {
        self.get(&format!("/project/{}", encode(project_id))).await
    }

    /// The newest release of `project_id` (an id or slug) compatible with `target`,
    /// falling back to betas when there is no release.
    pub async fn best_version(
        &self,
        project_id: &str,
        target: &Target,
        kind: ContentKind,
    ) -> Result<Version> {
        let versions = self.versions(project_id, target, kind).await?;
        Self::pick_best(&versions).cloned().ok_or_else(|| {
            Error::NoCompatibleVersion(
                self.cached_title(project_id)
                    .unwrap_or_else(|| project_id.to_owned()),
            )
        })
    }

    /// The first release in a newest-first list, else whatever is newest.
    pub fn pick_best(versions: &[Version]) -> Option<&Version> {
        versions
            .iter()
            .find(|v| v.version_type == "release")
            .or(versions.first())
    }

    /// Newest compatible versions for installed files, keyed by their SHA-512.
    pub async fn latest_for_hashes(
        &self,
        hashes: &[String],
        target: &Target,
        kind: ContentKind,
    ) -> Result<HashMap<String, Version>> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }
        let mut body = serde_json::json!({
            "hashes": hashes,
            "algorithm": "sha512",
            "game_versions": [target.game_version],
        });
        if let Some(loaders) = kind.loaders(target) {
            body["loaders"] = serde_json::json!(loaders);
        }
        self.post("/version_files/update", &body).await
    }

    /// Title and icon of a project, cached for the session.
    pub async fn project(&self, project_id: &str) -> Result<Project> {
        if let Some(project) = self.projects.lock().expect("projects lock").get(project_id) {
            return Ok(project.clone());
        }
        let project: Project = self
            .get(&format!("/project/{}", encode(project_id)))
            .await?;
        self.projects
            .lock()
            .expect("projects lock")
            .insert(project_id.to_owned(), project.clone());
        Ok(project)
    }

    fn cached_title(&self, project_id: &str) -> Option<String> {
        self.projects
            .lock()
            .expect("projects lock")
            .get(project_id)
            .map(|p| p.title.clone())
    }
}

/// Percent-encodes a query value (RFC 3986 unreserved characters pass through).
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
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
        assert_eq!(
            encode(r#"[["versions:1.21.4"]]"#),
            "%5B%5B%22versions%3A1.21.4%22%5D%5D"
        );
        assert_eq!(encode("кот"), "%D0%BA%D0%BE%D1%82");
    }
}
