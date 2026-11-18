use crate::state::{document_name, AppState, Command, SidebarView};
use crate::theme::Theme;
use egui::{
    pos2, vec2, Align, Color32, CornerRadius, FontFamily, FontId, Frame, Key, Layout, Margin,
    RichText, Sense, Stroke, Vec2,
};
use jide_core::Eol;

const ACTIVITY_RAIL_WIDTH: f32 = 40.0;
const STATUS_BAR_HEIGHT: f32 = 24.0;
const DEFAULT_SIDEBAR_WIDTH: f32 = 260.0;
const MIN_SIDEBAR_WIDTH: f32 = 180.0;

/// Renders the application shell and collects emitted commands.
#[must_use]
pub fn render_shell(
    ui: &mut egui::Ui,
    state: &AppState,
    editor: &mut crate::editor_view::EditorView,
    palette: &mut crate::palette::PaletteState,
) -> Vec<Command> {
    let mut commands = Vec::new();
    let theme = &state.theme;

    if !state.zen_mode {
        render_dock_panels(ui, state, theme, &mut commands);
    }

    // 4. Central editor area (always last).
    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(theme.colors.bg_base))
        .show(ui, |ui| {
            if state.settings_open {
                commands.extend(crate::editor_view::EditorView::idle_commands(
                    ui.ctx(),
                    state,
                ));
                if ui.input(|i| i.key_pressed(Key::Escape)) && state.overlay.is_none() {
                    commands.push(Command::CloseSettings);
                }
                crate::settings_view::render_settings_view(ui, state, theme, &mut commands);
            } else if state.documents.is_empty() {
                render_empty_editor(ui, state, theme, &mut commands);
                commands.extend(editor.show(ui, state));
            } else {
                if !state.zen_mode {
                    crate::tabs::render_tabs(ui, state, theme, &mut commands);
                    if state.breadcrumbs_visible {
                        render_breadcrumbs(ui, state, theme, &mut commands);
                    }
                }
                render_large_file_banner(ui, state, theme, &mut commands);
                let editor_rect = ui.available_rect_before_wrap();
                if state.split_open {
                    render_split_editors(ui, state, theme, editor, editor_rect, &mut commands);
                } else {
                    commands.extend(editor.show(ui, state));
                }
                commands.extend(crate::find_widget::render_find_widget(
                    ui,
                    state,
                    editor_rect,
                ));
            }
        });

    // 5. Overlay palette, MRU switcher, modals, and floating toast notification.
    commands.extend(crate::palette::render_palette(ui, state, palette));
    commands.extend(crate::palette::render_mru_switcher(ui, state));
    render_pending_close_modal(ui, state, theme, &mut commands);
    crate::search_panel::render_replace_in_files_modal(ui, state, theme, &mut commands);
    render_toast_card(ui.ctx(), state, theme, &mut commands);

    commands
}

fn breadcrumb_segments(doc: &jide_core::Document, state: &AppState) -> Vec<String> {
    let mut segments = Vec::new();
    let Some(path) = &doc.path else {
        segments.push(document_name(doc).clone());
        return segments;
    };
    if let Some(root) = &state.open_folder {
        if let Ok(rel) = path.strip_prefix(root) {
            if let Some(ws) = &state.workspace_name {
                segments.push(ws.clone());
            }
            for comp in rel.components() {
                segments.push(comp.as_os_str().to_string_lossy().into_owned());
            }
        }
    }
    if segments.is_empty() {
        if let Some(parent) = path.parent().and_then(|p| p.file_name()) {
            segments.push(parent.to_string_lossy().into_owned());
        }
        segments.push(document_name(doc).clone());
    }
    segments
}

