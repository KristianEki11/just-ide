//! Built-in offline Prettier code formatter for Just IDE.
//!
//! Pure-Rust formatting across P0 languages respecting document `tab_size` and
//! `insert_spaces` without external runtimes or network calls (H1–H3).

use crate::document::Document;
use crate::error::CoreError;
use crate::lines::split_delimiter;
use crate::types::{Change, EditKind, Range, Selection};

/// Formats `text` according to `language_id`, `tab_size`, and `insert_spaces`.
#[must_use]
pub fn format_code(text: &str, language_id: &str, tab_size: usize, insert_spaces: bool) -> String {
    let eol = detect_dominant_eol(text);
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let indent_unit = if insert_spaces {
        " ".repeat(tab_size.max(1))
    } else {
        "\t".to_owned()
    };

    let mut out = match language_id.to_ascii_lowercase().as_str() {
        "json" | "jsonc" => format_json_like(&normalized, &indent_unit),
        "toml" => format_toml(&normalized, &indent_unit),
        "html" | "xml" | "svg" => format_markup(&normalized, &indent_unit),
        "css" | "scss" | "less" => format_css(&normalized, &indent_unit),
        "javascript" | "typescript" | "jsx" | "tsx" | "rust" | "c" | "cpp" | "c++" | "csharp"
        | "go" | "java" | "kotlin" | "swift" | "zig" | "php" | "dart" | "scala" => {
            format_c_style(&normalized, &indent_unit, tab_size)
        }
        _ => format_line_oriented(&normalized, tab_size, insert_spaces),
    };

    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if eol == "\r\n" {
        out.replace('\n', "\r\n")
    } else {
        out
    }
}

impl Document {
    /// Formats the entire document as a single undoable edit. Returns `true` if modified.
    pub fn format_document(&mut self, now_ms: u64) -> Result<bool, CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        let current = self.rope.to_string();
        let formatted = format_code(
            &current,
            self.language.as_str(),
            self.tab_size,
            self.insert_spaces,
        );
        if formatted == current {
            return Ok(false);
        }
        let (line, col) = self.line_col_of(self.selection.primary_range().head);
        let old_len = self.rope.len_chars();
        self.apply_changes(
            vec![Change {
                start: 0,
                end: old_len,
                text: formatted,
            }],
            Selection::point(0),
            EditKind::Structural,
            now_ms,
        )?;
        let clamped = self.char_idx_of_line_col(line.saturating_sub(1), col.saturating_sub(1));
        self.selection = Selection::point(clamped);
        Ok(true)
    }

    /// Formats the selected lines (or full document when selection is empty) as one undo step.
    pub fn format_selection(&mut self, now_ms: u64) -> Result<bool, CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }
        let primary = self.selection.primary_range();
        if primary.is_empty() {
            return self.format_document(now_ms);
        }

        let first_line = self.rope.char_to_line(primary.start());
        let last_line = self
            .rope
            .char_to_line(primary.end().saturating_sub(1).max(primary.start()));
        let start_char = self.rope.line_to_char(first_line);
        let end_char = self.line_end(last_line);
        let snippet = self.rope.slice(start_char..end_char).to_string();

        let formatted = format_code(
            &snippet,
            self.language.as_str(),
            self.tab_size,
            self.insert_spaces,
        );
        let (snippet_body, orig_delim) = split_delimiter(&snippet);
        if snippet_body.trim().is_empty() {
            return Ok(false);
        }
        let mut fmt_body = formatted.trim_end_matches(['\n', '\r']).to_owned();
        if !orig_delim.is_empty() {
            fmt_body.push_str(orig_delim);
        }
        let base_indent: String = snippet
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        let reindented = apply_base_indent(&fmt_body, &base_indent);
        if reindented == snippet {
            return Ok(false);
        }
        let new_len = reindented.chars().count();
        self.apply_changes(
            vec![Change {
                start: start_char,
                end: end_char,
                text: reindented,
            }],
            Selection::single(Range {
                anchor: start_char,
                head: start_char + new_len,
            }),
            EditKind::Structural,
            now_ms,
        )?;
        Ok(true)
    }
}

fn apply_base_indent(block: &str, base: &str) -> String {
    if base.is_empty() {
        return block.to_owned();
    }
    let mut out = String::with_capacity(block.len() + base.len() * 4);
    for (idx, part) in block.split_inclusive('\n').enumerate() {
        let trimmed = part.trim_end_matches(['\n', '\r']);
        if trimmed.is_empty() {
            out.push_str(part);
        } else {
            let _ = idx;
            out.push_str(base);
            out.push_str(part);
        }
    }
    out
}

