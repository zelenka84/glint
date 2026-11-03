//! Unpacking a build archive into its own folder.

use std::fs::File;
use std::path::{Path, PathBuf};

use crate::build::exe;
use crate::{Error, Result, io};

/// Unpacks `archive` (`.zip` or `.tar.gz`) so that `dest` holds `horizon-client`
/// and the files beside it. Unpacks into a scratch folder first, so a failure
/// never leaves half a build at `dest`.
pub(crate) async fn unpack_build(archive: &Path, dest: &Path) -> Result<()> {
    let parent = dest.parent().expect("builds live in a folder");
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| io(parent, e))?;
    let staging = parent.join(format!(
        ".{}.part-{}",
        dest.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    let _ = tokio::fs::remove_dir_all(&staging).await;

    let (from, to) = (archive.to_owned(), staging.clone());
    let unpacked = tokio::task::spawn_blocking(move || unpack(&from, &to))
        .await
        .expect("unpack task does not panic");
    let result = match unpacked {
        Ok(()) => match build_root(&staging) {
            Some(root) => match tokio::fs::rename(&root, dest).await {
                Ok(()) => Ok(()),
                // Another launch finished the same build first.
                Err(_) if dest.join(exe("horizon-client")).is_file() => Ok(()),
                Err(e) => Err(io(dest, e)),
            },
            None => Err(Error::BadArchive(archive.to_owned())),
        },
        Err(e) => Err(e),
    };
    let _ = tokio::fs::remove_dir_all(&staging).await;
    result
}

fn unpack(archive: &Path, dest: &Path) -> Result<()> {
    let fail = |message: String| Error::Unpack {
        path: archive.to_owned(),
        message,
    };
    let file = File::open(archive).map_err(|e| io(archive, e))?;
    let name = archive.to_string_lossy();
    if name.ends_with(".tar.gz") {
        tar::Archive::new(flate2::read::GzDecoder::new(file))
            .unpack(dest)
            .map_err(|e| fail(e.to_string()))
    } else {
        // `extract` refuses paths that leave `dest` and keeps Unix permissions.
        zip::ZipArchive::new(file)
            .and_then(|mut zip| zip.extract(dest))
            .map_err(|e| fail(e.to_string()))
    }
}

/// The folder holding `horizon-client`: the unpacked folder or its only subfolder.
fn build_root(dir: &Path) -> Option<PathBuf> {
    let client = exe("horizon-client");
    if dir.join(&client).is_file() {
        return Some(dir.to_owned());
    }
    let mut folders = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir());
    let only = folders.next()?.path();
    (folders.next().is_none() && only.join(&client).is_file()).then_some(only)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_client_at_the_top_or_one_folder_down() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("horizon");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(build_root(tmp.path()), None);
        std::fs::write(nested.join(exe("horizon-client")), b"").unwrap();
        assert_eq!(build_root(tmp.path()), Some(nested.clone()));
        std::fs::write(tmp.path().join(exe("horizon-client")), b"").unwrap();
        assert_eq!(build_root(tmp.path()), Some(tmp.path().to_owned()));
    }
}