fn render_breadcrumb_meta(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    bar_rect: egui::Rect,
    doc: &jide_core::Document,
    theme: &Theme,
    commands: &mut Vec<Command>,
) -> f32 {
    let (line, col) = doc.line_col_of(doc.selection.primary_range().head);
    let lang_label = jide_syntax::find_language_by_id(doc.language.as_str())
        .map_or(doc.language.as_str(), |spec| spec.name);
    let meta_text = format!("{lang_label} · Ln {line}, Col {col}");
    let meta_font = FontId::new(11.5, FontFamily::Proportional);
    let meta_galley = painter.layout_no_wrap(meta_text, meta_font, theme.colors.text_muted);
    let meta_x = (bar_rect.right() - 12.0 - meta_galley.size().x).max(bar_rect.left() + 80.0);
    let meta_rect = egui::Rect::from_min_size(
        pos2(meta_x - 6.0, bar_rect.top() + 3.0),
        vec2(meta_galley.size().x + 12.0, bar_rect.height() - 6.0),
    );
    let meta_resp = ui.interact(
        meta_rect,
        ui.scope_id().with("breadcrumb_loc"),
        Sense::click(),
    );
    if meta_resp.hovered() {
        painter.rect_filled(meta_rect, CornerRadius::same(4), theme.colors.bg_hover);
    }
    painter.galley(
        pos2(meta_x, bar_rect.center().y - meta_galley.size().y * 0.5),
        meta_galley,
        theme.colors.text_muted,
    );
    if meta_resp.clicked() {
        commands.push(Command::RunById("__goto_line__".into()));
    }
    meta_rect.left()
}

fn render_breadcrumbs(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    const BAR_HEIGHT: f32 = 26.0;
    let Some(doc) = state.active_document() else {
        return;
    };
    let bar_rect = ui
        .allocate_exact_size(
            vec2(ui.available_width().max(1.0), BAR_HEIGHT),
            Sense::hover(),
        )
        .0;
    let painter = ui.painter().with_clip_rect(bar_rect);
    painter.rect_filled(bar_rect, CornerRadius::ZERO, theme.colors.bg_base);
    painter.hline(
        bar_rect.x_range(),
        bar_rect.bottom() - 0.5,
        Stroke::new(1.0, theme.colors.border_subtle),
    );

    let segments = breadcrumb_segments(doc, state);
    let meta_left = render_breadcrumb_meta(ui, &painter, bar_rect, doc, theme, commands);
    let mut x = bar_rect.left() + 12.0;
    let max_x = (meta_left - 12.0).max(x + 40.0);
    let last_idx = segments.len().saturating_sub(1);
    let seg_font = FontId::new(12.0, FontFamily::Proportional);

    for (idx, seg) in segments.iter().enumerate() {
        if x >= max_x {
            break;
        }
        let is_last = idx == last_idx;
        if is_last && x + 18.0 < max_x {
            let icon_rect =
                egui::Rect::from_center_size(pos2(x + 7.0, bar_rect.center().y), vec2(14.0, 14.0));
            crate::file_icons::paint_file_icon(&painter, icon_rect, seg, theme);
            x += 18.0;
        }
        let color = if is_last {
            theme.colors.text_primary
        } else {
            theme.colors.text_secondary
        };
        let galley = painter.layout_no_wrap(seg.clone(), seg_font.clone(), color);
        let seg_w = galley.size().x;
        let seg_rect = egui::Rect::from_min_size(
            pos2(x - 3.0, bar_rect.top() + 3.0),
            vec2((seg_w + 6.0).min((max_x - x).max(16.0)), BAR_HEIGHT - 6.0),
        );
        let seg_resp = ui.interact(
            seg_rect,
            ui.scope_id().with(("crumb", idx, seg.as_str())),
            Sense::click(),
        );
        if seg_resp.hovered() {
            painter.rect_filled(seg_rect, CornerRadius::same(4), theme.colors.bg_hover);
        }
        if is_last {
            if let Some(p) = &doc.path {
                let _ = seg_resp.clone().on_hover_text(p.display().to_string());
            }
        }
        if seg_resp.clicked() {
            commands.push(Command::OpenQuickOpen);
        }
        painter
            .with_clip_rect(egui::Rect::from_min_max(
                pos2(x, bar_rect.top()),
                pos2(max_x, bar_rect.bottom()),
            ))
            .galley(
                pos2(x, bar_rect.center().y - galley.size().y * 0.5),
                galley,
                color,
            );
        x += seg_w + 4.0;

        if !is_last && x + 10.0 < max_x {
            let cy = bar_rect.center().y;
            let cx = x + 3.0;
            let ch_stroke = Stroke::new(1.2, theme.colors.text_muted);
            painter.line_segment([pos2(cx - 2.0, cy - 3.0), pos2(cx + 1.5, cy)], ch_stroke);
            painter.line_segment([pos2(cx + 1.5, cy), pos2(cx - 2.0, cy + 3.0)], ch_stroke);
            x += 10.0;
        }
    }
}

