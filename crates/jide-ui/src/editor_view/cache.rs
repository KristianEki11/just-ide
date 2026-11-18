use crate::theme::{SyntaxStyle, Theme, ThemeKind};
use egui::{text::LayoutJob, Color32, FontId, TextFormat};
use jide_core::{DocId, Document};
use jide_syntax::{HighlightSpan, SyntaxToken};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

/// Parameters describing one line layout lookup.
#[derive(Clone, Copy)]
pub struct LineRequest<'a> {
    /// Document containing the line.
    pub doc: &'a Document,
    /// Zero-based line index.
    pub line: usize,
    /// Logical font size in points.
    pub font: f32,
    /// Active theme for text colors and styling.
    pub theme: &'a Theme,
    /// Wrap width in points (`f32::INFINITY` disables wrap).
    pub wrap: f32,
    /// Whether to colorize matching `(`, `[`, `{` pairs with a rotating rainbow palette.
    pub rainbow_brackets: bool,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct Key {
    doc: DocId,
    line: usize,
    hash: u64,
    font: u32,
    theme_hash: u64,
    color: Color32,
    wrap: u32,
    rainbow: bool,
}

/// Bounded least-recently-used line layout cache.
#[derive(Default)]
pub struct GalleyCache {
    entries: HashMap<Key, (Arc<egui::Galley>, u64)>,
    clock: u64,
    scratch: String,
}

impl GalleyCache {
    /// Number of cached lines.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Evict entries not used in the recent window or belonging to closed documents.
    pub fn evict_inactive_docs(&mut self, active_ids: &[DocId], retain_clock_window: u64) {
        let cutoff = self.clock.saturating_sub(retain_clock_window);
        self.entries
            .retain(|k, (_, used)| active_ids.contains(&k.doc) || *used >= cutoff);
    }

    /// Layout one line, reusing a matching cached galley.
    pub fn line(
        &mut self,
        ctx: &egui::Context,
        doc: &Document,
        line: usize,
        font: f32,
        theme: &Theme,
        wrap: f32,
    ) -> Arc<egui::Galley> {
        self.line_highlighted(
            ctx,
            LineRequest {
                doc,
                line,
                font,
                theme,
                wrap,
                rainbow_brackets: true,
            },
            None,
        )
    }

    /// Layout one line with optional precomputed syntax highlight spans.
    pub fn line_highlighted(
        &mut self,
        ctx: &egui::Context,
        req: LineRequest<'_>,
        spans: Option<&[HighlightSpan]>,
    ) -> Arc<egui::Galley> {
        let (key, text_len) = self.prepare_key(req, spans);
        self.clock += 1;
        if let Some((galley, used)) = self.entries.get_mut(&key) {
            *used = self.clock;
            return Arc::clone(galley);
        }
        let line_text = &self.scratch[..text_len];
        let galley = build_highlighted_galley(
            ctx,
            line_text,
            req.doc.tab_size,
            spans.unwrap_or(&[]),
            req.font,
            req.theme,
            req.wrap,
            req.rainbow_brackets,
        );
        self.insert_evict(key, Arc::clone(&galley));
        galley
    }

    /// Layout a range of visible lines, batching all cache misses under a single `ctx.fonts_mut` lock.
    pub fn prepare_range(
        &mut self,
        ctx: &egui::Context,
        doc: &Document,
        lines: std::ops::Range<usize>,
        font: f32,
        theme: &Theme,
        wrap: f32,
    ) -> Vec<Arc<egui::Galley>> {
        let end = lines.end.min(doc.line_count());
        let start = lines.start.min(end);
        let count = end - start;
        let mut results: Vec<Option<Arc<egui::Galley>>> = vec![None; count];
        let mut misses: Vec<(usize, Key, usize, usize)> = Vec::new();
        let mut batch_text = String::new();

        for (idx, line) in (start..end).enumerate() {
            let (key, text_len) = self.prepare_key(
                LineRequest {
                    doc,
                    line,
                    font,
                    theme,
                    wrap,
                    rainbow_brackets: false,
                },
                None,
            );
            self.clock += 1;
            if let Some((galley, used)) = self.entries.get_mut(&key) {
                *used = self.clock;
                results[idx] = Some(Arc::clone(galley));
            } else {
                let offset = batch_text.len();
                batch_text.push_str(&self.scratch[..text_len]);
                misses.push((idx, key, offset, offset + text_len));
            }
        }

        if !misses.is_empty() {
            let format = TextFormat {
                font_id: FontId::monospace(font),
                color: theme.colors.text_primary,
                line_height: Some(font * 1.5),
                extra_letter_spacing: 0.0,
                ..TextFormat::default()
            };
            let tab_size = doc.tab_size;
            let built: Vec<Arc<egui::Galley>> = ctx.fonts_mut(|fonts| {
                misses
                    .iter()
                    .map(|(_, _, s, e)| {
                        let (expanded, _) = expand_line_tabs(&batch_text[*s..*e], tab_size);
                        let mut job = LayoutJob::default();
                        job.wrap.max_width = wrap;
                        job.append(&expanded, 0.0, format.clone());
                        job.break_on_newline = false;
                        fonts.layout_job(job)
                    })
                    .collect()
            });

            for ((idx, key, _, _), galley) in misses.into_iter().zip(built) {
                self.insert_evict(key, Arc::clone(&galley));
                results[idx] = Some(galley);
            }
        }

        results.into_iter().flatten().collect()
    }

