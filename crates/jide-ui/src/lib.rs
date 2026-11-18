#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

//! UI components, theme system, font registration, and layout shell for Just IDE.

pub mod commands;
pub mod editor_view;
pub mod file_icons;
pub mod find_widget;
pub mod fonts;
pub mod palette;
pub mod search_panel;
pub mod settings_view;
pub mod shell;
pub mod sidebar_tree;
pub mod state;
pub mod tabs;
pub mod theme;

/// Returns the default window width and height in logical pixels (`04-UX-SPEC.md` §11).
#[must_use]
pub const fn default_window_size() -> [f32; 2] {
    [1280.0, 800.0]
}
