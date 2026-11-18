//! Unified overlay widget: Quick Open (Ctrl+P), Command Palette (Ctrl+Shift+P), Go-to-line (Ctrl+G).

use crate::commands::CommandRegistry;
use crate::state::{document_name, AppState, Command, OverlayMode};
use crate::theme::{Theme, ThemeId};
use egui::{
    vec2, Align, FontFamily, FontId, Frame, Key, Layout, Margin, RichText, ScrollArea, Stroke,
};
use jide_workspace::fuzzy::FuzzyIndex;
use std::collections::VecDeque;
use std::path::PathBuf;

const OVERLAY_WIDTH: f32 = 600.0;
const MAX_RESULTS: usize = 50;
const RESULT_HEIGHT: f32 = 28.0;
const INPUT_HEIGHT: f32 = 36.0;

/// Persistent state for the overlay (fuzzy index etc.).
#[derive(Default)]
pub struct PaletteState {
    fuzzy_index: Option<FuzzyIndex>,
    indexed_root: Option<PathBuf>,
    indexed_generation: u64,
    /// MRU command ids (most-recent first).
    command_mru: VecDeque<String>,
    /// Keyboard selection index within current results.
    selected: usize,
}

impl PaletteState {
    /// Call when a folder is opened or the workspace changes.
    pub fn rebuild_fuzzy_index(&mut self, root: &std::path::Path) {
        if let Ok(idx) = FuzzyIndex::build(root.to_path_buf()) {
            self.fuzzy_index = Some(idx);
            self.indexed_root = Some(root.to_path_buf());
        }
    }

    /// Synchronize fuzzy index automatically when `open_folder` or `tree_generation` changes.
    pub fn sync_index(&mut self, state: &AppState) {
        if state.open_folder != self.indexed_root
            || state.tree_generation != self.indexed_generation
        {
            self.indexed_generation = state.tree_generation;
            self.indexed_root.clone_from(&state.open_folder);
            if let Some(root) = &state.open_folder {
                self.fuzzy_index = FuzzyIndex::build(root.clone()).ok();
            } else {
                self.fuzzy_index = None;
            }
        }
    }

    /// Record a command id as recently used.
    pub fn push_mru(&mut self, id: &str) {
        self.command_mru.retain(|c| c != id);
        self.command_mru.push_front(id.to_owned());
        if self.command_mru.len() > 20 {
            self.command_mru.pop_back();
        }
    }
}

/// Render the overlay and return commands.
#[allow(clippy::too_many_lines)]
pub fn render_palette(
    ui: &mut egui::Ui,
    state: &AppState,
    palette: &mut PaletteState,
) -> Vec<Command> {
    let mut commands = Vec::new();
    let Some(mode) = &state.overlay else {
        return commands;
    };

    if *mode == OverlayMode::QuickOpen {
        palette.sync_index(state);
    }

    let theme = &state.theme;

    // Consume keys before the overlay renders so Escape closes and moves don't leak.
    let (close, move_up, move_down, confirm) = ui.input(|inp| {
        let close = inp.key_pressed(Key::Escape);
        let up = inp.key_pressed(Key::ArrowUp);
        let down = inp.key_pressed(Key::ArrowDown);
        let enter = inp.key_pressed(Key::Enter);
        (close, up, down, enter)
    });

    if close {
        commands.push(Command::CloseOverlay);
        return commands;
    }

    // Centered-top position.
    let screen_rect = ui.ctx().input(egui::InputState::viewport_rect);
    let overlay_pos = egui::pos2((screen_rect.width() - OVERLAY_WIDTH) * 0.5, 60.0);

    #[allow(deprecated)]
    egui::Area::new(egui::Id::new("palette_overlay"))
        .order(egui::Order::Foreground)
        .fixed_pos(overlay_pos)
        .show(ui.ctx(), |ui| {
            let max_height = screen_rect.height() * 0.7;
            Frame::NONE
                .fill(theme.colors.bg_elevated)
                .stroke(Stroke::new(1.0, theme.colors.border_strong))
                .corner_radius(egui::CornerRadius::same(8))
                .shadow(egui::Shadow {
                    offset: [0, 0],
                    blur: 16,
                    spread: 2,
                    color: egui::Color32::from_black_alpha(80),
                })
                .inner_margin(Margin::same(0))
                .show(ui, |ui| {
                    ui.set_width(OVERLAY_WIDTH);
                    ui.set_max_height(max_height);

                    // Query input row.
                    Frame::NONE
                        .inner_margin(Margin::symmetric(12, 0))
                        .show(ui, |ui| {
                            ui.set_height(INPUT_HEIGHT);
                            ui.horizontal_centered(|ui| {
                                let mode_prefix = match mode {
                                    OverlayMode::QuickOpen => "",
                                    OverlayMode::CommandPalette => ">",
                                    OverlayMode::GotoLine => ":",
                                    OverlayMode::ThemePicker => "Theme:",
                                };
                                if !mode_prefix.is_empty() {
                                    ui.label(
                                        RichText::new(mode_prefix)
                                            .font(FontId::monospace(14.0))
                                            .color(theme.colors.accent),
                                    );
                                }

                                let mut query = state.overlay_query.clone();
                                let resp = ui.add(
                                    egui::TextEdit::singleline(&mut query)
                                        .desired_width(f32::INFINITY)
                                        .font(FontId::proportional(14.0))
                                        .hint_text(match mode {
                                            OverlayMode::QuickOpen => "Type to search files...",
                                            OverlayMode::CommandPalette => {
                                                "Type to search commands..."
                                            }
                                            OverlayMode::GotoLine => "Go to line (e.g. 42:3)",
                                            OverlayMode::ThemePicker => "Select Color Theme...",
                                        }),
                                );

                                if resp.changed() {
                                    // Emit a special command to propagate query change.
                                    commands.push(Command::RunById(format!(
                                        "__overlay_query__:{query}"
                                    )));
                                }
                                resp.request_focus();
                            });
                        });

                    ui.separator();

                    // Results list.
                    let result_cmds = match mode {
                        OverlayMode::QuickOpen => render_quick_open_results(
                            ui, state, palette, move_up, move_down, confirm,
                        ),
                        OverlayMode::CommandPalette => render_command_palette_results(
                            ui,
                            theme,
                            palette,
                            &state.overlay_query,
                            move_up,
                            move_down,
                            confirm,
                        ),
                        OverlayMode::GotoLine => {
                            render_goto_line_results(ui, theme, &state.overlay_query, confirm)
                        }
                        OverlayMode::ThemePicker => render_theme_picker_results(
                            ui, state, palette, move_up, move_down, confirm,
                        ),
                    };
                    commands.extend(result_cmds);
                });
        });

    commands
}

