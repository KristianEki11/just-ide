#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

//! Core text buffer, selection, edit history, and file I/O primitives for Just IDE.
//!
//! `jide-core` intentionally has zero UI dependencies.

pub mod bracket;
pub mod document;
pub mod edit;
pub mod error;
pub mod format;
pub mod grapheme;
pub mod history;
pub mod io;
pub mod lines;
pub mod movement;
pub mod multicursor;
pub mod power_edit;
pub mod transform;
pub mod types;

pub use bracket::{NavEntry, NavigationHistory};
pub use document::Document;
pub use error::CoreError;
pub use format::format_code;
pub use history::History;
pub use io::SaveFault;
pub use movement::MoveOp;
pub use types::{Change, DocId, EditKind, Eol, LanguageId, Range, Selection, Transaction};

/// Smoke-checks core buffer primitives for `--self-check`.
#[must_use]
pub fn self_check_buffer() -> bool {
    let mut doc = Document::from_text(DocId(1), "hello\nworld\n");
    doc.selection = Selection::point(5);
    if doc.insert_text(" just", 1000).is_err() {
        return false;
    }
    if doc.rope != "hello just\nworld\n" {
        return false;
    }
    if !matches!(doc.undo(), Ok(true)) || doc.rope != "hello\nworld\n" {
        return false;
    }
    if !matches!(doc.redo(), Ok(true)) || doc.rope != "hello just\nworld\n" {
        return false;
    }
    true
}
