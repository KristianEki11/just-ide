//! Two-pane Settings screen (`Ctrl+,`), covering Prettier, Text Editor, Appearance,
//! Files, Explorer, Search, and searchable Keyboard Shortcuts.

use crate::commands::CommandRegistry;
use crate::state::{AppState, Command};
use crate::theme::{Theme, ThemeId};
use egui::{
    vec2, Align, CornerRadius, FontFamily, FontId, Frame, Margin, RichText, ScrollArea, Sense,
    Stroke,
};

const CATEGORIES: &[(&str, &str)] = &[
    ("all", "All Settings"),
    ("prettier", "Prettier & Format"),
    ("editor", "Text Editor"),
    ("appearance", "Appearance & Layout"),
    ("files", "Files & Saving"),
    ("explorer", "Explorer & Tree"),
    ("search", "Search"),
    ("keybindings", "Keyboard Shortcuts"),
];

/// Renders the two-pane VS Code-style Settings view inside the central panel.
pub fn render_settings_view(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    commands: &mut Vec<Command>,
) {
    Frame::NONE.fill(theme.colors.bg_base).show(ui, |ui| {
        render_header(ui, state, theme, commands);
        let q = state.settings_query.to_ascii_lowercase();
        let active_cat = if state.settings_category.is_empty() {
            "all"
        } else {
            state.settings_category.as_str()
        };

        let body_rect = ui.available_rect_before_wrap();
        ui.allocate_ui(body_rect.size(), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                render_category_sidebar(ui, active_cat, theme, body_rect.height(), commands);

                let right_w = (ui.available_width()).max(200.0);
                ui.allocate_ui(vec2(right_w, body_rect.height()), |ui| {
                    Frame::NONE
                        .inner_margin(Margin::symmetric(24, 16))
                        .show(ui, |ui| {
                            ScrollArea::vertical()
                                .id_salt("settings_right_scroll")
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.set_max_width(760.0);
                                    render_selected_sections(
                                        ui, state, theme, active_cat, &q, commands,
                                    );
                                });
                        });
                });
            });
        });
    });
}

fn render_header(ui: &mut egui::Ui, state: &AppState, theme: &Theme, commands: &mut Vec<Command>) {
    Frame::NONE
        .fill(theme.colors.bg_surface)
        .stroke(Stroke::new(1.0, theme.colors.border_subtle))
        .inner_margin(Margin::symmetric(24, 14))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Settings")
                            .font(FontId::new(18.0, FontFamily::Proportional))
                            .color(theme.colors.text_primary)
                            .strong(),
                    );
                    ui.label(
                        RichText::new("Offline preferences persisted to settings.toml")
                            .font(FontId::new(12.0, FontFamily::Proportional))
                            .color(theme.colors.text_muted),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Close (Esc)").clicked() {
                        commands.push(Command::CloseSettings);
                    }
                    if ui.button("Open settings.toml").clicked() {
                        commands.push(Command::OpenSettingsToml);
                    }
                    if ui.button("Reset Defaults").clicked() {
                        commands.push(Command::ResetSettingsDefaults);
                    }
                });
            });
            ui.add_space(10.0);
            let mut filter = state.settings_query.clone();
            if ui
                .add(
                    egui::TextEdit::singleline(&mut filter)
                        .desired_width(480.0)
                        .hint_text("Search settings or shortcuts (e.g. font, ligatures, tab, prettier, line height)")
                        .font(FontId::new(13.0, FontFamily::Proportional)),
                )
                .changed()
            {
                commands.push(Command::SettingsQueryChanged(filter));
            }
        });
}

