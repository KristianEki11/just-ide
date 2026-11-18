//! Editor match highlights and 10 px scrollbar tick markers.

use crate::find_widget::FindState;
use crate::theme::ThemeColors;
use egui::{pos2, vec2, Color32, FontId, Galley, Painter, Rect, Stroke};
use jide_core::{Document, Selection};
use std::collections::BTreeSet;
use std::sync::Arc;

/// Line geometry & offset context for painting char spans on one editor line.
#[derive(Clone, Copy)]
pub struct LineSpanCtx<'a> {
    pub doc: &'a Document,
    pub line_idx: usize,
    pub galley: &'a Arc<Galley>,
    pub origin: egui::Pos2,
    pub line_start_char: usize,
    pub line_char_len: usize,
    pub cell: f32,
}

/// Paints a character range `[range_start..range_end]` across wrapped/unwrapped rows of one line.
pub fn paint_line_char_span(
    text_painter: &Painter,
    line: &LineSpanCtx<'_>,
    range_start: usize,
    range_end: usize,
    fill: Color32,
) {
    let line_end = line.line_start_char + line.line_char_len;
    if range_start >= line_end || range_end <= line.line_start_char {
        return;
    }
    let raw_from = range_start.saturating_sub(line.line_start_char);
    let raw_to = range_end.saturating_sub(line.line_start_char);
    let galley_chars = line.galley.text().chars().count();
    let from = super::cache::raw_char_to_expanded_char(line.doc, line.line_idx, raw_from)
        .min(galley_chars);
    let to =
        super::cache::raw_char_to_expanded_char(line.doc, line.line_idx, raw_to).min(galley_chars);
    let a = line.galley.pos_from_cursor(egui::text::CCursor::new(from));
    let b = line.galley.pos_from_cursor(egui::text::CCursor::new(to));
    for row in &line.galley.rows {
        let row_rect = row.rect().translate(line.origin.to_vec2());
        if row_rect.bottom() >= line.origin.y + a.top() && row_rect.top() <= line.origin.y + b.top()
        {
            let left = if (row_rect.top() - line.origin.y - a.top()).abs() < 1.0 {
                line.origin.x + a.left()
            } else {
                line.origin.x
            };
            let right = if (row_rect.top() - line.origin.y - b.top()).abs() < 1.0 {
                line.origin.x + b.left()
            } else {
                row_rect.right().max(line.origin.x + line.cell)
            };
            text_painter.rect_filled(
                Rect::from_min_max(pos2(left, row_rect.top()), pos2(right, row_rect.bottom())),
                0.0,
                fill,
            );
        }
    }
}

/// Paints all in-buffer search matches overlapping `line`.
pub fn paint_line_find_matches(
    text_painter: &Painter,
    find: &FindState,
    line: &LineSpanCtx<'_>,
    colors: &ThemeColors,
) {
    if !find.open || find.matches.is_empty() {
        return;
    }
    let line_end = line.line_start_char + line.line_char_len;
    let first = find
        .matches
        .partition_point(|m| m.end_char <= line.line_start_char);
    for (offset, mat) in find.matches[first..].iter().enumerate() {
        if mat.start_char >= line_end {
            break;
        }
        let idx = first + offset;
        let fill = if idx == find.active_match {
            colors.editor_match_current
        } else {
            colors.editor_match
        };
        paint_line_char_span(text_painter, line, mat.start_char, mat.end_char, fill);
    }
}

