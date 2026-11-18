//! Text editing operations, multi-cursor transactions, and selection edits.

use crate::document::Document;
use crate::error::CoreError;
use crate::grapheme::{next_grapheme_char_idx, prev_grapheme_char_idx};
use crate::types::{Change, EditKind, Range, Selection, Transaction};

impl Document {
    /// Inserts text at all active selections.
    pub fn insert_text(&mut self, text: &str, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }

        let mut changes = Vec::with_capacity(self.selection.ranges.len());
        let mut new_ranges = Vec::with_capacity(self.selection.ranges.len());
        let insert_len = text.chars().count();
        let mut delta_acc: isize = 0;

        for r in &self.selection.ranges {
            let start = r.start();
            let end = r.end();
            changes.push(Change {
                start,
                end,
                text: text.to_string(),
            });

            #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
            let new_pos = (start as isize + delta_acc) as usize + insert_len;
            new_ranges.push(Range::point(new_pos));

            #[allow(clippy::cast_possible_wrap)]
            let delta = insert_len as isize - (end - start) as isize;
            delta_acc += delta;
        }

        let new_selection = Selection::new(new_ranges, self.selection.primary);
        self.apply_edit_batch(changes, new_selection, EditKind::InsertBulk, now_ms);
        Ok(())
    }

    /// Inserts a single character at all active selections.
    pub fn insert_char(&mut self, ch: char, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }

        let mut ch_buf = [0u8; 4];
        let ch_str = ch.encode_utf8(&mut ch_buf);

        let mut changes = Vec::with_capacity(self.selection.ranges.len());
        let mut new_ranges = Vec::with_capacity(self.selection.ranges.len());
        let mut delta_acc: isize = 0;

        for r in &self.selection.ranges {
            let start = r.start();
            let end = r.end();
            changes.push(Change {
                start,
                end,
                text: ch_str.to_string(),
            });

            #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
            let new_pos = (start as isize + delta_acc) as usize + 1;
            new_ranges.push(Range::point(new_pos));

            #[allow(clippy::cast_possible_wrap)]
            let delta = 1isize - (end - start) as isize;
            delta_acc += delta;
        }

        let new_selection = Selection::new(new_ranges, self.selection.primary);
        self.apply_edit_batch(changes, new_selection, EditKind::InsertChar, now_ms);
        Ok(())
    }

    /// Deletes backward from all active selections.
    pub fn delete_backward(&mut self, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }

        let mut changes = Vec::with_capacity(self.selection.ranges.len());
        let mut new_ranges = Vec::with_capacity(self.selection.ranges.len());
        let mut delta_acc: isize = 0;

        for r in &self.selection.ranges {
            let (start, end) = if r.is_empty() {
                if r.head == 0 {
                    (0, 0)
                } else {
                    let prev = prev_grapheme_char_idx(&self.rope, r.head);
                    (prev, r.head)
                }
            } else {
                (r.start(), r.end())
            };

            if start < end {
                changes.push(Change {
                    start,
                    end,
                    text: String::new(),
                });
            }

            #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
            let new_pos = (start as isize + delta_acc).max(0) as usize;
            new_ranges.push(Range::point(new_pos));

            #[allow(clippy::cast_possible_wrap)]
            let delta = -((end - start) as isize);
            delta_acc += delta;
        }

        if changes.is_empty() {
            return Ok(());
        }

        let new_selection = Selection::new(new_ranges, self.selection.primary);
        self.apply_edit_batch(changes, new_selection, EditKind::Delete, now_ms);
        Ok(())
    }

    /// Deletes forward from all active selections.
    pub fn delete_forward(&mut self, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }

        let mut changes = Vec::with_capacity(self.selection.ranges.len());
        let mut new_ranges = Vec::with_capacity(self.selection.ranges.len());
        let mut delta_acc: isize = 0;
        let total_chars = self.rope.len_chars();

        for r in &self.selection.ranges {
            let (start, end) = if r.is_empty() {
                if r.head >= total_chars {
                    (total_chars, total_chars)
                } else {
                    let next = next_grapheme_char_idx(&self.rope, r.head);
                    (r.head, next)
                }
            } else {
                (r.start(), r.end())
            };

            if start < end {
                changes.push(Change {
                    start,
                    end,
                    text: String::new(),
                });
            }

            #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
            let new_pos = (start as isize + delta_acc).max(0) as usize;
            new_ranges.push(Range::point(new_pos));

            #[allow(clippy::cast_possible_wrap)]
            let delta = -((end - start) as isize);
            delta_acc += delta;
        }

        if changes.is_empty() {
            return Ok(());
        }

        let new_selection = Selection::new(new_ranges, self.selection.primary);
        self.apply_edit_batch(changes, new_selection, EditKind::Delete, now_ms);
        Ok(())
    }

    /// Inserts a newline with auto-indentation matching the current context.
    pub fn insert_newline_auto_indent(&mut self, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }

        let indent_unit = if self.insert_spaces {
            " ".repeat(self.tab_size)
        } else {
            "\t".to_string()
        };

        let mut changes = Vec::with_capacity(self.selection.ranges.len());
        let mut new_ranges = Vec::with_capacity(self.selection.ranges.len());
        let mut delta_acc: isize = 0;

        for r in &self.selection.ranges {
            let start = r.start();
            let end = r.end();
            let line_idx = self.rope.char_to_line(r.head);
            let line_start = self.rope.line_to_char(line_idx);
            let head_offset = r.head.saturating_sub(line_start);
            let line_slice = self.rope.line(line_idx);
            let line_str = line_slice.to_string();

            let mut base_indent = String::new();
            for ch in line_str.chars() {
                if ch == ' ' || ch == '\t' {
                    base_indent.push(ch);
                } else {
                    break;
                }
            }

            let before_str: String = line_str.chars().take(head_offset).collect();
            let trimmed_before = before_str.trim_end();
            let after_str: String = line_str.chars().skip(head_offset).collect();
            let trimmed_after = after_str.trim_start();

            let between_braces = trimmed_before.ends_with('{') && trimmed_after.starts_with('}');
            let extra_indent = trimmed_before.ends_with('{')
                || trimmed_before.ends_with('[')
                || trimmed_before.ends_with('(')
                || trimmed_before.ends_with(':');

            let (text, caret_offset) = if between_braces {
                let full = format!("\n{base_indent}{indent_unit}\n{base_indent}");
                let offset = 1 + base_indent.chars().count() + indent_unit.chars().count();
                (full, offset)
            } else if extra_indent {
                let full = format!("\n{base_indent}{indent_unit}");
                (full.clone(), full.chars().count())
            } else {
                let full = format!("\n{base_indent}");
                (full.clone(), full.chars().count())
            };

            let text_char_len = text.chars().count();
            changes.push(Change { start, end, text });

            #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
            let new_pos = (start as isize + delta_acc) as usize + caret_offset;
            new_ranges.push(Range::point(new_pos));

            #[allow(clippy::cast_possible_wrap)]
            let delta = text_char_len as isize - (end - start) as isize;
            delta_acc += delta;
        }

        let new_selection = Selection::new(new_ranges, self.selection.primary);
        self.apply_edit_batch(changes, new_selection, EditKind::InsertBulk, now_ms);
        Ok(())
    }

    /// Reverts the most recent transaction from history.
    pub fn undo(&mut self) -> Result<bool, CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }

        if let Some((tx, inverse_changes)) = self.history.pop_undo_entry() {
            let mut sorted_inv = inverse_changes;
            sorted_inv.sort_by_key(|c| c.start);
            apply_sorted_changes_to_rope(&mut self.rope, &sorted_inv);

            self.selection = tx.before;
            self.version += 1;
            self.preferred_col = None;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Re-applies the most recently undone transaction from history.
    pub fn redo(&mut self) -> Result<bool, CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }

        if let Some((tx, _)) = self.history.pop_redo_entry() {
            let mut sorted = tx.changes;
            sorted.sort_by_key(|c| c.start);
            apply_sorted_changes_to_rope(&mut self.rope, &sorted);

            self.selection = tx.after;
            self.version += 1;
            self.preferred_col = None;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Selects all text in the document.
    pub fn select_all(&mut self) {
        let total = self.rope.len_chars();
        self.selection = Selection::single(Range {
            anchor: 0,
            head: total,
        });
        self.preferred_col = None;
    }

    /// Applies a batch of non-overlapping character-index changes as one undoable transaction.
    pub fn apply_changes(
        &mut self,
        changes: Vec<Change>,
        new_selection: Selection,
        kind: EditKind,
        now_ms: u64,
    ) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        if changes.is_empty() {
            self.selection = new_selection;
            return Ok(());
        }
        self.apply_edit_batch(changes, new_selection, kind, now_ms);
        Ok(())
    }

    /// Helper to apply a batch of sorted changes in reverse order (or single-pass `RopeBuilder` for large batches).
    pub(crate) fn apply_edit_batch(
        &mut self,
        changes: Vec<Change>,
        new_selection: Selection,
        kind: EditKind,
        now_ms: u64,
    ) {
        let before_selection = self.selection.clone();
        let mut sorted_changes = changes;
        sorted_changes.sort_by_key(|c| c.start);

        let len_chars = self.rope.len_chars();
        let non_overlapping = sorted_changes.windows(2).all(|w| w[0].end <= w[1].start)
            && sorted_changes.last().is_none_or(|c| c.end <= len_chars);

        let inverses = if sorted_changes.len() > 256 && non_overlapping {
            let mut builder = ropey::RopeBuilder::new();
            let mut walker = RopeChunkWalker::new(&self.rope);
            let mut inverses = Vec::with_capacity(sorted_changes.len());
            let mut cursor = 0usize;
            let mut removed = 0usize;
            let mut inserted = 0usize;

            for change in &sorted_changes {
                if change.start > cursor {
                    walker.consume(change.start - cursor, |s| builder.append(s));
                }
                let old_len = change.end - change.start;
                let mut old_text = String::with_capacity(old_len);
                walker.consume(old_len, |s| old_text.push_str(s));
                builder.append(&change.text);
                cursor = change.end;

                let inv_start = change.start - removed + inserted;
                let inserted_len = change.text.chars().count();
                removed += old_len;
                inserted += inserted_len;
                inverses.push(Change {
                    start: inv_start,
                    end: inv_start + inserted_len,
                    text: old_text,
                });
            }
            if cursor < len_chars {
                walker.consume(len_chars - cursor, |s| builder.append(s));
            }
            self.rope = builder.finish();
            inverses
        } else {
            let mut inverses = Vec::with_capacity(sorted_changes.len());
            for change in sorted_changes.iter().rev() {
                let old_text = self.rope.slice(change.start..change.end).to_string();
                self.rope.remove(change.start..change.end);
                if !change.text.is_empty() {
                    self.rope.insert(change.start, &change.text);
                }
                let inserted_len = change.text.chars().count();
                inverses.push(Change {
                    start: change.start,
                    end: change.start + inserted_len,
                    text: old_text,
                });
            }
            inverses.reverse();
            let mut removed = 0;
            let mut inserted = 0;
            for (inverse, change) in inverses.iter_mut().zip(&sorted_changes) {
                inverse.start = change.start - removed + inserted;
                inverse.end = inverse.start + change.text.chars().count();
                removed += change.end - change.start;
                inserted += change.text.chars().count();
            }
            inverses
        };

        let tx = Transaction {
            changes: sorted_changes,
            before: before_selection,
            after: new_selection.clone(),
            timestamp_ms: now_ms,
            kind,
        };

        self.history.push_with_inverse(tx, inverses);
        self.selection = new_selection;
        self.version += 1;
        self.preferred_col = None;
    }
}

