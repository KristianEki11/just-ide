#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

pub mod cache;
mod input;
pub mod markers;
use crate::state::{AppState, Command, EditorCmd};
pub use cache::{build_line_galley, render_visible_lines_headless, GalleyCache, LineRequest};
use egui::{pos2, vec2, FontId, Rect, Sense, Stroke};
use jide_core::Document;
use jide_syntax::{HighlightCache, HighlightSpan};
use std::collections::HashMap;

/// Persistent rendering resources; document state remains in `AppState`.
#[derive(Default)]
pub struct EditorView {
    cache: GalleyCache,
    last_activity: f64,
    active: Option<jide_core::DocId>,
    cursor: Option<(jide_core::DocId, u64, usize)>,
    wrapped_heights: std::collections::BTreeMap<usize, f32>,
    layout_key: Option<(jide_core::DocId, u64, u32, u32)>,
    highlight_caches: HashMap<jide_core::DocId, jide_syntax::HighlightCache>,
    column_drag_anchor: Option<usize>,
}

/// Map a monospace viewport coordinate to a grapheme-aware document index.
#[must_use]
pub fn hit_test(doc: &Document, line: usize, x: f32, cell_width: f32) -> usize {
    doc.char_idx_of_line_col(line, (x.max(0.0) / cell_width.max(1.0)).round() as usize)
}

impl EditorView {
    /// Evict galley and syntax caches for closed or inactive background tabs.
    pub fn evict_inactive_docs(
        &mut self,
        active_ids: &[jide_core::DocId],
        retain_clock_window: u64,
    ) {
        self.cache
            .evict_inactive_docs(active_ids, retain_clock_window);
        self.highlight_caches
            .retain(|id, _| active_ids.contains(id));
    }

    /// Collect global keyboard commands when no editor buffer has focus (e.g. Empty State / Settings).
    #[must_use]
    pub fn idle_commands(ctx: &egui::Context, state: &AppState) -> Vec<Command> {
        input::commands(ctx, state, false, 1)
    }

    /// Draw the focused editor and collect commands without mutating document state.
    pub fn show(&mut self, ui: &mut egui::Ui, state: &AppState) -> Vec<Command> {
        self.show_for_doc(ui, state, state.focused_doc_idx(), None, true)
    }

