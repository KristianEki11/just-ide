//! File tree explorer panel backed by `jide_workspace::tree::FileTree`.

use crate::file_icons::{paint_chevron, paint_file_icon, paint_folder_icon};
use crate::state::{document_name, paths_match, AppState, Command};
use crate::theme::Theme;
use egui::{
    pos2, vec2, Align2, CornerRadius, FontFamily, FontId, Frame, Margin, RichText, Sense, Stroke,
};
use jide_workspace::tree::TreeNode;
use std::path::Path;

const ROW_HEIGHT: f32 = 24.0;
const INDENT: f32 = 14.0;
const BASE_PAD_X: f32 = 10.0;

/// Render the file-tree explorer sidebar content and collect commands.
pub fn render_file_tree(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    Frame::NONE.inner_margin(Margin::same(0)).show(ui, |ui| {
        render_explorer_title_bar(ui, theme);

        if state.open_folder.is_none() {
            if !state.documents.is_empty() {
                render_open_editors_section(ui, state, theme, commands);
            }
            render_no_folder(ui, theme, commands);
            return;
        }

        let folder_name = state
            .workspace_name
            .as_deref()
            .unwrap_or("WORKSPACE")
            .to_uppercase();

        egui::ScrollArea::vertical()
            .id_salt("explorer_tree_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if !state.documents.is_empty() {
                    render_open_editors_section(ui, state, theme, commands);
                }

                if let Some(tree) = &state.file_tree {
                    let count = tree.root_node.children.as_ref().map_or(0, Vec::len);
                    render_section_header(ui, theme, &folder_name, Some(count));
                    if let Some(children) = &tree.root_node.children {
                        for child in children {
                            render_node(ui, theme, child, 0, state, commands);
                        }
                    }
                    ui.add_space(12.0);
                }
            });
    });
}

fn render_explorer_title_bar(ui: &mut egui::Ui, theme: &Theme) {
    let rect = ui.allocate_space(vec2(ui.available_width(), 28.0)).1;
    ui.painter().text(
        pos2(rect.left() + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        "EXPLORER",
        FontId::new(11.0, FontFamily::Proportional),
        theme.colors.text_secondary,
    );
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, theme.colors.border_subtle),
    );
}

fn render_open_editors_section(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    render_section_header(ui, theme, "OPEN EDITORS", Some(state.documents.len()));
    for (idx, doc) in state.documents.iter().enumerate() {
        let active = state.focused_doc_idx() == Some(idx);
        let name = document_name(doc);
        let preview = state.tab_states.get(idx).is_some_and(|m| m.preview);
        let dirty = doc.is_dirty();
        let row_resp = render_file_row(
            ui,
            theme,
            state,
            &TreeRow {
                depth: 0,
                path: doc.path.as_deref(),
                name: &name,
                kind: NodeKind::File,
                active,
                preview,
                dirty,
            },
            commands,
        );
        if row_resp.clicked() || row_resp.double_clicked() {
            commands.push(Command::SelectTab(idx));
        }
    }
    ui.add_space(6.0);
}

fn render_no_folder(ui: &mut egui::Ui, theme: &Theme, commands: &mut Vec<Command>) {
    Frame::NONE
        .inner_margin(Margin::symmetric(12, 12))
        .show(ui, |ui| {
            ui.label(
                RichText::new("No folder opened.")
                    .font(FontId::new(12.5, FontFamily::Proportional))
                    .color(theme.colors.text_muted),
            );
            ui.add_space(8.0);
            let btn = egui::Button::new(
                RichText::new("Open Folder...")
                    .font(FontId::new(12.5, FontFamily::Proportional))
                    .color(theme.colors.text_on_accent),
            )
            .fill(theme.colors.accent)
            .corner_radius(CornerRadius::same(4))
            .min_size(vec2(ui.available_width(), 28.0));
            if ui.add(btn).clicked() {
                commands.push(Command::OpenFolder);
            }
        });
}

