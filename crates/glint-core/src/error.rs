use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not find the user data directory")]
    NoDataDir,

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid instance manifest: {0}")]
    ManifestParse(#[from] toml::de::Error),

    #[error("could not serialize instance manifest: {0}")]
    ManifestWrite(#[from] toml::ser::Error),

    #[error("invalid instance name {0:?}")]
    InvalidInstanceName(String),
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
