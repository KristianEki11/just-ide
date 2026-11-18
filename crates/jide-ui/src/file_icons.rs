//! Vector file, folder, and chevron icons modeled after Simple Icons & VS Code Material/Seti themes.

use crate::theme::Theme;
use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2, Rect, Stroke,
};

/// Paints a crisp 2-segment vector tree chevron (`>` collapsed, `v` expanded).
pub fn paint_chevron(painter: &Painter, center: Pos2, expanded: bool, color: Color32) {
    let stroke = Stroke::new(1.4, color);
    if expanded {
        let a = pos2(center.x - 3.5, center.y - 1.5);
        let b = pos2(center.x, center.y + 2.0);
        let c = pos2(center.x + 3.5, center.y - 1.5);
        painter.line_segment([a, b], stroke);
        painter.line_segment([b, c], stroke);
    } else {
        let a = pos2(center.x - 1.5, center.y - 3.5);
        let b = pos2(center.x + 2.0, center.y);
        let c = pos2(center.x - 1.5, center.y + 3.5);
        painter.line_segment([a, b], stroke);
        painter.line_segment([b, c], stroke);
    }
}

/// Paints a colored vector folder icon inside `rect` (tab + body + open flap when `expanded`).
pub fn paint_folder_icon(
    painter: &Painter,
    rect: Rect,
    name: &str,
    expanded: bool,
    _theme: &Theme,
) {
    let color = folder_color(name);
    let r = rect.shrink(1.5);
    let tab_h = (r.height() * 0.24).max(2.5);
    let tab_rect = Rect::from_min_size(r.min, vec2(r.width() * 0.45, tab_h + 1.5));
    painter.rect_filled(tab_rect, CornerRadius::same(2), color);

    let body_top = r.top() + tab_h;
    let body_rect = Rect::from_min_max(pos2(r.left(), body_top), r.max);
    if expanded {
        let back_color = darken(color, 0.72);
        painter.rect_filled(body_rect, CornerRadius::same(2), back_color);
        let flap = Rect::from_min_max(pos2(r.left() + 1.0, body_top + r.height() * 0.22), r.max);
        painter.rect_filled(flap, CornerRadius::same(2), color);
    } else {
        painter.rect_filled(body_rect, CornerRadius::same(2), color);
    }
}

