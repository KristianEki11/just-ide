//! Error types for `jide-core`.

use std::io;
use thiserror::Error;

/// Core errors emitted by text buffer, history, and file I/O operations.
#[derive(Debug, Error)]
pub enum CoreError {
    /// Standard I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Character encoding or decoding failure.
    #[error("Encoding error: {0}")]
    Encoding(String),

    /// Document is marked read-only and cannot be modified.
    #[error("Document is read-only")]
    ReadOnly,

    /// Provided range is invalid for the buffer.
    #[error("Invalid range")]
    InvalidRange,

    /// Simulated fault injection for testing atomic save resilience.
    #[error("Simulated save fault")]
    SimulatedFault,
}