fn apply_sorted_changes_to_rope(rope: &mut ropey::Rope, sorted: &[Change]) {
    let len_chars = rope.len_chars();
    let non_overlapping = sorted.windows(2).all(|w| w[0].end <= w[1].start)
        && sorted.last().is_none_or(|c| c.end <= len_chars);

    if sorted.len() > 256 && non_overlapping {
        let mut builder = ropey::RopeBuilder::new();
        let mut walker = RopeChunkWalker::new(rope);
        let mut cursor = 0usize;
        for c in sorted {
            if c.start > cursor {
                walker.consume(c.start - cursor, |s| builder.append(s));
            }
            if c.end > c.start {
                walker.consume(c.end - c.start, |_| {});
            }
            builder.append(&c.text);
            cursor = c.end;
        }
        if cursor < len_chars {
            walker.consume(len_chars - cursor, |s| builder.append(s));
        }
        *rope = builder.finish();
    } else {
        for c in sorted.iter().rev() {
            rope.remove(c.start..c.end);
            if !c.text.is_empty() {
                rope.insert(c.start, &c.text);
            }
        }
    }
}

struct RopeChunkWalker<'a> {
    chunks: ropey::iter::Chunks<'a>,
    curr: &'a str,
    curr_chars: usize,
}

impl<'a> RopeChunkWalker<'a> {
    fn new(rope: &'a ropey::Rope) -> Self {
        Self {
            chunks: rope.chunks(),
            curr: "",
            curr_chars: 0,
        }
    }

    fn consume(&mut self, mut count: usize, mut sink: impl FnMut(&str)) {
        while count > 0 {
            if self.curr_chars == 0 {
                let Some(next) = self.chunks.next() else {
                    break;
                };
                self.curr = next;
                self.curr_chars = if next.is_ascii() {
                    next.len()
                } else {
                    next.chars().count()
                };
                continue;
            }
            if count >= self.curr_chars {
                sink(self.curr);
                count -= self.curr_chars;
                self.curr = "";
                self.curr_chars = 0;
            } else {
                let split_byte = if self.curr.is_ascii() {
                    count
                } else {
                    self.curr
                        .char_indices()
                        .nth(count)
                        .map_or(self.curr.len(), |(idx, _)| idx)
                };
                let (head, tail) = self.curr.split_at(split_byte);
                sink(head);
                self.curr = tail;
                self.curr_chars -= count;
                count = 0;
            }
        }
    }
}
