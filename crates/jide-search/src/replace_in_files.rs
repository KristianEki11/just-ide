//! Replace in files operations with atomic disk updates.

use crate::error::SearchResult;
use crate::in_buffer::{replace_all_in_doc, SearchQuery};
use crate::in_files::{search_in_files_streaming, FilesSearchQuery, SearchStreamEvent};
use crossbeam_channel::unbounded;
use jide_core::Document;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// A plan for replacing text in a single file.
#[derive(Debug, Clone)]
pub struct FileReplacePlan {
    /// Path to the file.
    pub path: PathBuf,
    /// Number of matches to replace.
    pub match_count: usize,
}

/// A preview of all replacements across files.
#[derive(Debug, Clone)]
pub struct ReplacePreview {
    /// Total number of files affected.
    pub file_count: usize,
    /// Total number of matches to be replaced.
    pub match_count: usize,
    /// Per-file replacement plans.
    pub files: Vec<FileReplacePlan>,
}

/// Generate a preview of replacements across files.
///
/// # Errors
/// Returns errors from file searching.
#[allow(clippy::needless_pass_by_value)]
pub fn preview_replace_in_files(
    query: FilesSearchQuery,
    cancel: Arc<AtomicBool>,
) -> SearchResult<ReplacePreview> {
    let (tx, rx) = unbounded();

    // Run search in background thread
    let query_clone = query.clone();
    let cancel_clone = Arc::clone(&cancel);
    std::thread::spawn(move || {
        let _ = search_in_files_streaming(query_clone, cancel_clone, tx);
    });

    let mut files = Vec::new();
    let mut total_matches = 0;

    for event in rx {
        if let SearchStreamEvent::Results(results) = event {
            for result in results {
                let match_count: usize = result.matches.iter().map(|m| m.submatches.len()).sum();
                total_matches += match_count;
                files.push(FileReplacePlan {
                    path: result.path,
                    match_count,
                });
            }
        }
    }

    Ok(ReplacePreview {
        file_count: files.len(),
        match_count: total_matches,
        files,
    })
}

/// Result of executing replace in files.
#[derive(Debug, Clone)]
pub struct ReplaceResult {
    /// Files successfully modified on disk.
    pub modified_files: Vec<PathBuf>,
    /// Files that were skipped because they are open in memory.
    /// The UI should handle these separately.
    pub skipped_open_files: Vec<PathBuf>,
    /// Errors encountered during replacement.
    pub errors: Vec<(PathBuf, String)>,
}

/// Apply replacements across multiple files atomically.
///
/// Files that are already open in memory (specified in `open_buffers`) are skipped,
/// and their paths are returned in `skipped_open_files` so the UI can apply
/// replacements in-memory as single undoable transactions.
///
/// # Errors
/// Returns errors if preview generation fails.
pub fn apply_replace_in_files(
    query: FilesSearchQuery,
    replacement: &str,
    open_buffers: &[&Path],
    now_ms: u64,
) -> SearchResult<ReplaceResult> {
    let cancel = Arc::new(AtomicBool::new(false));
    let preview = preview_replace_in_files(query.clone(), cancel)?;

    let open_set: HashSet<&Path> = open_buffers.iter().copied().collect();

    let mut modified_files = Vec::new();
    let mut skipped_open_files = Vec::new();
    let mut errors = Vec::new();

    let in_buffer_query = SearchQuery {
        pattern: query.pattern,
        case_sensitive: query.case_sensitive,
        whole_word: query.whole_word,
        is_regex: query.is_regex,
        in_selection: None,
    };

    for plan in preview.files {
        // Skip files that are open in memory
        if open_set.contains(plan.path.as_path()) {
            skipped_open_files.push(plan.path);
            continue;
        }

        // Apply replacement atomically to file on disk
        match apply_replace_to_file(&plan.path, &in_buffer_query, replacement, now_ms) {
            Ok(()) => modified_files.push(plan.path),
            Err(e) => errors.push((plan.path, e.to_string())),
        }
    }

    Ok(ReplaceResult {
        modified_files,
        skipped_open_files,
        errors,
    })
}

/// Helper function to atomically replace text in a single file on disk.
fn apply_replace_to_file(
    path: &Path,
    query: &SearchQuery,
    replacement: &str,
    now_ms: u64,
) -> SearchResult<()> {
    let mut doc = Document::open(jide_core::DocId(1), path)?;
    replace_all_in_doc(&mut doc, query, replacement, now_ms)?;
    doc.save_to(path, jide_core::SaveFault::None)?;
    Ok(())
}