fn render_toast_card(
    ctx: &egui::Context,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    let Some(toast) = &state.toast else {
        return;
    };
    let bottom_offset = if state.status_bar_visible && !state.zen_mode {
        -(STATUS_BAR_HEIGHT + 12.0)
    } else {
        -12.0
    };
    egui::Area::new(egui::Id::unique("jide_toast_card"))
        .anchor(egui::Align2::RIGHT_BOTTOM, vec2(-16.0, bottom_offset))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            Frame::NONE
                .fill(theme.colors.bg_elevated)
                .stroke(Stroke::new(1.0, theme.colors.border_strong))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(12, 8))
                .shadow(egui::Shadow {
                    offset: [0, 4],
                    blur: 12,
                    spread: 0,
                    color: Color32::from_black_alpha(96),
                })
                .show(ui, |ui| {
                    ui.set_max_width(360.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let (dot_rect, _) =
                            ui.allocate_exact_size(vec2(10.0, 14.0), Sense::hover());
                        ui.painter()
                            .circle_filled(dot_rect.center(), 3.5, theme.colors.warning);
                        ui.add(
                            egui::Label::new(
                                RichText::new(toast)
                                    .font(FontId::new(12.5, FontFamily::Proportional))
                                    .color(theme.colors.text_primary),
                            )
                            .wrap(),
                        );
                        let (close_rect, close_resp) =
                            ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                        if close_resp.hovered() {
                            ui.painter().rect_filled(
                                close_rect,
                                CornerRadius::same(4),
                                theme.colors.bg_hover,
                            );
                        }
                        let c = close_rect.center();
                        let fg = if close_resp.hovered() {
                            theme.colors.text_primary
                        } else {
                            theme.colors.text_muted
                        };
                        let st = Stroke::new(1.3, fg);
                        ui.painter().line_segment(
                            [pos2(c.x - 3.5, c.y - 3.5), pos2(c.x + 3.5, c.y + 3.5)],
                            st,
                        );
                        ui.painter().line_segment(
                            [pos2(c.x + 3.5, c.y - 3.5), pos2(c.x - 3.5, c.y + 3.5)],
                            st,
                        );
                        if close_resp.on_hover_text("Dismiss").clicked() {
                            commands.push(Command::DismissToast);
                        }
                    });
                });
        });
}

fn render_large_file_banner(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    let Some(doc) = state.active_document() else {
        return;
    };
    if !doc.large_file || state.force_highlight.contains(&doc.id) {
        return;
    }
    let warn_bg = Color32::from_rgba_unmultiplied(
        theme.colors.warning.r(),
        theme.colors.warning.g(),
        theme.colors.warning.b(),
        31,
    );
    Frame::NONE
        .fill(warn_bg)
        .stroke(Stroke::new(1.0, theme.colors.border_subtle))
        .inner_margin(Margin::symmetric(12, 4))
        .show(ui, |ui| {
            ui.set_height(24.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Large file. Highlighting and word wrap are off for speed.")
                        .font(FontId::new(12.0, FontFamily::Proportional))
                        .color(theme.colors.text_primary),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Enable highlighting").clicked() {
                        commands.push(Command::ForceEnableHighlighting);
                    }
                });
            });
        });
}

fn render_activity_rail(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        let right = state.side_bar_right;
        ui.spacing_mut().item_spacing.y = 4.0;
        let explorer_active = state.sidebar_visible && state.sidebar_view == SidebarView::Explorer;
        if rail_icon_button(
            ui,
            theme,
            RailIcon::Explorer,
            explorer_active,
            right,
            "Explorer",
        ) {
            commands.push(Command::ActivateSidebar(SidebarView::Explorer));
        }
        let search_active = state.sidebar_visible && state.sidebar_view == SidebarView::Search;
        if rail_icon_button(ui, theme, RailIcon::Search, search_active, right, "Search") {
            commands.push(Command::ActivateSidebar(SidebarView::Search));
        }

        // Push settings action to bottom of rail.
        ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
            if rail_icon_button(
                ui,
                theme,
                RailIcon::Settings,
                state.settings_open,
                right,
                "Settings (Ctrl+,)",
            ) {
                commands.push(if state.settings_open {
                    Command::CloseSettings
                } else {
                    Command::OpenSettings
                });
            }
        });
    });
}

