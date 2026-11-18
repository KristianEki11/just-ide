//! Project-wide Find & Replace in Files panel (`04-UX-SPEC.md` §5.2).

use crate::find_widget::toggle_chip;
use crate::state::{AppState, Command};
use crate::theme::Theme;
use egui::{CornerRadius, FontFamily, FontId, Frame, Margin, RichText, ScrollArea, Stroke};
use jide_search::in_files::{FileMatchLine, FileSearchResult, FilesSearchQuery};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// State for workspace-wide streaming Find & Replace in Files.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct ProjectSearchState {
    pub query: String,
    pub replacement: String,
    pub replace_expanded: bool,
    pub include_glob: String,
    pub exclude_glob: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub use_regex: bool,
    pub use_gitignore: bool,
    pub results: Vec<FileSearchResult>,
    pub collapsed_files: HashSet<PathBuf>,
    pub total_matches: usize,
    pub searching: bool,
    pub elapsed_ms: u64,
    pub search_id: u64,
    pub regex_error: Option<String>,
    /// Confirmation modal counts: `(file_count, match_count)`.
    pub confirm_replace: Option<(usize, usize)>,
    pub in_bottom_panel: bool,
    pub request_focus: bool,
}

impl Default for ProjectSearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            replacement: String::new(),
            replace_expanded: false,
            include_glob: String::new(),
            exclude_glob: String::new(),
            case_sensitive: false,
            whole_word: false,
            use_regex: false,
            use_gitignore: true,
            results: Vec::new(),
            collapsed_files: HashSet::new(),
            total_matches: 0,
            searching: false,
            elapsed_ms: 0,
            search_id: 0,
            regex_error: None,
            confirm_replace: None,
            in_bottom_panel: false,
            request_focus: false,
        }
    }
}

impl ProjectSearchState {
    /// Converts panel fields into a [`FilesSearchQuery`] rooted at `root`.
    #[must_use]
    pub fn to_files_query(&self, root: PathBuf) -> FilesSearchQuery {
        let mut defaults = FilesSearchQuery::default();
        let include_globs = split_globs(&self.include_glob);
        let mut exclude_globs = defaults.exclude_globs;
        exclude_globs.extend(split_globs(&self.exclude_glob));

        defaults.root = root;
        defaults.pattern.clone_from(&self.query);
        defaults.case_sensitive = self.case_sensitive;
        defaults.whole_word = self.whole_word;
        defaults.is_regex = self.use_regex;
        defaults.include_globs = include_globs;
        defaults.exclude_globs = exclude_globs;
        defaults.use_gitignore = self.use_gitignore;
        defaults
    }

    /// Resets streamed results cleanly and allocates a new `search_id`.
    pub fn begin_new_search(&mut self) -> u64 {
        self.search_id = self.search_id.wrapping_add(1);
        self.results.clear();
        self.collapsed_files.clear();
        self.total_matches = 0;
        self.elapsed_ms = 0;
        self.regex_error = None;
        self.searching = !self.query.is_empty();
        self.search_id
    }

    /// Appends a streamed batch if `batch_id` matches the latest `search_id`.
    pub fn push_batch(&mut self, batch_id: u64, batch: Vec<FileSearchResult>) -> bool {
        if batch_id != self.search_id {
            return false;
        }
        for file_res in batch {
            let file_hits: usize = file_res.matches.iter().map(|m| m.submatches.len()).sum();
            self.total_matches += file_hits;
            if let Some(existing) = self.results.iter_mut().find(|r| r.path == file_res.path) {
                existing.matches.extend(file_res.matches);
            } else {
                self.results.push(file_res);
            }
        }
        true
    }

    /// Formatted summary string (`"1,204 results in 87 files · 0.42 s"`).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn summary_label(&self) -> String {
        if self.query.is_empty() {
            return "Type to search across workspace files.".to_owned();
        }
        if self.searching && self.results.is_empty() {
            return "Searching...".to_owned();
        }
        let secs = self.elapsed_ms as f64 / 1000.0;
        let prefix = if self.searching { "Searching · " } else { "" };
        format!(
            "{prefix}{} results in {} files · {secs:.2} s",
            self.total_matches,
            self.results.len()
        )
    }
}

