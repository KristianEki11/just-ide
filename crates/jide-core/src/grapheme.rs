//! Unicode grapheme cluster, word boundary, and line boundary calculations.

use unicode_segmentation::GraphemeCursor;

/// Checks if a character is considered a word character (`\w` or `_`).
#[must_use]
pub fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Computes previous grapheme cluster boundary from `char_idx` using `GraphemeCursor`.
#[must_use]
pub fn prev_grapheme_char_idx(rope: &ropey::Rope, char_idx: usize) -> usize {
    if char_idx == 0 || rope.len_chars() == 0 {
        return 0;
    }

    let clamped = char_idx.min(rope.len_chars());
    let line_idx = rope.char_to_line(clamped);
    let line_start_char = rope.line_to_char(line_idx);

    if clamped == line_start_char {
        let prev_line = line_idx.saturating_sub(1);
        let prev_line_start = rope.line_to_char(prev_line);
        let prev_line_slice = rope.line(prev_line);
        let prev_line_str = prev_line_slice.to_string();

        let byte_len = prev_line_str.len();
        let mut gc = GraphemeCursor::new(byte_len, byte_len, true);
        if let Ok(Some(prev_byte)) = gc.prev_boundary(&prev_line_str, 0) {
            let offset_chars = prev_line_str[..prev_byte].chars().count();
            return prev_line_start + offset_chars;
        }
        return prev_line_start;
    }

    let line_slice = rope.line(line_idx);
    let line_str = line_slice.to_string();
    let char_offset_in_line = clamped - line_start_char;

    let byte_offset = line_str
        .chars()
        .take(char_offset_in_line)
        .map(char::len_utf8)
        .sum::<usize>();

    let mut gc = GraphemeCursor::new(byte_offset, line_str.len(), true);
    if let Ok(Some(prev_byte)) = gc.prev_boundary(&line_str, 0) {
        let prev_chars = line_str[..prev_byte].chars().count();
        line_start_char + prev_chars
    } else {
        line_start_char
    }
}

/// Computes next grapheme cluster boundary from `char_idx` using `GraphemeCursor`.
#[must_use]
pub fn next_grapheme_char_idx(rope: &ropey::Rope, char_idx: usize) -> usize {
    let total_chars = rope.len_chars();
    if char_idx >= total_chars {
        return total_chars;
    }

    let line_idx = rope.char_to_line(char_idx);
    let line_start_char = rope.line_to_char(line_idx);
    let line_slice = rope.line(line_idx);
    let line_str = line_slice.to_string();
    let char_offset_in_line = char_idx - line_start_char;

    let byte_offset = line_str
        .chars()
        .take(char_offset_in_line)
        .map(char::len_utf8)
        .sum::<usize>();

    let mut gc = GraphemeCursor::new(byte_offset, line_str.len(), true);
    if let Ok(Some(next_byte)) = gc.next_boundary(&line_str, 0) {
        let next_chars = line_str[..next_byte].chars().count();
        line_start_char + next_chars
    } else {
        (char_idx + 1).min(total_chars)
    }
}

/// Finds previous word boundary before `char_idx`.
#[must_use]
pub fn prev_word_boundary(rope: &ropey::Rope, char_idx: usize) -> usize {
    if char_idx == 0 || rope.len_chars() == 0 {
        return 0;
    }

    let mut pos = char_idx.min(rope.len_chars());

    if pos > 0 {
        let ch = rope.char(pos - 1);
        if ch == '\n' {
            pos -= 1;
            if pos > 0 && rope.char(pos - 1) == '\r' {
                pos -= 1;
            }
            return pos;
        } else if ch == '\r' {
            return pos - 1;
        }
    }

    while pos > 0 {
        let ch = rope.char(pos - 1);
        if ch.is_whitespace() && ch != '\n' && ch != '\r' {
            pos -= 1;
        } else {
            break;
        }
    }

    if pos == 0 {
        return 0;
    }

    let ch = rope.char(pos - 1);
    if is_word_char(ch) {
        while pos > 0 && is_word_char(rope.char(pos - 1)) {
            pos -= 1;
        }
    } else if !ch.is_whitespace() {
        while pos > 0 {
            let prev = rope.char(pos - 1);
            if !is_word_char(prev) && !prev.is_whitespace() {
                pos -= 1;
            } else {
                break;
            }
        }
    }

    pos
}

/// Finds next word boundary after `char_idx`.
#[must_use]
pub fn next_word_boundary(rope: &ropey::Rope, char_idx: usize) -> usize {
    let total = rope.len_chars();
    if char_idx >= total {
        return total;
    }

    let mut pos = char_idx;

    let ch = rope.char(pos);
    if ch == '\r' {
        pos += 1;
        if pos < total && rope.char(pos) == '\n' {
            pos += 1;
        }
        return pos;
    } else if ch == '\n' {
        return pos + 1;
    }

    if is_word_char(ch) {
        while pos < total && is_word_char(rope.char(pos)) {
            pos += 1;
        }
    } else if !ch.is_whitespace() {
        while pos < total {
            let c = rope.char(pos);
            if !is_word_char(c) && !c.is_whitespace() {
                pos += 1;
            } else {
                break;
            }
        }
    }

    while pos < total {
        let c = rope.char(pos);
        if c.is_whitespace() && c != '\n' && c != '\r' {
            pos += 1;
        } else {
            break;
        }
    }

    pos
}

/// Toggles between first non-whitespace character on current line and column 0.
#[must_use]
pub fn smart_line_start(rope: &ropey::Rope, char_idx: usize) -> usize {
    if rope.len_chars() == 0 {
        return 0;
    }

    let clamped = char_idx.min(rope.len_chars());
    let line_idx = rope.char_to_line(clamped);
    let line_start = rope.line_to_char(line_idx);
    let line_slice = rope.line(line_idx);

    let mut first_non_ws = None;
    for (offset, ch) in line_slice.chars().enumerate() {
        if ch == '\r' || ch == '\n' {
            break;
        }
        if !ch.is_whitespace() {
            first_non_ws = Some(line_start + offset);
            break;
        }
    }

    match first_non_ws {
        Some(pos) if clamped == pos => line_start,
        Some(pos) => pos,
        None => line_start,
    }
}

/// Computes character index at end of line (before `\r` or `\n`).
#[must_use]
pub fn line_end(rope: &ropey::Rope, char_idx: usize) -> usize {
    if rope.len_chars() == 0 {
        return 0;
    }

    let clamped = char_idx.min(rope.len_chars());
    let line_idx = rope.char_to_line(clamped);
    let line_start = rope.line_to_char(line_idx);
    let line_slice = rope.line(line_idx);

    let mut line_end_offset = 0;
    for ch in line_slice.chars() {
        if ch == '\r' || ch == '\n' {
            break;
        }
        line_end_offset += 1;
    }

    line_start + line_end_offset
}