#[derive(Clone, Copy)]
enum RailIcon {
    Explorer,
    Search,
    Settings,
}

fn rail_icon_button(
    ui: &mut egui::Ui,
    theme: &Theme,
    icon: RailIcon,
    active: bool,
    right_side: bool,
    tooltip: &str,
) -> bool {
    let desired = vec2(ACTIVITY_RAIL_WIDTH, 36.0);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());
    let response = response.on_hover_text(tooltip);

    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, CornerRadius::ZERO, theme.colors.bg_hover);
    }
    if active {
        let bar_x = if right_side {
            rect.right() - 2.0
        } else {
            rect.left()
        };
        let bar = egui::Rect::from_min_size(pos2(bar_x, rect.top()), vec2(2.0, rect.height()));
        painter.rect_filled(bar, CornerRadius::ZERO, theme.colors.accent);
    }

    let fg = if active {
        theme.colors.accent
    } else if response.hovered() {
        theme.colors.text_primary
    } else {
        theme.colors.text_secondary
    };

    let c = rect.center();
    let stroke = Stroke::new(1.5, fg);
    match icon {
        RailIcon::Explorer => {
            // Simple crisp document stack glyph (16x16).
            let r1 = egui::Rect::from_center_size(c + vec2(-1.5, 1.0), vec2(10.0, 12.0));
            let r2 = egui::Rect::from_center_size(c + vec2(2.0, -1.5), vec2(10.0, 12.0));
            painter.rect_stroke(r1, CornerRadius::same(1), stroke, egui::StrokeKind::Inside);
            painter.rect_filled(r2, CornerRadius::same(1), theme.colors.bg_surface);
            painter.rect_stroke(r2, CornerRadius::same(1), stroke, egui::StrokeKind::Inside);
        }
        RailIcon::Search => {
            let circle_center = c + vec2(-1.5, -1.5);
            painter.circle_stroke(circle_center, 5.0, stroke);
            painter.line_segment([circle_center + vec2(3.5, 3.5), c + vec2(6.5, 6.5)], stroke);
        }
        RailIcon::Settings => {
            // Clean 3-line slider/preferences icon.
            for (dy, knob_x) in [(-4.5_f32, -3.0_f32), (0.0, 2.5), (4.5, -1.5)] {
                let y = c.y + dy;
                painter.line_segment([pos2(c.x - 6.5, y), pos2(c.x + 6.5, y)], stroke);
                painter.circle_filled(pos2(c.x + knob_x, y), 2.0, fg);
            }
        }
    }

    response.clicked()
}

fn render_sidebar(ui: &mut egui::Ui, state: &AppState, theme: &Theme, commands: &mut Vec<Command>) {
    match state.sidebar_view {
        SidebarView::Explorer => {
            crate::sidebar_tree::render_file_tree(ui, state, theme, commands);
        }
        SidebarView::Search => {
            if state.project_search.in_bottom_panel {
                Frame::NONE
                    .inner_margin(Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("SEARCH")
                                .font(FontId::new(11.0, FontFamily::Proportional))
                                .color(theme.colors.text_secondary)
                                .strong(),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Search is docked in the bottom panel (Ctrl+J).")
                                .font(FontId::new(12.5, FontFamily::Proportional))
                                .color(theme.colors.text_muted),
                        );
                        ui.add_space(6.0);
                        if ui.button("Move back to Sidebar").clicked() {
                            commands.push(Command::ToggleBottomSearchPanel);
                        }
                    });
            } else {
                crate::search_panel::render_search_panel(ui, state, theme, commands);
            }
        }
    }
}

