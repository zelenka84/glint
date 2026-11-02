//! End-to-end: manifest → version → asset index → plan → install, against a fake Mojang.

use std::sync::Mutex;

use glint_meta::{Arch, Endpoints, MetaClient, MetaLayout, Os, Platform};
use serde_json::json;
use sha1::{Digest, Sha1};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sha1_hex(bytes: &[u8]) -> String {
    hex::encode(Sha1::digest(bytes))
}

async fn serve(server: &MockServer, route: &str, body: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(server)
        .await;
}

/// Mounts a tiny but complete fake version and returns the manifest body.
async fn fake_mojang(server: &MockServer) -> Vec<u8> {
    let base = server.uri();
    let client_jar = b"client jar".to_vec();
    let library = b"brigadier".to_vec();
    let asset = b"click.ogg".to_vec();
    let asset_hash = sha1_hex(&asset);

    let index = serde_json::to_vec(&json!({
        "objects": {"minecraft/sounds/click.ogg": {"hash": asset_hash, "size": asset.len()}}
    }))
    .unwrap();

    let version = serde_json::to_vec(&json!({
        "id": "1.0-test", "type": "release", "mainClass": "net.minecraft.client.main.Main",
        "assetIndex": {"id": "test", "sha1": sha1_hex(&index), "size": index.len(), "url": format!("{base}/index.json")},
        "assets": "test",
        "downloads": {"client": {"sha1": sha1_hex(&client_jar), "size": client_jar.len(), "url": format!("{base}/client.jar")}},
        "libraries": [{"name": "com.mojang:brigadier:1.3.10", "downloads": {"artifact": {
            "path": "com/mojang/brigadier/1.3.10/brigadier-1.3.10.jar",
            "sha1": sha1_hex(&library), "size": library.len(), "url": format!("{base}/brigadier.jar")}}}]
    }))
    .unwrap();

    let manifest = serde_json::to_vec(&json!({
        "latest": {"release": "1.0-test", "snapshot": "1.0-test"},
        "versions": [{"id": "1.0-test", "type": "release", "url": format!("{base}/1.0-test.json"),
                      "sha1": sha1_hex(&version), "releaseTime": "2026-01-01T00:00:00+00:00"}]
    }))
    .unwrap();

    serve(server, "/manifest.json", manifest.clone()).await;
    serve(server, "/1.0-test.json", version).await;
    serve(server, "/index.json", index).await;
    serve(server, "/client.jar", client_jar).await;
    serve(server, "/brigadier.jar", library).await;
    serve(server, &format!("/{}/{}", &asset_hash[..2], asset_hash), asset).await;
    manifest
}

fn client(server: &MockServer, meta_root: &std::path::Path) -> MetaClient {
    let endpoints = Endpoints { manifest: format!("{}/manifest.json", server.uri()), resources: server.uri() };
    MetaClient::new(glint_net::http_client(), MetaLayout::new(meta_root), endpoints)
}

#[tokio::test]
async fn installs_a_version_end_to_end() {
    let server = MockServer::start().await;
    fake_mojang(&server).await;
    let tmp = tempfile::tempdir().unwrap();
    let meta = client(&server, tmp.path());

    let manifest = meta.manifest().await.unwrap();
    let entry = manifest.find(&manifest.latest.release).unwrap();
    let version = meta.version(entry).await.unwrap();
    let index = meta.asset_index(&version).await.unwrap();
    let plan = meta.plan(&version, index.as_ref(), &Platform::new(Os::Windows, Arch::X86_64));
    assert_eq!(plan.files.len(), 3);

    let reports = Mutex::new(Vec::new());
    meta.install(&plan, |p| reports.lock().unwrap().push(p)).await.unwrap();

    let reports = reports.into_inner().unwrap();
    let last = reports.last().unwrap();
    assert_eq!((last.done_files, last.total_files), (3, 3));
    assert_eq!(last.done_bytes, last.total_bytes);

    let layout = meta.layout();
    assert_eq!(std::fs::read(layout.client_jar("1.0-test")).unwrap(), b"client jar");
    assert!(layout.libraries().join("com/mojang/brigadier/1.3.10/brigadier-1.3.10.jar").is_file());
    assert_eq!(plan.classpath.last().unwrap(), &layout.client_jar("1.0-test"));

    // A second install finds everything verified on disk.
    meta.install(&plan, |_| {}).await.unwrap();
}

#[tokio::test]
async fn manifest_falls_back_to_cache_when_offline() {
    let server = MockServer::start().await;
    fake_mojang(&server).await;
    let tmp = tempfile::tempdir().unwrap();

    let online = client(&server, tmp.path());
    assert_eq!(online.manifest().await.unwrap().latest.release, "1.0-test");

    server.reset().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(503)).mount(&server).await;

    let offline = client(&server, tmp.path());
    assert_eq!(offline.manifest().await.unwrap().latest.release, "1.0-test");
}

#[tokio::test]
async fn manifest_without_cache_reports_the_network_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(503)).mount(&server).await;
    let tmp = tempfile::tempdir().unwrap();

    let err = client(&server, tmp.path()).manifest().await.unwrap_err();
    assert!(err.to_string().contains("503"), "{err}");
}
