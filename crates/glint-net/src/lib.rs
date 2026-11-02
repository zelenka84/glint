//! Networking for Glint: one shared HTTP client and verified, atomic file downloads.

mod download;
mod hash;

pub use download::{DownloadOutcome, Downloader};
pub use hash::{ExpectedHash, file_matches};

use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("request to {url} failed: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("{url} returned HTTP {status}")]
    Status { url: String, status: u16 },

    #[error("hash mismatch for {url}: expected {expected}, got {actual}")]
    HashMismatch { url: String, expected: String, actual: String },

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// User agent sent with every request. Modrinth asks clients to identify themselves.
pub const USER_AGENT: &str = concat!("glint-launcher/", env!("CARGO_PKG_VERSION"));

/// Builds the HTTP client shared by the whole launcher.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("static reqwest configuration is valid")
}