fn render_category_sidebar(
    ui: &mut egui::Ui,
    active_cat: &str,
    theme: &Theme,
    height: f32,
    commands: &mut Vec<Command>,
) {
    let nav_w = 184.0;
    let (nav_rect, _) = ui.allocate_exact_size(vec2(nav_w, height.max(80.0)), Sense::hover());
    ui.painter()
        .rect_filled(nav_rect, CornerRadius::ZERO, theme.colors.bg_surface);
    ui.painter().vline(
        nav_rect.right() - 0.5,
        nav_rect.y_range(),
        Stroke::new(1.0, theme.colors.border_subtle),
    );

    ui.scope_builder(
        egui::UiBuilder::new().max_rect(nav_rect.shrink2(vec2(8.0, 12.0))),
        |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            for (id, label) in CATEGORIES {
                let selected = active_cat == *id;
                let row_rect = ui.allocate_space(vec2(nav_w - 16.0, 28.0)).1;
                let resp = ui.interact(
                    row_rect,
                    ui.scope_id().with(("settings_cat", *id)),
                    Sense::click(),
                );
                if selected {
                    ui.painter().rect_filled(
                        row_rect,
                        CornerRadius::same(5),
                        theme.colors.bg_active,
                    );
                    let accent_bar = egui::Rect::from_min_size(
                        egui::pos2(row_rect.left() + 2.0, row_rect.top() + 5.0),
                        vec2(2.5, row_rect.height() - 10.0),
                    );
                    ui.painter().rect_filled(
                        accent_bar,
                        CornerRadius::same(2),
                        theme.colors.accent,
                    );
                } else if resp.hovered() {
                    ui.painter().rect_filled(
                        row_rect,
                        CornerRadius::same(5),
                        theme.colors.bg_hover,
                    );
                }
                ui.painter().text(
                    egui::pos2(row_rect.left() + 12.0, row_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    *label,
                    FontId::new(12.5, FontFamily::Proportional),
                    if selected {
                        theme.colors.text_primary
                    } else {
                        theme.colors.text_secondary
                    },
                );
                if resp.clicked() {
                    commands.push(Command::SetSettingsCategory((*id).to_owned()));
                }
            }
        },
    );
}

fn render_selected_sections(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    cat: &str,
    q: &str,
    commands: &mut Vec<Command>,
) {
    let show_all = cat == "all";
    if show_all || cat == "prettier" {
        render_prettier_section(ui, state, theme, q, commands);
    }
    if show_all || cat == "editor" {
        render_editor_section(ui, state, theme, q, commands);
    }
    if show_all || cat == "appearance" {
        render_appearance_section(ui, state, theme, q, commands);
    }
    if show_all || cat == "files" {
        render_files_section(ui, state, theme, q, commands);
    }
    if show_all || cat == "explorer" {
        render_explorer_section(ui, state, theme, q, commands);
    }
    if show_all || cat == "search" {
        render_search_section(ui, state, theme, q, commands);
    }
    if cat == "keybindings" || (show_all && !q.is_empty()) {
        render_keybindings_section(ui, theme, q, commands);
    } else if show_all {
        section_heading(ui, theme, "Keyboard Shortcuts");
        setting_card(
            ui,
            theme,
            "Command Keybindings Reference",
            &format!(
                "Browse {} built-in commands and default shortcuts.",
                CommandRegistry::all().len()
            ),
            |ui| {
                if ui.button("Browse Shortcuts ->").clicked() {
                    commands.push(Command::SetSettingsCategory("keybindings".into()));
                }
            },
        );
    }
}

fn matches_filter(query: &str, tags: &str) -> bool {
    query.is_empty() || tags.contains(query)
}

fn section_heading(ui: &mut egui::Ui, theme: &Theme, title: &str) {
    ui.add_space(4.0);
    ui.label(
        RichText::new(title)
            .font(FontId::new(15.0, FontFamily::Proportional))
            .color(theme.colors.text_primary)
            .strong(),
    );
    ui.add_space(6.0);
}

fn setting_card(
    ui: &mut egui::Ui,
    theme: &Theme,
    title: &str,
    description: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    Frame::NONE
        .fill(theme.colors.bg_surface)
        .stroke(Stroke::new(1.0, theme.colors.border_subtle))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(title)
                    .font(FontId::new(13.0, FontFamily::Proportional))
                    .color(theme.colors.text_primary)
                    .strong(),
            );
            ui.label(
                RichText::new(description)
                    .font(FontId::new(11.5, FontFamily::Proportional))
                    .color(theme.colors.text_muted),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(add_contents);
        });
    ui.add_space(8.0);
}

