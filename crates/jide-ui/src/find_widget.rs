//! Floating Find & Replace widget (`04-UX-SPEC.md` §5.1) and buffer match state.

use crate::state::{AppState, Command};
use crate::theme::Theme;
use egui::{vec2, Align, CornerRadius, FontId, Frame, Key, Layout, Margin, RichText, Stroke};
use jide_core::{DocId, Document, Range, Selection};
use jide_search::in_buffer::{find_in_rope, BufferMatch, SearchQuery};

const WIDGET_WIDTH: f32 = 420.0;

/// Persistent state for in-buffer Find & Replace stored on [`AppState`].
#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct FindState {
    pub open: bool,
    pub replace_open: bool,
    pub query: String,
    pub replacement: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub use_regex: bool,
    pub in_selection: bool,
    pub selection_bounds: Option<Range>,
    pub matches: Vec<BufferMatch>,
    pub active_match: usize,
    pub regex_error: Option<String>,
    pub cached_doc_id: Option<DocId>,
    pub cached_version: u64,
    pub searching: bool,
    /// When `open` became true (triggers input focus request once).
    pub request_focus: bool,
}

impl FindState {
    /// Builds a [`SearchQuery`] reflecting current widget toggles.
    #[must_use]
    pub fn to_query(&self) -> SearchQuery {
        SearchQuery {
            pattern: self.query.clone(),
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
            is_regex: self.use_regex,
            in_selection: if self.in_selection {
                self.selection_bounds
            } else {
                None
            },
        }
    }

    /// Recomputes in-buffer matches synchronously (for files <= 1 MB) or clears matches if empty.
    pub fn recompute_sync(&mut self, doc: Option<&Document>) {
        let Some(doc) = doc else {
            self.matches.clear();
            self.active_match = 0;
            self.regex_error = None;
            self.searching = false;
            return;
        };

        self.cached_doc_id = Some(doc.id);
        self.cached_version = doc.version;

        if self.query.is_empty() {
            self.matches.clear();
            self.active_match = 0;
            self.regex_error = None;
            self.searching = false;
            return;
        }

        let query = self.to_query();
        match find_in_rope(&doc.rope, &query, None) {
            Ok(found) => {
                self.matches = found;
                self.regex_error = None;
                self.searching = false;
                if self.matches.is_empty() {
                    self.active_match = 0;
                } else {
                    let head = doc.selection.primary_range().head;
                    self.active_match = self
                        .matches
                        .iter()
                        .position(|m| m.start_char >= head)
                        .unwrap_or(0);
                }
            }
            Err(err) => {
                self.matches.clear();
                self.active_match = 0;
                self.regex_error = Some(err.to_string());
                self.searching = false;
            }
        }
    }

    /// Opens the widget, pre-filling from active document selection when applicable.
    pub fn open_with_doc(&mut self, doc: Option<&Document>, replace: bool) {
        self.open = true;
        if replace {
            self.replace_open = true;
        }
        self.request_focus = true;

        if let Some(doc) = doc {
            let primary = doc.selection.primary_range();
            if !primary.is_empty() {
                let selected = doc.selected_text();
                if selected.contains('\n') {
                    self.in_selection = true;
                    self.selection_bounds = Some(primary);
                } else if !selected.is_empty() {
                    self.query = selected;
                    self.in_selection = false;
                    self.selection_bounds = None;
                }
            }
        }
    }

    /// Advances active match (`forward = true` for next, `false` for prev) with wrap-around.
    pub fn step_match(&mut self, doc: &mut Document, forward: bool) -> bool {
        if self.matches.is_empty() {
            return false;
        }
        if forward {
            self.active_match = (self.active_match + 1) % self.matches.len();
        } else {
            self.active_match = self
                .active_match
                .checked_sub(1)
                .unwrap_or(self.matches.len() - 1);
        }
        if let Some(m) = self.matches.get(self.active_match) {
            doc.selection = Selection::single(Range {
                anchor: m.start_char,
                head: m.end_char,
            });
            return true;
        }
        false
    }

    /// Selects every current match as simultaneous multi-cursors (`Alt+Enter`).
    pub fn select_all_matches(&self, doc: &mut Document) -> usize {
        if self.matches.is_empty() {
            return 0;
        }
        let ranges: Vec<Range> = self
            .matches
            .iter()
            .map(|m| Range {
                anchor: m.start_char,
                head: m.end_char,
            })
            .collect();
        let count = ranges.len();
        doc.selection = Selection::new(ranges, 0);
        count
    }

    /// Formatted counter badge text (`"3 of 27"`, `"No results"`, etc.).
    #[must_use]
    pub fn counter_label(&self) -> String {
        if self.query.is_empty() {
            String::new()
        } else if self.searching {
            "Searching...".to_owned()
        } else if self.matches.is_empty() {
            "No results".to_owned()
        } else {
            format!("{} of {}", self.active_match + 1, self.matches.len())
        }
    }
}