    fn prepare_key(
        &mut self,
        req: LineRequest<'_>,
        spans: Option<&[HighlightSpan]>,
    ) -> (Key, usize) {
        self.scratch.clear();
        for chunk in req.doc.rope.line(req.line).chunks() {
            self.scratch.push_str(chunk);
        }
        let trimmed_len = self.scratch.trim_end_matches(['\n', '\r']).len();
        let mut hasher = DefaultHasher::new();
        self.scratch[..trimmed_len].hash(&mut hasher);
        req.doc.tab_size.hash(&mut hasher);
        if let Some(spans) = spans {
            true.hash(&mut hasher);
            req.doc.language.as_str().hash(&mut hasher);
            spans.len().hash(&mut hasher);
            for span in spans {
                span.start_byte.hash(&mut hasher);
                span.end_byte.hash(&mut hasher);
                span.token.hash(&mut hasher);
            }
        } else {
            false.hash(&mut hasher);
        }

        let mut theme_hasher = DefaultHasher::new();
        req.theme.id.hash(&mut theme_hasher);

        let key = Key {
            doc: req.doc.id,
            line: req.line,
            hash: hasher.finish(),
            font: req.font.to_bits(),
            theme_hash: theme_hasher.finish(),
            color: req.theme.colors.text_primary,
            wrap: req.wrap.to_bits(),
            rainbow: req.rainbow_brackets,
        };
        (key, trimmed_len)
    }