fn detect_dominant_eol(text: &str) -> &'static str {
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count().saturating_sub(crlf);
    if crlf > lf {
        "\r\n"
    } else {
        "\n"
    }
}

fn push_indent(out: &mut String, depth: usize, unit: &str) {
    for _ in 0..depth {
        out.push_str(unit);
    }
}

fn format_json_like(src: &str, unit: &str) -> String {
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    let mut out = String::with_capacity(src.len() + 64);
    let mut depth = 0usize;
    let mut at_line_start = true;

    while i < len {
        let ch = src[i..].chars().next().unwrap_or('\0');
        let clen = ch.len_utf8();

        if ch.is_ascii_whitespace() {
            i += clen;
            continue;
        }

        if ch == '"' {
            if at_line_start {
                push_indent(&mut out, depth, unit);
                at_line_start = false;
            }
            let end = scan_quoted_end(src, i, '"');
            out.push_str(&src[i..end]);
            i = end;
            continue;
        }

        if ch == '/' && i + 1 < len && (bytes[i + 1] == b'/' || bytes[i + 1] == b'*') {
            if !at_line_start && !out.ends_with(' ') && !out.ends_with('\n') {
                out.push(' ');
            } else if at_line_start {
                push_indent(&mut out, depth, unit);
            }
            let end = if bytes[i + 1] == b'/' {
                src[i..].find('\n').map_or(len, |rel| i + rel)
            } else {
                src[i + 2..].find("*/").map_or(len, |rel| i + 2 + rel + 2)
            };
            out.push_str(src[i..end].trim_end());
            out.push('\n');
            at_line_start = true;
            i = end;
            continue;
        }

        match ch {
            '{' | '[' => {
                if at_line_start {
                    push_indent(&mut out, depth, unit);
                }
                out.push(ch);
                let next_sig = next_non_ws_char(src, i + clen);
                let close = if ch == '{' { '}' } else { ']' };
                if next_sig == Some(close) {
                    out.push(close);
                    i = advance_past_char(src, i + clen, close);
                    at_line_start = false;
                } else {
                    depth += 1;
                    out.push('\n');
                    at_line_start = true;
                    i += clen;
                }
            }
            '}' | ']' => {
                depth = depth.saturating_sub(1);
                if !at_line_start {
                    out.push('\n');
                }
                push_indent(&mut out, depth, unit);
                out.push(ch);
                at_line_start = false;
                i += clen;
            }
            ',' => {
                out.push(',');
                out.push('\n');
                at_line_start = true;
                i += clen;
            }
            ':' => {
                out.push_str(": ");
                at_line_start = false;
                i += clen;
            }
            _ => {
                if at_line_start {
                    push_indent(&mut out, depth, unit);
                    at_line_start = false;
                }
                out.push(ch);
                i += clen;
            }
        }
    }

    cleanup_blank_lines(&out)
}

fn format_toml(src: &str, unit: &str) -> String {
    let mut out = String::with_capacity(src.len() + 32);
    let mut bracket_depth = 0usize;

    for raw_line in src.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }
        if trimmed.starts_with('#') {
            push_indent(&mut out, bracket_depth, unit);
            out.push_str(trimmed);
            out.push('\n');
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') && !trimmed.contains('=') {
            bracket_depth = 0;
            if !out.is_empty() && !out.ends_with("\n\n") {
                out.push('\n');
            }
            out.push_str(trimmed);
            out.push('\n');
            continue;
        }
        let closing_prefix = trimmed.starts_with(']') || trimmed.starts_with('}');
        let indent = if closing_prefix {
            bracket_depth.saturating_sub(1)
        } else {
            bracket_depth
        };
        push_indent(&mut out, indent, unit);
        out.push_str(&normalize_toml_assign(trimmed));
        out.push('\n');

        let (opens, closes) = count_brackets_outside_quotes(trimmed, '#');
        bracket_depth = bracket_depth.saturating_add(opens).saturating_sub(closes);
    }

    cleanup_blank_lines(&out)
}

fn normalize_toml_assign(line: &str) -> String {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_double {
            escaped = true;
            continue;
        }
        if ch == '"' && !in_single {
            in_double = !in_double;
        } else if ch == '\'' && !in_double {
            in_single = !in_single;
        } else if !in_single && !in_double {
            if ch == '#' {
                break;
            }
            if ch == '=' {
                let left = line[..idx].trim_end();
                let right = line[idx + 1..].trim_start();
                return format!("{left} = {right}");
            }
        }
    }
    line.to_owned()
}

