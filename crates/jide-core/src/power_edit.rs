//! Auto-close pairs and comment toggling primitives.

use crate::document::Document;
use crate::error::CoreError;
use crate::types::{Change, EditKind, Range, Selection};

// ---------------------------------------------------------------------------
// Auto-close pair tables
// ---------------------------------------------------------------------------

const OPEN_PAIRS: &[(char, char)] = &[
    ('(', ')'),
    ('[', ']'),
    ('{', '}'),
    ('"', '"'),
    ('\'', '\''),
    ('`', '`'),
];

fn matching_close(open: char) -> Option<char> {
    OPEN_PAIRS.iter().find(|(o, _)| *o == open).map(|(_, c)| *c)
}

fn is_close_char(ch: char) -> bool {
    OPEN_PAIRS.iter().any(|(_, c)| *c == ch)
}

/// Character class: whitespace, closing bracket, or EOL.
fn next_char_favors_autoclose(rope: &ropey::Rope, pos: usize) -> bool {
    if pos >= rope.len_chars() {
        return true; // EOL / end-of-file
    }
    let ch = rope.char(pos);
    ch.is_whitespace() || is_close_char(ch) || ch == '\n' || ch == '\r'
}

// ---------------------------------------------------------------------------
// Document impl
// ---------------------------------------------------------------------------

impl Document {
    // ------------------------------------------------------------------
    // Auto-close pairs
    // ------------------------------------------------------------------

    /// Types `ch` with auto-closing / skip-over logic.
    pub fn insert_with_auto_close(&mut self, ch: char, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }

        // Skip-over: if ch is a closing char and the next char already matches, just move caret.
        if is_close_char(ch) {
            let all_skip = self.selection.ranges.iter().all(|r| {
                r.is_empty() && r.head < self.rope.len_chars() && self.rope.char(r.head) == ch
            });
            if all_skip {
                let new_ranges: Vec<Range> = self
                    .selection
                    .ranges
                    .iter()
                    .map(|r| Range::point(r.head + 1))
                    .collect();
                self.selection = Selection::new(new_ranges, self.selection.primary);
                return Ok(());
            }
        }

        if let Some(close) = matching_close(ch) {
            // Wrap non-empty selections in pair.
            let any_non_empty = self.selection.ranges.iter().any(|r| !r.is_empty());
            if any_non_empty {
                let mut changes = Vec::new();
                let mut new_ranges = Vec::new();
                let mut delta: isize = 0;
                for r in &self.selection.ranges {
                    if r.is_empty() {
                        continue;
                    }
                    let s = r.start();
                    let e = r.end();
                    let old_text: String = self.rope.slice(s..e).to_string();
                    let new_text = format!("{ch}{old_text}{close}");
                    #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
                    let ns = (s as isize + delta) as usize;
                    changes.push(Change {
                        start: s,
                        end: e,
                        text: new_text.clone(),
                    });
                    new_ranges.push(Range {
                        anchor: ns + 1,
                        head: ns + 1 + old_text.chars().count(),
                    });
                    #[allow(clippy::cast_possible_wrap)]
                    {
                        delta += new_text.chars().count() as isize - (e - s) as isize;
                    }
                }
                if !changes.is_empty() {
                    let sel = Selection::new(new_ranges, self.selection.primary);
                    self.apply_edit_batch(changes, sel, EditKind::InsertChar, now_ms);
                    return Ok(());
                }
            }

            // Auto-insert pair at empty carets where appropriate.
            let should_pair: Vec<bool> = self
                .selection
                .ranges
                .iter()
                .map(|r| r.is_empty() && next_char_favors_autoclose(&self.rope, r.head))
                .collect();

            if should_pair.iter().any(|&b| b) {
                let mut changes = Vec::new();
                let mut new_ranges = Vec::new();
                let mut delta: isize = 0;
                for (r, &pair) in self.selection.ranges.iter().zip(&should_pair) {
                    let text = if pair {
                        format!("{ch}{close}")
                    } else {
                        ch.to_string()
                    };
                    changes.push(Change {
                        start: r.start(),
                        end: r.end(),
                        text: text.clone(),
                    });
                    #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
                    let np = (r.start() as isize + delta) as usize + 1;
                    new_ranges.push(Range::point(np));
                    #[allow(clippy::cast_possible_wrap)]
                    {
                        delta += text.chars().count() as isize;
                    }
                }
                let sel = Selection::new(new_ranges, self.selection.primary);
                self.apply_edit_batch(changes, sel, EditKind::InsertChar, now_ms);
                return Ok(());
            }
        }