fn render_section_header(ui: &mut egui::Ui, theme: &Theme, text: &str, count: Option<usize>) {
    let desired = vec2(ui.available_width(), 22.0);
    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, theme.colors.bg_surface);
    paint_chevron(
        ui.painter(),
        pos2(rect.left() + 10.0, rect.center().y),
        true,
        theme.colors.text_secondary,
    );
    ui.painter().text(
        pos2(rect.left() + 20.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        FontId::new(10.5, FontFamily::Proportional),
        theme.colors.text_secondary,
    );
    if let Some(n) = count {
        ui.painter().text(
            pos2(rect.right() - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            format!("{n}"),
            FontId::new(10.0, FontFamily::Monospace),
            theme.colors.text_muted,
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeKind {
    File,
    Dir { expanded: bool },
}

struct TreeRow<'a> {
    depth: usize,
    path: Option<&'a Path>,
    name: &'a str,
    kind: NodeKind,
    active: bool,
    preview: bool,
    dirty: bool,
}

#[allow(clippy::too_many_lines)]
fn render_file_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &AppState,
    row: &TreeRow<'_>,
    commands: &mut Vec<Command>,
) -> egui::Response {
    let indent = f32::from(u16::try_from(row.depth).unwrap_or(32)) * INDENT;
    let desired = vec2(ui.available_width(), ROW_HEIGHT);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());

    let pill = rect.shrink2(vec2(4.0, 1.0));
    if row.active {
        ui.painter()
            .rect_filled(pill, CornerRadius::same(4), theme.colors.bg_active);
    } else if resp.hovered() {
        ui.painter()
            .rect_filled(pill, CornerRadius::same(4), theme.colors.bg_hover);
    }

    if state.show_tree_guides {
        for level in 0..row.depth {
            let guide_x = rect.left()
                + BASE_PAD_X
                + 4.0
                + f32::from(u16::try_from(level).unwrap_or(32)) * INDENT;
            ui.painter().line_segment(
                [pos2(guide_x, rect.top()), pos2(guide_x, rect.bottom())],
                Stroke::new(1.0, theme.colors.editor_indent_guide),
            );
        }
    }

    let x_start = rect.left() + BASE_PAD_X + indent;
    let cy = rect.center().y;

    let label_x = match row.kind {
        NodeKind::Dir { expanded } => {
            paint_chevron(
                ui.painter(),
                pos2(x_start + 4.0, cy),
                expanded,
                theme.colors.text_secondary,
            );
            if state.show_file_icons {
                let icon_rect =
                    egui::Rect::from_center_size(pos2(x_start + 20.0, cy), vec2(14.0, 13.0));
                paint_folder_icon(ui.painter(), icon_rect, row.name, expanded, theme);
                x_start + 32.0
            } else {
                x_start + 14.0
            }
        }
        NodeKind::File => {
            if state.show_file_icons {
                let icon_rect =
                    egui::Rect::from_center_size(pos2(x_start + 18.0, cy), vec2(15.0, 15.0));
                paint_file_icon(ui.painter(), icon_rect, row.name, theme);
                x_start + 31.0
            } else {
                x_start + 14.0
            }
        }
    };

    let label_color = if row.active || matches!(row.kind, NodeKind::Dir { .. }) {
        theme.colors.text_primary
    } else if row.preview {
        theme.colors.text_muted
    } else {
        theme.colors.text_secondary
    };

    let font = if row.preview {
        FontId::new(12.5, FontFamily::Proportional)
    } else {
        FontId::new(13.0, FontFamily::Proportional)
    };
    let max_label_w = (rect.right() - label_x - 20.0).max(20.0);
    let galley = ui
        .painter()
        .layout(row.name.to_owned(), font, label_color, max_label_w);
    ui.painter().galley(
        pos2(label_x, cy - galley.size().y * 0.5),
        galley,
        label_color,
    );

    if row.dirty {
        ui.painter()
            .circle_filled(pos2(rect.right() - 12.0, cy), 3.0, theme.colors.accent);
    }

    if let Some(path) = row.path {
        resp.context_menu(|ui| {
            if ui.button("Copy path").clicked() {
                commands.push(Command::CopyPath(path.to_path_buf()));
                ui.close();
            }
            if ui.button("Reveal in File Explorer").clicked() {
                commands.push(Command::RevealInFileManager(path.to_path_buf()));
                ui.close();
            }
        });
    }

    resp
}

fn render_node(
    ui: &mut egui::Ui,
    theme: &Theme,
    node: &TreeNode,
    depth: usize,
    state: &AppState,
    commands: &mut Vec<Command>,
) {
    let open_match = (!node.is_dir).then(|| {
        state.documents.iter().enumerate().find(|(_, d)| {
            d.path
                .as_deref()
                .is_some_and(|p| paths_match(p, &node.path))
        })
    });
    let matched = open_match.flatten();
    let active = matched.is_some_and(|(idx, _)| state.focused_doc_idx() == Some(idx));
    let preview =
        matched.is_some_and(|(idx, _)| state.tab_states.get(idx).is_some_and(|m| m.preview));
    let dirty = matched.is_some_and(|(_, doc)| doc.is_dirty());
    let kind = if node.is_dir {
        NodeKind::Dir {
            expanded: node.expanded,
        }
    } else {
        NodeKind::File
    };

    let resp = render_file_row(
        ui,
        theme,
        state,
        &TreeRow {
            depth,
            path: Some(&node.path),
            name: &node.name,
            kind,
            active,
            preview,
            dirty,
        },
        commands,
    );

    if node.is_dir {
        if resp.clicked() {
            commands.push(Command::ToggleFolderNode(node.path.clone()));
        }
        if node.expanded {
            if let Some(children) = &node.children {
                for child in children {
                    render_node(ui, theme, child, depth + 1, state, commands);
                }
            }
        }
    } else if resp.double_clicked() {
        commands.push(Command::OpenFilePath {
            path: node.path.clone(),
            line: 1,
            col: 1,
            preview: false,
        });
    } else if resp.clicked() {
        commands.push(Command::OpenFilePath {
            path: node.path.clone(),
            line: 1,
            col: 1,
            preview: true,
        });
    }
}