fn render_empty_editor(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    let rect = ui.available_rect_before_wrap();
    let center = rect.center();

    // Centered app mark "{ }" at 40% opacity (DESIGN §7 EmptyState).
    let mark_color = Color32::from_rgba_unmultiplied(
        theme.colors.text_muted.r(),
        theme.colors.text_muted.g(),
        theme.colors.text_muted.b(),
        102,
    );

    let box_h = 430.0_f32.min((rect.height() - 16.0).max(200.0));
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(egui::Rect::from_center_size(center, vec2(380.0, box_h))),
        |ui| {
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                ui.label(
                    RichText::new("{ }")
                        .font(FontId::new(40.0, FontFamily::Monospace))
                        .color(mark_color),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Just IDE")
                        .font(FontId::new(18.0, FontFamily::Proportional))
                        .color(theme.colors.text_primary)
                        .strong(),
                );
                ui.add_space(16.0);

                let hints = [
                    ("Open file", "Ctrl+O", Command::OpenFile),
                    ("Open folder", "Ctrl+K Ctrl+O", Command::OpenFolder),
                    ("New file", "Ctrl+N", Command::NewFile),
                    ("Quick open", "Ctrl+P", Command::OpenQuickOpen),
                    (
                        "Command Palette",
                        "Ctrl+Shift+P",
                        Command::OpenCommandPalette,
                    ),
                    ("Settings", "Ctrl+,", Command::OpenSettings),
                ];

                for (label, shortcut, cmd) in hints {
                    render_empty_hint_row(ui, theme, label, shortcut, cmd, commands);
                }

                if let Some(hint) = &state.status_hint {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(hint)
                            .font(FontId::new(12.0, FontFamily::Proportional))
                            .color(theme.colors.text_muted),
                    );
                }

                // Recent files section (if any).
                render_empty_recent_files(ui, state, theme, commands);
            });
        },
    );
}

fn render_empty_hint_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    label: &str,
    shortcut: &str,
    cmd: Command,
    commands: &mut Vec<Command>,
) {
    let row_rect = ui.allocate_space(vec2(300.0, 26.0)).1;
    let resp = ui.interact(row_rect, ui.scope_id().with(label), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(row_rect, CornerRadius::same(4), theme.colors.bg_hover);
    }
    ui.painter().text(
        pos2(row_rect.left() + 10.0, row_rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::new(13.0, FontFamily::Proportional),
        theme.colors.text_secondary,
    );
    let key_galley = ui.painter().layout_no_wrap(
        shortcut.to_owned(),
        FontId::new(11.0, FontFamily::Monospace),
        theme.colors.text_muted,
    );
    let pill_w = key_galley.size().x + 12.0;
    let pill_rect = egui::Rect::from_center_size(
        pos2(row_rect.right() - 8.0 - pill_w * 0.5, row_rect.center().y),
        vec2(pill_w, 18.0),
    );
    ui.painter()
        .rect_filled(pill_rect, CornerRadius::same(4), theme.colors.bg_surface);
    ui.painter().rect_stroke(
        pill_rect,
        CornerRadius::same(4),
        Stroke::new(1.0, theme.colors.border_subtle),
        egui::StrokeKind::Inside,
    );
    ui.painter().galley(
        pos2(
            pill_rect.center().x - key_galley.size().x * 0.5,
            pill_rect.center().y - key_galley.size().y * 0.5,
        ),
        key_galley,
        theme.colors.text_muted,
    );
    if resp.clicked() {
        commands.push(cmd);
    }
}

fn render_empty_recent_files(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    if state.recent_files.is_empty() {
        return;
    }
    ui.add_space(14.0);
    ui.label(
        RichText::new("RECENT FILES")
            .font(FontId::new(11.0, FontFamily::Proportional))
            .color(theme.colors.text_secondary)
            .strong(),
    );
    ui.add_space(4.0);
    for path in state.recent_files.iter().take(5) {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let row_rect = ui.allocate_space(vec2(300.0, 24.0)).1;
        let resp = ui.interact(
            row_rect,
            ui.scope_id().with(("recent_file", path.to_str())),
            Sense::click(),
        );
        if resp.hovered() {
            ui.painter()
                .rect_filled(row_rect, CornerRadius::same(4), theme.colors.bg_hover);
        }
        let icon_rect = egui::Rect::from_center_size(
            pos2(row_rect.left() + 16.0, row_rect.center().y),
            vec2(14.0, 14.0),
        );
        crate::file_icons::paint_file_icon(ui.painter(), icon_rect, name, theme);
        ui.painter().text(
            pos2(row_rect.left() + 28.0, row_rect.center().y),
            egui::Align2::LEFT_CENTER,
            name,
            FontId::new(12.5, FontFamily::Proportional),
            theme.colors.text_secondary,
        );
        let full_path = path.display().to_string();
        if resp.on_hover_text(full_path).clicked() {
            commands.push(Command::OpenFilePath {
                path: path.clone(),
                line: 1,
                col: 1,
                preview: false,
            });
        }
    }
}

fn render_pending_close_modal(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    let Some(doc_id) = state
        .pending_close
        .as_ref()
        .and_then(|ids| ids.first().copied())
    else {
        return;
    };
    let doc_name = state
        .documents
        .iter()
        .find(|d| d.id == doc_id)
        .map_or_else(|| "this document".to_owned(), document_name);

    if ui.input(|inp| inp.key_pressed(Key::Escape)) {
        commands.push(Command::ConfirmCloseCancel);
        return;
    }

    let screen_rect = ui.ctx().input(egui::InputState::viewport_rect);
    let modal_w = 400.0_f32;
    let pos = pos2(
        (screen_rect.width() - modal_w) * 0.5,
        screen_rect.height() * 0.25,
    );

    #[allow(deprecated)]
    egui::Area::new(egui::Id::new("dirty_close_modal"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ui.ctx(), |ui| {
            Frame::NONE
                .fill(theme.colors.bg_elevated)
                .stroke(Stroke::new(1.0, theme.colors.border_strong))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(18))
                .show(ui, |ui| {
                    ui.set_width(modal_w);
                    ui.label(
                        RichText::new(format!("Do you want to save changes to {doc_name}?"))
                            .font(FontId::new(14.0, FontFamily::Proportional))
                            .color(theme.colors.text_primary)
                            .strong(),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("Your changes will be lost if you don't save them.")
                            .font(FontId::new(12.5, FontFamily::Proportional))
                            .color(theme.colors.text_secondary),
                    );
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        if ui.button("Don't Save").clicked() {
                            commands.push(Command::ConfirmCloseDiscard);
                        }
                        if ui.button("Cancel").clicked() {
                            commands.push(Command::ConfirmCloseCancel);
                        }
                        let save_btn = egui::Button::new(
                            RichText::new("Save").color(theme.colors.text_on_accent),
                        )
                        .fill(theme.colors.accent);
                        if ui.add(save_btn).clicked() {
                            commands.push(Command::ConfirmCloseSave);
                        }
                    });
                });
        });
}