fn split_globs(input: &str) -> Vec<String> {
    input
        .split([',', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Renders the Search in Files panel (either in sidebar or bottom panel).
pub fn render_search_panel(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    Frame::NONE
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            render_header(ui, state, theme, commands);
            if state.open_folder.is_none() {
                ui.add_space(10.0);
                ui.label(
                    RichText::new("Open a folder to search across files.")
                        .font(FontId::new(13.0, FontFamily::Proportional))
                        .color(theme.colors.text_muted),
                );
                return;
            }
            ui.add_space(6.0);
            render_inputs(ui, state, theme, commands);
            if let Some(err) = &state.project_search.regex_error {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(err)
                        .font(FontId::proportional(11.0))
                        .color(theme.colors.danger),
                );
            }
            ui.add_space(6.0);
            ui.label(
                RichText::new(state.project_search.summary_label())
                    .font(FontId::proportional(11.5))
                    .color(theme.colors.text_secondary),
            );
            ui.add_space(4.0);
            render_results_list(ui, state, theme, commands);
        });
}

fn render_header(ui: &mut egui::Ui, state: &AppState, theme: &Theme, commands: &mut Vec<Command>) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("SEARCH")
                .font(FontId::new(11.0, FontFamily::Proportional))
                .color(theme.colors.text_secondary)
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let panel_tip = if state.project_search.in_bottom_panel {
                "Dock Search in Sidebar (Ctrl+J)"
            } else {
                "Dock Search in Bottom Panel (Ctrl+J)"
            };
            if ui.small_button("⬓").on_hover_text(panel_tip).clicked() {
                commands.push(Command::ToggleBottomSearchPanel);
            }
            if state.project_search.searching
                && ui
                    .small_button("⏹")
                    .on_hover_text("Cancel running search")
                    .clicked()
            {
                commands.push(Command::CancelProjectSearch);
            }
        });
    });
}

fn render_inputs(ui: &mut egui::Ui, state: &AppState, theme: &Theme, commands: &mut Vec<Command>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let chevron = if state.project_search.replace_expanded {
            "▾"
        } else {
            "▸"
        };
        if ui
            .small_button(chevron)
            .on_hover_text("Toggle Replace in Files (Ctrl+Shift+H)")
            .clicked()
        {
            commands.push(Command::ToggleProjectReplaceRow);
        }
        let mut query = state.project_search.query.clone();
        let resp = ui.add(
            egui::TextEdit::singleline(&mut query)
                .desired_width((ui.available_width() - 4.0).max(80.0))
                .hint_text("Search")
                .font(FontId::monospace(12.5)),
        );
        if state.project_search.request_focus {
            resp.request_focus();
            commands.push(Command::AckSearchFocus);
        }
        if resp.changed() {
            commands.push(Command::ProjectSearchQueryChanged(query));
        }
    });

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        toggle_chip(
            ui,
            theme,
            "Aa",
            state.project_search.case_sensitive,
            "Match Case",
            Command::ToggleProjectCase,
            commands,
        );
        toggle_chip(
            ui,
            theme,
            "ab",
            state.project_search.whole_word,
            "Match Whole Word",
            Command::ToggleProjectWholeWord,
            commands,
        );
        toggle_chip(
            ui,
            theme,
            ".*",
            state.project_search.use_regex,
            "Use Regular Expression",
            Command::ToggleProjectRegex,
            commands,
        );
        toggle_chip(
            ui,
            theme,
            ".git",
            state.project_search.use_gitignore,
            "Respect .gitignore & excludes",
            Command::ToggleProjectGitignore,
            commands,
        );
    });

    if state.project_search.replace_expanded {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let mut replacement = state.project_search.replacement.clone();
            let resp = ui.add(
                egui::TextEdit::singleline(&mut replacement)
                    .desired_width((ui.available_width() - 78.0).max(60.0))
                    .hint_text("Replace in files")
                    .font(FontId::monospace(12.5)),
            );
            if resp.changed() {
                commands.push(Command::ProjectReplaceChanged(replacement));
            }
            let enabled =
                !state.project_search.query.is_empty() && state.project_search.total_matches > 0;
            if ui
                .add_enabled(enabled, egui::Button::new("Replace All"))
                .clicked()
            {
                commands.push(Command::ProjectReplacePreviewRequest);
            }
        });
    }

    ui.add_space(4.0);
    let mut include = state.project_search.include_glob.clone();
    if ui
        .add(
            egui::TextEdit::singleline(&mut include)
                .desired_width(ui.available_width())
                .hint_text("files to include (e.g. *.rs, src/**)")
                .font(FontId::proportional(11.5)),
        )
        .changed()
    {
        commands.push(Command::ProjectIncludeChanged(include));
    }

    ui.add_space(3.0);
    let mut exclude = state.project_search.exclude_glob.clone();
    if ui
        .add(
            egui::TextEdit::singleline(&mut exclude)
                .desired_width(ui.available_width())
                .hint_text("files to exclude (e.g. *.lock, tests/**)")
                .font(FontId::proportional(11.5)),
        )
        .changed()
    {
        commands.push(Command::ProjectExcludeChanged(exclude));
    }
}