fn toggle_setting(
    ui: &mut egui::Ui,
    theme: &Theme,
    title: &str,
    description: &str,
    value: bool,
    cmd: Command,
    commands: &mut Vec<Command>,
) {
    let mut checked = value;
    setting_card(ui, theme, title, description, |ui| {
        let label = if checked { "Enabled" } else { "Disabled" };
        if ui.checkbox(&mut checked, label).changed() {
            commands.push(cmd);
        }
    });
}

fn render_prettier_section(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    q: &str,
    commands: &mut Vec<Command>,
) {
    let items = [
        matches_filter(q, "prettier format save on save"),
        matches_filter(q, "rainbow bracket pair colorization"),
        matches_filter(q, "auto close pairs brackets quotes"),
    ];
    if !items.iter().any(|&v| v) {
        return;
    }
    section_heading(ui, theme, "Built-in Prettier & Colorization");
    if items[0] {
        setting_card(
            ui,
            theme,
            "Format on Save",
            "Automatically run the built-in offline formatter on Ctrl+S.",
            |ui| {
                let mut fos = state.format_on_save;
                if ui.checkbox(&mut fos, "Enable format on save").changed() {
                    commands.push(Command::ToggleFormatOnSave);
                }
                if ui.button("Format Active Document (Shift+Alt+F)").clicked() {
                    commands.push(Command::FormatDocument);
                }
            },
        );
    }
    if items[1] {
        toggle_setting(
            ui,
            theme,
            "Rainbow Bracket Pair Colorization",
            "Colorize matching (), [], and {} nesting levels across the viewport.",
            state.bracket_pair_colorization,
            Command::ToggleBracketColorization,
            commands,
        );
    }
    if items[2] {
        toggle_setting(
            ui,
            theme,
            "Auto Close Pairs",
            "Automatically insert matching closing brackets, parentheses, and quotes.",
            state.auto_close_pairs,
            Command::ToggleAutoClosePairs,
            commands,
        );
    }
    ui.add_space(6.0);
}

fn render_editor_section(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    q: &str,
    commands: &mut Vec<Command>,
) {
    let flags = [
        matches_filter(q, "editor font size px monospace"),
        matches_filter(q, "editor line height spacing"),
        matches_filter(q, "editor font ligatures calt liga symbols characters"),
        matches_filter(q, "editor word wrap soft wrap"),
        matches_filter(q, "editor line numbers gutter"),
        matches_filter(q, "editor highlight active current line"),
        matches_filter(q, "editor indent guides"),
        matches_filter(q, "editor render whitespace spaces tabs"),
        matches_filter(q, "editor tab size indent spaces"),
    ];
    if !flags.iter().any(|&v| v) {
        return;
    }
    section_heading(ui, theme, "Text Editor");
    render_editor_font_cards(ui, state, theme, &flags[..4], commands);
    render_editor_layout_cards(ui, state, theme, &flags[4..], commands);
    ui.add_space(6.0);
}

fn render_editor_font_cards(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    flags: &[bool],
    commands: &mut Vec<Command>,
) {
    if flags[0] {
        setting_card(
            ui,
            theme,
            "Font Size",
            "Base monospace editor font size (Zoom scales on top of this value).",
            |ui| {
                for sz in [12_u16, 13, 14, 15, 16, 18, 20] {
                    let active = (state.editor_font_size - f32::from(sz)).abs() < 0.1;
                    if ui.selectable_label(active, format!("{sz} px")).clicked() {
                        commands.push(Command::SetEditorFontSize(u32::from(sz)));
                    }
                }
            },
        );
    }
    if flags[1] {
        setting_card(
            ui,
            theme,
            "Line Height",
            "Vertical line spacing multiplier inside the code editor.",
            |ui| {
                for lh in [1.25_f32, 1.35, 1.5, 1.65, 1.8] {
                    let active = (state.editor_line_height - lh).abs() < 0.03;
                    if ui.selectable_label(active, format!("{lh:.2}x")).clicked() {
                        commands.push(Command::SetLineHeight(lh.to_bits()));
                    }
                }
            },
        );
    }
    if flags[2] {
        toggle_setting(
            ui,
            theme,
            "Programming Font Ligatures",
            "Disable (recommended) so characters like !=, ->, and => always render literally glyph-for-glyph.",
            state.font_ligatures,
            Command::ToggleFontLigatures,
            commands,
        );
    }
    if flags[3] {
        toggle_setting(
            ui,
            theme,
            "Word Wrap",
            "Wrap long lines to fit the editor width instead of horizontal scrolling.",
            state.word_wrap,
            Command::ToggleWordWrap,
            commands,
        );
    }
}