fn render_split_editors(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    editor: &mut crate::editor_view::EditorView,
    editor_rect: egui::Rect,
    commands: &mut Vec<Command>,
) {
    let total_w = editor_rect.width().max(120.0);
    let left_w = (total_w * state.split_ratio.clamp(0.2, 0.8) - 1.0).max(60.0);
    let right_w = (total_w - left_w - 1.0).max(60.0);
    let h = editor_rect.height().max(40.0);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        ui.allocate_ui(vec2(left_w, h), |ui| {
            ui.push_id("split_group_0", |ui| {
                if state.active_group == 0 {
                    let top = egui::Rect::from_min_size(
                        ui.available_rect_before_wrap().min,
                        vec2(left_w, 2.0),
                    );
                    ui.painter().rect_filled(top, 0.0, theme.colors.accent);
                }
                commands.extend(editor.show_for_doc(
                    ui,
                    state,
                    state.active_doc,
                    Some(0),
                    state.active_group == 0,
                ));
            });
        });

        let (div_rect, _) = ui.allocate_exact_size(vec2(1.0, h), Sense::hover());
        ui.painter()
            .rect_filled(div_rect, 0.0, theme.colors.border_subtle);

        ui.allocate_ui(vec2(right_w, h), |ui| {
            ui.push_id("split_group_1", |ui| {
                if state.active_group == 1 {
                    let top = egui::Rect::from_min_size(
                        ui.available_rect_before_wrap().min,
                        vec2(right_w, 2.0),
                    );
                    ui.painter().rect_filled(top, 0.0, theme.colors.accent);
                }
                commands.extend(editor.show_for_doc(
                    ui,
                    state,
                    state.group2_doc,
                    Some(1),
                    state.active_group == 1,
                ));
            });
        });
    });
}