    /// Draw a specific document slot (supporting split editor groups) and collect commands.
    #[allow(clippy::too_many_lines)]
    pub fn show_for_doc(
        &mut self,
        ui: &mut egui::Ui,
        state: &AppState,
        doc_idx: Option<usize>,
        group_idx: Option<usize>,
        is_focused_group: bool,
    ) -> Vec<Command> {
        let Some(doc) = doc_idx.and_then(|i| state.documents.get(i)) else {
            return if is_focused_group {
                input::commands(ui.ctx(), state, false, 1)
            } else {
                Vec::new()
            };
        };
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        if is_focused_group && self.active != Some(doc.id) {
            self.active = Some(doc.id);
            response.request_focus();
            self.last_activity = ui.input(|input| input.time);
        }
        let focused = is_focused_group
            && !state.settings_open
            && state.overlay.is_none()
            && response.has_focus()
            && ui.input(|input| input.focused);
        if focused {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                );
            });
        }
        let font = state.editor_font_size * state.zoom;
        let height = (font * state.editor_line_height.max(1.0)).round().max(1.0);
        let cell = ui.fonts_mut(|fonts| fonts.glyph_width(&FontId::monospace(font), ' '));
        let gutter = if state.line_numbers {
            16.0 + doc.line_count().to_string().len().max(2) as f32 * cell + 16.0
        } else {
            12.0
        };
        let text_rect = Rect::from_min_max(
            pos2(rect.left() + gutter, rect.top()),
            pos2(rect.right() - 12.0, rect.bottom()),
        );
        let mut commands = if is_focused_group {
            input::commands(
                ui.ctx(),
                state,
                focused,
                (rect.height() / height).max(1.0) as usize,
            )
        } else {
            Vec::new()
        };
        let mut scroll = state.scroll_y.min(
            (doc.line_count() as f32 * height + self.wrapped_heights.values().sum::<f32>()
                - height)
                .max(0.0),
        );
        let mut scroll_x = state.scroll_x;
        if response.hovered() {
            ui.input(|input| {
                for event in &input.events {
                    if let egui::Event::MouseWheel {
                        unit,
                        delta,
                        modifiers,
                        ..
                    } = event
                    {
                        if modifiers.command {
                            commands.push(if delta.y > 0.0 {
                                Command::ZoomIn
                            } else {
                                Command::ZoomOut
                            });
                        } else {
                            let scale = match unit {
                                egui::MouseWheelUnit::Line => height * 3.0,
                                egui::MouseWheelUnit::Page => rect.height(),
                                egui::MouseWheelUnit::Point => 1.0,
                            };
                            scroll = (scroll - delta.y * scale).max(0.0);
                            scroll_x = (scroll_x - delta.x * scale).max(0.0);
                        }
                    }
                }
            });
        }
        let word_wrap =
            state.word_wrap && (!doc.large_file || state.force_highlight.contains(&doc.id));
        if word_wrap {
            scroll_x = 0.0;
        }
        let layout_key = (
            doc.id,
            doc.version,
            font.to_bits(),
            if word_wrap {
                text_rect.width().to_bits()
            } else {
                0
            },
        );
        if self.layout_key != Some(layout_key) {
            self.wrapped_heights.clear();
            self.layout_key = Some(layout_key);
        }
        let cursor = (doc.id, doc.version, doc.selection.primary_range().head);
        if self.cursor != Some(cursor) {
            let line = doc.rope.char_to_line(cursor.2);
            let mut top = line as f32 * height
                + self
                    .wrapped_heights
                    .range(..line)
                    .map(|(_, extra)| extra)
                    .sum::<f32>();
            if word_wrap {
                let galley = self.cache.line(
                    ui.ctx(),
                    doc,
                    line,
                    font,
                    &state.theme,
                    text_rect.width().max(cell),
                );
                self.wrapped_heights
                    .insert(line, galley.size().y.max(height) - height);
                let raw_col = cursor.2.saturating_sub(doc.rope.line_to_char(line));
                let exp_col = cache::raw_char_to_expanded_char(doc, line, raw_col);
                top += galley
                    .pos_from_cursor(egui::text::CCursor::new(exp_col))
                    .top();
            }
            if top < scroll {
                scroll = top;
            }
            if top + height > scroll + rect.height() {
                scroll = (top + height - rect.height()).max(0.0);
            }
            if !word_wrap {
                let raw_col = cursor.2.saturating_sub(doc.rope.line_to_char(line));
                let exp_col = cache::raw_char_to_expanded_char(doc, line, raw_col);
                let x = exp_col as f32 * cell;
                if x < scroll_x {
                    scroll_x = x;
                }
                if x + cell > scroll_x + text_rect.width() {
                    scroll_x = (x + cell - text_rect.width()).max(0.0);
                }
            }
            self.cursor = Some(cursor);
        }
        let (first, offset) = resolve_scroll(scroll, height, &self.wrapped_heights);
        let painter = ui.painter().with_clip_rect(rect);
        let colors = &state.theme.colors;
        painter.rect_filled(rect, 0.0, colors.bg_base);
        let text_painter = ui.painter().with_clip_rect(text_rect);
        if state.line_numbers {
            painter.vline(
                text_rect.left() - 6.0,
                rect.y_range(),
                Stroke::new(1.0, colors.border_subtle),
            );
        }
        let mut y = rect.top() - offset;
        let wrap = if word_wrap {
            text_rect.width().max(cell)
        } else {
            f32::INFINITY
        };
        let visible_end =
            (first + (rect.height() / height).ceil() as usize + 4).min(doc.line_count());
        let visible_spans = self.visible_highlight_spans(state, doc, first..visible_end);
        let mut rows = Vec::new();
        let active_line = doc.rope.char_to_line(doc.selection.primary_range().head);
        let raw_active_col = doc
            .selection
            .primary_range()
            .head
            .saturating_sub(doc.rope.line_to_char(active_line));
        let active_col = cache::raw_char_to_expanded_char(doc, active_line, raw_active_col);
        for line in first..doc.line_count() {
            if y >= rect.bottom() {
                break;
            }
            let spans = visible_spans
                .as_ref()
                .and_then(|all| line.checked_sub(first).and_then(|idx| all.get(idx)))
                .map(Vec::as_slice);
            let galley = self.cache.line_highlighted(
                ui.ctx(),
                LineRequest {
                    doc,
                    line,
                    font,
                    theme: &state.theme,
                    wrap,
                    rainbow_brackets: state.bracket_pair_colorization,
                },
                spans,
            );
            let line_height = galley.size().y.max(height);
            if word_wrap {
                self.wrapped_heights.insert(line, line_height - height);
            }
            let origin = pos2(text_rect.left() - scroll_x, y);
            if state.highlight_active_line && line == active_line {
                painter.rect_filled(
                    Rect::from_min_size(
                        pos2(text_rect.left(), y),
                        vec2(text_rect.width(), line_height),
                    ),
                    0.0,
                    colors.editor_current_line,
                );
            }
            let start = doc.rope.line_to_char(line);
            let count = doc.rope.line(line).len_chars();
            let line_ctx = markers::LineSpanCtx {
                doc,
                line_idx: line,
                galley: &galley,
                origin,
                line_start_char: start,
                line_char_len: count,
                cell,
            };
            markers::paint_line_find_matches(&text_painter, &state.find, &line_ctx, colors);
            let sel_fill = if focused {
                colors.editor_selection
            } else {
                colors.editor_selection_inactive
            };
            for range in &doc.selection.ranges {
                markers::paint_line_char_span(
                    &text_painter,
                    &line_ctx,
                    range.start(),
                    range.end(),
                    sel_fill,
                );
            }
            if state.indent_guides {
                markers::paint_indent_guides(
                    &text_painter,
                    doc,
                    line,
                    origin,
                    line_height,
                    cell,
                    active_col,
                    colors,
                );
            }
            if state.line_numbers {
                painter.text(
                    pos2(text_rect.left() - 16.0, y + height * 0.5),
                    egui::Align2::RIGHT_CENTER,
                    (line + 1).to_string(),
                    FontId::monospace(font),
                    if line == active_line {
                        colors.editor_line_number_active
                    } else {
                        colors.editor_line_number
                    },
                );
            }
            text_painter.galley(origin, galley.clone(), colors.text_primary);
            markers::paint_whitespace(
                &text_painter,
                doc,
                line,
                origin,
                font,
                line_height,
                cell,
                &doc.selection,
                &state.render_whitespace,
                colors,
            );
            rows.push((line, origin, galley));
            y += line_height;
        }
        if !response.dragged() {
            self.column_drag_anchor = None;
        }
        if response.clicked() || response.dragged() {
            if let Some(g) = group_idx.filter(|_| !is_focused_group) {
                commands.push(Command::FocusGroup(g));
            }
            response.request_focus();
            if let Some(pointer) = response.interact_pointer_pos() {
                if let Some((line, origin, galley)) = rows.iter().find(|(_, origin, galley)| {
                    pointer.y >= origin.y && pointer.y < origin.y + galley.size().y.max(height)
                }) {
                    let local_exp = galley.cursor_from_pos(pointer - *origin).index.0;
                    let local_raw = cache::expanded_char_to_raw_char(doc, *line, local_exp);
                    let candidate = doc.rope.line_to_char(*line) + local_raw;
                    let (line_no, col) = doc.line_col_of(candidate);
                    let index = doc.char_idx_of_line_col(line_no - 1, col - 1);
                    let (shift, alt) =
                        ui.input(|input| (input.modifiers.shift, input.modifiers.alt));
                    let cmd = if response.triple_clicked() {
                        EditorCmd::TripleClick(index)
                    } else if response.double_clicked() {
                        EditorCmd::DoubleClick(index)
                    } else if shift && alt && response.dragged() {
                        let anchor_idx = *self.column_drag_anchor.get_or_insert(index);
                        EditorCmd::ColumnBoxSelect {
                            anchor_idx,
                            head_idx: index,
                        }
                    } else if alt && !shift && response.clicked() {
                        EditorCmd::AddCursorAtClick(index)
                    } else if response.dragged() && !response.drag_started() {
                        EditorCmd::Drag(index)
                    } else {
                        EditorCmd::Click {
                            index,
                            extend: shift,
                        }
                    };
                    commands.push(Command::Editor(cmd));
                }
            }
        }
        if let Some((left, right)) = matching_brackets(doc) {
            for index in [left, right] {
                let line = doc.rope.char_to_line(index);
                if let Some((_, origin, galley)) = rows.iter().find(|(row, _, _)| *row == line) {
                    let raw_col = index.saturating_sub(doc.rope.line_to_char(line));
                    let exp_col = cache::raw_char_to_expanded_char(doc, line, raw_col);
                    let cursor = galley.pos_from_cursor(egui::text::CCursor::new(exp_col));
                    let bounds =
                        Rect::from_min_size(*origin + cursor.min.to_vec2(), vec2(cell, height));
                    text_painter.rect_filled(bounds, 0.0, colors.editor_bracket_match);
                    text_painter.rect_stroke(
                        bounds,
                        0.0,
                        Stroke::new(1.0, colors.accent),
                        egui::StrokeKind::Inside,
                    );
                }
            }
        }
        let now = ui.input(|input| input.time);
        if !commands.is_empty() {
            self.last_activity = now;
        }
        let age = now - self.last_activity;
        if focused && (age >= 10.0 || ((age / 0.53) as u64).is_multiple_of(2)) {
            for (idx, range) in doc.selection.ranges.iter().enumerate() {
                let line = doc.rope.char_to_line(range.head);
                if let Some((_, origin, galley)) = rows.iter().find(|(index, _, _)| *index == line)
                {
                    let raw_col = range.head.saturating_sub(doc.rope.line_to_char(line));
                    let exp_col = cache::raw_char_to_expanded_char(doc, line, raw_col);
                    let cursor = galley.pos_from_cursor(egui::text::CCursor::new(exp_col));
                    let is_primary = idx == doc.selection.primary;
                    let (cur_w, cur_color) = if is_primary {
                        (2.0, colors.editor_cursor)
                    } else {
                        (1.5, colors.editor_cursor_secondary)
                    };
                    text_painter.rect_filled(
                        Rect::from_min_size(*origin + cursor.min.to_vec2(), vec2(cur_w, height)),
                        0.0,
                        cur_color,
                    );
                }
            }
        }
        if focused && age < 10.0 {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(530));
        }
        let total = doc.line_count() as f32 * height + self.wrapped_heights.values().sum::<f32>();
        let track = Rect::from_min_max(pos2(rect.right() - 12.0, rect.top()), rect.max);
        painter.vline(
            track.left(),
            track.y_range(),
            Stroke::new(1.0, colors.border_subtle),
        );
        markers::paint_scrollbar_ticks(&painter, track, doc, &state.find, colors);
        let thumb_height = (rect.height() * rect.height() / total)
            .clamp(20.0_f32.min(rect.height()), rect.height());
        let max_scroll = (total - rect.height()).max(0.0);
        let thumb_y =
            rect.top() + (rect.height() - thumb_height) * (scroll / max_scroll.max(1.0)).min(1.0);
        let scrollbar = ui.interact(
            track,
            response.id.with("scrollbar"),
            Sense::click_and_drag(),
        );
        let thumb_color = if scrollbar.hovered() || scrollbar.dragged() {
            colors.text_muted
        } else {
            colors.border_strong
        };
        painter.rect_filled(
            Rect::from_min_size(pos2(track.left() + 2.0, thumb_y), vec2(8.0, thumb_height)),
            egui::CornerRadius::same(4),
            thumb_color,
        );
        if scrollbar.dragged() || scrollbar.clicked() {
            if let Some(pos) = scrollbar.interact_pointer_pos() {
                scroll = ((pos.y - track.top() - thumb_height * 0.5)
                    / (track.height() - thumb_height).max(1.0))
                .clamp(0.0, 1.0)
                    * max_scroll;
            }
        }
        if scroll.to_bits() != state.scroll_y.to_bits()
            || scroll_x.to_bits() != state.scroll_x.to_bits()
        {
            commands.push(Command::Scroll {
                x_bits: scroll_x.to_bits(),
                y_bits: scroll.to_bits(),
            });
        }
        commands
    }

    fn visible_highlight_spans(
        &mut self,
        state: &AppState,
        doc: &Document,
        visible: std::ops::Range<usize>,
    ) -> Option<Vec<Vec<HighlightSpan>>> {
        if !state.should_highlight(doc) || visible.start >= visible.end {
            return None;
        }
        let syntax_set = state.syntax_set.as_deref()?;
        let hl_cache = self
            .highlight_caches
            .entry(doc.id)
            .or_insert_with(|| HighlightCache::new(doc.language.as_str()));
        if hl_cache.language_id() != doc.language.as_str() {
            hl_cache.reset(doc.language.as_str(), doc.version);
        } else if hl_cache.doc_version() != doc.version {
            let edited_line = doc
                .history
                .undo_stack
                .last()
                .filter(|_| doc.version == hl_cache.doc_version() + 1)
                .and_then(|tx| tx.changes.iter().map(|c| c.start).min())
                .or_else(|| {
                    doc.history
                        .redo_stack
                        .last()
                        .and_then(|tx| tx.changes.iter().map(|c| c.start).min())
                })
                .map_or(0, |ch| doc.rope.char_to_line(ch.min(doc.rope.len_chars())));
            hl_cache.invalidate_from(edited_line, doc.version);
        }
        Some(hl_cache.highlight_visible(syntax_set, doc, visible))
    }
}

