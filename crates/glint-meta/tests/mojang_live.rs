//! Checks that real Mojang metadata parses and plans. Needs the internet, so it is
//! ignored by default: `cargo test -p glint-meta -- --ignored`.

use glint_meta::{Endpoints, MetaClient, MetaLayout, Platform};

#[tokio::test]
#[ignore = "talks to Mojang servers"]
async fn latest_release_and_a_legacy_version_plan_on_this_platform() {
    let tmp = tempfile::tempdir().unwrap();
    let meta = MetaClient::new(
        glint_net::http_client(),
        MetaLayout::new(tmp.path()),
        Endpoints::default(),
    );
    let manifest = meta.manifest().await.unwrap();
    assert!(manifest.versions.len() > 500);

    for id in [manifest.latest.release.as_str(), "1.12.2", "1.8.9"] {
        let entry = manifest
            .find(id)
            .unwrap_or_else(|| panic!("{id} missing from manifest"));
        let version = meta.version(entry).await.unwrap();
        let index = meta
            .asset_index(&version)
            .await
            .unwrap()
            .expect("vanilla versions have an asset index");
        let plan = meta.plan(&version, Some(&index), &Platform::current());

        assert!(
            plan.classpath.len() > 10,
            "{id}: only {} classpath entries",
            plan.classpath.len()
        );
        // Old versions ship far fewer assets (1.8.9 has ~750 files in total).
        assert!(
            plan.files.len() > 500,
            "{id}: only {} files",
            plan.files.len()
        );
        println!(
            "{id}: {} files, {:.0} MB, {} classpath entries, {} legacy natives",
            plan.files.len(),
            plan.total_bytes() as f64 / 1e6,
            plan.classpath.len(),
            plan.natives.len()
        );
    }
}
