//! Minecraft news and Java patch notes from the feeds the official launcher reads.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{AppState, CmdResult, err};

const BASE: &str = "https://launchercontent.mojang.com";

#[derive(Deserialize)]
struct Feed<T> {
    entries: Vec<T>,
}

#[derive(Deserialize)]
struct Image {
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawNews {
    id: String,
    title: String,
    #[serde(default)]
    tag: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    text: String,
    play_page_image: Option<Image>,
    news_page_image: Option<Image>,
    #[serde(default)]
    read_more_link: String,
    #[serde(default)]
    news_type: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    id: String,
    title: String,
    tag: String,
    date: String,
    text: String,
    image: Option<String>,
    link: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPatchNote {
    id: String,
    title: String,
    #[serde(default)]
    version: String,
    #[serde(default, rename = "type")]
    kind: String,
    image: Option<Image>,
    #[serde(default)]
    date: String,
    #[serde(default)]
    short_text: String,
    content_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchNote {
    id: String,
    title: String,
    version: String,
    kind: String,
    image: Option<String>,
    date: String,
    short_text: String,
    content_path: String,
}

#[derive(Deserialize)]
struct PatchBody {
    body: String,
}

fn absolute(url: &str) -> String {
    if url.starts_with("http") {
        url.to_owned()
    } else {
        format!("{BASE}{url}")
    }
}

async fn fetch<T: serde::de::DeserializeOwned>(state: &AppState, path: &str) -> CmdResult<T> {
    state
        .http
        .get(format!("{BASE}{path}"))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(err)?
        .json()
        .await
        .map_err(err)
}

/// Java Edition news, newest first.
#[tauri::command]
pub async fn minecraft_news(state: State<'_, AppState>) -> CmdResult<Vec<NewsItem>> {
    let feed: Feed<RawNews> = fetch(&state, "/v2/news.json").await?;
    Ok(feed
        .entries
        .into_iter()
        .filter(|n| n.news_type.iter().any(|t| t == "Java"))
        .take(12)
        .map(|n| NewsItem {
            id: n.id,
            title: n.title,
            tag: n.tag,
            date: n.date,
            text: n.text,
            image: n
                .play_page_image
                .or(n.news_page_image)
                .map(|i| absolute(&i.url)),
            link: n.read_more_link,
        })
        .collect())
}

/// Java Edition patch notes (releases and snapshots), newest first.
#[tauri::command]
pub async fn patch_notes(state: State<'_, AppState>) -> CmdResult<Vec<PatchNote>> {
    let feed: Feed<RawPatchNote> = fetch(&state, "/v2/javaPatchNotes.json").await?;
    Ok(feed
        .entries
        .into_iter()
        .take(12)
        .map(|p| PatchNote {
            id: p.id,
            title: p.title,
            version: p.version,
            kind: p.kind,
            image: p.image.map(|i| absolute(&i.url)),
            date: p.date,
            short_text: p.short_text,
            content_path: p.content_path,
        })
        .collect())
}

/// The HTML body of one patch note.
#[tauri::command]
pub async fn patch_note_body(
    state: State<'_, AppState>,
    content_path: String,
) -> CmdResult<String> {
    if !content_path.starts_with("javaPatchNotes/") || content_path.contains("..") {
        return Err(format!("not a patch note: {content_path}"));
    }
    let body: PatchBody = fetch(&state, &format!("/v2/{content_path}")).await?;
    Ok(body.body)
}
