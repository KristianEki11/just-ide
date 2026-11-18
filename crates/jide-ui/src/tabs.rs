//! Full tab bar widget supporting preview tabs, pinned tabs, dirty dots, file icons, and actions.

use crate::file_icons::paint_file_icon;
use crate::state::{document_name, AppState, Command};
use crate::theme::Theme;
use egui::{pos2, vec2, Align, Align2, CornerRadius, FontFamily, FontId, Layout, Sense, Stroke};

pub const TAB_HEIGHT: f32 = 36.0;
pub const MIN_TAB_WIDTH: f32 = 96.0;
pub const MAX_TAB_WIDTH: f32 = 200.0;

/// Render the tab bar row and collect emitted commands.
pub fn render_tabs(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    if state.documents.is_empty() {
        return;
    }

    let desired = vec2(ui.available_width(), TAB_HEIGHT);
    let (bar_rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    ui.painter()
        .rect_filled(bar_rect, CornerRadius::ZERO, theme.colors.bg_surface);
    ui.painter().line_segment(
        [bar_rect.left_bottom(), bar_rect.right_bottom()],
        Stroke::new(1.0, theme.colors.border_subtle),
    );

    let actions_w = 104.0;
    let tabs_rect = egui::Rect::from_min_max(
        bar_rect.min,
        pos2(
            (bar_rect.right() - actions_w).max(bar_rect.left() + 40.0),
            bar_rect.bottom(),
        ),
    );
    let actions_rect =
        egui::Rect::from_min_max(pos2(tabs_rect.right(), bar_rect.top()), bar_rect.max);

    ui.scope_builder(egui::UiBuilder::new().max_rect(tabs_rect), |ui| {
        egui::ScrollArea::horizontal()
            .id_salt("editor_tabs_scroll")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_height(TAB_HEIGHT);
                    ui.spacing_mut().item_spacing.x = 0.0;

                    for (index, doc) in state.documents.iter().enumerate() {
                        let active = state.focused_doc_idx() == Some(index);
                        let meta = state.tab_states.get(index);
                        let pinned = meta.is_some_and(|m| m.pinned);
                        let preview = meta.is_some_and(|m| m.preview);
                        let mode = if pinned {
                            TabMode::Pinned
                        } else if preview {
                            TabMode::Preview
                        } else {
                            TabMode::Normal
                        };
                        let dirty = doc.is_dirty();
                        let name = document_name(doc);

                        render_single_tab(
                            ui,
                            state,
                            theme,
                            &TabItem {
                                index,
                                name: &name,
                                active,
                                dirty,
                                mode,
                            },
                            commands,
                        );
                    }
                });
            });
    });

    ui.scope_builder(egui::UiBuilder::new().max_rect(actions_rect), |ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(6.0);
            if tab_action_btn(ui, theme, "⚙", "Open Settings (Ctrl+,)").clicked() {
                commands.push(Command::OpenSettings);
            }
            if tab_action_btn(ui, theme, "◫", "Split Editor Right (Ctrl+\\)").clicked() {
                commands.push(Command::ToggleSplitEditor);
            }
            if tab_action_btn(ui, theme, "{ }", "Prettier: Format Document (Shift+Alt+F)").clicked()
            {
                commands.push(Command::FormatDocument);
            }
        });
    });
}

fn tab_action_btn(ui: &mut egui::Ui, theme: &Theme, label: &str, tooltip: &str) -> egui::Response {
    let size = vec2(26.0, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), theme.colors.bg_hover);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::new(12.0, FontFamily::Monospace),
        if resp.hovered() {
            theme.colors.text_primary
        } else {
            theme.colors.text_secondary
        },
    );
    resp.on_hover_text(tooltip)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TabMode {
    Normal,
    Preview,
    Pinned,
}

#[derive(Clone, Copy)]
struct TabItem<'a> {
    index: usize,
    name: &'a str,
    active: bool,
    dirty: bool,
    mode: TabMode,
}

