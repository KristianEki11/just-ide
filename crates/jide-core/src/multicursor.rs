//! Multi-cursor, column-box selection, and occurrence-finding helpers.

use unicode_segmentation::UnicodeSegmentation;

use crate::document::Document;
use crate::types::{Range, Selection};

// ---------- word helpers -----------------------------------------------

/// Returns the word boundaries (start, end) surrounding `caret` using the same rules
/// as VS Code: sequences of `\w` (`[A-Za-z0-9_]`).
fn word_at(rope: &ropey::Rope, caret: usize) -> Option<(usize, usize)> {
    let total = rope.len_chars();
    if total == 0 {
        return None;
    }
    let c = caret.min(total.saturating_sub(1));
    let ch = rope.char(c);
    if !is_word_char(ch) {
        return None;
    }
    let mut start = c;
    while start > 0 && is_word_char(rope.char(start - 1)) {
        start -= 1;
    }
    let mut end = c + 1;
    while end < total && is_word_char(rope.char(end)) {
        end += 1;
    }
    Some((start, end))
}

#[inline]
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Searches for `needle` in `rope` starting at char-index `from`, returning the
/// start char-index if found.  Wraps around the document once.
fn find_next_occurrence(rope: &ropey::Rope, needle: &str, from: usize) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    let total = rope.len_chars();
    let text: String = rope.chars().collect();
    let needle_len = needle.chars().count();
    let byte_from = char_to_byte(&text, from);

    // Search from `from` to end.
    if let Some(pos) = text[byte_from..].find(needle) {
        let byte_abs = byte_from + pos;
        return Some(byte_to_char(&text, byte_abs));
    }
    // Wrap: search from 0 to `from + needle_len - 1`.
    let stop_byte = char_to_byte(&text, (from + needle_len).min(total));
    if let Some(pos) = text[..stop_byte].find(needle) {
        return Some(byte_to_char(&text, pos));
    }
    None
}

/// Collects all start char-indices of `needle` in `rope`.
fn find_all_occurrences(rope: &ropey::Rope, needle: &str) -> Vec<usize> {
    if needle.is_empty() {
        return vec![];
    }
    let text: String = rope.chars().collect();
    let mut results = Vec::new();
    let mut start = 0usize;
    while let Some(pos) = text[start..].find(needle) {
        let byte_abs = start + pos;
        results.push(byte_to_char(&text, byte_abs));
        start = byte_abs + needle.len().max(1);
    }
    results
}

#[inline]
fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map_or(s.len(), |(b, _)| b)
}

#[inline]
fn byte_to_char(s: &str, byte_idx: usize) -> usize {
    s[..byte_idx].chars().count()
}

// ---------- Document methods -------------------------------------------

impl Document {
    /// Ctrl+D: if primary is empty, select word at caret; otherwise find and add
    /// the next occurrence of the primary selection text (wrapping to doc start).
    pub fn add_next_occurrence(&mut self) {
        let primary = self.selection.primary_range();

        if primary.is_empty() {
            if let Some((ws, we)) = word_at(&self.rope, primary.head) {
                let new_range = Range {
                    anchor: ws,
                    head: we,
                };
                let mut ranges: Vec<Range> = self.selection.ranges.to_vec();
                ranges[self.selection.primary] = new_range;
                self.selection = Selection::new(ranges, self.selection.primary);
            }
            return;
        }

        let needle: String = self.rope.slice(primary.start()..primary.end()).to_string();
        let search_from = primary.end();

        if let Some(hit) = find_next_occurrence(&self.rope, &needle, search_from) {
            let hit_end = hit + needle.chars().count();
            let new_range = Range {
                anchor: hit,
                head: hit_end,
            };
            let mut ranges: Vec<Range> = self.selection.ranges.to_vec();
            let new_primary = ranges.len();
            ranges.push(new_range);
            self.selection = Selection::new(ranges, new_primary);
        }
    }