fn render_editor_layout_cards(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    flags: &[bool],
    commands: &mut Vec<Command>,
) {
    if flags[0] {
        toggle_setting(
            ui,
            theme,
            "Line Numbers",
            "Display line numbers and gutter separator to the left of the buffer.",
            state.line_numbers,
            Command::ToggleLineNumbers,
            commands,
        );
    }
    if flags[1] {
        toggle_setting(
            ui,
            theme,
            "Highlight Active Line",
            "Tint the background of the line containing the primary cursor.",
            state.highlight_active_line,
            Command::ToggleHighlightActiveLine,
            commands,
        );
    }
    if flags[2] {
        toggle_setting(
            ui,
            theme,
            "Indent Guides",
            "Render vertical indentation guides for tabs and space blocks.",
            state.indent_guides,
            Command::ToggleIndentGuides,
            commands,
        );
    }
    if flags[3] {
        setting_card(
            ui,
            theme,
            "Render Whitespace",
            "Visualize space dots and tab arrows inside code lines.",
            |ui| {
                for mode in ["none", "boundary", "selection", "all"] {
                    if ui
                        .selectable_label(state.render_whitespace == mode, mode)
                        .clicked()
                    {
                        commands.push(Command::SetRenderWhitespace(mode.to_owned()));
                    }
                }
            },
        );
    }
    if flags[4] {
        setting_card(
            ui,
            theme,
            "Indentation & Tab Size",
            "Default spaces per tab stop and whether Tab inserts spaces.",
            |ui| {
                for sz in [2_usize, 4, 8] {
                    if ui
                        .selectable_label(state.default_tab_size == sz, format!("{sz} spaces"))
                        .clicked()
                    {
                        commands.push(Command::SetTabSize(sz));
                    }
                }
                let mut sp = state.default_insert_spaces;
                if ui.checkbox(&mut sp, "Insert Spaces").changed() {
                    commands.push(Command::ToggleInsertSpaces);
                }
            },
        );
    }
}

fn render_appearance_section(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    q: &str,
    commands: &mut Vec<Command>,
) {
    let flags = [
        matches_filter(q, "appearance theme dark light color"),
        matches_filter(q, "appearance zoom scale ui"),
        matches_filter(q, "appearance breadcrumbs path bar"),
        matches_filter(q, "appearance activity bar icon rail"),
        matches_filter(q, "appearance status bar bottom"),
        matches_filter(q, "appearance sidebar position left right"),
        matches_filter(q, "appearance zen mode distraction free"),
    ];
    if !flags.iter().any(|&v| v) {
        return;
    }
    section_heading(ui, theme, "Appearance & Layout");
    if flags[0] {
        setting_card(
            ui,
            theme,
            "Color Theme",
            "Switch UI and syntax colors instantaneously.",
            |ui| {
                for (id, label) in [
                    (ThemeId::JustDark, "Just Dark"),
                    (ThemeId::JustLight, "Just Light"),
                    (ThemeId::HighContrast, "High Contrast"),
                ] {
                    if ui.selectable_label(state.theme.id == id, label).clicked() {
                        commands.push(Command::SwitchTheme(id));
                    }
                }
                for custom in &state.available_custom_themes {
                    if ui
                        .selectable_label(state.theme.id == custom.id, &custom.name)
                        .clicked()
                    {
                        commands.push(Command::SwitchTheme(custom.id.clone()));
                    }
                }
                if ui.button("Theme Picker (Ctrl+K Ctrl+T)").clicked() {
                    commands.push(Command::OpenThemePicker);
                }
            },
        );
    }
    if flags[1] {
        #[allow(clippy::cast_possible_truncation)]
        let zoom_pct = (state.zoom * 100.0).round() as i32;
        setting_card(
            ui,
            theme,
            "Editor Zoom",
            "Magnify or shrink the editor font relative to base size.",
            |ui| {
                if ui.button(" - ").clicked() {
                    commands.push(Command::ZoomOut);
                }
                ui.label(
                    RichText::new(format!("{zoom_pct}%"))
                        .font(FontId::new(12.5, FontFamily::Monospace))
                        .color(theme.colors.text_primary),
                );
                if ui.button(" + ").clicked() {
                    commands.push(Command::ZoomIn);
                }
                if ui.button("Reset (100%)").clicked() {
                    commands.push(Command::ResetZoom);
                }
            },
        );
    }
    render_appearance_chrome_cards(ui, state, theme, &flags[2..], commands);
    ui.add_space(6.0);
}

