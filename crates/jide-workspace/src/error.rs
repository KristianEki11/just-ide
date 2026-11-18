use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Trash error: {0}")]
    Trash(#[from] trash::Error),

    #[error("File watcher error: {0}")]
    Notify(#[from] notify::Error),

    #[error("Invalid path: {0}")]
    InvalidPath(PathBuf),
}

pub type Result<T> = std::result::Result<T, WorkspaceError>;