fn render_dock_panels(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    if state.status_bar_visible {
        egui::Panel::bottom("status_bar")
            .exact_size(STATUS_BAR_HEIGHT)
            .show_separator_line(false)
            .frame(
                Frame::NONE
                    .fill(theme.colors.bg_surface)
                    .stroke(Stroke::new(1.0, theme.colors.border_subtle))
                    .inner_margin(Margin::symmetric(8, 0)),
            )
            .show(ui, |ui| {
                render_status_bar(ui, state, theme, commands);
            });
    }

    if state.activity_bar_visible {
        let rail_panel = if state.side_bar_right {
            egui::Panel::right("activity_rail")
        } else {
            egui::Panel::left("activity_rail")
        };
        rail_panel
            .exact_size(ACTIVITY_RAIL_WIDTH)
            .show_separator_line(false)
            .frame(
                Frame::NONE
                    .fill(theme.colors.bg_surface)
                    .stroke(Stroke::new(1.0, theme.colors.border_subtle))
                    .inner_margin(Margin::symmetric(0, 6)),
            )
            .show(ui, |ui| {
                render_activity_rail(ui, state, theme, commands);
            });
    }

    if state.sidebar_visible {
        let max_sidebar = (ui.available_width() * 0.5).max(MIN_SIDEBAR_WIDTH);
        let sidebar_panel = if state.side_bar_right {
            egui::Panel::right("sidebar")
        } else {
            egui::Panel::left("sidebar")
        };
        sidebar_panel
            .resizable(true)
            .default_size(DEFAULT_SIDEBAR_WIDTH)
            .min_size(MIN_SIDEBAR_WIDTH)
            .max_size(max_sidebar)
            .show_separator_line(true)
            .frame(
                Frame::NONE
                    .fill(theme.colors.bg_surface)
                    .inner_margin(Margin::same(0)),
            )
            .show(ui, |ui| {
                render_sidebar(ui, state, theme, commands);
            });
    }

    if state.project_search.in_bottom_panel {
        egui::Panel::bottom("search_bottom_panel")
            .resizable(true)
            .default_size(220.0)
            .min_size(120.0)
            .max_size((ui.available_height() * 0.5).max(160.0))
            .show_separator_line(true)
            .frame(
                Frame::NONE
                    .fill(theme.colors.bg_surface)
                    .inner_margin(Margin::same(0)),
            )
            .show(ui, |ui| {
                crate::search_panel::render_search_panel(ui, state, theme, commands);
            });
    }
}

fn render_status_bar(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    ui.horizontal_centered(|ui| {
        let workspace_label = state.workspace_name.as_deref().unwrap_or("—");
        ui.label(
            RichText::new(workspace_label)
                .font(FontId::new(12.0, FontFamily::Proportional))
                .color(theme.colors.text_secondary),
        );
        if state.chord_ctrl_k {
            ui.add_space(10.0);
            ui.label(
                RichText::new("(Ctrl+K) was pressed. Waiting for second key of chord...")
                    .font(FontId::new(12.0, FontFamily::Proportional))
                    .color(theme.colors.accent),
            );
        }
        if state.split_open {
            ui.add_space(8.0);
            let grp = if state.active_group == 0 {
                "Split: Group 1"
            } else {
                "Split: Group 2"
            };
            if status_item(ui, theme, grp).clicked() {
                commands.push(Command::FocusGroup(usize::from(state.active_group == 0)));
            }
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            render_prettier_status_menu(ui, state, theme, commands);

            #[allow(clippy::cast_possible_truncation)]
            let zoom_pct = (state.zoom * 100.0).round() as i32;
            if status_item(ui, theme, &format!("{zoom_pct}%")).clicked() {
                commands.push(Command::ResetZoom);
            }

            if let Some(doc) = state.active_document() {
                render_doc_status_items(ui, doc, theme, commands);
            }
        });
    });
}

fn render_prettier_status_menu(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    let prettier_label = if state.format_on_save {
        "Prettier ✓"
    } else {
        "Prettier"
    };
    let resp =
        status_item(ui, theme, prettier_label).on_hover_text("Built-in Prettier formatter options");
    egui::Popup::menu(&resp).show(|ui| {
        if ui.button("Format Document (Shift+Alt+F)").clicked() {
            commands.push(Command::FormatDocument);
            ui.close();
        }
        if ui.button("Format Selection").clicked() {
            commands.push(Command::FormatSelection);
            ui.close();
        }
        ui.separator();
        let mut fos = state.format_on_save;
        if ui.checkbox(&mut fos, "Format on Save").changed() {
            commands.push(Command::ToggleFormatOnSave);
        }
        let mut rainbow = state.bracket_pair_colorization;
        if ui.checkbox(&mut rainbow, "Rainbow Brackets").changed() {
            commands.push(Command::ToggleBracketColorization);
        }
        let mut guides = state.indent_guides;
        if ui.checkbox(&mut guides, "Indent Guides").changed() {
            commands.push(Command::ToggleIndentGuides);
        }
        ui.separator();
        if ui.button("Open Settings (Ctrl+,)").clicked() {
            commands.push(Command::OpenSettings);
            ui.close();
        }
    });
}