fn render_results_list(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    let root = state.open_folder.as_deref();
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for file_res in &state.project_search.results {
                let collapsed = state
                    .project_search
                    .collapsed_files
                    .contains(&file_res.path);
                let hit_count: usize = file_res.matches.iter().map(|m| m.submatches.len()).sum();
                let (name, rel) = format_file_labels(&file_res.path, root);
                let arrow = if collapsed { "▸" } else { "▾" };
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(false, format!("{arrow} {name}  ({hit_count})"))
                        .on_hover_text(&rel)
                        .clicked()
                    {
                        commands.push(Command::ProjectSearchToggleFile(file_res.path.clone()));
                    }
                    ui.label(
                        RichText::new(&rel)
                            .font(FontId::proportional(11.0))
                            .color(theme.colors.text_muted),
                    );
                });
                if !collapsed {
                    for line_match in &file_res.matches {
                        render_match_line(ui, theme, &file_res.path, line_match, commands);
                    }
                }
            }
        });
}

fn format_file_labels(path: &Path, root: Option<&Path>) -> (String, String) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_owned();
    let rel = root
        .and_then(|r| path.strip_prefix(r).ok())
        .and_then(|p| p.parent())
        .map_or_else(String::new, |p| p.display().to_string());
    (name, rel)
}

fn render_match_line(
    ui: &mut egui::Ui,
    theme: &Theme,
    file_path: &Path,
    line_match: &FileMatchLine,
    commands: &mut Vec<Command>,
) {
    let trimmed = line_match.line_text.trim();
    let col = line_match.submatches.first().map_or(1, |(s, _)| {
        line_match.line_text[..(*s).min(line_match.line_text.len())]
            .chars()
            .count()
            + 1
    });
    let label = format!("  {:>4}: {}", line_match.line_number, trimmed);
    let resp = ui.selectable_label(
        false,
        RichText::new(label)
            .font(FontId::monospace(11.5))
            .color(theme.colors.text_secondary),
    );
    if resp.clicked() {
        commands.push(Command::OpenFilePath {
            path: file_path.to_path_buf(),
            line: line_match.line_number,
            col,
            preview: true,
        });
    }
}

/// Renders the Replace-in-Files confirmation modal when pending.
pub fn render_replace_in_files_modal(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    let Some((file_count, match_count)) = state.project_search.confirm_replace else {
        return;
    };

    let screen_rect = ui.ctx().input(egui::InputState::viewport_rect);
    let modal_w = 400.0_f32;
    let pos = egui::pos2(
        (screen_rect.width() - modal_w) * 0.5,
        screen_rect.height() * 0.25,
    );

    #[allow(deprecated)]
    egui::Area::new(egui::Id::new("replace_in_files_modal"))
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
                        RichText::new(format!(
                            "Replace {match_count} occurrences across {file_count} files?"
                        ))
                        .font(FontId::proportional(14.0))
                        .color(theme.colors.text_primary)
                        .strong(),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!("With: \"{}\"", state.project_search.replacement))
                            .font(FontId::monospace(12.0))
                            .color(theme.colors.text_secondary),
                    );
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            commands.push(Command::ProjectReplaceCancel);
                        }
                        let btn = egui::Button::new(
                            RichText::new("Replace All").color(theme.colors.text_on_accent),
                        )
                        .fill(theme.colors.accent);
                        if ui.add(btn).clicked() {
                            commands.push(Command::ProjectReplaceConfirmExecute);
                        }
                    });
                });
        });
}