fn render_appearance_chrome_cards(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    flags: &[bool],
    commands: &mut Vec<Command>,
) {
    if flags[0] {
        toggle_setting(
            ui,
            theme,
            "Breadcrumbs",
            "Show workspace folder path and active file symbol above the editor.",
            state.breadcrumbs_visible,
            Command::ToggleBreadcrumbs,
            commands,
        );
    }
    if flags[1] {
        toggle_setting(
            ui,
            theme,
            "Activity Bar",
            "Display the icon rail for switching Explorer, Search, and Settings.",
            state.activity_bar_visible,
            Command::ToggleActivityBar,
            commands,
        );
    }
    if flags[2] {
        toggle_setting(
            ui,
            theme,
            "Status Bar",
            "Display workspace, cursor position, encoding, EOL, and Prettier bar at the bottom.",
            state.status_bar_visible,
            Command::ToggleStatusBar,
            commands,
        );
    }
    if flags[3] {
        setting_card(
            ui,
            theme,
            "Primary Sidebar Location",
            "Dock Explorer/Search Sidebar and Activity Rail on the left or right side.",
            |ui| {
                if ui.selectable_label(!state.side_bar_right, "Left").clicked()
                    && state.side_bar_right
                {
                    commands.push(Command::ToggleSidebarPosition);
                }
                if ui.selectable_label(state.side_bar_right, "Right").clicked()
                    && !state.side_bar_right
                {
                    commands.push(Command::ToggleSidebarPosition);
                }
            },
        );
    }
    if flags[4] {
        toggle_setting(
            ui,
            theme,
            "Zen Mode",
            "Hide tabs, sidebar, activity rail, and status bar (Ctrl+K Z).",
            state.zen_mode,
            Command::ToggleZenMode,
            commands,
        );
    }
}

fn render_files_section(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    q: &str,
    commands: &mut Vec<Command>,
) {
    let show_auto = matches_filter(q, "files auto save delay focus");
    let show_trim = matches_filter(q, "files trim trailing whitespace save");
    let show_eol = matches_filter(q, "files insert final newline save");
    if !(show_auto || show_trim || show_eol) {
        return;
    }
    section_heading(ui, theme, "Files & Saving");
    if show_auto {
        setting_card(
            ui,
            theme,
            "Auto Save",
            "Save dirty buffers automatically or only on explicit Ctrl+S.",
            |ui| {
                for mode in ["off", "afterDelay", "onFocusChange"] {
                    if ui
                        .selectable_label(state.auto_save_mode == mode, mode)
                        .clicked()
                    {
                        commands.push(Command::SetAutoSaveMode(mode.to_owned()));
                    }
                }
            },
        );
    }
    if show_trim {
        toggle_setting(
            ui,
            theme,
            "Trim Trailing Whitespace on Save",
            "Remove extra spaces or tabs at the end of lines before atomic file writes.",
            state.trim_trailing_whitespace_on_save,
            Command::ToggleTrimOnSave,
            commands,
        );
    }
    if show_eol {
        toggle_setting(
            ui,
            theme,
            "Insert Final Newline on Save",
            "Ensure non-empty files end with a trailing newline character on save.",
            state.insert_final_newline_on_save,
            Command::ToggleFinalNewlineOnSave,
            commands,
        );
    }
    ui.add_space(6.0);
}