/// Paints a Simple Icons / Seti-style file icon inside `rect`.
pub fn paint_file_icon(painter: &Painter, rect: Rect, filename: &str, theme: &Theme) {
    let r = rect.shrink(1.0);
    let spec = icon_spec_for_filename(filename);

    match spec.kind {
        IconKind::Markdown => {
            painter.rect_filled(r, CornerRadius::same(3), spec.bg);
            painter.text(
                pos2(r.center().x - 2.0, r.center().y),
                Align2::CENTER_CENTER,
                "M",
                FontId::new(8.5, FontFamily::Monospace),
                spec.fg,
            );
            let ax = r.right() - 3.6;
            let top = pos2(ax, r.top() + 3.5);
            let bot = pos2(ax, r.bottom() - 3.5);
            let stroke = Stroke::new(1.2, spec.fg);
            painter.line_segment([top, bot], stroke);
            painter.line_segment([pos2(ax - 2.0, bot.y - 2.2), bot], stroke);
            painter.line_segment([pos2(ax + 2.0, bot.y - 2.2), bot], stroke);
        }
        IconKind::Database => {
            let cx = r.center().x;
            let rx = r.width() * 0.40;
            let ry = r.height() * 0.16;
            let top_y = r.top() + 3.0;
            let bot_y = r.bottom() - 3.0;
            let body = Rect::from_min_max(pos2(cx - rx, top_y), pos2(cx + rx, bot_y));
            painter.rect_filled(body, CornerRadius::same(3), spec.bg);
            painter.line_segment(
                [
                    pos2(cx - rx + 1.0, r.center().y),
                    pos2(cx + rx - 1.0, r.center().y),
                ],
                Stroke::new(1.0, theme.colors.bg_surface),
            );
            painter.line_segment(
                [
                    pos2(cx - rx + 1.0, top_y + ry * 1.4),
                    pos2(cx + rx - 1.0, top_y + ry * 1.4),
                ],
                Stroke::new(1.0, theme.colors.bg_surface),
            );
        }
        IconKind::Badge(label) => {
            painter.rect_filled(r, CornerRadius::same(3), spec.bg);
            let size = match label.len() {
                1 => 9.5,
                2 => 8.0,
                _ => 6.6,
            };
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::new(size, FontFamily::Monospace),
                spec.fg,
            );
        }
        IconKind::DefaultDoc => {
            let doc_rect = Rect::from_min_max(
                pos2(r.left() + 2.0, r.top() + 1.0),
                pos2(r.right() - 2.0, r.bottom() - 1.0),
            );
            painter.rect_stroke(
                doc_rect,
                CornerRadius::same(2),
                Stroke::new(1.2, theme.colors.text_muted),
                egui::StrokeKind::Inside,
            );
            painter.line_segment(
                [
                    pos2(doc_rect.left() + 2.5, doc_rect.center().y - 1.5),
                    pos2(doc_rect.right() - 2.5, doc_rect.center().y - 1.5),
                ],
                Stroke::new(1.0, theme.colors.text_muted),
            );
            painter.line_segment(
                [
                    pos2(doc_rect.left() + 2.5, doc_rect.center().y + 1.5),
                    pos2(doc_rect.right() - 4.0, doc_rect.center().y + 1.5),
                ],
                Stroke::new(1.0, theme.colors.text_muted),
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Markdown,
    Database,
    Badge(&'static str),
    DefaultDoc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIconSpec {
    pub kind: IconKind,
    pub bg: Color32,
    pub fg: Color32,
}

const WHITE: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
const DARK_INK: Color32 = Color32::from_rgb(0x18, 0x1A, 0x1F);

/// Resolves Simple Icons / Seti color & symbol badge for a filename.
#[must_use]
pub fn icon_spec_for_filename(filename: &str) -> FileIconSpec {
    let lower = filename.to_ascii_lowercase();
    match lower.as_str() {
        "cargo.toml" | "cargo.lock" => {
            return badge("RS", Color32::from_rgb(0xDE, 0xA5, 0x84), DARK_INK);
        }
        "dockerfile" => return badge("DK", Color32::from_rgb(0x24, 0x96, 0xED), WHITE),
        ".gitignore" | ".gitattributes" | ".gitmodules" => {
            return badge("GT", Color32::from_rgb(0xF0, 0x50, 0x32), WHITE);
        }
        "license" | "license.md" | "license.txt" => {
            return badge("LC", Color32::from_rgb(0xD4, 0xAF, 0x37), DARK_INK);
        }
        "makefile" => return badge("MK", Color32::from_rgb(0x6D, 0x80, 0x86), WHITE),
        _ => {}
    }

    let ext = lower.rsplit_once('.').map_or_default(|(_, e)| e);

    match ext {
        "md" | "mdx" => FileIconSpec {
            kind: IconKind::Markdown,
            bg: Color32::from_rgb(0x3B, 0x82, 0xF6),
            fg: WHITE,
        },
        "rs" => badge("RS", Color32::from_rgb(0xDE, 0xA5, 0x84), DARK_INK),
        "ts" | "mts" | "cts" => badge("TS", Color32::from_rgb(0x31, 0x78, 0xC6), WHITE),
        "tsx" => badge("TX", Color32::from_rgb(0x00, 0xB4, 0xD8), DARK_INK),
        "jsx" => badge("JX", Color32::from_rgb(0x61, 0xDA, 0xFB), DARK_INK),
        "js" | "mjs" | "cjs" => badge("JS", Color32::from_rgb(0xF7, 0xDF, 0x1E), DARK_INK),
        "json" | "jsonc" | "json5" => badge("{}", Color32::from_rgb(0xCB, 0xCB, 0x41), DARK_INK),
        "toml" => badge("TM", Color32::from_rgb(0x9C, 0x42, 0x21), WHITE),
        "yml" | "yaml" => badge("YM", Color32::from_rgb(0xCB, 0x17, 0x1E), WHITE),
        "html" | "htm" => badge("H5", Color32::from_rgb(0xE3, 0x4F, 0x26), WHITE),
        "css" => badge("C3", Color32::from_rgb(0x15, 0x72, 0xB6), WHITE),
        "scss" | "sass" | "less" => badge("SC", Color32::from_rgb(0xCC, 0x66, 0x99), WHITE),
        "svg" | "xml" | "manifest" | "nsi" => {
            badge("<>", Color32::from_rgb(0xFF, 0x98, 0x00), DARK_INK)
        }
        "py" | "pyw" => badge("PY", Color32::from_rgb(0x37, 0x76, 0xAB), WHITE),
        "go" => badge("GO", Color32::from_rgb(0x00, 0xAD, 0xD8), DARK_INK),
        "c" | "h" => badge("C", Color32::from_rgb(0x55, 0x55, 0xEE), WHITE),
        "cpp" | "cc" | "cxx" | "hpp" => badge("C+", Color32::from_rgb(0x00, 0x59, 0x9C), WHITE),
        "cs" => badge("C#", Color32::from_rgb(0x51, 0x2B, 0xD4), WHITE),
        "java" => badge("JV", Color32::from_rgb(0xED, 0x8B, 0x00), DARK_INK),
        "kt" | "kts" => badge("KT", Color32::from_rgb(0x7F, 0x52, 0xFF), WHITE),
        "php" => badge("PH", Color32::from_rgb(0x77, 0x7B, 0xB4), WHITE),
        "rb" => badge("RB", Color32::from_rgb(0xCC, 0x34, 0x2D), WHITE),
        "lua" => badge("LU", Color32::from_rgb(0x00, 0x00, 0x80), WHITE),
        "sh" | "bash" | "zsh" | "ps1" | "bat" | "cmd" => {
            badge(">_", Color32::from_rgb(0x2D, 0x8A, 0x4E), WHITE)
        }
        "sql" | "db" | "sqlite" => FileIconSpec {
            kind: IconKind::Database,
            bg: Color32::from_rgb(0xE3, 0x8C, 0x00),
            fg: DARK_INK,
        },
        "txt" | "log" => badge("TX", Color32::from_rgb(0x64, 0x74, 0x8B), WHITE),
        "ini" | "conf" | "cfg" | "env" => {
            badge("CF", Color32::from_rgb(0x78, 0x90, 0x9C), DARK_INK)
        }
        "diff" | "patch" => badge("+-", Color32::from_rgb(0x41, 0xB8, 0x83), DARK_INK),
        "png" | "jpg" | "jpeg" | "ico" | "webp" => {
            badge("IM", Color32::from_rgb(0xA8, 0x55, 0xF7), WHITE)
        }
        _ => FileIconSpec {
            kind: IconKind::DefaultDoc,
            bg: Color32::TRANSPARENT,
            fg: WHITE,
        },
    }
}

const fn badge(label: &'static str, bg: Color32, fg: Color32) -> FileIconSpec {
    FileIconSpec {
        kind: IconKind::Badge(label),
        bg,
        fg,
    }
}

fn folder_color(name: &str) -> Color32 {
    match name.to_ascii_lowercase().as_str() {
        "src" | "crates" | "lib" => Color32::from_rgb(0x4F, 0xA3, 0xF7),
        "docs" | "doc" => Color32::from_rgb(0xA7, 0x8B, 0xFA),
        "tests" | "test" | "benches" => Color32::from_rgb(0x4A, 0xDE, 0x80),
        "assets" | "images" | "icons" | "fonts" => Color32::from_rgb(0xFB, 0xBF, 0x24),
        "scripts" | "installer" => Color32::from_rgb(0x34, 0xD3, 0x99),
        ".git" | ".github" | ".cargo" => Color32::from_rgb(0x94, 0xA3, 0xB8),
        _ => Color32::from_rgb(0xE2, 0xB3, 0x56),
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn darken(color: Color32, factor: f32) -> Color32 {
    Color32::from_rgb(
        (f32::from(color.r()) * factor) as u8,
        (f32::from(color.g()) * factor) as u8,
        (f32::from(color.b()) * factor) as u8,
    )
}
