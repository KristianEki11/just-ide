//! Command handlers for M5 (power editing, settings, split groups, themes, Built-in Prettier).

use super::{AppState, Command, Effect, OverlayMode};
use jide_core::{Change, Document, EditKind, Eol, Selection};
use std::path::PathBuf;

/// Returns `true` when `format_on_save` should automatically run Prettier on `doc`.
#[must_use]
pub fn should_format_on_save(doc: &Document) -> bool {
    if doc.is_binary || doc.large_file || doc.read_only {
        return false;
    }
    let lang = doc.language.as_str().to_ascii_lowercase();
    !matches!(
        lang.as_str(),
        "plain text" | "plaintext" | "markdown" | "log"
    )
}

/// Applies configured save-time normalizations (`format_on_save`, trim whitespace, final newline).
pub(super) fn prepare_doc_for_save(
    doc: &mut Document,
    format_on_save: bool,
    trim_trailing_whitespace: bool,
    insert_final_newline: bool,
    now_ms: u64,
) {
    if format_on_save && should_format_on_save(doc) {
        let _ = doc.format_document(now_ms);
    }
    if trim_trailing_whitespace && !doc.is_binary && !doc.large_file && !doc.read_only {
        let _ = doc.trim_trailing_whitespace(now_ms);
    }
    if insert_final_newline && !doc.is_binary && !doc.large_file && !doc.read_only {
        let len = doc.rope.len_chars();
        if len > 0 && doc.rope.char(len - 1) != '\n' {
            let _ = doc.apply_changes(
                vec![Change {
                    start: len,
                    end: len,
                    text: "\n".to_owned(),
                }],
                doc.selection.clone(),
                EditKind::Structural,
                now_ms,
            );
        }
    }
}

