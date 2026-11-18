//! Cursor and selection movement operations with grapheme and column awareness.

use crate::document::Document;
use crate::grapheme::{
    line_end, next_grapheme_char_idx, next_word_boundary, prev_grapheme_char_idx,
    prev_word_boundary, smart_line_start,
};
use crate::types::{Range, Selection};

/// Semantic movement operations for cursors and selections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveOp {
    /// Move left by one Unicode grapheme cluster.
    Left,
    /// Move right by one Unicode grapheme cluster.
    Right,
    /// Move up by one visual line, preserving preferred visual column.
    Up,
    /// Move down by one visual line, preserving preferred visual column.
    Down,
    /// Move left to previous word boundary (`\w` + `_`).
    WordLeft,
    /// Move right to next word boundary (`\w` + `_`).
    WordRight,
    /// Toggles between the first non-whitespace character on current line and column 0.
    LineStartSmart,
    /// Move to the end of the current line (before line terminator).
    LineEnd,
    /// Move up by N lines (page up), preserving preferred visual column.
    PageUp(usize),
    /// Move down by N lines (page down), preserving preferred visual column.
    PageDown(usize),
    /// Move to the very start of the document (character 0).
    DocStart,
    /// Move to the very end of the document.
    DocEnd,
}

impl Document {
    /// Moves all active cursors according to `op`.
    ///
    /// When `extend` is `false`, the selection collapses into a caret at the new position.
    /// When `extend` is `true`, `anchor` remains fixed and `head` expands or contracts.
    pub fn move_cursor(&mut self, op: MoveOp, extend: bool) {
        let is_vertical = matches!(
            op,
            MoveOp::Up | MoveOp::Down | MoveOp::PageUp(_) | MoveOp::PageDown(_)
        );

        if is_vertical {
            if self.preferred_col.is_none() {
                let primary_head = self.selection.primary_range().head;
                let (_, vis_col_1based) = self.line_col_of(primary_head);
                self.preferred_col = Some(vis_col_1based.saturating_sub(1));
            }
        } else {
            self.preferred_col = None;
        }

        let preferred_col = self.preferred_col.unwrap_or(0);
        let mut new_ranges = Vec::with_capacity(self.selection.ranges.len());

        for range in &self.selection.ranges {
            let new_head = match op {
                MoveOp::Left => {
                    if !extend && !range.is_empty() {
                        range.start()
                    } else {
                        prev_grapheme_char_idx(&self.rope, range.head)
                    }
                }
                MoveOp::Right => {
                    if !extend && !range.is_empty() {
                        range.end()
                    } else {
                        next_grapheme_char_idx(&self.rope, range.head)
                    }
                }
                MoveOp::Up => {
                    let curr_line = self.rope.char_to_line(range.head);
                    let target_line = curr_line.saturating_sub(1);
                    self.char_idx_of_line_col(target_line, preferred_col)
                }
                MoveOp::Down => {
                    let curr_line = self.rope.char_to_line(range.head);
                    let max_line = self.rope.len_lines().saturating_sub(1);
                    let target_line = (curr_line + 1).min(max_line);
                    self.char_idx_of_line_col(target_line, preferred_col)
                }
                MoveOp::PageUp(lines) => {
                    let curr_line = self.rope.char_to_line(range.head);
                    let target_line = curr_line.saturating_sub(lines);
                    self.char_idx_of_line_col(target_line, preferred_col)
                }
                MoveOp::PageDown(lines) => {
                    let curr_line = self.rope.char_to_line(range.head);
                    let max_line = self.rope.len_lines().saturating_sub(1);
                    let target_line = curr_line.saturating_add(lines).min(max_line);
                    self.char_idx_of_line_col(target_line, preferred_col)
                }
                MoveOp::WordLeft => prev_word_boundary(&self.rope, range.head),
                MoveOp::WordRight => next_word_boundary(&self.rope, range.head),
                MoveOp::LineStartSmart => smart_line_start(&self.rope, range.head),
                MoveOp::LineEnd => line_end(&self.rope, range.head),
                MoveOp::DocStart => 0,
                MoveOp::DocEnd => self.rope.len_chars(),
            };

            let new_range = if extend {
                Range {
                    anchor: range.anchor,
                    head: new_head,
                }
            } else {
                Range::point(new_head)
            };

            new_ranges.push(new_range);
        }

        self.selection = Selection::new(new_ranges, self.selection.primary);
    }
}
