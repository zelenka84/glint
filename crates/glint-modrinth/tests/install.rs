//! Installing a mod with a required dependency against a fake Modrinth.

use glint_core::LoaderKind;
use glint_modrinth::{ContentKind, Lock, ModManager, ModrinthApi, Target};
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
    let kind = if id.starts_with("beta") {
        "beta"
    } else {
        "release"
    };
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
    mount(
        &server,
        "/project/api/version",
        json!([version("api1", "fabric-api.jar", &base, json!([]))]),
    )
    .await;
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
    let manager = ModManager::new(&api, glint_net::http_client(), tmp.path(), ContentKind::Mod);
    let target = Target {
        game_version: "1.21.4".into(),
        loader: Some(LoaderKind::Fabric),
    };

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
    assert_eq!(
        titles,
        [("handmade", false), ("Sodium", true), ("Fabric API", true)]
    );

    manager.set_enabled("sodium.jar", false).await.unwrap();
    assert!(tmp.path().join("game/mods/sodium.jar.disabled").is_file());
    assert!(
        !manager
            .list()
            .unwrap()
            .iter()
            .find(|m| m.filename == "sodium.jar")
            .unwrap()
            .enabled
    );

    manager.remove("sodium.jar").await.unwrap();
    assert!(!tmp.path().join("game/mods/sodium.jar.disabled").exists());
    assert!(
        Lock::load(&tmp.path().join("glint.lock"))
            .unwrap()
            .find_project("sodium")
            .is_none()
    );
}

#[tokio::test]
async fn shader_packs_bring_iris_and_updates_are_found() {
    let server = MockServer::start().await;
    let base = server.uri();
    let mut pack = version("pack1", "glow.zip", &base, json!([]));
    pack["files"][0]["hashes"] = json!({"sha512": GLOW_SHA512});
    mount(&server, "/project/glow/version", json!([pack])).await;
    let mut iris = version("iris1", "iris.jar", &base, json!([]));
    iris["project_id"] = json!("YL57xq9U");
    mount(&server, "/project/iris/version", json!([iris])).await;
    mount(
        &server,
        "/project/glow",
        json!({"title": "Glow Shaders", "icon_url": "https://cdn/glow.png"}),
    )
    .await;
    mount(&server, "/project/YL57xq9U", json!({"title": "Iris"})).await;
    for file in ["glow.zip", "iris.jar"] {
        Mock::given(method("GET"))
            .and(path(format!("/files/{file}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(file.as_bytes()))
            .mount(&server)
            .await;
    }
    Mock::given(method("POST"))
        .and(path("/version_files/update"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            GLOW_SHA512: {"id": "pack2", "name": "pack2", "version_number": "2.0", "files": []}
        })))
        .mount(&server)
        .await;

    let tmp = tempfile::tempdir().unwrap();
    let api = ModrinthApi::with_base(glint_net::http_client(), &base);
    let shaders = ModManager::new(
        &api,
        glint_net::http_client(),
        tmp.path(),
        ContentKind::Shader,
    );
    let target = Target {
        game_version: "1.21.4".into(),
        loader: Some(LoaderKind::Fabric),
    };

    assert_eq!(
        shaders.install("glow", &target).await.unwrap(),
        ["Glow Shaders", "Iris"]
    );
    assert!(tmp.path().join("game/shaderpacks/glow.zip").is_file());
    assert!(tmp.path().join("game/mods/iris.jar").is_file());

    let packs = shaders.list().unwrap();
    assert_eq!(packs.len(), 1);
    assert_eq!(packs[0].icon_url.as_deref(), Some("https://cdn/glow.png"));
    let mods = ModManager::new(&api, glint_net::http_client(), tmp.path(), ContentKind::Mod)
        .list()
        .unwrap();
    assert!(
        mods[0].dependency,
        "Iris counts as a dependency of the shader pack"
    );

    // A second shader pack leaves the existing Iris alone.
    assert_eq!(
        shaders.install("glow", &target).await.unwrap(),
        Vec::<String>::new()
    );

    let updates = shaders.check_updates(&target).await.unwrap();
    assert_eq!(updates.len(), 1);
    assert_eq!(
        (updates[0].current.as_str(), updates[0].latest.as_str()),
        ("pack1-1.0", "2.0")
    );
}

/// SHA-512 of the bytes `b"glow.zip"` the fake server returns.
const GLOW_SHA512: &str = "eafef05a2286e918178d34e010690d8fe3545faf6806b723dc059476d1002ce98dcf9082597841618d711bb7ad52a5ddf260c037047b1f51a611e7a81f81fc0e";

#[tokio::test]
async fn project_page_lists_versions_and_dependencies_and_installs_a_chosen_version() {
    let server = MockServer::start().await;
    let base = server.uri();
    mount(&server, "/project/sodium/version", json!([
        version("beta1", "sodium-beta.jar", &base, json!([])),
        version("rel1", "sodium.jar", &base, json!([{"project_id": "api", "version_id": null, "dependency_type": "required"},
                                                   {"project_id": "extras", "version_id": null, "dependency_type": "optional"},
                                                   {"project_id": "old", "version_id": null, "dependency_type": "incompatible"}])),
    ]))
    .await;
    mount(
        &server,
        "/project/sodium",
        json!({"id": "sodium", "title": "Sodium", "body": "# Fast", "downloads": 10}),
    )
    .await;
    mount(
        &server,
        "/project/api",
        json!({"id": "api", "title": "Fabric API"}),
    )
    .await;
    mount(
        &server,
        "/project/extras",
        json!({"id": "extras", "title": "Extras"}),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/files/sodium-beta.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"beta".as_slice()))
        .mount(&server)
        .await;

    let tmp = tempfile::tempdir().unwrap();
    let api = ModrinthApi::with_base(glint_net::http_client(), &base);
    let manager = ModManager::new(&api, glint_net::http_client(), tmp.path(), ContentKind::Mod);
    let target = Target {
        game_version: "1.21.4".into(),
        loader: Some(LoaderKind::Fabric),
    };

    let page = manager.project_page("sodium", &target).await.unwrap();
    assert_eq!(page.project.body, "# Fast");
    assert_eq!(
        page.versions
            .iter()
            .map(|v| v.id.as_str())
            .collect::<Vec<_>>(),
        ["beta1", "rel1"]
    );
    assert_eq!(page.recommended.as_deref(), Some("rel1"));
    assert_eq!(page.installed_version, None);
    let deps: Vec<_> = page
        .dependencies
        .iter()
        .map(|d| (d.title.as_str(), d.required, d.installed))
        .collect();
    assert_eq!(
        deps,
        [("Fabric API", true, false), ("Extras", false, false)]
    );

    // A chosen version installs instead of the recommended one; the beta has no dependencies.
    mount(
        &server,
        "/version/beta1",
        version("beta1", "sodium-beta.jar", &base, json!([])),
    )
    .await;
    let installed = manager
        .install_version("sodium", Some("beta1"), &target)
        .await
        .unwrap();
    assert_eq!(installed, ["Sodium"]);
    assert!(tmp.path().join("game/mods/sodium-beta.jar").is_file());
    let page = manager.project_page("sodium", &target).await.unwrap();
    assert_eq!(page.installed_version.as_deref(), Some("beta1"));
}
