//! Line-level operations on distinct contiguous groups of touched lines.

use crate::document::Document;
use crate::error::CoreError;
use crate::types::{Change, EditKind, Range, Selection};

// Changes are sorted and use coordinates in the original document. Insertions
// have right affinity; positions inside removed indentation collapse to its start.
pub(crate) fn map_position(pos: usize, changes: &[Change]) -> usize {
    let mut removed = 0;
    let mut inserted = 0;
    for change in changes {
        if change.start > pos {
            break;
        }
        let len = change.text.chars().count();
        if pos < change.end {
            return change.start - removed + inserted + len;
        }
        removed += change.end - change.start;
        inserted += len;
    }
    pos - removed + inserted
}

pub(crate) fn split_delimiter(text: &str) -> (&str, &str) {
    let len = if text.ends_with("\r\n") {
        2
    } else if text.ends_with([
        '\n', '\r', '\u{b}', '\u{c}', '\u{85}', '\u{2028}', '\u{2029}',
    ]) {
        text.chars().next_back().map_or(0, char::len_utf8)
    } else {
        0
    };
    text.split_at(text.len() - len)
}

impl Document {
    /// Indents all lines touched by active selections.
    pub fn indent_lines(&mut self, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        let unit = if self.insert_spaces {
            " ".repeat(self.tab_size)
        } else {
            "\t".into()
        };
        let changes = self
            .touched_lines()
            .into_iter()
            .map(|line| {
                let start = self.rope.line_to_char(line);
                Change {
                    start,
                    end: start,
                    text: unit.clone(),
                }
            })
            .collect::<Vec<_>>();
        let selection = self.map_line_selection(&changes);
        self.apply_edit_batch(changes, selection, EditKind::Structural, now_ms);
        Ok(())
    }

    /// Outdents all lines touched by active selections.
    pub fn outdent_lines(&mut self, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        let mut changes = Vec::new();
        for line in self.touched_lines() {
            let start = self.rope.line_to_char(line);
            let text = self.rope.line(line).to_string();
            let count = if text.starts_with('\t') {
                1
            } else {
                text.chars()
                    .take(self.tab_size)
                    .take_while(|&c| c == ' ')
                    .count()
            };
            if count > 0 {
                changes.push(Change {
                    start,
                    end: start + count,
                    text: String::new(),
                });
            }
        }
        if !changes.is_empty() {
            let selection = self.map_line_selection(&changes);
            self.apply_edit_batch(changes, selection, EditKind::Structural, now_ms);
        }
        Ok(())
    }

    /// Moves each selected line group up by one line.
    pub fn move_lines_up(&mut self, now_ms: u64) -> Result<(), CoreError> {
        self.move_line_groups(false, now_ms)
    }

    /// Moves each selected line group down by one line.
    pub fn move_lines_down(&mut self, now_ms: u64) -> Result<(), CoreError> {
        self.move_line_groups(true, now_ms)
    }

    fn move_line_groups(&mut self, down: bool, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        // The synthetic empty line after a trailing delimiter is not a line
        // that can be swapped: doing so would remove the trailing newline.
        let texts = self.rope.lines().map(|l| l.to_string()).collect::<Vec<_>>();
        let count = texts.len()
            - usize::from(texts.len() > 1 && texts.last().is_some_and(String::is_empty));
        let parts = texts.iter().map(|s| split_delimiter(s)).collect::<Vec<_>>();
        let mut order = (0..texts.len()).collect::<Vec<_>>();
        let mut spans = Vec::new();
        for (first, last) in self.line_groups() {
            if last >= count || (!down && first == 0) || (down && last + 1 >= count) {
                continue;
            }
            let (start, end) = if down {
                (first, last + 1)
            } else {
                (first - 1, last)
            };
            if down {
                order[start..=end].rotate_right(1);
            } else {
                order[start..=end].rotate_left(1);
            }
            spans.push((start, end));
        }
        if spans.is_empty() {
            return Ok(());
        }
        // Delimiters belong to slots, not line bodies. In particular the final
        // unterminated slot remains unterminated when its body moves elsewhere.
        let mut starts = vec![0; texts.len()];
        let mut destinations = vec![0; texts.len()];
        let mut offset = 0;
        for (slot, &original) in order.iter().enumerate() {
            starts[slot] = offset;
            destinations[original] = slot;
            offset += parts[original].0.chars().count() + parts[slot].1.chars().count();
        }
        let changes = spans
            .into_iter()
            .map(|(start, end)| Change {
                start: self.rope.line_to_char(start),
                end: self.line_end(end),
                text: (start..=end).fold(String::new(), |mut text, slot| {
                    text.push_str(parts[order[slot]].0);
                    text.push_str(parts[slot].1);
                    text
                }),
            })
            .collect();
        let ranges = self
            .selection
            .ranges
            .iter()
            .map(|r| {
                let map = |pos| {
                    let line = self.rope.char_to_line(pos);
                    // A selection ending at the next line start follows the end
                    // of the selected group, rather than that unselected line.
                    if !r.is_empty() && pos == r.end() && pos == self.rope.line_to_char(line) {
                        let slot = destinations[line - 1];
                        return starts[slot]
                            + parts[line - 1].0.chars().count()
                            + parts[slot].1.chars().count();
                    }
                    let slot = destinations[line];
                    let col = pos - self.rope.line_to_char(line);
                    starts[slot]
                        + col.min(parts[line].0.chars().count() + parts[slot].1.chars().count())
                };
                Range {
                    anchor: map(r.anchor),
                    head: map(r.head),
                }
            })
            .collect::<Vec<_>>();
        self.apply_edit_batch(
            changes,
            Selection::new(ranges, self.selection.primary),
            EditKind::Structural,
            now_ms,
        );
        Ok(())
    }

