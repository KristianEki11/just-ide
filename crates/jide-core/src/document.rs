//! In-memory document buffer with rope, encoding metadata, selection, and history.

use std::borrow::Cow;
use std::path::PathBuf;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::history::History;
use crate::types::{DocId, Eol, LanguageId, Selection};

/// In-memory representation of an open text file or untitled buffer.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct Document {
    /// Unique document identifier in this session.
    pub id: DocId,
    /// High-performance rope buffer containing text.
    pub rope: ropey::Rope,
    /// Absolute filesystem path if saved to or loaded from disk.
    pub path: Option<PathBuf>,
    /// Character encoding used for file I/O.
    pub encoding: &'static encoding_rs::Encoding,
    /// Whether the file began with a Byte Order Mark (BOM).
    pub has_bom: bool,
    /// Preferred line ending convention.
    pub eol: Eol,
    /// Language identifier for syntax highlighting and mode settings.
    pub language: LanguageId,
    /// Active multi-cursor selection.
    pub selection: Selection,
    /// Undo and redo history for this document.
    pub history: History,
    /// Monotonically increasing version bumped on every edit.
    pub version: u64,
    /// Document version at last successful disk save.
    pub saved_version: u64,
    /// Flag indicating large file mode (>50 MB or >1,000,000 lines).
    pub large_file: bool,
    /// Flag indicating binary content was detected on load.
    pub is_binary: bool,
    /// Read-only flag preventing edit operations.
    pub read_only: bool,
    /// Number of spaces per tab stop (default 4).
    pub tab_size: usize,
    /// Whether pressing Tab inserts spaces instead of `\t` (default true).
    pub insert_spaces: bool,
    /// Cached visual column for vertical cursor navigation across lines.
    pub preferred_col: Option<usize>,
}

impl Document {
    /// Creates a new untitled document with default settings.
    #[must_use]
    pub fn new_untitled(id: DocId) -> Self {
        Self {
            id,
            rope: ropey::Rope::new(),
            path: None,
            encoding: encoding_rs::UTF_8,
            has_bom: false,
            eol: Eol::Lf,
            language: LanguageId::default(),
            selection: Selection::point(0),
            history: History::new(),
            version: 0,
            saved_version: 0,
            large_file: false,
            is_binary: false,
            read_only: false,
            tab_size: 4,
            insert_spaces: true,
            preferred_col: None,
        }
    }

    /// Creates a document from an existing text string.
    #[must_use]
    pub fn from_text(id: DocId, text: &str) -> Self {
        let rope = ropey::Rope::from_str(text);
        Self {
            id,
            rope,
            path: None,
            encoding: encoding_rs::UTF_8,
            has_bom: false,
            eol: Eol::Lf,
            language: LanguageId::default(),
            selection: Selection::point(0),
            history: History::new(),
            version: 0,
            saved_version: 0,
            large_file: false,
            is_binary: false,
            read_only: false,
            tab_size: 4,
            insert_spaces: true,
            preferred_col: None,
        }
    }

    /// Returns `true` if the document has modifications since the last save.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.version != self.saved_version
    }

    /// Returns the number of lines in the buffer.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Returns the total number of Unicode scalar values (characters) in the buffer.
    #[must_use]
    pub fn char_count(&self) -> usize {
        self.rope.len_chars()
    }

    /// Returns 1-based (line, `visual_column`) for a character index in the buffer.
    ///
    /// The visual column accounts for tab expansion via `self.tab_size` and wide
    /// characters via Unicode width calculations.
    #[must_use]
    pub fn line_col_of(&self, char_idx: usize) -> (usize, usize) {
        let clamped = char_idx.min(self.rope.len_chars());
        let line_idx = self.rope.char_to_line(clamped);
        let line_start = self.rope.line_to_char(line_idx);

        let mut visual_col = 0usize;
        let line_slice = self.rope.line(line_idx);
        let count = clamped.saturating_sub(line_start);
        let tab = self.tab_size.max(1);

        // Borrow contiguous lines; copy only this line when it spans rope chunks.
        let text: Cow<'_, str> = line_slice.into();
        let mut consumed = 0;
        for grapheme in text.graphemes(true) {
            consumed += grapheme.chars().count();
            // An index inside a cluster maps to its leading visual column.
            if consumed > count || matches!(grapheme, "\r" | "\n" | "\r\n") {
                break;
            }
            visual_col += if grapheme == "\t" {
                tab - visual_col % tab
            } else {
                UnicodeWidthStr::width(grapheme)
            };
        }

        (line_idx + 1, visual_col + 1)
    }

    /// Converts a 0-based line and 0-based visual column to a character index.
    ///
    /// Returns the closest character index on the line without exceeding the
    /// requested visual column, stopping before any line terminator.
    #[must_use]
    pub fn char_idx_of_line_col(&self, line_0based: usize, visual_col_0based: usize) -> usize {
        if self.rope.len_lines() == 0 {
            return 0;
        }

        let line_idx = line_0based.min(self.rope.len_lines().saturating_sub(1));
        let line_start = self.rope.line_to_char(line_idx);
        let line_slice = self.rope.line(line_idx);
        let tab = self.tab_size.max(1);

        let mut curr_char_idx = line_start;
        let mut curr_vis_col = 0usize;

        let text: Cow<'_, str> = line_slice.into();
        for grapheme in text.graphemes(true) {
            if matches!(grapheme, "\r" | "\n" | "\r\n") {
                break;
            }

            let grapheme_width = if grapheme == "\t" {
                tab - curr_vis_col % tab
            } else {
                UnicodeWidthStr::width(grapheme)
            };

            if curr_vis_col + grapheme_width > visual_col_0based {
                break;
            }

            curr_vis_col += grapheme_width;
            curr_char_idx += grapheme.chars().count();

            if curr_vis_col == visual_col_0based {
                break;
            }
        }

        curr_char_idx
    }

    /// Returns the text currently selected by active cursor ranges.
    ///
    /// For multiple selections, the selected snippets are joined with newline characters.
    #[must_use]
    pub fn selected_text(&self) -> String {
        if self.selection.ranges.len() <= 1 {
            let r = self.selection.primary_range();
            if r.is_empty() {
                return String::new();
            }
            return self.rope.slice(r.start()..r.end()).to_string();
        }

        let non_empty: Vec<String> = self
            .selection
            .ranges
            .iter()
            .filter(|r| !r.is_empty())
            .map(|r| self.rope.slice(r.start()..r.end()).to_string())
            .collect();

        if non_empty.is_empty() {
            String::new()
        } else {
            non_empty.join("\n")
        }
    }

    /// Returns a human-friendly string for the encoding and BOM configuration.
    #[must_use]
    pub fn display_encoding(&self) -> &'static str {
        if self.encoding == encoding_rs::UTF_8 {
            if self.has_bom {
                "UTF-8 BOM"
            } else {
                "UTF-8"
            }
        } else if self.encoding == encoding_rs::UTF_16LE {
            if self.has_bom {
                "UTF-16 LE BOM"
            } else {
                "UTF-16 LE"
            }
        } else if self.encoding == encoding_rs::UTF_16BE {
            if self.has_bom {
                "UTF-16 BE BOM"
            } else {
                "UTF-16 BE"
            }
        } else if self.encoding == encoding_rs::WINDOWS_1252 {
            "Windows-1252"
        } else if self.encoding == encoding_rs::SHIFT_JIS {
            "Shift_JIS"
        } else if self.encoding == encoding_rs::GBK {
            "GBK"
        } else {
            self.encoding.name()
        }
    }
}