fn render_single_tab(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    tab: &TabItem<'_>,
    commands: &mut Vec<Command>,
) {
    let preview = tab.mode == TabMode::Preview;
    let pinned = tab.mode == TabMode::Pinned;
    let font = if preview {
        FontId::new(12.5, FontFamily::Proportional)
    } else {
        FontId::new(13.0, FontFamily::Proportional)
    };

    let icon_pad = if state.show_file_icons { 22.0 } else { 0.0 };
    let close_w = if pinned { 6.0 } else { 22.0 };
    let text_w = ui
        .painter()
        .layout_no_wrap(tab.name.to_owned(), font.clone(), theme.colors.text_primary)
        .size()
        .x;
    let tab_w = (text_w + icon_pad + close_w + 22.0).clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH);

    let (rect, resp) = ui.allocate_exact_size(vec2(tab_w, TAB_HEIGHT), Sense::click());

    let bg = if tab.active {
        theme.colors.bg_base
    } else if resp.hovered() {
        theme.colors.bg_hover
    } else {
        theme.colors.bg_surface
    };
    ui.painter().rect_filled(rect, CornerRadius::ZERO, bg);

    // Divider on right edge of each tab.
    ui.painter().line_segment(
        [
            pos2(rect.right(), rect.top()),
            pos2(rect.right(), rect.bottom()),
        ],
        Stroke::new(1.0, theme.colors.border_subtle),
    );

    if tab.active {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.min, vec2(rect.width(), 2.0)),
            0.0,
            theme.colors.accent,
        );
    }

    let mut label_x = rect.left() + 10.0;
    if state.show_file_icons {
        let icon_rect =
            egui::Rect::from_center_size(pos2(label_x + 7.0, rect.center().y), vec2(14.0, 14.0));
        paint_file_icon(ui.painter(), icon_rect, tab.name, theme);
        label_x += 20.0;
    }

    let text_color = if tab.active {
        theme.colors.text_primary
    } else if preview {
        theme.colors.text_muted
    } else {
        theme.colors.text_secondary
    };
    let text_pos = pos2(label_x, rect.center().y);
    ui.painter()
        .text(text_pos, Align2::LEFT_CENTER, tab.name, font, text_color);

    let icon_rect =
        egui::Rect::from_center_size(pos2(rect.right() - 13.0, rect.center().y), vec2(18.0, 18.0));

    if pinned {
        ui.painter().text(
            icon_rect.center(),
            Align2::CENTER_CENTER,
            "P",
            FontId::new(10.0, FontFamily::Monospace),
            theme.colors.accent,
        );
    } else if tab.dirty && !resp.hovered() {
        ui.painter()
            .circle_filled(icon_rect.center(), 3.2, theme.colors.accent);
    } else {
        let close_resp = ui.interact(
            icon_rect,
            ui.scope_id().with(("close_tab", tab.index)),
            Sense::click(),
        );
        if close_resp.hovered() {
            ui.painter()
                .rect_filled(icon_rect, CornerRadius::same(4), theme.colors.bg_active);
        }
        let close_color = if close_resp.hovered() {
            theme.colors.text_primary
        } else {
            theme.colors.text_muted
        };
        ui.painter().text(
            icon_rect.center(),
            Align2::CENTER_CENTER,
            "×",
            FontId::new(13.0, FontFamily::Proportional),
            close_color,
        );
        if close_resp.clicked() {
            commands.push(Command::CloseTab(tab.index));
        }
    }

    render_tab_context_menu(&resp, tab.index, pinned, commands);

    if resp.middle_clicked() {
        commands.push(Command::CloseTab(tab.index));
    } else if resp.clicked() || resp.double_clicked() {
        commands.push(Command::SelectTab(tab.index));
    }
}

fn render_tab_context_menu(
    resp: &egui::Response,
    index: usize,
    pinned: bool,
    commands: &mut Vec<Command>,
) {
    resp.context_menu(|ui| {
        if ui.button("Close").clicked() {
            commands.push(Command::CloseTab(index));
            ui.close();
        }
        if ui.button("Close Others").clicked() {
            commands.push(Command::CloseOtherTabs(index));
            ui.close();
        }
        if ui.button("Close to the Right").clicked() {
            commands.push(Command::CloseTabsToRight(index));
            ui.close();
        }
        if ui.button("Close Saved").clicked() {
            commands.push(Command::CloseAllSavedTabs);
            ui.close();
        }
        if ui.button("Close All").clicked() {
            commands.push(Command::CloseAllTabs);
            ui.close();
        }
        ui.separator();
        if pinned {
            if ui.button("Unpin Tab").clicked() {
                commands.push(Command::UnpinTab(index));
                ui.close();
            }
        } else if ui.button("Pin Tab").clicked() {
            commands.push(Command::PinTab(index));
            ui.close();
        }
    });
}