fn collect_quick_open_paths(state: &AppState, palette: &PaletteState) -> Vec<PathBuf> {
    let query = &state.overlay_query;
    if query.is_empty() {
        let mut paths = Vec::new();
        for &idx in &state.tab_mru {
            if let Some(path) = state.documents.get(idx).and_then(|d| d.path.as_ref()) {
                if !paths.contains(path) {
                    paths.push(path.clone());
                }
            }
        }
        for path in &state.recent_files {
            if !paths.contains(path) {
                paths.push(path.clone());
            }
        }
        paths.truncate(MAX_RESULTS);
        return paths;
    }
    if let Some(idx) = &palette.fuzzy_index {
        return idx
            .filter(query, MAX_RESULTS)
            .into_iter()
            .map(|m| m.path)
            .collect();
    }
    let q_lower = query.to_lowercase();
    state
        .recent_files
        .iter()
        .filter(|p| p.to_string_lossy().to_lowercase().contains(&q_lower))
        .take(MAX_RESULTS)
        .cloned()
        .collect()
}

fn render_quick_open_results(
    ui: &mut egui::Ui,
    state: &AppState,
    palette: &mut PaletteState,
    move_up: bool,
    move_down: bool,
    confirm: bool,
) -> Vec<Command> {
    let mut commands = Vec::new();
    let theme = &state.theme;
    let query = &state.overlay_query;
    let results = collect_quick_open_paths(state, palette);

    let count = results.len();
    if count == 0 {
        ui.with_layout(Layout::top_down(Align::Center), |ui| {
            ui.add_space(16.0);
            ui.label(
                RichText::new(if query.is_empty() {
                    "No folder or recent files open."
                } else {
                    "No results."
                })
                .font(FontId::proportional(13.0))
                .color(theme.colors.text_muted),
            );
            ui.add_space(16.0);
        });
        return commands;
    }

    if move_up && palette.selected > 0 {
        palette.selected -= 1;
    }
    if move_down && palette.selected + 1 < count {
        palette.selected += 1;
    }
    palette.selected = palette.selected.min(count.saturating_sub(1));

    ScrollArea::vertical()
        .max_height(RESULT_HEIGHT * 8.0)
        .show(ui, |ui| {
            for (i, path) in results.iter().enumerate() {
                let selected = palette.selected == i;
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_owned();
                let dir = path
                    .parent()
                    .and_then(|p| p.to_str())
                    .unwrap_or("")
                    .to_owned();

                let (rect, resp) = ui
                    .allocate_exact_size(vec2(OVERLAY_WIDTH, RESULT_HEIGHT), egui::Sense::click());
                if selected || resp.hovered() {
                    ui.painter()
                        .rect_filled(rect, egui::CornerRadius::ZERO, theme.colors.bg_hover);
                }
                ui.painter().text(
                    egui::pos2(rect.left() + 12.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &name,
                    FontId::proportional(13.0),
                    theme.colors.text_primary,
                );
                ui.painter().text(
                    egui::pos2(rect.right() - 12.0, rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    &dir,
                    FontId::proportional(11.0),
                    theme.colors.text_muted,
                );

                if resp.clicked() || (confirm && selected) {
                    commands.push(Command::CloseOverlay);
                    commands.push(Command::OpenFilePath {
                        path: path.clone(),
                        line: 1,
                        col: 1,
                        preview: false,
                    });
                }
            }
        });

    commands
}

#[allow(clippy::too_many_lines)]
fn render_command_palette_results(
    ui: &mut egui::Ui,
    theme: &Theme,
    palette: &mut PaletteState,
    query: &str,
    move_up: bool,
    move_down: bool,
    confirm: bool,
) -> Vec<Command> {
    let mut commands = Vec::new();
    let all = CommandRegistry::all();

    let q_lower = query.to_lowercase();
    let mut listed: Vec<&crate::commands::CommandMeta> = Vec::new();
    if q_lower.is_empty() && !palette.command_mru.is_empty() {
        for id in palette.command_mru.iter().take(5) {
            if let Some(meta) = CommandRegistry::by_id(id) {
                listed.push(meta);
            }
        }
    }
    for cmd in all {
        if (q_lower.is_empty()
            || cmd.title.to_lowercase().contains(&q_lower)
            || cmd.id.to_lowercase().contains(&q_lower)
            || cmd.category.to_lowercase().contains(&q_lower))
            && !listed.iter().any(|existing| existing.id == cmd.id)
        {
            listed.push(cmd);
        }
    }

    let count = listed.len();
    if move_up && palette.selected > 0 {
        palette.selected -= 1;
    }
    if move_down && palette.selected + 1 < count {
        palette.selected += 1;
    }
    palette.selected = palette.selected.min(count.saturating_sub(1));

    if q_lower.is_empty() && !palette.command_mru.is_empty() {
        Frame::NONE
            .inner_margin(Margin::symmetric(12, 4))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("RECENTLY USED")
                        .font(FontId::new(10.0, FontFamily::Proportional))
                        .color(theme.colors.text_secondary),
                );
            });
    }

    ScrollArea::vertical()
        .max_height(RESULT_HEIGHT * 12.0)
        .show(ui, |ui| {
            for (i, cmd) in listed.iter().enumerate() {
                let selected = palette.selected == i;
                let resp = render_command_row(
                    ui,
                    theme,
                    cmd.title,
                    cmd.category,
                    cmd.default_shortcut,
                    selected,
                );
                if resp.clicked() || (confirm && selected) {
                    palette.push_mru(cmd.id);
                    commands.push(Command::CloseOverlay);
                    commands.push(Command::RunById(cmd.id.to_owned()));
                }
            }
        });

    commands
}

