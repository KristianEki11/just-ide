//! Find in files operations with streaming results.

use crate::error::{SearchError, SearchResult};
use crossbeam_channel::Sender;
use grep_matcher::Matcher;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A search query for find-in-files operations.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct FilesSearchQuery {
    /// Root directory to search.
    pub root: PathBuf,
    /// The pattern to search for (literal or regex).
    pub pattern: String,
    /// Whether the search is case-sensitive.
    pub case_sensitive: bool,
    /// Whether to match whole words only.
    pub whole_word: bool,
    /// Whether the pattern is a regular expression.
    pub is_regex: bool,
    /// Include glob patterns (empty = include all).
    pub include_globs: Vec<String>,
    /// Exclude glob patterns.
    pub exclude_globs: Vec<String>,
    /// Whether to respect `.gitignore` files.
    pub use_gitignore: bool,
    /// Maximum file size to search (in bytes).
    pub max_file_size_bytes: u64,
}

impl Default for FilesSearchQuery {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            pattern: String::new(),
            case_sensitive: true,
            whole_word: false,
            is_regex: false,
            include_globs: Vec::new(),
            exclude_globs: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                "dist".to_string(),
                "build".to_string(),
                ".next".to_string(),
                "vendor".to_string(),
                "__pycache__".to_string(),
            ],
            use_gitignore: true,
            max_file_size_bytes: 20 * 1024 * 1024, // 20 MB
        }
    }
}

/// A line in a file with matches.
#[derive(Debug, Clone)]
pub struct FileMatchLine {
    /// Line number (1-indexed).
    pub line_number: usize,
    /// The text of the line.
    pub line_text: String,
    /// Byte offsets of submatches within the line (start, end).
    pub submatches: Vec<(usize, usize)>,
}

/// A file with search results.
#[derive(Debug, Clone)]
pub struct FileSearchResult {
    /// Path to the file.
    pub path: PathBuf,
    /// Matching lines.
    pub matches: Vec<FileMatchLine>,
}

/// Events streamed during file search.
#[derive(Debug, Clone)]
pub enum SearchStreamEvent {
    /// A batch of results (up to 200 matches).
    Results(Vec<FileSearchResult>),
    /// Search completed with total counts.
    Complete {
        file_count: usize,
        match_count: usize,
    },
    /// An error occurred for a specific file.
    FileError { path: PathBuf, error: String },
}

