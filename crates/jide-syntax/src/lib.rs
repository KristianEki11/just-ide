#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

//! Syntax highlighting registry and incremental parse cache for Just IDE.

pub mod highlighter;
pub mod registry;
pub mod scope_map;

pub use highlighter::{
    load_syntax_set, HighlightCache, HighlightSpan, SyntaxSet, CHECKPOINT_INTERVAL,
};
pub use registry::{all_p0_languages, detect_language, find_language_by_id, LanguageSpec};
pub use scope_map::{map_scope_stack, SyntaxToken};

/// Number of P0 languages supported by Just IDE.
#[must_use]
pub const fn planned_p0_language_count() -> usize {
    33
}
