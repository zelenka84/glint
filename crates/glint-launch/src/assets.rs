//! Asset layouts for very old versions.
//!
//! Modern versions read `assets/objects/` through the index. Versions up to 1.7.2
//! instead expect real files under `assets/virtual/<index>/` or `<game dir>/resources/`.

use std::path::{Path, PathBuf};

use glint_meta::MetaLayout;
use glint_meta::mojang::AssetIndex;

use crate::{Result, io};

/// Returns the folder to use as `${game_assets}`, copying objects there if needed.
pub async fn prepare_legacy(
    layout: &MetaLayout,
    index_id: &str,
    index: &AssetIndex,
    game_dir: &Path,
) -> Result<PathBuf> {
    let target = if index.map_to_resources {
        game_dir.join("resources")
    } else if index.is_virtual {
        layout.assets().join("virtual").join(index_id)
    } else {
        return Ok(layout.assets());
    };

    for (name, object) in &index.objects {
        let dest = target.join(name);
        if dest.is_file() {
            continue;
        }
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| io(parent, e))?;
        }
        let src = layout.asset_objects().join(object.object_path());
        tokio::fs::copy(&src, &dest)
            .await
            .map_err(|e| io(&src, e))?;
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn virtual_index_copies_objects_by_name() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = MetaLayout::new(tmp.path().join("meta"));
        let object = layout.asset_objects().join("ab/abcd");
        std::fs::create_dir_all(object.parent().unwrap()).unwrap();
        std::fs::write(&object, b"ogg").unwrap();
        let index: AssetIndex = serde_json::from_str(
            r#"{"virtual": true, "objects": {"sound/step.ogg": {"hash": "abcd", "size": 3}}}"#,
        )
        .unwrap();

        let dir = prepare_legacy(&layout, "legacy", &index, &tmp.path().join("game"))
            .await
            .unwrap();
        assert_eq!(dir, layout.assets().join("virtual/legacy"));
        assert_eq!(std::fs::read(dir.join("sound/step.ogg")).unwrap(), b"ogg");
    }

    #[tokio::test]
    async fn modern_index_uses_assets_root() {
        let layout = MetaLayout::new("/meta");
        let index: AssetIndex = serde_json::from_str(r#"{"objects": {}}"#).unwrap();
        let dir = prepare_legacy(&layout, "19", &index, Path::new("/game"))
            .await
            .unwrap();
        assert_eq!(dir, layout.assets());
    }
}