fn format_css(src: &str, unit: &str) -> String {
    let mut out = String::with_capacity(src.len() + 32);
    let mut depth = 0usize;
    let mut buf = String::new();
    let bytes = src.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        let ch = src[i..].chars().next().unwrap_or('\0');
        let clen = ch.len_utf8();
        if ch == '"' || ch == '\'' {
            let end = scan_quoted_end(src, i, ch);
            buf.push_str(&src[i..end]);
            i = end;
            continue;
        }
        if ch == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            flush_css_chunk(&mut out, &buf, depth, unit);
            buf.clear();
            let end = src[i + 2..]
                .find("*/")
                .map_or(bytes.len(), |r| i + 2 + r + 2);
            push_indent(&mut out, depth, unit);
            out.push_str(src[i..end].trim());
            out.push('\n');
            i = end;
            continue;
        }
        match ch {
            '{' => {
                let selector = collapse_ws(buf.trim());
                buf.clear();
                push_indent(&mut out, depth, unit);
                out.push_str(&selector);
                if !selector.is_empty() {
                    out.push(' ');
                }
                out.push_str("{\n");
                depth += 1;
                i += clen;
            }
            '}' => {
                flush_css_chunk(&mut out, &buf, depth, unit);
                buf.clear();
                depth = depth.saturating_sub(1);
                push_indent(&mut out, depth, unit);
                out.push_str("}\n");
                if depth == 0 {
                    out.push('\n');
                }
                i += clen;
            }
            ';' => {
                let decl = format_css_decl(buf.trim());
                buf.clear();
                if !decl.is_empty() {
                    push_indent(&mut out, depth, unit);
                    out.push_str(&decl);
                    out.push_str(";\n");
                }
                i += clen;
            }
            _ => {
                buf.push(ch);
                i += clen;
            }
        }
    }
    flush_css_chunk(&mut out, &buf, depth, unit);
    cleanup_blank_lines(&out)
}

fn flush_css_chunk(out: &mut String, chunk: &str, depth: usize, unit: &str) {
    let trimmed = chunk.trim();
    if !trimmed.is_empty() {
        push_indent(out, depth, unit);
        out.push_str(&format_css_decl(trimmed));
        out.push('\n');
    }
}

fn format_css_decl(decl: &str) -> String {
    if let Some((prop, val)) = decl.split_once(':') {
        if !prop.contains(' ') && !prop.starts_with('@') {
            return format!("{}: {}", prop.trim(), collapse_ws(val.trim()));
        }
    }
    collapse_ws(decl)
}

fn format_markup(src: &str, unit: &str) -> String {
    let mut out = String::with_capacity(src.len() + 32);
    let mut depth = 0usize;
    let mut i = 0usize;
    let len = src.len();
    let mut raw_end_tag: Option<&'static str> = None;

    while i < len {
        if let Some(end_tag) = raw_end_tag {
            let lower = src[i..].to_ascii_lowercase();
            if let Some(pos) = lower.find(end_tag) {
                let inner = &src[i..i + pos];
                for l in inner.lines() {
                    let t = l.trim_end();
                    if !t.is_empty() {
                        push_indent(&mut out, depth, unit);
                        out.push_str(t.trim_start());
                        out.push('\n');
                    }
                }
                depth = depth.saturating_sub(1);
                push_indent(&mut out, depth, unit);
                out.push_str(&src[i + pos..i + pos + end_tag.len()]);
                out.push('\n');
                raw_end_tag = None;
                i += pos + end_tag.len();
                continue;
            }
        }

        let rest = &src[i..];
        if let Some(lt_rel) = rest.find('<') {
            let text_chunk = collapse_ws(rest[..lt_rel].trim());
            if !text_chunk.is_empty() {
                push_indent(&mut out, depth, unit);
                out.push_str(&text_chunk);
                out.push('\n');
            }
            let tag_start = i + lt_rel;
            let tag_end = if src[tag_start..].starts_with("<!--") {
                src[tag_start + 4..]
                    .find("-->")
                    .map_or(len, |r| tag_start + 4 + r + 3)
            } else {
                src[tag_start..]
                    .find('>')
                    .map_or(len, |r| tag_start + r + 1)
            };
            let tag = src[tag_start..tag_end].trim();
            if tag.starts_with("</") {
                depth = depth.saturating_sub(1);
                push_indent(&mut out, depth, unit);
                out.push_str(tag);
                out.push('\n');
            } else if tag.starts_with("<!")
                || tag.starts_with("<?")
                || tag.ends_with("/>")
                || is_void_html_tag(tag)
            {
                push_indent(&mut out, depth, unit);
                out.push_str(tag);
                out.push('\n');
            } else {
                push_indent(&mut out, depth, unit);
                out.push_str(tag);
                out.push('\n');
                depth += 1;
                let tag_name = extract_tag_name(tag);
                raw_end_tag = match tag_name.as_str() {
                    "script" => Some("</script>"),
                    "style" => Some("</style>"),
                    "pre" => Some("</pre>"),
                    _ => None,
                };
            }
            i = tag_end;
        } else {
            let tail = collapse_ws(rest.trim());
            if !tail.is_empty() {
                push_indent(&mut out, depth, unit);
                out.push_str(&tail);
                out.push('\n');
            }
            break;
        }
    }

    cleanup_blank_lines(&out)
}

