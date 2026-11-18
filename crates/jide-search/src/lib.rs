//! `jide-search` provides fast in-buffer search and replace,
//! cancellable streaming find in files, and atomic replace in files.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

pub mod error;
pub mod in_buffer;
pub mod in_files;
pub mod replace_in_files;

pub use error::{SearchError, SearchResult};
pub use in_buffer::{
    find_in_rope, replace_all_in_doc, replace_one_in_doc, BufferMatch, SearchQuery,
};
pub use in_files::{
    search_in_files_streaming, FileMatchLine, FileSearchResult, FilesSearchQuery, SearchStreamEvent,
};
pub use replace_in_files::{
    apply_replace_in_files, preview_replace_in_files, FileReplacePlan, ReplacePreview,
    ReplaceResult,
};

/// Smoke-checks the search engine for `--self-check`.
#[must_use]
pub fn self_check_search() -> bool {
    let rope = ropey::Rope::from_str("hello world\nhello rust");
    let query = SearchQuery {
        pattern: "rust".to_string(),
        case_sensitive: true,
        whole_word: true,
        is_regex: false,
        in_selection: None,
    };
    matches!(find_in_rope(&rope, &query, None), Ok(found) if found.len() == 1)
}
