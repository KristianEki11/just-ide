#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

//! Workspace file tree, settings, keybindings, and session persistence for Just IDE.

pub mod crash;
pub mod error;
pub mod fuzzy;
pub mod hot_exit;
pub mod keybindings;
pub mod paths;
pub mod session;
pub mod settings;
pub mod tree;
pub mod watcher;

pub use error::{Result, WorkspaceError};

/// Returns the canonical application config directory name.
#[must_use]
pub const fn app_dir_name() -> &'static str {
    "JustIDE"
}