/// Renders the floating top-right Find/Replace card inside the editor area.
#[must_use]
pub fn render_find_widget(
    ui: &mut egui::Ui,
    state: &AppState,
    editor_rect: egui::Rect,
) -> Vec<Command> {
    let mut commands = Vec::new();
    if !state.find.open {
        return commands;
    }

    let theme = &state.theme;
    let pos = egui::pos2(
        (editor_rect.right() - WIDGET_WIDTH - 16.0).max(editor_rect.left() + 8.0),
        editor_rect.top() + 8.0,
    );

    #[allow(deprecated)]
    egui::Area::new(egui::Id::new("find_replace_widget"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ui.ctx(), |ui| {
            Frame::NONE
                .fill(theme.colors.bg_elevated)
                .stroke(Stroke::new(1.0, theme.colors.border_strong))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::same(8))
                .show(ui, |ui| {
                    ui.set_width(WIDGET_WIDTH - 16.0);
                    render_find_row(ui, state, theme, &mut commands);
                    if let Some(err) = &state.find.regex_error {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(err)
                                .font(FontId::proportional(11.0))
                                .color(theme.colors.danger),
                        );
                    }
                    if state.find.replace_open {
                        ui.add_space(6.0);
                        render_replace_row(ui, state, theme, &mut commands);
                    }
                });
        });

    commands
}

fn render_find_row(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let chevron = if state.find.replace_open {
            "▾"
        } else {
            "▸"
        };
        if ui
            .small_button(chevron)
            .on_hover_text("Toggle Replace (Ctrl+H)")
            .clicked()
        {
            commands.push(Command::ToggleFindReplaceRow);
        }

        let mut query = state.find.query.clone();
        let resp = ui.add(
            egui::TextEdit::singleline(&mut query)
                .desired_width(150.0)
                .hint_text("Find")
                .font(FontId::monospace(12.5)),
        );
        if state.find.request_focus {
            resp.request_focus();
            commands.push(Command::AckSearchFocus);
        }
        if resp.changed() {
            commands.push(Command::FindQueryChanged(query));
        }

        if resp.has_focus() {
            handle_find_input_keys(ui, commands);
        }

        toggle_chip(
            ui,
            theme,
            "Aa",
            state.find.case_sensitive,
            "Match Case",
            Command::ToggleFindCase,
            commands,
        );
        toggle_chip(
            ui,
            theme,
            "ab",
            state.find.whole_word,
            "Match Whole Word",
            Command::ToggleFindWholeWord,
            commands,
        );
        toggle_chip(
            ui,
            theme,
            ".*",
            state.find.use_regex,
            "Use Regular Expression",
            Command::ToggleFindRegex,
            commands,
        );
        toggle_chip(
            ui,
            theme,
            "≡",
            state.find.in_selection,
            "Find in Selection",
            Command::ToggleFindInSelection,
            commands,
        );

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.small_button("✕").on_hover_text("Close (Esc)").clicked() {
                commands.push(Command::CloseFind);
            }
            if ui
                .small_button("↓")
                .on_hover_text("Next Match (F3)")
                .clicked()
            {
                commands.push(Command::FindNext);
            }
            if ui
                .small_button("↑")
                .on_hover_text("Previous Match (Shift+F3)")
                .clicked()
            {
                commands.push(Command::FindPrev);
            }
            let badge = state.find.counter_label();
            if !badge.is_empty() {
                ui.label(RichText::new(badge).font(FontId::proportional(11.0)).color(
                    if state.find.matches.is_empty() {
                        theme.colors.danger
                    } else {
                        theme.colors.text_secondary
                    },
                ));
            }
        });
    });
}

fn handle_find_input_keys(ui: &egui::Ui, commands: &mut Vec<Command>) {
    ui.input(|inp| {
        if inp.key_pressed(Key::Escape) {
            commands.push(Command::CloseFind);
        } else if inp.key_pressed(Key::Enter) {
            if inp.modifiers.alt {
                commands.push(Command::FindSelectAllMatches);
            } else if inp.modifiers.shift {
                commands.push(Command::FindPrev);
            } else {
                commands.push(Command::FindNext);
            }
        }
    });
}

fn render_replace_row(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.add_space(22.0);
        let mut replacement = state.find.replacement.clone();
        let resp = ui.add(
            egui::TextEdit::singleline(&mut replacement)
                .desired_width(196.0)
                .hint_text("Replace (use $1 for capture groups)")
                .font(FontId::monospace(12.5)),
        );
        if resp.changed() {
            commands.push(Command::ReplaceTextChanged(replacement));
        }
        if resp.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
            commands.push(Command::CloseFind);
        }
        let one_btn = egui::Button::new(
            RichText::new("⇄ one")
                .font(FontId::proportional(11.5))
                .color(theme.colors.text_primary),
        )
        .corner_radius(CornerRadius::same(4));
        if ui.add(one_btn).on_hover_text("Replace current").clicked() {
            commands.push(Command::ReplaceCurrentMatch);
        }
        let all_btn = egui::Button::new(
            RichText::new("⇄ all")
                .font(FontId::proportional(11.5))
                .color(theme.colors.text_primary),
        )
        .corner_radius(CornerRadius::same(4));
        if ui
            .add(all_btn)
            .on_hover_text("Replace all matches")
            .clicked()
        {
            commands.push(Command::ReplaceAllMatches);
        }
    });
}

pub(crate) fn toggle_chip(
    ui: &mut egui::Ui,
    theme: &Theme,
    label: &str,
    active: bool,
    tooltip: &str,
    cmd: Command,
    commands: &mut Vec<Command>,
) {
    let mut btn = egui::Button::new(RichText::new(label).font(FontId::monospace(11.0)).color(
        if active {
            theme.colors.text_on_accent
        } else {
            theme.colors.text_secondary
        },
    ))
    .corner_radius(CornerRadius::same(4))
    .min_size(vec2(24.0, 22.0));
    if active {
        btn = btn.fill(theme.colors.accent);
    }
    if ui.add(btn).on_hover_text(tooltip).clicked() {
        commands.push(cmd);
    }
}