    fn insert_evict(&mut self, key: Key, galley: Arc<egui::Galley>) {
        if self.entries.len() >= 2000 {
            if let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(k, _)| *k)
            {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(key, (galley, self.clock));
    }
}

/// Converts a raw character offset within `line_slice` into an expanded character offset
/// matching `GalleyCache` (where `\t` expands to tab stops of width `tab_size`).
#[must_use]
pub fn raw_char_to_expanded_char(doc: &Document, line_idx: usize, raw_char_offset: usize) -> usize {
    let tab = doc.tab_size.max(1);
    let mut expanded = 0usize;
    for (idx, ch) in doc.rope.line(line_idx).chars().enumerate() {
        if idx >= raw_char_offset || ch == '\n' || ch == '\r' {
            break;
        }
        if ch == '\t' {
            expanded += tab - (expanded % tab);
        } else {
            expanded += 1;
        }
    }
    expanded
}

/// Converts a `Galley` `CCursor` character offset on a tab-expanded line back to a raw
/// character offset within the line.
#[must_use]
pub fn expanded_char_to_raw_char(
    doc: &Document,
    line_idx: usize,
    expanded_char_offset: usize,
) -> usize {
    let tab = doc.tab_size.max(1);
    let mut expanded = 0usize;
    let mut raw = 0usize;
    for ch in doc.rope.line(line_idx).chars() {
        if ch == '\n' || ch == '\r' {
            break;
        }
        if expanded >= expanded_char_offset {
            break;
        }
        let step = if ch == '\t' {
            tab - (expanded % tab)
        } else {
            1
        };
        if ch == '\t' && expanded_char_offset < expanded + step {
            let dist_left = expanded_char_offset - expanded;
            let dist_right = expanded + step - expanded_char_offset;
            if dist_right < dist_left {
                raw += 1;
            }
            break;
        }
        expanded += step;
        raw += 1;
    }
    raw
}

fn expand_line_tabs(line_text: &str, tab_size: usize) -> (String, Vec<usize>) {
    let tab = tab_size.max(1);
    let mut expanded = String::with_capacity(line_text.len() + 8);
    let mut byte_map = vec![0usize; line_text.len() + 1];
    let mut col = 0usize;
    for (idx, ch) in line_text.char_indices() {
        let exp_pos = expanded.len();
        for b in idx..(idx + ch.len_utf8()) {
            if let Some(slot) = byte_map.get_mut(b) {
                *slot = exp_pos;
            }
        }
        if ch == '\t' {
            let spaces = tab - (col % tab);
            for _ in 0..spaces {
                expanded.push(' ');
            }
            col += spaces;
        } else if ch.is_control() {
            expanded.push(' ');
            col += 1;
        } else {
            expanded.push(ch);
            col += 1;
        }
    }
    byte_map[line_text.len()] = expanded.len();
    (expanded, byte_map)
}

/// Build a line galley directly for headless layout benchmarks.
#[must_use]
pub fn build_line_galley(
    ctx: &egui::Context,
    text: &str,
    font: f32,
    color: egui::Color32,
    wrap: f32,
    tab_size: usize,
) -> Arc<egui::Galley> {
    let (expanded, _) = expand_line_tabs(text.trim_end_matches(['\n', '\r']), tab_size);
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    job.append(
        &expanded,
        0.0,
        TextFormat {
            font_id: FontId::monospace(font),
            color,
            line_height: Some(font * 1.5),
            extra_letter_spacing: 0.0,
            ..TextFormat::default()
        },
    );
    job.break_on_newline = false;
    ctx.fonts_mut(|fonts| fonts.layout_job(job))
}

/// Layout a visible range without constructing a widget.
pub fn render_visible_lines_headless(
    ctx: &egui::Context,
    cache: &mut GalleyCache,
    doc: &Document,
    lines: std::ops::Range<usize>,
    font: f32,
    theme: &Theme,
    wrap: f32,
) -> Vec<Arc<egui::Galley>> {
    cache.prepare_range(ctx, doc, lines, font, theme, wrap)
}

#[allow(clippy::too_many_arguments)]
fn build_highlighted_galley(
    ctx: &egui::Context,
    raw_line_text: &str,
    tab_size: usize,
    spans: &[HighlightSpan],
    font: f32,
    theme: &Theme,
    wrap: f32,
    rainbow_brackets: bool,
) -> Arc<egui::Galley> {
    let (line_text, byte_map) = expand_line_tabs(raw_line_text, tab_size);
    ctx.fonts_mut(|fonts| {
        let mut job = LayoutJob::default();
        job.wrap.max_width = wrap;
        let line_height = Some(font * 1.5);
        let default_style = token_style(SyntaxToken::Default, theme);
        let mut bracket_depth = 0usize;

        if spans.is_empty() {
            append_rainbow_segment(
                &mut job,
                &line_text,
                default_style,
                font,
                line_height,
                rainbow_brackets,
                theme.kind,
                &mut bracket_depth,
            );
        } else {
            let mut cursor = 0usize;
            for span in spans {
                let raw_s = span.start_byte.min(raw_line_text.len());
                let raw_e = span.end_byte.min(raw_line_text.len());
                let start = byte_map[raw_s];
                let end = byte_map[raw_e];
                if start > cursor {
                    if let Some(gap) = line_text.get(cursor..start) {
                        append_rainbow_segment(
                            &mut job,
                            gap,
                            default_style,
                            font,
                            line_height,
                            rainbow_brackets,
                            theme.kind,
                            &mut bracket_depth,
                        );
                    }
                }
                if end > start {
                    if let Some(text) = line_text.get(start..end) {
                        let style = token_style(span.token, theme);
                        let is_bracket_candidate =
                            matches!(span.token, SyntaxToken::Punctuation | SyntaxToken::Default);
                        append_rainbow_segment(
                            &mut job,
                            text,
                            style,
                            font,
                            line_height,
                            rainbow_brackets && is_bracket_candidate,
                            theme.kind,
                            &mut bracket_depth,
                        );
                    }
                }
                cursor = cursor.max(end);
            }
            if cursor < line_text.len() {
                if let Some(tail) = line_text.get(cursor..) {
                    append_rainbow_segment(
                        &mut job,
                        tail,
                        default_style,
                        font,
                        line_height,
                        rainbow_brackets,
                        theme.kind,
                        &mut bracket_depth,
                    );
                }
            }
        }

        job.break_on_newline = false;
        fonts.layout_job(job)
    })
}

#[allow(clippy::too_many_arguments)]
fn append_rainbow_segment(
    job: &mut LayoutJob,
    text: &str,
    style: SyntaxStyle,
    font: f32,
    line_height: Option<f32>,
    rainbow: bool,
    theme_kind: ThemeKind,
    bracket_depth: &mut usize,
) {
    let underline = if style.underline {
        egui::Stroke::new(1.0, style.fg)
    } else {
        egui::Stroke::NONE
    };
    if !rainbow
        || !text
            .bytes()
            .any(|b| matches!(b, b'(' | b')' | b'[' | b']' | b'{' | b'}'))
    {
        job.append(
            text,
            0.0,
            TextFormat {
                font_id: FontId::monospace(font),
                color: style.fg,
                italics: style.italic,
                underline,
                line_height,
                ..TextFormat::default()
            },
        );
        return;
    }

    let mut run_start = 0usize;
    for (idx, ch) in text.char_indices() {
        if matches!(ch, '(' | '[' | '{' | ')' | ']' | '}') {
            if idx > run_start {
                job.append(
                    &text[run_start..idx],
                    0.0,
                    TextFormat {
                        font_id: FontId::monospace(font),
                        color: style.fg,
                        italics: style.italic,
                        underline,
                        line_height,
                        ..TextFormat::default()
                    },
                );
            }
            let level = if matches!(ch, '(' | '[' | '{') {
                let d = *bracket_depth;
                *bracket_depth += 1;
                d
            } else {
                *bracket_depth = bracket_depth.saturating_sub(1);
                *bracket_depth
            };
            let bcolor = rainbow_bracket_color(level, theme_kind);
            let b_end = idx + ch.len_utf8();
            job.append(
                &text[idx..b_end],
                0.0,
                TextFormat {
                    font_id: FontId::monospace(font),
                    color: bcolor,
                    italics: style.italic,
                    underline,
                    line_height,
                    ..TextFormat::default()
                },
            );
            run_start = b_end;
        }
    }
    if run_start < text.len() {
        job.append(
            &text[run_start..],
            0.0,
            TextFormat {
                font_id: FontId::monospace(font),
                color: style.fg,
                italics: style.italic,
                underline,
                line_height,
                ..TextFormat::default()
            },
        );
    }
}

fn rainbow_bracket_color(level: usize, kind: ThemeKind) -> Color32 {
    match kind {
        ThemeKind::Light => match level % 4 {
            0 => Color32::from_rgb(0xB8, 0x6E, 0x00),
            1 => Color32::from_rgb(0x9C, 0x27, 0xB0),
            2 => Color32::from_rgb(0x02, 0x77, 0xBD),
            _ => Color32::from_rgb(0x00, 0x79, 0x6B),
        },
        ThemeKind::Dark | ThemeKind::HighContrast => match level % 4 {
            0 => Color32::from_rgb(0xFF, 0xD7, 0x00),
            1 => Color32::from_rgb(0xDA, 0x70, 0xD6),
            2 => Color32::from_rgb(0x4F, 0xC3, 0xF7),
            _ => Color32::from_rgb(0x69, 0xF0, 0xAE),
        },
    }
}

fn token_style(token: SyntaxToken, theme: &Theme) -> SyntaxStyle {
    let syntax = &theme.syntax;
    match token {
        SyntaxToken::Comment => syntax.comment,
        SyntaxToken::String => syntax.string,
        SyntaxToken::Number => syntax.number,
        SyntaxToken::Keyword => syntax.keyword,
        SyntaxToken::Function => syntax.function,
        SyntaxToken::Type => syntax.type_name,
        SyntaxToken::Variable => syntax.variable,
        SyntaxToken::Constant => syntax.constant,
        SyntaxToken::Operator => syntax.operator,
        SyntaxToken::Punctuation => syntax.punctuation,
        SyntaxToken::Class => syntax.class,
        SyntaxToken::Parameter => syntax.parameter,
        SyntaxToken::Tag => syntax.tag,
        SyntaxToken::Attribute => syntax.attribute,
        SyntaxToken::Regex => syntax.regex,
        SyntaxToken::Escape => syntax.escape,
        SyntaxToken::Heading => syntax.heading,
        SyntaxToken::Link => syntax.link,
        SyntaxToken::Invalid => syntax.invalid,
        SyntaxToken::DiffAdded => syntax.diff_added,
        SyntaxToken::DiffRemoved => syntax.diff_removed,
        SyntaxToken::Default => SyntaxStyle {
            fg: theme.colors.text_primary,
            bold: false,
            italic: false,
            underline: false,
        },
    }
}
