//! Core data types for text representation, selections, and transactions.

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// Stable document identifier within an editor session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocId(pub u64);

/// End-of-line delimiter format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Eol {
    /// Unix line ending (`\n`).
    #[default]
    Lf,
    /// Windows line ending (`\r\n`).
    CrLf,
    /// Classic Mac line ending (`\r`).
    Cr,
}

impl Eol {
    /// Returns the character sequence of this EOL format.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        }
    }

    /// Returns a short human-readable label for UI status indicators.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::CrLf => "CRLF",
            Self::Cr => "CR",
        }
    }
}

/// Language identifier used for syntax highlighting and file mode detection.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LanguageId(pub String);

impl Default for LanguageId {
    fn default() -> Self {
        Self("Plain Text".to_string())
    }
}

impl LanguageId {
    /// Creates a new language identifier.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the language identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A contiguous range in character indices within a buffer.
///
/// An empty range (`anchor == head`) represents a point caret.
/// The `anchor` is where selection started; `head` is where the active cursor is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Range {
    /// The fixed origin of the selection.
    pub anchor: usize,
    /// The moving head of the selection (the caret).
    pub head: usize,
}

impl Range {
    /// Creates a point caret at `pos`.
    #[must_use]
    pub const fn point(pos: usize) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    /// Returns the minimum character index of the range.
    #[must_use]
    pub fn start(&self) -> usize {
        self.anchor.min(self.head)
    }

    /// Returns the maximum character index of the range.
    #[must_use]
    pub fn end(&self) -> usize {
        self.anchor.max(self.head)
    }

    /// Returns `true` if the range is a zero-length caret.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Returns the length of the range in characters.
    #[must_use]
    pub fn len(&self) -> usize {
        self.end() - self.start()
    }
}

/// A multi-cursor selection containing one or more ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// The ordered, non-overlapping selection ranges.
    pub ranges: SmallVec<[Range; 1]>,
    /// Index of the primary cursor range within `ranges`.
    pub primary: usize,
}

impl Selection {
    /// Creates a selection with a single point caret.
    #[must_use]
    pub fn point(pos: usize) -> Self {
        Self {
            ranges: smallvec::smallvec![Range::point(pos)],
            primary: 0,
        }
    }

    /// Creates a selection with a single range.
    #[must_use]
    pub fn single(range: Range) -> Self {
        Self {
            ranges: smallvec::smallvec![range],
            primary: 0,
        }
    }

    /// Creates a selection by sorting and merging overlapping or adjacent ranges.
    ///
    /// Duplicate point ranges and overlapping/adjacent non-empty ranges are merged
    /// while preserving and tracking the primary selection range.
    #[must_use]
    pub fn new(ranges: impl IntoIterator<Item = Range>, primary: usize) -> Self {
        let mut raw: Vec<(Range, bool)> = ranges
            .into_iter()
            .enumerate()
            .map(|(i, r)| (r, i == primary))
            .collect();

        if raw.is_empty() {
            return Self::point(0);
        }

        // If primary was out of bounds, designate the last item as primary.
        if !raw.iter().any(|(_, is_prim)| *is_prim) {
            if let Some(last) = raw.last_mut() {
                last.1 = true;
            }
        }

        // Sort by start position, then by end position.
        raw.sort_by(|(a, _), (b, _)| {
            a.start()
                .cmp(&b.start())
                .then_with(|| a.end().cmp(&b.end()))
        });

        let mut merged: SmallVec<[(Range, bool); 1]> = SmallVec::new();

        for (curr, curr_is_primary) in raw {
            if let Some((prev, prev_is_primary)) = merged.last_mut() {
                let both_empty = prev.is_empty() && curr.is_empty();
                let overlaps_or_adjacent = if both_empty {
                    prev.start() == curr.start()
                } else {
                    curr.start() <= prev.end()
                };

                if overlaps_or_adjacent {
                    let is_backward = if curr_is_primary {
                        curr.head < curr.anchor
                    } else {
                        prev.head < prev.anchor
                    };
                    let new_start = prev.start().min(curr.start());
                    let new_end = prev.end().max(curr.end());
                    let (new_anchor, new_head) = if is_backward {
                        (new_end, new_start)
                    } else {
                        (new_start, new_end)
                    };

                    *prev = Range {
                        anchor: new_anchor,
                        head: new_head,
                    };
                    *prev_is_primary = *prev_is_primary || curr_is_primary;
                    continue;
                }
            }

            merged.push((curr, curr_is_primary));
        }

        let primary_idx = merged.iter().position(|(_, p)| *p).unwrap_or(0);
        let final_ranges = merged.into_iter().map(|(r, _)| r).collect();

        Self {
            ranges: final_ranges,
            primary: primary_idx,
        }
    }

    /// Returns the primary selection range.
    #[must_use]
    pub fn primary_range(&self) -> Range {
        self.ranges
            .get(self.primary)
            .copied()
            .unwrap_or_else(|| self.ranges.first().copied().unwrap_or(Range::point(0)))
    }

    /// Clamps all selection ranges to be within `[0, max_char]`.
    #[must_use]
    pub fn clamp(&self, max_char: usize) -> Self {
        let clamped: SmallVec<[Range; 1]> = self
            .ranges
            .iter()
            .map(|r| Range {
                anchor: r.anchor.min(max_char),
                head: r.head.min(max_char),
            })
            .collect();
        Self::new(clamped, self.primary)
    }
}

/// Semantic kind of edit performed on a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EditKind {
    /// Single-character insertion (e.g. typing a character).
    InsertChar,
    /// Bulk insertion (e.g. paste, snippet, auto-indent).
    InsertBulk,
    /// Deletion (forward or backward).
    Delete,
    /// Structural edit (line move, duplicate, indent/outdent).
    Structural,
}

/// An atomic text replacement within a buffer range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    /// Starting character index of text replaced.
    pub start: usize,
    /// Ending character index of text replaced.
    pub end: usize,
    /// New replacement text inserted in place of `start..end`.
    pub text: String,
}

/// An atomic transaction representing an edit and the selection state transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// Atomic changes applied in this transaction.
    pub changes: Vec<Change>,
    /// Selection before changes were applied.
    pub before: Selection,
    /// Selection after changes were applied.
    pub after: Selection,
    /// Monotonic wall-clock timestamp in milliseconds when transaction was created.
    pub timestamp_ms: u64,
    /// Semantic kind of edit for grouping heuristics.
    pub kind: EditKind,
}