fn render_explorer_section(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    q: &str,
    commands: &mut Vec<Command>,
) {
    let show_icons = matches_filter(q, "explorer file folder icons simple logos badges");
    let show_guides = matches_filter(q, "explorer tree indent guides hierarchy");
    if !(show_icons || show_guides) {
        return;
    }
    section_heading(ui, theme, "Explorer & File Tree");
    if show_icons {
        toggle_setting(
            ui,
            theme,
            "File & Folder Icons",
            "Show colored directory icons and language badges (Markdown M↓, Rust, JS/TS, TOML, etc.).",
            state.show_file_icons,
            Command::ToggleFileIcons,
            commands,
        );
    }
    if show_guides {
        toggle_setting(
            ui,
            theme,
            "Folder Tree Indent Guides",
            "Draw vertical hierarchy lines connecting nested parent and child folders.",
            state.show_tree_guides,
            Command::ToggleTreeGuides,
            commands,
        );
    }
    ui.add_space(6.0);
}

fn render_search_section(
    ui: &mut egui::Ui,
    state: &AppState,
    theme: &Theme,
    q: &str,
    commands: &mut Vec<Command>,
) {
    let show_gi = matches_filter(q, "search gitignore ignore files");
    let show_sc = matches_filter(q, "search smart case sensitivity");
    if !(show_gi || show_sc) {
        return;
    }
    section_heading(ui, theme, "Search");
    if show_gi {
        toggle_setting(
            ui,
            theme,
            "Use .gitignore in Find in Files",
            "Skip ignored build artifacts and dependencies when searching workspace folders.",
            state.project_search.use_gitignore,
            Command::ToggleProjectGitignore,
            commands,
        );
    }
    if show_sc {
        toggle_setting(
            ui,
            theme,
            "Smart Case Search",
            "Match case-insensitively unless the search query contains uppercase letters.",
            state.smart_case_search,
            Command::ToggleSmartCase,
            commands,
        );
    }
    ui.add_space(6.0);
}

fn render_keybindings_section(
    ui: &mut egui::Ui,
    theme: &Theme,
    q: &str,
    commands: &mut Vec<Command>,
) {
    section_heading(ui, theme, "Keyboard Shortcuts");
    let entries: Vec<_> = CommandRegistry::all()
        .iter()
        .filter(|meta| {
            q.is_empty()
                || meta.category.to_ascii_lowercase().contains(q)
                || meta.title.to_ascii_lowercase().contains(q)
                || meta.default_shortcut.to_ascii_lowercase().contains(q)
                || meta.id.to_ascii_lowercase().contains(q)
        })
        .collect();

    if entries.is_empty() {
        ui.label(
            RichText::new("No matching commands.")
                .font(FontId::new(12.5, FontFamily::Proportional))
                .color(theme.colors.text_muted),
        );
        return;
    }

    Frame::NONE
        .fill(theme.colors.bg_surface)
        .stroke(Stroke::new(1.0, theme.colors.border_subtle))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            for meta in entries {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("[{}]", meta.category))
                            .font(FontId::new(11.0, FontFamily::Proportional))
                            .color(theme.colors.text_muted),
                    );
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(meta.title)
                                    .font(FontId::new(12.5, FontFamily::Proportional))
                                    .color(theme.colors.text_primary),
                            )
                            .frame(false),
                        )
                        .on_hover_text(format!("Click to run ({})", meta.id))
                        .clicked()
                    {
                        commands.push(Command::CloseSettings);
                        commands.push(Command::RunById(meta.id.to_owned()));
                    }
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        let shortcut = if meta.default_shortcut.is_empty() {
                            "—"
                        } else {
                            meta.default_shortcut
                        };
                        Frame::NONE
                            .fill(theme.colors.bg_base)
                            .stroke(Stroke::new(1.0, theme.colors.border_subtle))
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(Margin::symmetric(8, 2))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(shortcut)
                                        .font(FontId::new(11.0, FontFamily::Monospace))
                                        .color(theme.colors.text_secondary),
                                );
                            });
                    });
                });
                ui.separator();
            }
        });
}