fn resolve_scroll(
    scroll: f32,
    height: f32,
    extras: &std::collections::BTreeMap<usize, f32>,
) -> (usize, f32) {
    let mut remaining = scroll;
    let mut previous = 0;
    for (&line, &extra) in extras {
        let gap = (line - previous) as f32 * height;
        if remaining < gap {
            return (previous + (remaining / height) as usize, remaining % height);
        }
        remaining -= gap;
        if remaining < height + extra {
            return (line, remaining);
        }
        remaining -= height + extra;
        previous = line + 1;
    }
    (previous + (remaining / height) as usize, remaining % height)
}

fn matching_brackets(doc: &Document) -> Option<(usize, usize)> {
    let head = doc.selection.primary_range().head;
    let index = [Some(head), head.checked_sub(1)]
        .into_iter()
        .flatten()
        .find(|index| *index < doc.char_count() && "()[]{}".contains(doc.rope.char(*index)))?;
    let ch = doc.rope.char(index);
    let (other, forward) = match ch {
        '(' => (')', true),
        '[' => (']', true),
        '{' => ('}', true),
        ')' => ('(', false),
        ']' => ('[', false),
        '}' => ('{', false),
        _ => return None,
    };
    let mut depth = 1;
    // Bound scanning so unmatched brackets cannot stall a large-file frame.
    for distance in 1..=10_000 {
        let candidate = if forward {
            index.checked_add(distance)
        } else {
            index.checked_sub(distance)
        }?;
        if candidate >= doc.char_count() {
            return None;
        }
        let current = doc.rope.char(candidate);
        if current == ch {
            depth += 1;
        }
        if current == other {
            depth -= 1;
        }
        if depth == 0 {
            return Some((index, candidate));
        }
    }
    None
}