/// Search in files with streaming results.
///
/// Results are batched (up to 200 matches) and sent via `tx`.
/// The function checks `cancel` frequently and returns early if cancelled.
///
/// # Errors
/// Returns `SearchError::Cancelled` if cancelled, or `SearchError::InvalidRegex`
/// if the pattern is invalid.
#[allow(clippy::needless_pass_by_value)]
pub fn search_in_files_streaming(
    query: FilesSearchQuery,
    cancel: Arc<AtomicBool>,
    tx: Sender<SearchStreamEvent>,
) -> SearchResult<()> {
    const BATCH_SIZE: usize = 200;
    // Build the regex pattern
    let mut pattern = query.pattern.clone();

    if !query.is_regex {
        pattern = regex::escape(&pattern);
    }

    if query.whole_word {
        pattern = format!(r"\b{pattern}\b");
    }

    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(!query.case_sensitive)
        .multi_line(true)
        .unicode(true)
        .build(&pattern)
        .map_err(|e| SearchError::InvalidRegex(e.to_string()))?;

    // Build the file walker with include/exclude overrides
    let mut walker = WalkBuilder::new(&query.root);
    walker
        .git_ignore(query.use_gitignore)
        .hidden(false)
        .max_filesize(Some(query.max_file_size_bytes));

    if !query.include_globs.is_empty() || !query.exclude_globs.is_empty() {
        let mut overrides = OverrideBuilder::new(&query.root);
        for inc in &query.include_globs {
            let trimmed = inc.trim();
            if !trimmed.is_empty() {
                overrides
                    .add(trimmed)
                    .map_err(|e| SearchError::InvalidRegex(e.to_string()))?;
            }
        }
        for exc in &query.exclude_globs {
            let trimmed = exc.trim();
            if trimmed.is_empty() {
                continue;
            }
            let negated = if let Some(stripped) = trimmed.strip_prefix('!') {
                stripped.to_string()
            } else {
                format!("!{trimmed}")
            };
            overrides
                .add(&negated)
                .map_err(|e| SearchError::InvalidRegex(e.to_string()))?;
            if !trimmed.contains(['/', '\\', '*']) {
                overrides
                    .add(&format!("!{trimmed}/**"))
                    .map_err(|e| SearchError::InvalidRegex(e.to_string()))?;
            }
        }
        let built = overrides
            .build()
            .map_err(|e| SearchError::InvalidRegex(e.to_string()))?;
        walker.overrides(built);
    }

    // Build the searcher
    let searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .line_number(true)
        .build();

    let (result_tx, result_rx) = crossbeam_channel::unbounded();
    let cancel_clone = Arc::clone(&cancel);

    // Spawn parallel walker
    std::thread::spawn(move || {
        walker.build_parallel().run(|| {
            let matcher = matcher.clone();
            let mut searcher = searcher.clone();
            let result_tx = result_tx.clone();
            let cancel = Arc::clone(&cancel_clone);

            Box::new(move |entry_result| {
                if cancel.load(Ordering::Relaxed) {
                    return ignore::WalkState::Quit;
                }

                let Ok(entry) = entry_result else {
                    return ignore::WalkState::Continue;
                };

                if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                    return ignore::WalkState::Continue;
                }

                let path = entry.path();
                let mut file_matches = Vec::new();

                let search_result = searcher.search_path(
                    &matcher,
                    path,
                    grep_searcher::sinks::UTF8(|line_num, line_text| {
                        if cancel.load(Ordering::Relaxed) {
                            return Ok(false);
                        }

                        let mut submatches = Vec::new();
                        let _ = matcher.find_iter(line_text.as_bytes(), |m| {
                            submatches.push((m.start(), m.end()));
                            true
                        });

                        if !submatches.is_empty() {
                            #[allow(clippy::cast_possible_truncation)]
                            file_matches.push(FileMatchLine {
                                line_number: line_num as usize,
                                line_text: line_text.to_string(),
                                submatches,
                            });
                        }

                        Ok(true)
                    }),
                );

                if let Err(e) = search_result {
                    let _ = result_tx.send(SearchStreamEvent::FileError {
                        path: path.to_path_buf(),
                        error: e.to_string(),
                    });
                }

                if !file_matches.is_empty() {
                    let _ = result_tx.send(SearchStreamEvent::Results(vec![FileSearchResult {
                        path: path.to_path_buf(),
                        matches: file_matches,
                    }]));
                }

                if cancel.load(Ordering::Relaxed) {
                    ignore::WalkState::Quit
                } else {
                    ignore::WalkState::Continue
                }
            })
        });
    });

    // Collect and batch results (<= 200 matches per batch for fast first-batch streaming)
    let mut batch = Vec::new();
    let mut batch_matches = 0usize;
    let mut total_files = 0usize;
    let mut total_matches = 0usize;

    for event in result_rx {
        if cancel.load(Ordering::Relaxed) {
            return Err(SearchError::Cancelled);
        }

        match event {
            SearchStreamEvent::Results(mut results) => {
                for result in &results {
                    total_files += 1;
                    total_matches += result.matches.len();
                    batch_matches += result.matches.len();
                }
                batch.append(&mut results);

                if batch_matches >= BATCH_SIZE {
                    tx.send(SearchStreamEvent::Results(std::mem::take(&mut batch)))
                        .map_err(|_| SearchError::Cancelled)?;
                    batch_matches = 0;
                }
            }
            SearchStreamEvent::FileError { path, error } => {
                tx.send(SearchStreamEvent::FileError { path, error })
                    .map_err(|_| SearchError::Cancelled)?;
            }
            SearchStreamEvent::Complete { .. } => {}
        }
    }

    // Send remaining batch
    if !batch.is_empty() {
        tx.send(SearchStreamEvent::Results(batch))
            .map_err(|_| SearchError::Cancelled)?;
    }

    // Send completion
    tx.send(SearchStreamEvent::Complete {
        file_count: total_files,
        match_count: total_matches,
    })
    .map_err(|_| SearchError::Cancelled)?;

    if cancel.load(Ordering::Relaxed) {
        Err(SearchError::Cancelled)
    } else {
        Ok(())
    }
}