#[allow(clippy::too_many_lines)]
pub(super) fn dispatch_m5(state: &mut AppState, cmd: Command) -> Vec<Effect> {
    match cmd {
        Command::FormatDocument => format_active(state, false),
        Command::FormatSelection => format_active(state, true),
        Command::ToggleFormatOnSave => {
            state.format_on_save = !state.format_on_save;
            vec![Effect::SaveSettings]
        }
        Command::ToggleBracketColorization => {
            state.bracket_pair_colorization = !state.bracket_pair_colorization;
            vec![Effect::SaveSettings]
        }
        Command::ToggleIndentGuides => {
            state.indent_guides = !state.indent_guides;
            vec![Effect::SaveSettings]
        }
        Command::ToggleAutoClosePairs => {
            state.auto_close_pairs = !state.auto_close_pairs;
            vec![Effect::SaveSettings]
        }
        Command::SetRenderWhitespace(mode) => {
            state.render_whitespace = mode;
            vec![Effect::SaveSettings]
        }
        Command::SetEditorFontSize(bits) => {
            state.editor_font_size = f32::from_bits(bits).clamp(8.0, 48.0);
            vec![Effect::SaveSettings]
        }
        Command::SetLineHeight(bits) => {
            state.editor_line_height = f32::from_bits(bits).clamp(1.1, 2.2);
            vec![Effect::SaveSettings]
        }
        Command::ToggleFontLigatures => {
            state.font_ligatures = !state.font_ligatures;
            vec![Effect::FontsChanged, Effect::SaveSettings]
        }
        Command::ToggleLineNumbers => {
            state.line_numbers = !state.line_numbers;
            vec![Effect::SaveSettings]
        }
        Command::ToggleHighlightActiveLine => {
            state.highlight_active_line = !state.highlight_active_line;
            vec![Effect::SaveSettings]
        }
        Command::ToggleBreadcrumbs => {
            state.breadcrumbs_visible = !state.breadcrumbs_visible;
            vec![Effect::SaveSettings]
        }
        Command::ToggleActivityBar => {
            state.activity_bar_visible = !state.activity_bar_visible;
            vec![Effect::SaveSettings]
        }
        Command::ToggleStatusBar => {
            state.status_bar_visible = !state.status_bar_visible;
            vec![Effect::SaveSettings]
        }
        Command::ToggleSidebarPosition => {
            state.side_bar_right = !state.side_bar_right;
            vec![Effect::SaveSettings]
        }
        Command::ToggleTrimOnSave => {
            state.trim_trailing_whitespace_on_save = !state.trim_trailing_whitespace_on_save;
            vec![Effect::SaveSettings]
        }
        Command::ToggleFinalNewlineOnSave => {
            state.insert_final_newline_on_save = !state.insert_final_newline_on_save;
            vec![Effect::SaveSettings]
        }
        Command::SetAutoSaveMode(mode) => {
            state.auto_save_mode = mode;
            vec![Effect::SaveSettings]
        }
        Command::ToggleFileIcons => {
            state.show_file_icons = !state.show_file_icons;
            vec![Effect::SaveSettings]
        }
        Command::ToggleTreeGuides => {
            state.show_tree_guides = !state.show_tree_guides;
            vec![Effect::SaveSettings]
        }
        Command::ToggleSmartCase => {
            state.smart_case_search = !state.smart_case_search;
            vec![Effect::SaveSettings]
        }
        Command::SetSettingsCategory(cat) => {
            state.settings_category = cat;
            vec![]
        }
        Command::ResetSettingsDefaults => reset_settings_defaults(state),
        Command::DismissToast => {
            state.toast = None;
            vec![]
        }
        Command::SetTabSize(sz) => {
            let clamped = sz.clamp(1, 16);
            state.default_tab_size = clamped;
            if let Some(idx) = state.focused_doc_idx() {
                if let Some(doc) = state.documents.get_mut(idx) {
                    doc.tab_size = clamped;
                }
            }
            vec![Effect::SaveSettings]
        }
        Command::ToggleInsertSpaces => {
            state.default_insert_spaces = !state.default_insert_spaces;
            if let Some(idx) = state.focused_doc_idx() {
                if let Some(doc) = state.documents.get_mut(idx) {
                    doc.insert_spaces = state.default_insert_spaces;
                }
            }
            vec![Effect::SaveSettings]
        }
        Command::ConvertIndentToSpaces => convert_indentation(state, true),
        Command::ConvertIndentToTabs => convert_indentation(state, false),
        Command::ConvertEol(eol) => convert_eol(state, eol),
        Command::OpenSettings => {
            state.settings_open = true;
            vec![]
        }
        Command::CloseSettings => {
            state.settings_open = false;
            state.settings_query.clear();
            vec![]
        }
        Command::SettingsQueryChanged(query) => {
            state.settings_query = query;
            vec![]
        }
        Command::OpenSettingsToml => vec![Effect::OpenSettingsToml],
        Command::OpenThemePicker => {
            state.overlay = Some(OverlayMode::ThemePicker);
            state.overlay_query.clear();
            vec![Effect::ReloadThemeList]
        }
        Command::ToggleZenMode => {
            state.zen_mode = !state.zen_mode;
            vec![]
        }
        Command::ToggleSplitEditor => {
            state.split_open = !state.split_open;
            if state.split_open {
                if state.group2_doc.is_none() {
                    state.group2_doc = state.active_doc;
                }
            } else {
                state.active_group = 0;
            }
            vec![]
        }
        Command::FocusGroup(group) => {
            if group == 1 {
                if !state.split_open {
                    state.split_open = true;
                }
                if state.group2_doc.is_none() {
                    state.group2_doc = state.active_doc;
                }
                state.active_group = 1;
            } else {
                state.active_group = 0;
            }
            vec![Effect::UpdateWindowTitle(state.window_title())]
        }
        Command::NextTab => cycle_tab(state, 1),
        Command::PrevTab => cycle_tab(state, -1),
        Command::ReopenClosedTab => {
            if let Some((path, meta)) = state.closed_tabs_history.pop() {
                vec![Effect::RequestOpenFilePath {
                    path,
                    line: 1,
                    col: 1,
                    preview: meta.preview,
                }]
            } else {
                state.toast = Some("No closed tabs in history".into());
                vec![]
            }
        }
        Command::SaveAll => save_all_dirty(state),
        Command::NavBack => navigate_history(state, false),
        Command::NavForward => navigate_history(state, true),
        Command::BeginChordCtrlK => {
            state.chord_ctrl_k = true;
            vec![]
        }
        Command::CancelChord => {
            state.chord_ctrl_k = false;
            vec![]
        }
        Command::OpenTerminalHere => {
            let dir = state
                .open_folder
                .clone()
                .or_else(|| {
                    state
                        .active_document()
                        .and_then(|d| d.path.as_deref())
                        .and_then(|p| p.parent())
                        .map(PathBuf::from)
                })
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            vec![Effect::OpenTerminalHere(dir)]
        }
        Command::QuitApp => vec![Effect::RequestQuit],
        _ => vec![],
    }
}

