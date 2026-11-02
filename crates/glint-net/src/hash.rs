use std::path::Path;

use sha1::Digest;
use tokio::io::AsyncReadExt;

/// A hash a downloaded file must match. Mojang publishes SHA-1, Modrinth SHA-512,
/// authlib-injector SHA-256.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpectedHash {
    Sha1(String),
    Sha256(String),
    Sha512(String),
}

impl ExpectedHash {
    pub(crate) fn hex(&self) -> &str {
        match self {
            Self::Sha1(h) | Self::Sha256(h) | Self::Sha512(h) => h,
        }
    }

    pub(crate) fn hasher(&self) -> Hasher {
        match self {
            Self::Sha1(_) => Hasher::Sha1(sha1::Sha1::new()),
            Self::Sha256(_) => Hasher::Sha256(sha2::Sha256::new()),
            Self::Sha512(_) => Hasher::Sha512(sha2::Sha512::new()),
        }
    }
}

pub(crate) enum Hasher {
    Sha1(sha1::Sha1),
    Sha256(sha2::Sha256),
    Sha512(sha2::Sha512),
}

impl Hasher {
    pub(crate) fn update(&mut self, bytes: &[u8]) {
        match self {
            Self::Sha1(h) => h.update(bytes),
            Self::Sha256(h) => h.update(bytes),
            Self::Sha512(h) => h.update(bytes),
        }
    }

    pub(crate) fn finish_hex(self) -> String {
        match self {
            Self::Sha1(h) => hex::encode(h.finalize()),
            Self::Sha256(h) => hex::encode(h.finalize()),
            Self::Sha512(h) => hex::encode(h.finalize()),
        }
    }
}

/// True when `path` exists and its contents match `expected`.
pub async fn file_matches(path: &Path, expected: &ExpectedHash) -> bool {
    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return false;
    };
    let mut hasher = expected.hasher();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        match file.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => hasher.update(&buf[..n]),
            Err(_) => return false,
        }
    }
    hasher.finish_hex().eq_ignore_ascii_case(expected.hex())
}

#[cfg(test)]
mod tests {
    use super::*;

    // sha1, sha256 and sha512 of "hello"
    const HELLO_SHA1: &str = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d";
    const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
    const HELLO_SHA512: &str = "9b71d224bd62f3785d96d46ad3ea3d73319bfbc2890caadae2dff72519673ca72323c3d99ba5c11d7c7acc6e14b8c5da0c4663475c2e5c3adef46f73bcdec043";

    #[tokio::test]
    async fn matches_known_hashes() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("hello.txt");
        tokio::fs::write(&path, b"hello").await.unwrap();

        assert!(file_matches(&path, &ExpectedHash::Sha1(HELLO_SHA1.into())).await);
        assert!(file_matches(&path, &ExpectedHash::Sha1(HELLO_SHA1.to_uppercase())).await);
        assert!(file_matches(&path, &ExpectedHash::Sha512(HELLO_SHA512.into())).await);
        assert!(file_matches(&path, &ExpectedHash::Sha256(HELLO_SHA256.into())).await);
        assert!(!file_matches(&path, &ExpectedHash::Sha1("00".repeat(20))).await);
    }

    #[tokio::test]
    async fn missing_file_does_not_match() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            !file_matches(
                &tmp.path().join("nope"),
                &ExpectedHash::Sha1(HELLO_SHA1.into())
            )
            .await
        );
    }
}
