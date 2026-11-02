//! Minecraft version metadata for Glint.
//!
//! Reads Mojang's version manifest, version JSONs and asset indexes, evaluates
//! platform rules and turns a version into a list of files to download.

mod client;
pub mod library;
pub mod mojang;
mod plan;
pub mod rules;

pub use client::{Endpoints, MetaClient, Progress};
pub use plan::{InstallPlan, MetaLayout, PlannedFile, plan_install};
pub use rules::{Arch, Os, Platform};

use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Net(#[from] glint_net::Error),

    #[error("invalid JSON in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
