//! Error types for search operations.

use thiserror::Error;

/// Errors that can occur during search operations.
#[derive(Debug, Error)]
pub enum SearchError {
    /// Invalid regular expression pattern.
    #[error("Invalid regex pattern: {0}")]
    InvalidRegex(String),

    /// I/O error during file operations.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Operation was cancelled.
    #[error("Search operation was cancelled")]
    Cancelled,

    /// Core library error.
    #[error("Core error: {0}")]
    Core(#[from] jide_core::CoreError),
}

/// Result type for search operations.
pub type SearchResult<T> = Result<T, SearchError>;