/// Renders the Ctrl+Tab MRU tab switcher overlay while `state.mru_switcher_open` is true.
#[must_use]
pub fn render_mru_switcher(ui: &mut egui::Ui, state: &AppState) -> Vec<Command> {
    let mut commands = Vec::new();
    if !state.mru_switcher_open || state.tab_mru.is_empty() {
        return commands;
    }

    let ctrl_held = ui.input(|inp| inp.modifiers.command || inp.modifiers.ctrl);
    if ui.input(|inp| inp.key_pressed(Key::Escape)) {
        commands.push(Command::MruCancel);
        return commands;
    }
    if !ctrl_held {
        commands.push(Command::MruCommit);
        return commands;
    }

    let theme = &state.theme;
    let screen_rect = ui.ctx().input(egui::InputState::viewport_rect);
    let width = 380.0_f32;
    let overlay_pos = egui::pos2((screen_rect.width() - width) * 0.5, 96.0);

    #[allow(deprecated)]
    egui::Area::new(egui::Id::new("mru_switcher_overlay"))
        .order(egui::Order::Foreground)
        .fixed_pos(overlay_pos)
        .show(ui.ctx(), |ui| {
            Frame::NONE
                .fill(theme.colors.bg_elevated)
                .stroke(Stroke::new(1.0, theme.colors.border_strong))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(Margin::same(6))
                .show(ui, |ui| {
                    ui.set_width(width);
                    for (mru_pos, &doc_idx) in state.tab_mru.iter().enumerate() {
                        let Some(doc) = state.documents.get(doc_idx) else {
                            continue;
                        };
                        let name = document_name(doc);
                        let selected = state.mru_switcher_idx == mru_pos;
                        let (rect, resp) = ui.allocate_exact_size(
                            vec2(width - 12.0, RESULT_HEIGHT),
                            egui::Sense::click(),
                        );
                        if selected || resp.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                egui::CornerRadius::same(4),
                                theme.colors.bg_hover,
                            );
                        }
                        ui.painter().text(
                            egui::pos2(rect.left() + 10.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &name,
                            FontId::proportional(13.0),
                            if selected {
                                theme.colors.text_primary
                            } else {
                                theme.colors.text_secondary
                            },
                        );
                        if resp.clicked() {
                            commands.push(Command::SelectTab(doc_idx));
                            commands.push(Command::MruCancel);
                        }
                    }
                });
        });

    commands
}

