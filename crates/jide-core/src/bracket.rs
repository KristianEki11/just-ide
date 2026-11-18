//! Bracket matching and navigation history primitives.

use crate::document::Document;

// ---------------------------------------------------------------------------
// NavigationHistory
// ---------------------------------------------------------------------------

/// Entry in the navigation history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavEntry {
    pub doc_id: crate::types::DocId,
    pub char_idx: usize,
}

/// Cursor-position navigation history (Alt+Left / Alt+Right).
#[derive(Debug, Clone, Default)]
pub struct NavigationHistory {
    entries: Vec<NavEntry>,
    cursor: usize,
}

impl NavigationHistory {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a navigation location, truncating any forward history.
    pub fn record(&mut self, doc_id: crate::types::DocId, char_idx: usize) {
        self.entries.truncate(self.cursor);
        // Avoid duplicate consecutive entries.
        if self
            .entries
            .last()
            .is_some_and(|e| e.doc_id == doc_id && e.char_idx == char_idx)
        {
            return;
        }
        self.entries.push(NavEntry { doc_id, char_idx });
        self.cursor = self.entries.len();
    }

    /// Returns the previous entry (Alt+Left) and moves the internal cursor back.
    pub fn back(&mut self) -> Option<NavEntry> {
        if self.cursor == 0 {
            return None;
        }
        self.cursor -= 1;
        self.entries.get(self.cursor).copied()
    }

    /// Returns the next entry (Alt+Right) and moves the internal cursor forward.
    pub fn forward(&mut self) -> Option<NavEntry> {
        if self.cursor + 1 >= self.entries.len() {
            return None;
        }
        self.cursor += 1;
        self.entries.get(self.cursor).copied()
    }
}

// ---------------------------------------------------------------------------
// Document bracket matching
// ---------------------------------------------------------------------------

impl Document {
    /// Returns `(open_char_idx, close_char_idx)` for the bracket at `char_idx`.
    /// Scans at most 100 000 characters in each direction.
    #[must_use]
    pub fn find_matching_bracket(&self, char_idx: usize) -> Option<(usize, usize)> {
        const LIMIT: usize = 100_000;
        let total = self.rope.len_chars();
        if char_idx >= total {
            return None;
        }
        let ch = self.rope.char(char_idx);

        if let Some(close) = bracket_match_close(ch) {
            // scanning forward for close
            let mut depth = 1usize;
            let scan_end = (char_idx + 1 + LIMIT).min(total);
            for i in (char_idx + 1)..scan_end {
                let c = self.rope.char(i);
                if c == ch {
                    depth += 1;
                } else if c == close {
                    depth -= 1;
                    if depth == 0 {
                        return Some((char_idx, i));
                    }
                }
            }
            return None;
        }

        if let Some(open) = bracket_match_open(ch) {
            // scanning backward for open
            let mut depth = 1usize;
            let scan_start = char_idx.saturating_sub(LIMIT);
            let mut i = char_idx;
            while i > scan_start {
                i -= 1;
                let c = self.rope.char(i);
                if c == ch {
                    depth += 1;
                } else if c == open {
                    depth -= 1;
                    if depth == 0 {
                        return Some((i, char_idx));
                    }
                }
            }
            return None;
        }

        None
    }
}

fn bracket_match_close(open: char) -> Option<char> {
    match open {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        _ => None,
    }
}

fn bracket_match_open(close: char) -> Option<char> {
    match close {
        ')' => Some('('),
        ']' => Some('['),
        '}' => Some('{'),
        _ => None,
    }
}