fn extract_tag_name(tag: &str) -> String {
    tag.trim_start_matches('<')
        .split(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn is_void_html_tag(tag: &str) -> bool {
    matches!(
        extract_tag_name(tag).as_str(),
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn format_c_style(src: &str, unit: &str, tab_size: usize) -> String {
    let expanded = expand_single_line_braces(src);
    let mut out = String::with_capacity(expanded.len() + 32);
    let mut depth = 0usize;
    let mut in_block_comment = false;

    for raw_line in expanded.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }

        if in_block_comment {
            push_indent(&mut out, depth, unit);
            out.push_str(trimmed);
            out.push('\n');
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }

        let leading_closes = trimmed
            .chars()
            .take_while(|c| matches!(c, '}' | ']' | ')'))
            .count();
        let line_depth = depth.saturating_sub(leading_closes);
        push_indent(&mut out, line_depth, unit);

        let cleaned = normalize_c_line_spacing(trimmed);
        out.push_str(&cleaned);
        out.push('\n');

        let (opens, closes, starts_block_comment) = scan_line_nesting(&cleaned, tab_size);
        in_block_comment = starts_block_comment;
        depth = depth.saturating_add(opens).saturating_sub(closes);
    }

    cleanup_blank_lines(&out)
}

fn expand_single_line_braces(src: &str) -> String {
    let mut out = String::with_capacity(src.len() + 32);
    for raw_line in src.lines() {
        let trimmed = raw_line.trim();
        if trimmed.contains('{')
            && trimmed.contains(';')
            && !trimmed.starts_with("//")
            && !trimmed.starts_with("/*")
        {
            let mut in_str = None::<char>;
            let mut escaped = false;
            let mut cur = String::new();
            for ch in trimmed.chars() {
                if escaped {
                    cur.push(ch);
                    escaped = false;
                    continue;
                }
                if let Some(q) = in_str {
                    cur.push(ch);
                    if ch == '\\' {
                        escaped = true;
                    } else if ch == q {
                        in_str = None;
                    }
                    continue;
                }
                match ch {
                    '"' | '\'' | '`' => {
                        in_str = Some(ch);
                        cur.push(ch);
                    }
                    '{' => {
                        cur.push('{');
                        out.push_str(cur.trim_end());
                        out.push('\n');
                        cur.clear();
                    }
                    ';' => {
                        cur.push(';');
                        out.push_str(cur.trim());
                        out.push('\n');
                        cur.clear();
                    }
                    '}' => {
                        if !cur.trim().is_empty() {
                            out.push_str(cur.trim());
                            out.push('\n');
                            cur.clear();
                        }
                        cur.push('}');
                    }
                    _ => cur.push(ch),
                }
            }
            if !cur.trim().is_empty() {
                out.push_str(cur.trim());
                out.push('\n');
            }
        } else {
            out.push_str(raw_line);
            out.push('\n');
        }
    }
    out
}

fn normalize_c_line_spacing(line: &str) -> String {
    if line.starts_with("//") || line.starts_with("/*") || line.starts_with('*') {
        return line.to_owned();
    }
    let mut out = String::with_capacity(line.len() + 8);
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0usize;
    let mut in_quote = None::<char>;
    let mut escaped = false;

    while i < chars.len() {
        let ch = chars[i];
        if escaped {
            out.push(ch);
            escaped = false;
            i += 1;
            continue;
        }
        if let Some(q) = in_quote {
            out.push(ch);
            if ch == '\\' {
                escaped = true;
            } else if ch == q {
                in_quote = None;
            }
            i += 1;
            continue;
        }
        if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            for &rest_ch in &chars[i..] {
                out.push(rest_ch);
            }
            break;
        }
        if ch == '"' || ch == '\'' || ch == '`' {
            in_quote = Some(ch);
            out.push(ch);
            i += 1;
            continue;
        }
        if ch == ',' {
            out.push(',');
            if i + 1 < chars.len() && !matches!(chars[i + 1], ' ' | '\t' | ')' | ']' | '}') {
                out.push(' ');
            }
            i += 1;
            continue;
        }
        if ch == '{' && !out.is_empty() && !out.ends_with(' ') && !out.ends_with('(') {
            out.push(' ');
            out.push('{');
            i += 1;
            continue;
        }
        if ch == ' ' || ch == '\t' {
            if !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
            i += 1;
            continue;
        }
        out.push(ch);
        i += 1;
    }

    out.trim_end().to_owned()
}

fn scan_line_nesting(line: &str, _tab_size: usize) -> (usize, usize, bool) {
    let mut opens = 0usize;
    let mut closes = 0usize;
    let mut in_quote = None::<char>;
    let mut escaped = false;
    let mut block_comment = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0usize;

    while i < chars.len() {
        let ch = chars[i];
        if block_comment {
            if ch == '*' && i + 1 < chars.len() && chars[i + 1] == '/' {
                block_comment = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }
        if let Some(q) = in_quote {
            if ch == '\\' {
                escaped = true;
            } else if ch == q {
                in_quote = None;
            }
            i += 1;
            continue;
        }
        if ch == '/' && i + 1 < chars.len() {
            if chars[i + 1] == '/' {
                break;
            }
            if chars[i + 1] == '*' {
                block_comment = true;
                i += 2;
                continue;
            }
        }
        match ch {
            '"' | '`' => in_quote = Some(ch),
            '\'' => {
                // Only treat as char/string quote when not a Rust lifetime annotation.
                let next_is_alpha = i + 1 < chars.len() && chars[i + 1].is_alphabetic();
                let close_soon =
                    i + 2 < chars.len() && (chars[i + 2] == '\'' || chars[i + 1] == '\\');
                if !next_is_alpha || close_soon {
                    in_quote = Some('\'');
                }
            }
            '{' | '[' | '(' => opens += 1,
            '}' | ']' | ')' => closes += 1,
            _ => {}
        }
        i += 1;
    }

    (opens, closes, block_comment)
}

fn format_line_oriented(src: &str, tab_size: usize, insert_spaces: bool) -> String {
    let mut out = String::with_capacity(src.len());
    let tab_spaces = " ".repeat(tab_size.max(1));
    for raw_line in src.lines() {
        let trimmed = raw_line.trim_end_matches([' ', '\t']);
        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }
        let ws_len: usize = trimmed
            .bytes()
            .take_while(|b| *b == b' ' || *b == b'\t')
            .count();
        let leading = &trimmed[..ws_len];
        let body = &trimmed[ws_len..];
        if insert_spaces {
            out.push_str(&leading.replace('\t', &tab_spaces));
        } else {
            out.push_str(&leading.replace(&tab_spaces, "\t"));
        }
        out.push_str(body);
        out.push('\n');
    }
    cleanup_blank_lines(&out)
}

fn cleanup_blank_lines(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut blanks = 0usize;
    for line in src.lines() {
        let t = line.trim_end();
        if t.is_empty() {
            blanks += 1;
            if blanks <= 1 && !out.is_empty() {
                out.push('\n');
            }
        } else {
            blanks = 0;
            out.push_str(t);
            out.push('\n');
        }
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_ws && !out.is_empty() {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(ch);
            prev_ws = false;
        }
    }
    out.trim_end().to_owned()
}

fn scan_quoted_end(src: &str, start: usize, quote: char) -> usize {
    let mut escaped = false;
    let mut offset = start + quote.len_utf8();
    for ch in src[offset..].chars() {
        offset += ch.len_utf8();
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == quote {
            break;
        }
    }
    offset
}

fn next_non_ws_char(src: &str, start: usize) -> Option<char> {
    src[start..].chars().find(|c| !c.is_whitespace())
}

fn advance_past_char(src: &str, start: usize, target: char) -> usize {
    for (rel, ch) in src[start..].char_indices() {
        if ch == target {
            return start + rel + ch.len_utf8();
        }
    }
    src.len()
}

fn count_brackets_outside_quotes(line: &str, comment_char: char) -> (usize, usize) {
    let mut opens = 0usize;
    let mut closes = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    for ch in line.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_double {
            escaped = true;
            continue;
        }
        if ch == '"' && !in_single {
            in_double = !in_double;
        } else if ch == '\'' && !in_double {
            in_single = !in_single;
        } else if !in_single && !in_double {
            if ch == comment_char {
                break;
            }
            if ch == '[' || ch == '{' {
                opens += 1;
            } else if ch == ']' || ch == '}' {
                closes += 1;
            }
        }
    }
    (opens, closes)
}
