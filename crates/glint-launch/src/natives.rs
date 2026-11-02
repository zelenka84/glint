//! Extracting legacy (pre-1.19) native archives before launch.

use std::path::{Path, PathBuf};

use crate::{Error, Result, io};

/// Clears `dest` and extracts every archive into it, skipping excluded prefixes
/// (usually `META-INF/`). Modern versions have no archives; `dest` is still created
/// because their arguments point `java.library.path` at it.
pub async fn extract(archives: &[(PathBuf, Vec<String>)], dest: &Path) -> Result<()> {
    let archives = archives.to_vec();
    let dest = dest.to_owned();
    tokio::task::spawn_blocking(move || extract_blocking(&archives, &dest))
        .await
        .expect("native extraction task panicked")
}

fn extract_blocking(archives: &[(PathBuf, Vec<String>)], dest: &Path) -> Result<()> {
    if dest.exists() {
        std::fs::remove_dir_all(dest).map_err(|e| io(dest, e))?;
    }
    std::fs::create_dir_all(dest).map_err(|e| io(dest, e))?;

    for (archive_path, exclude) in archives {
        let zip_err = |source| Error::Zip { path: archive_path.clone(), source };
        let file = std::fs::File::open(archive_path).map_err(|e| io(archive_path, e))?;
        let mut archive = zip::ZipArchive::new(file).map_err(zip_err)?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(zip_err)?;
            let Some(rel) = entry.enclosed_name() else {
                continue;
            };
            let name = rel.to_string_lossy().replace('\\', "/");
            if entry.is_dir() || exclude.iter().any(|prefix| name.starts_with(prefix.as_str())) {
                continue;
            }
            let out = dest.join(&rel);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
            }
            let mut file = std::fs::File::create(&out).map_err(|e| io(&out, e))?;
            std::io::copy(&mut entry, &mut file).map_err(|e| io(&out, e))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(path: &Path, files: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, body) in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(body).unwrap();
        }
        zip.finish().unwrap();
    }

    #[tokio::test]
    async fn extracts_and_skips_excluded_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let archive = tmp.path().join("natives.jar");
        make_zip(&archive, &[("lwjgl64.dll", b"dll"), ("META-INF/MANIFEST.MF", b"m"), ("../escape.dll", b"x")]);
        let dest = tmp.path().join("natives");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("stale.dll"), b"old").unwrap();

        extract(&[(archive, vec!["META-INF/".into()])], &dest).await.unwrap();

        assert_eq!(std::fs::read(dest.join("lwjgl64.dll")).unwrap(), b"dll");
        assert!(!dest.join("META-INF").exists());
        assert!(!dest.join("stale.dll").exists());
        assert!(!tmp.path().join("escape.dll").exists());
    }
}