/// Paints pixel-quantized search match and primary cursor ticks on the 10 px scrollbar track.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn paint_scrollbar_ticks(
    painter: &Painter,
    track: Rect,
    doc: &Document,
    find: &FindState,
    colors: &ThemeColors,
) {
    let total_lines = doc.line_count().max(1) as f32;
    let doc_chars = doc.char_count();

    if find.open && !find.matches.is_empty() {
        let mut buckets: BTreeSet<i32> = BTreeSet::new();
        for mat in &find.matches {
            let line = doc.rope.char_to_line(mat.start_char.min(doc_chars));
            let y_px = (track.top() + (line as f32 / total_lines) * track.height()).round() as i32;
            buckets.insert(y_px);
        }
        for y_px in buckets {
            painter.rect_filled(
                Rect::from_min_size(pos2(track.left() + 1.0, y_px as f32), vec2(8.0, 2.0)),
                0.0,
                colors.editor_match,
            );
        }
        if let Some(active) = find.matches.get(find.active_match) {
            let line = doc.rope.char_to_line(active.start_char.min(doc_chars));
            let y = track.top() + (line as f32 / total_lines) * track.height();
            painter.rect_filled(
                Rect::from_min_size(pos2(track.left(), y), vec2(track.width(), 3.0)),
                0.0,
                colors.editor_match_current,
            );
        }
    }

    let cursor_line = doc
        .rope
        .char_to_line(doc.selection.primary_range().head.min(doc_chars));
    let cy = track.top() + (cursor_line as f32 / total_lines) * track.height();
    painter.rect_filled(
        Rect::from_min_size(pos2(track.left(), cy), vec2(track.width(), 2.0)),
        0.0,
        colors.editor_cursor,
    );
}

/// Draws vertical indentation guides across leading whitespace columns of one line.
#[allow(clippy::cast_precision_loss, clippy::too_many_arguments)]
pub fn paint_indent_guides(
    text_painter: &Painter,
    doc: &Document,
    line: usize,
    origin: egui::Pos2,
    height: f32,
    cell: f32,
    active_col: usize,
    colors: &ThemeColors,
) {
    let tab_size = doc.tab_size.max(1);
    let mut cols = 0usize;
    for ch in doc.rope.line(line).chars() {
        match ch {
            ' ' => cols += 1,
            '\t' => cols += tab_size - (cols % tab_size),
            _ => break,
        }
    }
    let mut col = tab_size;
    while col <= cols {
        let x = origin.x + (col as f32) * cell;
        let color = if col <= active_col && active_col > 0 && active_col - col < tab_size {
            colors.editor_indent_guide_active
        } else {
            colors.editor_indent_guide
        };
        text_painter.line_segment(
            [pos2(x, origin.y), pos2(x, origin.y + height)],
            Stroke::new(1.0, color),
        );
        col += tab_size;
    }
}

/// Draws subtle visible whitespace markers (`·` for space, `→` for tab).
#[allow(clippy::cast_precision_loss, clippy::too_many_arguments)]
pub fn paint_whitespace(
    text_painter: &Painter,
    doc: &Document,
    line: usize,
    origin: egui::Pos2,
    font: f32,
    height: f32,
    cell: f32,
    selection: &Selection,
    mode: &str,
    colors: &ThemeColors,
) {
    if mode == "none" {
        return;
    }
    let only_selection = mode == "selection";
    let line_start = doc.rope.line_to_char(line);
    let mut col = 0usize;
    for (offset, ch) in doc.rope.line(line).chars().enumerate() {
        if ch == '\n' || ch == '\r' {
            break;
        }
        let char_idx = line_start + offset;
        let visible = if only_selection {
            selection
                .ranges
                .iter()
                .any(|r| !r.is_empty() && char_idx >= r.start() && char_idx < r.end())
        } else {
            true
        };
        match ch {
            ' ' => {
                if visible {
                    text_painter.text(
                        pos2(
                            origin.x + (col as f32 + 0.5) * cell,
                            origin.y + height * 0.5,
                        ),
                        egui::Align2::CENTER_CENTER,
                        "·",
                        FontId::monospace(font),
                        colors.editor_whitespace,
                    );
                }
                col += 1;
            }
            '\t' => {
                let width = doc.tab_size.max(1) - (col % doc.tab_size.max(1));
                if visible {
                    text_painter.text(
                        pos2(
                            origin.x + (col as f32 + 0.5) * cell,
                            origin.y + height * 0.5,
                        ),
                        egui::Align2::CENTER_CENTER,
                        "→",
                        FontId::monospace(font),
                        colors.editor_whitespace,
                    );
                }
                col += width;
            }
            _ => col += 1,
        }
    }
}