fn reset_settings_defaults(state: &mut AppState) -> Vec<Effect> {
    let was_ligatures = state.font_ligatures;
    state.editor_font_size = 14.0;
    state.editor_line_height = 1.5;
    state.default_tab_size = 4;
    state.default_insert_spaces = true;
    state.word_wrap = false;
    state.font_ligatures = false;
    state.line_numbers = true;
    state.highlight_active_line = true;
    state.indent_guides = true;
    state.bracket_pair_colorization = true;
    state.auto_close_pairs = true;
    state.format_on_save = true;
    state.render_whitespace = "selection".into();
    state.breadcrumbs_visible = true;
    state.activity_bar_visible = true;
    state.status_bar_visible = true;
    state.side_bar_right = false;
    state.trim_trailing_whitespace_on_save = false;
    state.insert_final_newline_on_save = true;
    state.auto_save_mode = "off".into();
    state.show_file_icons = true;
    state.show_tree_guides = true;
    state.smart_case_search = true;
    state.project_search.use_gitignore = true;
    state.zoom = 1.0;
    state.toast = Some("Settings reset to defaults".into());
    if was_ligatures {
        vec![Effect::FontsChanged, Effect::SaveSettings]
    } else {
        vec![Effect::SaveSettings]
    }
}

fn format_active(state: &mut AppState, selection_only: bool) -> Vec<Effect> {
    let Some(idx) = state.focused_doc_idx() else {
        return vec![];
    };
    let now_ms = state.now_ms;
    let res = {
        let doc = &mut state.documents[idx];
        if doc.is_binary {
            state.toast = Some("Cannot format a binary file".into());
            return vec![];
        }
        if selection_only {
            doc.format_selection(now_ms)
        } else {
            doc.format_document(now_ms)
        }
    };
    match res {
        Ok(true) => {
            if let Some(meta) = state.tab_states.get_mut(idx) {
                meta.preview = false;
            }
            state.toast = Some("Prettier: formatted".into());
            let mut effects = vec![Effect::UpdateWindowTitle(state.window_title())];
            effects.extend(super::search_dispatch::refresh_find_if_open(state));
            effects
        }
        Ok(false) => {
            state.toast = Some("Prettier: already formatted".into());
            vec![]
        }
        Err(err) => {
            state.toast = Some(format!("Format error: {err}"));
            vec![]
        }
    }
}

