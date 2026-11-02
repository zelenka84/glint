//! Installing a mod with a required dependency against a fake Modrinth.

use glint_core::LoaderKind;
use glint_modrinth::{Lock, ModManager, ModrinthApi, Target};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn mount(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

fn version(id: &str, file: &str, base: &str, deps: serde_json::Value) -> serde_json::Value {
    let kind = if id.starts_with("beta") { "beta" } else { "release" };
    json!({
        "id": id, "name": id, "version_number": format!("{id}-1.0"), "version_type": kind,
        "files": [{"url": format!("{base}/files/{file}"), "filename": file, "primary": true, "hashes": {}}],
        "dependencies": deps
    })
}

#[tokio::test]
async fn installs_with_dependencies_toggles_and_removes() {
    let server = MockServer::start().await;
    let base = server.uri();
    mount(&server, "/project/sodium/version", json!([
        version("beta1", "sodium-beta.jar", &base, json!([])),
        version("rel1", "sodium.jar", &base, json!([{"project_id": "api", "version_id": null, "dependency_type": "required"},
                                                   {"project_id": "extras", "version_id": null, "dependency_type": "optional"}])),
    ]))
    .await;
    mount(&server, "/project/api/version", json!([version("api1", "fabric-api.jar", &base, json!([]))])).await;
    mount(&server, "/project/sodium", json!({"title": "Sodium"})).await;
    mount(&server, "/project/api", json!({"title": "Fabric API"})).await;
    for file in ["sodium.jar", "fabric-api.jar"] {
        Mock::given(method("GET"))
            .and(path(format!("/files/{file}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(file.as_bytes()))
            .mount(&server)
            .await;
    }

    let tmp = tempfile::tempdir().unwrap();
    let api = ModrinthApi::with_base(glint_net::http_client(), &base);
    let manager = ModManager::new(&api, glint_net::http_client(), tmp.path());
    let target = Target { game_version: "1.21.4".into(), loader: LoaderKind::Fabric };

    // The release wins over the newer beta; the required dependency comes along, the optional one does not.
    let installed = manager.install("sodium", &target).await.unwrap();
    assert_eq!(installed, ["Sodium", "Fabric API"]);
    assert!(tmp.path().join("game/mods/sodium.jar").is_file());
    assert!(tmp.path().join("game/mods/fabric-api.jar").is_file());

    let lock = Lock::load(&tmp.path().join("glint.lock")).unwrap();
    assert_eq!(lock.mods.len(), 2);
    assert!(lock.find_project("api").unwrap().dependency);

    // Installing again changes nothing.
    assert!(manager.install("sodium", &target).await.unwrap().is_empty());

    std::fs::write(tmp.path().join("game/mods/handmade.jar"), b"x").unwrap();
    let list = manager.list().unwrap();
    let titles: Vec<_> = list.iter().map(|m| (m.title.as_str(), m.managed)).collect();
    assert_eq!(titles, [("handmade", false), ("Sodium", true), ("Fabric API", true)]);

    manager.set_enabled("sodium.jar", false).await.unwrap();
    assert!(tmp.path().join("game/mods/sodium.jar.disabled").is_file());
    assert!(!manager.list().unwrap().iter().find(|m| m.filename == "sodium.jar").unwrap().enabled);

    manager.remove("sodium.jar").await.unwrap();
    assert!(!tmp.path().join("game/mods/sodium.jar.disabled").exists());
    assert!(Lock::load(&tmp.path().join("glint.lock")).unwrap().find_project("sodium").is_none());
}
