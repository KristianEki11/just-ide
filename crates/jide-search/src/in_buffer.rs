//! In-buffer search and replace operations.

use crate::error::{SearchError, SearchResult};
use jide_core::{Change, Document, EditKind, Range, Selection};
use regex::Regex;
use ropey::Rope;
use std::sync::atomic::{AtomicBool, Ordering};

/// A search query for in-buffer operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    /// The pattern to search for (literal or regex).
    pub pattern: String,
    /// Whether the search is case-sensitive.
    pub case_sensitive: bool,
    /// Whether to match whole words only.
    pub whole_word: bool,
    /// Whether the pattern is a regular expression.
    pub is_regex: bool,
    /// Optional selection range to search within.
    pub in_selection: Option<Range>,
}

/// A match found in a buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferMatch {
    /// Character index where the match starts.
    pub start_char: usize,
    /// Character index where the match ends.
    pub end_char: usize,
}

impl SearchQuery {
    /// Build a regex from this query.
    fn build_regex(&self) -> SearchResult<Regex> {
        let mut pattern = self.pattern.clone();

        if !self.is_regex {
            pattern = regex::escape(&pattern);
        }

        if self.whole_word {
            pattern = format!(r"\b{pattern}\b");
        }

        let pattern = if self.case_sensitive {
            pattern
        } else {
            format!("(?i){pattern}")
        };

        Regex::new(&pattern).map_err(|e| SearchError::InvalidRegex(e.to_string()))
    }
}

/// Find all matches in a rope.
///
/// # Errors
/// Returns `SearchError::InvalidRegex` if the pattern is invalid,
/// or `SearchError::Cancelled` if the operation was cancelled.
pub fn find_in_rope(
    rope: &Rope,
    query: &SearchQuery,
    cancel: Option<&AtomicBool>,
) -> SearchResult<Vec<BufferMatch>> {
    let regex = query.build_regex()?;
    let mut matches = Vec::new();

    // Determine search range
    let (start_char, end_char) = if let Some(sel) = &query.in_selection {
        (
            sel.start().min(rope.len_chars()),
            sel.end().min(rope.len_chars()),
        )
    } else {
        (0, rope.len_chars())
    };

    if start_char >= end_char && !query.is_regex {
        return Ok(matches);
    }

    let is_multiline = query.is_regex
        && (query.pattern.contains('\n')
            || query.pattern.contains("\\n")
            || query.pattern.contains("\\s")
            || query.pattern.contains("(?s"));

    if is_multiline {
        let text = rope.slice(start_char..end_char).to_string();
        let mut byte_cursor = 0usize;
        let mut char_cursor = 0usize;

        for mat in regex.find_iter(&text) {
            if let Some(cancel) = cancel {
                if cancel.load(Ordering::Relaxed) {
                    return Err(SearchError::Cancelled);
                }
            }

            char_cursor += text[byte_cursor..mat.start()].chars().count();
            let match_start = start_char + char_cursor;

            char_cursor += text[mat.start()..mat.end()].chars().count();
            byte_cursor = mat.end();
            let match_end = start_char + char_cursor;

            matches.push(BufferMatch {
                start_char: match_start,
                end_char: match_end,
            });
        }
    } else {
        let start_line = rope.char_to_line(start_char);
        let end_line = if rope.len_chars() == 0 {
            0
        } else {
            rope.char_to_line(end_char.min(rope.len_chars().saturating_sub(1)))
        };
        let mut line_buf = String::new();

        for line_idx in start_line..=end_line {
            if let Some(cancel) = cancel {
                if cancel.load(Ordering::Relaxed) {
                    return Err(SearchError::Cancelled);
                }
            }

            let line_start_char = rope.line_to_char(line_idx);
            let line_end_char = if line_idx + 1 < rope.len_lines() {
                rope.line_to_char(line_idx + 1)
            } else {
                rope.len_chars()
            };

            let search_start = line_start_char.max(start_char);
            let search_end = line_end_char.min(end_char);
            if search_start > search_end {
                continue;
            }

            line_buf.clear();
            for chunk in rope.slice(search_start..search_end).chunks() {
                line_buf.push_str(chunk);
            }

            collect_line_matches(&regex, &line_buf, search_start, &mut matches);
        }
    }

    Ok(matches)
}

fn collect_line_matches(
    regex: &Regex,
    line_text: &str,
    base_char: usize,
    out: &mut Vec<BufferMatch>,
) {
    if line_text.is_ascii() {
        for mat in regex.find_iter(line_text) {
            out.push(BufferMatch {
                start_char: base_char + mat.start(),
                end_char: base_char + mat.end(),
            });
        }
        return;
    }

    let mut byte_cursor = 0usize;
    let mut char_cursor = 0usize;
    for mat in regex.find_iter(line_text) {
        char_cursor += line_text[byte_cursor..mat.start()].chars().count();
        let match_start = base_char + char_cursor;

        char_cursor += line_text[mat.start()..mat.end()].chars().count();
        byte_cursor = mat.end();
        let match_end = base_char + char_cursor;

        out.push(BufferMatch {
            start_char: match_start,
            end_char: match_end,
        });
    }
}

/// Replace the first match in a document.
///
/// # Errors
/// Returns errors from search or document operations.
pub fn replace_one_in_doc(
    doc: &mut Document,
    query: &SearchQuery,
    replacement: &str,
    now_ms: u64,
) -> SearchResult<bool> {
    let matches = find_in_rope(&doc.rope, query, None)?;

    if let Some(first) = matches.first() {
        let actual_replacement = if query.is_regex {
            let regex = query.build_regex()?;
            let match_text = doc.rope.slice(first.start_char..first.end_char).to_string();
            regex.replace(&match_text, replacement).to_string()
        } else {
            replacement.to_string()
        };

        let change = Change {
            start: first.start_char,
            end: first.end_char,
            text: actual_replacement,
        };

        let new_selection = Selection::point(first.start_char);
        doc.apply_changes(vec![change], new_selection, EditKind::Structural, now_ms)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Replace all matches in a document as a single undoable transaction.
///
/// # Errors
/// Returns errors from search or document operations.
pub fn replace_all_in_doc(
    doc: &mut Document,
    query: &SearchQuery,
    replacement: &str,
    now_ms: u64,
) -> SearchResult<usize> {
    let matches = find_in_rope(&doc.rope, query, None)?;

    if matches.is_empty() {
        return Ok(0);
    }

    let regex = if query.is_regex {
        Some(query.build_regex()?)
    } else {
        None
    };

    let changes: Vec<Change> = matches
        .iter()
        .map(|m| {
            let actual_replacement = if let Some(ref regex) = regex {
                let match_text = doc.rope.slice(m.start_char..m.end_char).to_string();
                regex.replace(&match_text, replacement).to_string()
            } else {
                replacement.to_string()
            };

            Change {
                start: m.start_char,
                end: m.end_char,
                text: actual_replacement,
            }
        })
        .collect();

    let count = changes.len();
    let new_selection = matches
        .last()
        .map_or(Selection::point(0), |m| Selection::point(m.start_char));

    doc.apply_changes(changes, new_selection, EditKind::Structural, now_ms)?;

    Ok(count)
}
