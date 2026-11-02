//! Shared building blocks for Glint: on-disk layout, instance manifests and errors.
//!
//! This crate knows nothing about Tauri, networking or the game itself.

pub mod error;
pub mod instance;
pub mod paths;

pub use error::{Error, Result};
pub use instance::{Engine, InstanceManifest, JavaSettings, Loader, LoaderKind};
pub use paths::GlintPaths;