fn render_command_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    title: &str,
    category: &str,
    shortcut: &str,
    selected: bool,
) -> egui::Response {
    let desired = vec2(OVERLAY_WIDTH, RESULT_HEIGHT);
    let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
    if selected || resp.hovered() {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::ZERO, theme.colors.bg_hover);
    }
    let cat_w = 90.0f32;
    ui.painter().text(
        egui::pos2(rect.left() + 8.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        category,
        FontId::new(11.0, FontFamily::Proportional),
        theme.colors.text_muted,
    );
    ui.painter().text(
        egui::pos2(rect.left() + cat_w, rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        FontId::proportional(13.0),
        theme.colors.text_primary,
    );
    if !shortcut.is_empty() {
        ui.painter().text(
            egui::pos2(rect.right() - 8.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            shortcut,
            FontId::monospace(11.0),
            theme.colors.text_muted,
        );
    }
    resp
}

fn render_goto_line_results(
    ui: &mut egui::Ui,
    theme: &Theme,
    query: &str,
    confirm: bool,
) -> Vec<Command> {
    let mut commands = Vec::new();
    let hint = if query.is_empty() {
        "Type a line number, e.g. 42 or 42:3".to_owned()
    } else {
        format!("Go to line: {query}")
    };
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.add_space(12.0);
        ui.label(
            RichText::new(&hint)
                .font(FontId::proportional(13.0))
                .color(theme.colors.text_secondary),
        );
        ui.add_space(12.0);
    });

    if confirm && !query.is_empty() {
        let (line, col) = parse_line_col(query);
        commands.push(Command::GotoLine { line, col });
    }
    commands
}

fn parse_line_col(s: &str) -> (usize, usize) {
    if let Some((l, c)) = s.split_once(':') {
        let line = l.trim().parse::<usize>().unwrap_or(1);
        let col = c.trim().parse::<usize>().unwrap_or(1);
        (line, col)
    } else {
        let line = s.trim().parse::<usize>().unwrap_or(1);
        (line, 1)
    }
}

fn render_theme_picker_results(
    ui: &mut egui::Ui,
    state: &AppState,
    palette: &mut PaletteState,
    move_up: bool,
    move_down: bool,
    confirm: bool,
) -> Vec<Command> {
    let mut items: Vec<(ThemeId, &str, &str)> = vec![
        (ThemeId::JustDark, "Just Dark", "Built-in Dark"),
        (ThemeId::JustLight, "Just Light", "Built-in Light"),
        (
            ThemeId::HighContrast,
            "High Contrast",
            "Built-in High Contrast",
        ),
    ];
    for custom in &state.available_custom_themes {
        let cat = match custom.kind {
            crate::theme::ThemeKind::Light => "Custom Light",
            crate::theme::ThemeKind::Dark => "Custom Dark",
            crate::theme::ThemeKind::HighContrast => "Custom High Contrast",
        };
        items.push((custom.id.clone(), &custom.name, cat));
    }

    let q_lower = state.overlay_query.to_lowercase();
    items.retain(|(_, name, cat)| {
        q_lower.is_empty()
            || name.to_lowercase().contains(&q_lower)
            || cat.to_lowercase().contains(&q_lower)
    });

    let count = items.len();
    if move_up && palette.selected > 0 {
        palette.selected -= 1;
    }
    if move_down && palette.selected + 1 < count {
        palette.selected += 1;
    }
    palette.selected = palette.selected.min(count.saturating_sub(1));

    let mut commands = Vec::new();
    let theme = &state.theme;
    ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
        for (idx, (id, name, cat)) in items.iter().enumerate() {
            let selected = idx == palette.selected;
            let active_mark = if *id == state.theme.id { "✓" } else { "" };
            let resp = render_command_row(ui, theme, name, cat, active_mark, selected);
            if resp.clicked() || (confirm && selected) {
                commands.push(Command::SwitchTheme(id.clone()));
                commands.push(Command::CloseOverlay);
            }
        }
    });
    commands
}
