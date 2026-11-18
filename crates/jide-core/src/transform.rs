//! Text transform primitives: case conversion, line sorting, whitespace trimming,
//! line expansion, and line insertion above/below.

use crate::document::Document;
use crate::error::CoreError;
use crate::lines::split_delimiter;
use crate::types::{Change, EditKind, Range, Selection};

impl Document {
    /// Ctrl+K Ctrl+U: uppercases selected text.
    pub fn transform_case_upper(&mut self, now_ms: u64) -> Result<(), CoreError> {
        self.transform_case(true, now_ms)
    }

    /// Ctrl+K Ctrl+L: lowercases selected text.
    pub fn transform_case_lower(&mut self, now_ms: u64) -> Result<(), CoreError> {
        self.transform_case(false, now_ms)
    }

    fn transform_case(&mut self, upper: bool, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        let mut changes = Vec::new();
        let mut new_ranges = Vec::new();
        let mut delta: isize = 0;
        for r in &self.selection.ranges {
            if r.is_empty() {
                new_ranges.push(*r);
                continue;
            }
            let text: String = self.rope.slice(r.start()..r.end()).to_string();
            let new_text = if upper {
                text.to_uppercase()
            } else {
                text.to_lowercase()
            };
            let new_len = new_text.chars().count();
            #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
            let ns = (r.start() as isize + delta) as usize;
            new_ranges.push(Range {
                anchor: ns,
                head: ns + new_len,
            });
            changes.push(Change {
                start: r.start(),
                end: r.end(),
                text: new_text,
            });
            #[allow(clippy::cast_possible_wrap)]
            {
                delta += new_len as isize - r.len() as isize;
            }
        }
        if changes.is_empty() {
            return Ok(());
        }
        let sel = Selection::new(new_ranges, self.selection.primary);
        self.apply_edit_batch(changes, sel, EditKind::Structural, now_ms);
        Ok(())
    }

    /// Sort the lines spanned by the selection in-place.
    pub fn sort_lines(
        &mut self,
        descending: bool,
        unique: bool,
        now_ms: u64,
    ) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        let lines = self.touched_lines();
        if lines.is_empty() {
            return Ok(());
        }
        let first = *lines.first().unwrap_or(&0);
        let last = *lines.last().unwrap_or(&0);

        let region_start = self.rope.line_to_char(first);
        let region_end = self.line_end(last);

        let mut pairs: Vec<(String, String)> = Vec::new();
        for l in first..=last {
            let ls = self.rope.line_to_char(l);
            let le = self.line_end(l);
            let line_text: String = self.rope.slice(ls..le).to_string();
            let (body, delim) = split_delimiter(&line_text);
            pairs.push((body.to_string(), delim.to_string()));
        }

        let mut bodies: Vec<String> = pairs.iter().map(|(b, _)| b.clone()).collect();
        if unique {
            let mut seen = std::collections::HashSet::new();
            bodies.retain(|b| seen.insert(b.clone()));
        }
        if descending {
            bodies.sort_unstable_by(|a, b| b.cmp(a));
        } else {
            bodies.sort_unstable();
        }

        let delims: Vec<String> = pairs.iter().map(|(_, d)| d.clone()).collect();
        let n = bodies.len().min(delims.len().max(1));
        let mut new_text = String::new();
        for (i, body) in bodies.iter().enumerate().take(n) {
            new_text.push_str(body);
            new_text.push_str(delims.get(i).map_or("", String::as_str));
        }

        let len = new_text.chars().count();
        let changes = vec![Change {
            start: region_start,
            end: region_end,
            text: new_text,
        }];
        let sel = Selection::single(Range {
            anchor: region_start,
            head: region_start + len,
        });
        self.apply_edit_batch(changes, sel, EditKind::Structural, now_ms);
        Ok(())
    }

    /// Removes trailing whitespace from every line in the document.
    pub fn trim_trailing_whitespace(&mut self, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        let mut changes = Vec::new();
        for line in 0..self.rope.len_lines() {
            let ls = self.rope.line_to_char(line);
            let le = self.line_end(line);
            let text: String = self.rope.slice(ls..le).to_string();
            let (body, _delim) = split_delimiter(&text);
            let trimmed = body.trim_end_matches([' ', '\t']);
            if trimmed.len() < body.len() {
                let trim_start = ls + trimmed.chars().count();
                let trim_end = ls + body.chars().count();
                changes.push(Change {
                    start: trim_start,
                    end: trim_end,
                    text: String::new(),
                });
            }
        }
        if changes.is_empty() {
            return Ok(());
        }
        let sel = self.map_line_selection(&changes);
        self.apply_edit_batch(changes, sel, EditKind::Structural, now_ms);
        Ok(())
    }

    /// Expands each selection to cover entire lines.
    pub fn expand_line_selection(&mut self) {
        let new_ranges: Vec<Range> = self
            .selection
            .ranges
            .iter()
            .map(|r| {
                let first_line = self.rope.char_to_line(r.start());
                let last_line = self
                    .rope
                    .char_to_line(r.end().saturating_sub(1).max(r.start()));
                let s = self.rope.line_to_char(first_line);
                let e = self.line_end(last_line);
                Range { anchor: s, head: e }
            })
            .collect();
        self.selection = Selection::new(new_ranges, self.selection.primary);
    }

    /// Ctrl+Shift+Enter: inserts a new blank line above the current line.
    pub fn insert_line_above(&mut self, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        self.insert_line_relative(false, now_ms);
        Ok(())
    }

    /// Ctrl+Enter: inserts a new blank line below the current line.
    pub fn insert_line_below(&mut self, now_ms: u64) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        self.insert_line_relative(true, now_ms);
        Ok(())
    }

    fn insert_line_relative(&mut self, below: bool, now_ms: u64) {
        let mut changes = Vec::new();
        let mut new_ranges = Vec::new();
        let mut delta: isize = 0;

        for r in &self.selection.ranges {
            let cur_line = self.rope.char_to_line(r.head);
            let ls = self.rope.line_to_char(cur_line);
            let le = self.line_end(cur_line);
            let line_text: String = self.rope.slice(ls..le).to_string();
            let indent: String = line_text
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect();

            let (insert_pos, new_text) = if below {
                let body_len = split_delimiter(&line_text).0.chars().count();
                let pos = ls + body_len;
                (pos, format!("\n{indent}"))
            } else {
                (ls, format!("{indent}\n"))
            };

            let text_len = new_text.chars().count();
            changes.push(Change {
                start: insert_pos,
                end: insert_pos,
                text: new_text,
            });
            let caret = if below {
                #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
                {
                    (insert_pos as isize + delta) as usize + 1 + indent.chars().count()
                }
            } else {
                #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
                {
                    (insert_pos as isize + delta) as usize + indent.chars().count()
                }
            };
            new_ranges.push(Range::point(caret));
            #[allow(clippy::cast_possible_wrap)]
            {
                delta += text_len as isize;
            }
        }

        let sel = Selection::new(new_ranges, self.selection.primary);
        self.apply_edit_batch(changes, sel, EditKind::Structural, now_ms);
    }
}