fn convert_indentation(state: &mut AppState, to_spaces: bool) -> Vec<Effect> {
    let Some(idx) = state.focused_doc_idx() else {
        return vec![];
    };
    let now_ms = state.now_ms;
    let doc = &mut state.documents[idx];
    if doc.read_only {
        state.toast = Some("Document is read-only".into());
        return vec![];
    }
    let tab_width = doc.tab_size.max(1);
    let mut changes = Vec::new();
    for line_idx in 0..doc.rope.len_lines() {
        let start = doc.rope.line_to_char(line_idx);
        let line_text = doc.rope.line(line_idx).to_string();
        let leading_chars = line_text
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .count();
        if leading_chars == 0 {
            continue;
        }
        let prefix: String = line_text.chars().take(leading_chars).collect();
        let mut visual_cols = 0usize;
        for ch in prefix.chars() {
            if ch == '\t' {
                visual_cols += tab_width - (visual_cols % tab_width);
            } else {
                visual_cols += 1;
            }
        }
        let replacement = if to_spaces {
            " ".repeat(visual_cols)
        } else {
            let tabs = visual_cols / tab_width;
            let rem = visual_cols % tab_width;
            format!("{}{}", "\t".repeat(tabs), " ".repeat(rem))
        };
        if replacement != prefix {
            changes.push(Change {
                start,
                end: start + leading_chars,
                text: replacement,
            });
        }
    }
    doc.insert_spaces = to_spaces;
    if !changes.is_empty() {
        let clamped = doc.selection.primary_range().head.min(doc.char_count());
        if let Err(err) = doc.apply_changes(
            changes,
            Selection::point(clamped),
            EditKind::Structural,
            now_ms,
        ) {
            state.toast = Some(format!("Indent conversion failed: {err}"));
            return vec![];
        }
    }
    state.toast = Some(
        if to_spaces {
            "Converted indentation to Spaces"
        } else {
            "Converted indentation to Tabs"
        }
        .into(),
    );
    vec![Effect::UpdateWindowTitle(state.window_title())]
}

fn convert_eol(state: &mut AppState, eol: Eol) -> Vec<Effect> {
    let Some(idx) = state.focused_doc_idx() else {
        return vec![];
    };
    let doc = &mut state.documents[idx];
    if doc.eol != eol {
        doc.eol = eol;
        doc.version = doc.version.wrapping_add(1);
    }
    state.toast = Some(format!("Line endings set to {}", eol.label()));
    vec![Effect::UpdateWindowTitle(state.window_title())]
}

fn cycle_tab(state: &mut AppState, step: isize) -> Vec<Effect> {
    let len = state.documents.len();
    if len <= 1 {
        return vec![];
    }
    let cur = state.focused_doc_idx().unwrap_or(0);
    let next = if step >= 0 {
        (cur + 1) % len
    } else {
        (cur + len - 1) % len
    };
    state.set_focused_doc(Some(next));
    state.scroll_y = 0.0;
    state.scroll_x = 0.0;
    state.push_mru(next);
    let mut effects = vec![Effect::UpdateWindowTitle(state.window_title())];
    effects.extend(super::search_dispatch::refresh_find_if_open(state));
    effects
}

fn save_all_dirty(state: &mut AppState) -> Vec<Effect> {
    let mut effects = Vec::new();
    let format_on_save = state.format_on_save;
    let trim = state.trim_trailing_whitespace_on_save;
    let final_nl = state.insert_final_newline_on_save;
    let now_ms = state.now_ms;
    let focused = state.focused_doc_idx();
    for (idx, doc) in state.documents.iter_mut().enumerate() {
        if !doc.is_dirty() {
            continue;
        }
        prepare_doc_for_save(doc, format_on_save, trim, final_nl, now_ms);
        if let Some(path) = doc.path.clone() {
            effects.push(Effect::RequestSaveDoc {
                doc_id: doc.id,
                path,
            });
        } else if Some(idx) == focused {
            effects.push(Effect::RequestSaveAsDialog { doc_id: doc.id });
        }
    }
    effects.push(Effect::UpdateWindowTitle(state.window_title()));
    effects
}

fn navigate_history(state: &mut AppState, forward: bool) -> Vec<Effect> {
    let entry = if forward {
        state.nav_history.forward()
    } else {
        state.nav_history.back()
    };
    let Some(entry) = entry else {
        return vec![];
    };
    if let Some(pos) = state.documents.iter().position(|d| d.id == entry.doc_id) {
        let doc = &mut state.documents[pos];
        let clamped = entry.char_idx.min(doc.char_count());
        doc.selection = Selection::point(clamped);
        doc.preferred_col = None;
        state.set_focused_doc(Some(pos));
        state.push_mru(pos);
        return vec![Effect::UpdateWindowTitle(state.window_title())];
    }
    vec![]
}