    /// Duplicates each selected line group downwards.
    pub fn duplicate_lines_down(&mut self, now_ms: u64) -> Result<(), CoreError> {
        self.duplicate_line_groups(true, now_ms)
    }

    /// Duplicates each selected line group upwards.
    pub fn duplicate_lines_up(&mut self, now_ms: u64) -> Result<(), CoreError> {
        self.duplicate_line_groups(false, now_ms)
    }

    fn duplicate_line_groups(&mut self, down: bool, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        let groups = self.line_groups();
        let mut changes = Vec::new();
        let mut shifts = Vec::new();
        let mut preceding = 0;
        for &(first, last) in &groups {
            let start = self.rope.line_to_char(first);
            let end = self.line_end(last);
            let block = self.rope.slice(start..end).to_string();
            let terminated = !split_delimiter(&block).1.is_empty();
            let text = if terminated {
                block.clone()
            } else if down {
                format!("\n{block}")
            } else {
                format!("{block}\n")
            };
            let len = text.chars().count();
            shifts.push(preceding + if down { len } else { 0 });
            let pos = if down { end } else { start };
            changes.push(Change {
                start: pos,
                end: pos,
                text,
            });
            preceding += len;
        }
        let ranges = self
            .selection
            .ranges
            .iter()
            .map(|r| {
                let line = self.rope.char_to_line(r.start());
                let shift = groups
                    .iter()
                    .zip(&shifts)
                    .find(|((a, b), _)| *a <= line && line <= *b)
                    .map_or(0, |(_, shift)| *shift);
                Range {
                    anchor: r.anchor + shift,
                    head: r.head + shift,
                }
            })
            .collect::<Vec<_>>();
        self.apply_edit_batch(
            changes,
            Selection::new(ranges, self.selection.primary),
            EditKind::Structural,
            now_ms,
        );
        Ok(())
    }

    /// Deletes only the line groups touched by the current selection.
    pub fn delete_lines(&mut self, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        let changes = self
            .line_groups()
            .into_iter()
            .map(|(first, last)| Change {
                start: self.rope.line_to_char(first),
                end: self.line_end(last),
                text: String::new(),
            })
            .collect::<Vec<_>>();
        let len = self.rope.len_chars() - changes.iter().map(|c| c.end - c.start).sum::<usize>();
        let ranges = self
            .selection
            .ranges
            .iter()
            .map(|r| Range::point(map_position(r.start(), &changes).min(len)))
            .collect::<Vec<_>>();
        self.apply_edit_batch(
            changes,
            Selection::new(ranges, self.selection.primary),
            EditKind::Structural,
            now_ms,
        );
        Ok(())
    }

    pub(crate) fn map_line_selection(&self, changes: &[Change]) -> Selection {
        Selection::new(
            self.selection.ranges.iter().map(|r| Range {
                anchor: map_position(r.anchor, changes),
                head: map_position(r.head, changes),
            }),
            self.selection.primary,
        )
    }

    pub(crate) fn line_end(&self, line: usize) -> usize {
        if line + 1 < self.rope.len_lines() {
            self.rope.line_to_char(line + 1)
        } else {
            self.rope.len_chars()
        }
    }

    /// Half-open nonempty selections do not touch their terminal line start.
    pub(crate) fn touched_lines(&self) -> Vec<usize> {
        let mut lines = Vec::new();
        for r in &self.selection.ranges {
            let start = self.rope.char_to_line(r.start());
            let mut end = self.rope.char_to_line(r.end());
            if !r.is_empty() && self.rope.line_to_char(end) == r.end() {
                end -= 1;
            }
            lines.extend(start..=end);
        }
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    pub(crate) fn line_groups(&self) -> Vec<(usize, usize)> {
        let mut groups: Vec<(usize, usize)> = Vec::new();
        for line in self.touched_lines() {
            if let Some(last) = groups.last_mut() {
                if last.1 + 1 == line {
                    last.1 = line;
                    continue;
                }
            }
            groups.push((line, line));
        }
        groups
    }
}