        // Plain insertion.
        self.insert_char(ch, now_ms)
    }

    /// Backspace with auto-remove of empty pairs.
    pub fn backspace_with_auto_close(&mut self, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        let is_empty_pair = |r: &Range| -> bool {
            if !r.is_empty() || r.head == 0 || r.head >= self.rope.len_chars() {
                return false;
            }
            let prev = self.rope.char(r.head - 1);
            let next = self.rope.char(r.head);
            matching_close(prev).is_some_and(|c| c == next)
        };

        let any_pair = self.selection.ranges.iter().any(is_empty_pair);
        if any_pair {
            let mut changes = Vec::new();
            let mut new_ranges = Vec::new();
            let mut delta: isize = 0;
            for r in &self.selection.ranges {
                let (start, end) = if is_empty_pair(r) {
                    (r.head - 1, r.head + 1)
                } else if r.is_empty() && r.head > 0 {
                    (r.head - 1, r.head)
                } else {
                    (r.start(), r.end())
                };
                changes.push(Change {
                    start,
                    end,
                    text: String::new(),
                });
                #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
                let np = (start as isize + delta).max(0) as usize;
                new_ranges.push(Range::point(np));
                #[allow(clippy::cast_possible_wrap)]
                {
                    delta -= (end - start) as isize;
                }
            }
            let sel = Selection::new(new_ranges, self.selection.primary);
            self.apply_edit_batch(changes, sel, EditKind::Delete, now_ms);
            return Ok(());
        }

        self.delete_backward(now_ms)
    }

    // ------------------------------------------------------------------
    // Comment toggling
    // ------------------------------------------------------------------

    /// Ctrl+/: toggle `// ` style line comments.
    pub fn toggle_line_comment(&mut self, token: &str, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        let lines = self.touched_lines();
        if lines.is_empty() {
            return Ok(());
        }
        let commented_token = format!("{token} ");

        // Collect line texts.
        let line_texts: Vec<String> = lines
            .iter()
            .map(|&l| {
                let s = self.rope.line_to_char(l);
                let e = self.line_end(l);
                self.rope.slice(s..e).to_string()
            })
            .collect();

        // Check if all non-empty lines already have the comment token.
        let all_commented = line_texts.iter().all(|t| {
            let trimmed = t.trim_start_matches([' ', '\t']);
            trimmed.is_empty()
                || trimmed.starts_with(&commented_token)
                || trimmed.starts_with(token)
        });

        if all_commented {
            // Remove comments.
            let mut changes = Vec::new();
            for (&l, text) in lines.iter().zip(&line_texts) {
                let ls = self.rope.line_to_char(l);
                let indent: usize = text.chars().take_while(|c| *c == ' ' || *c == '\t').count();
                let trimmed = &text[text
                    .char_indices()
                    .nth(indent)
                    .map_or(text.len(), |(b, _)| b)..];
                let remove_len = if trimmed.starts_with(&commented_token) {
                    commented_token.chars().count()
                } else if trimmed.starts_with(token) {
                    token.chars().count()
                } else {
                    0
                };
                if remove_len > 0 {
                    changes.push(Change {
                        start: ls + indent,
                        end: ls + indent + remove_len,
                        text: String::new(),
                    });
                }
            }
            if !changes.is_empty() {
                let sel = self.map_line_selection(&changes);
                self.apply_edit_batch(changes, sel, EditKind::Structural, now_ms);
            }
        } else {
            // Find minimum indent across non-empty lines.
            let min_indent = line_texts
                .iter()
                .filter(|t| !t.trim().is_empty())
                .map(|t| t.chars().take_while(|c| *c == ' ' || *c == '\t').count())
                .min()
                .unwrap_or(0);

            let mut changes: Vec<Change> = lines
                .iter()
                .map(|&l| {
                    let ls = self.rope.line_to_char(l);
                    Change {
                        start: ls + min_indent,
                        end: ls + min_indent,
                        text: commented_token.clone(),
                    }
                })
                .collect();
            changes.sort_by_key(|c| c.start);
            let sel = self.map_line_selection(&changes);
            self.apply_edit_batch(changes, sel, EditKind::Structural, now_ms);
        }
        Ok(())
    }

    /// Toggle block comment around primary selection.
    pub fn toggle_block_comment(
        &mut self,
        open: &str,
        close: &str,
        now_ms: u64,
    ) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        let primary = self.selection.primary_range();
        let s = primary.start();
        let e = primary.end();
        let selected: String = self.rope.slice(s..e).to_string();

        if selected.starts_with(open) && selected.ends_with(close) {
            // Remove.
            let inner = &selected[open.len()..selected.len() - close.len()];
            let new_text = inner.to_string();
            let new_len = new_text.chars().count();
            let changes = vec![Change {
                start: s,
                end: e,
                text: new_text,
            }];
            let sel = Selection::new(
                vec![Range {
                    anchor: s,
                    head: s + new_len,
                }],
                0,
            );
            self.apply_edit_batch(changes, sel, EditKind::Structural, now_ms);
        } else {
            // Add.
            let new_text = format!("{open}{selected}{close}");
            let new_len = new_text.chars().count();
            let changes = vec![Change {
                start: s,
                end: e,
                text: new_text,
            }];
            let sel = Selection::new(
                vec![Range {
                    anchor: s,
                    head: s + new_len,
                }],
                0,
            );
            self.apply_edit_batch(changes, sel, EditKind::Structural, now_ms);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