    /// Ctrl+Shift+L: select all occurrences of the word at caret / primary selected text.
    pub fn select_all_occurrences(&mut self) {
        let primary = self.selection.primary_range();

        let needle = if primary.is_empty() {
            word_at(&self.rope, primary.head).map(|(s, e)| self.rope.slice(s..e).to_string())
        } else {
            Some(self.rope.slice(primary.start()..primary.end()).to_string())
        };

        let needle = match needle {
            Some(n) if !n.is_empty() => n,
            _ => return,
        };
        let needle_len = needle.chars().count();
        let hits = find_all_occurrences(&self.rope, &needle);
        if hits.is_empty() {
            return;
        }

        let primary_start = primary.start();
        let primary_idx = hits.iter().position(|&h| h == primary_start).unwrap_or(0);

        let ranges: Vec<Range> = hits
            .into_iter()
            .map(|h| Range {
                anchor: h,
                head: h + needle_len,
            })
            .collect();
        self.selection = Selection::new(ranges, primary_idx);
    }

    /// Ctrl+Alt+Up / Ctrl+Alt+Down: add a point cursor one line above/below each
    /// existing range head, preserving the current visual column.
    pub fn add_cursor_vertical(&mut self, delta_lines: isize) {
        let total_lines = self.rope.len_lines();
        let total_chars = self.rope.len_chars();
        if total_lines == 0 || total_chars == 0 {
            return;
        }
        let mut new_ranges: Vec<Range> = self.selection.ranges.to_vec();
        let mut new_primary = self.selection.primary;

        for r in &self.selection.ranges {
            let head = r.head.min(total_chars);
            let cur_line = self.rope.char_to_line(head);
            let line_start = self.rope.line_to_char(cur_line);
            let col = head.saturating_sub(line_start);

            #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
            let target_line = (cur_line as isize + delta_lines)
                .max(0)
                .min(total_lines as isize - 1) as usize;

            let tl_start = self.rope.line_to_char(target_line);
            let tl_end = self.line_end(target_line);
            let tl_len = tl_end.saturating_sub(tl_start);
            let tl_len_no_nl = if tl_len > 0 && tl_start + tl_len <= total_chars {
                let last = self.rope.char(tl_start + tl_len - 1);
                if last == '\n' || last == '\r' {
                    tl_len - 1
                } else {
                    tl_len
                }
            } else {
                tl_len
            };
            let new_col = col.min(tl_len_no_nl);
            let new_pos = tl_start + new_col;
            let add_idx = new_ranges.len();
            new_ranges.push(Range::point(new_pos));
            new_primary = add_idx;
        }

        self.selection = Selection::new(new_ranges, new_primary);
    }

    /// Shift+Alt+Drag: column/box selection generating per-line point/range selections.
    pub fn select_column_box(
        &mut self,
        anchor_line_col: (usize, usize),
        head_line_col: (usize, usize),
    ) {
        let first_line = anchor_line_col.0.min(head_line_col.0);
        let last_line = anchor_line_col.0.max(head_line_col.0);
        let left_col = anchor_line_col.1.min(head_line_col.1);
        let right_col = anchor_line_col.1.max(head_line_col.1);
        let total_lines = self.rope.len_lines();

        let mut ranges = Vec::with_capacity(last_line - first_line + 1);

        for line in first_line..=last_line {
            if line >= total_lines {
                break;
            }
            let line_start = self.rope.line_to_char(line);
            let line_end = self.line_end(line);
            let line_len_raw = line_end.saturating_sub(line_start);
            // Strip trailing newline characters for column clamping.
            let line_text: String = self.rope.slice(line_start..line_end).chars().collect();
            let content_len: usize = line_text
                .grapheme_indices(true)
                .filter(|(_, g)| !matches!(*g, "\n" | "\r" | "\r\n"))
                .count();
            let _ = line_len_raw;

            let col_a = left_col.min(content_len);
            let col_b = right_col.min(content_len);

            ranges.push(Range {
                anchor: line_start + col_a,
                head: line_start + col_b,
            });
        }

        let primary = ranges.len().saturating_sub(1);
        self.selection = Selection::new(ranges, primary);
    }
}