fn render_doc_status_items(
    ui: &mut egui::Ui,
    doc: &jide_core::Document,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    let (line, col) = doc.line_col_of(doc.selection.primary_range().head);
    let selected: usize = doc.selection.ranges.iter().map(jide_core::Range::len).sum();
    let indent = format!(
        "{}: {}",
        if doc.insert_spaces {
            "Spaces"
        } else {
            "Tab Size"
        },
        doc.tab_size
    );
    let lang_label = jide_syntax::find_language_by_id(doc.language.as_str())
        .map_or(doc.language.as_str(), |spec| spec.name);
    let lang_resp = status_item(ui, theme, lang_label).on_hover_text("Select syntax language mode");
    egui::Popup::menu(&lang_resp).show(|ui| {
        egui::ScrollArea::vertical()
            .max_height(320.0)
            .show(ui, |ui| {
                for spec in jide_syntax::all_p0_languages() {
                    let is_cur = doc.language.as_str() == spec.id;
                    if ui.selectable_label(is_cur, spec.name).clicked() {
                        commands.push(Command::SetLanguage(jide_core::LanguageId::new(spec.id)));
                        ui.close();
                    }
                }
            });
    });

    let indent_resp = status_item(ui, theme, &indent).on_hover_text("Configure indentation");
    egui::Popup::menu(&indent_resp).show(|ui| {
        if ui
            .selectable_label(doc.insert_spaces, "Indent Using Spaces")
            .clicked()
        {
            if !doc.insert_spaces {
                commands.push(Command::ToggleInsertSpaces);
            }
            ui.close();
        }
        if ui
            .selectable_label(!doc.insert_spaces, "Indent Using Tabs")
            .clicked()
        {
            if doc.insert_spaces {
                commands.push(Command::ToggleInsertSpaces);
            }
            ui.close();
        }
        ui.separator();
        for sz in [2_usize, 4, 8] {
            if ui
                .selectable_label(doc.tab_size == sz, format!("Tab Size: {sz}"))
                .clicked()
            {
                commands.push(Command::SetTabSize(sz));
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Convert Indentation to Spaces").clicked() {
            commands.push(Command::ConvertIndentToSpaces);
            ui.close();
        }
        if ui.button("Convert Indentation to Tabs").clicked() {
            commands.push(Command::ConvertIndentToTabs);
            ui.close();
        }
    });

    let _ = status_item(ui, theme, doc.display_encoding());

    let eol_resp =
        status_item(ui, theme, doc.eol.label()).on_hover_text("Select end-of-line sequence");
    egui::Popup::menu(&eol_resp).show(|ui| {
        for eol in [Eol::Lf, Eol::CrLf, Eol::Cr] {
            if ui.selectable_label(doc.eol == eol, eol.label()).clicked() {
                commands.push(Command::ConvertEol(eol));
                ui.close();
            }
        }
    });

    if selected > 0 {
        let _ = status_item(ui, theme, &format!("Sel {selected}"));
    }
    if status_item(ui, theme, &format!("Ln {line}, Col {col}"))
        .on_hover_text("Go to Line/Column (Ctrl+G)")
        .clicked()
    {
        commands.push(Command::RunById("__goto_line__".into()));
    }
}

fn status_item(ui: &mut egui::Ui, theme: &Theme, text: &str) -> egui::Response {
    let font = FontId::new(12.0, FontFamily::Proportional);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, theme.colors.text_secondary);
    let size = Vec2::new(galley.size().x + 8.0, STATUS_BAR_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, theme.colors.bg_hover);
    }
    ui.painter().galley(
        pos2(rect.left() + 4.0, rect.center().y - galley.size().y * 0.5),
        galley,
        theme.colors.text_secondary,
    );
    response
}
