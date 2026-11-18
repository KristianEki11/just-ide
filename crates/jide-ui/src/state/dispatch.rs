//! Command dispatch logic mutating `AppState` and emitting `Effect`s.

use super::{editor, paths_match, AppState, Command, Effect, OverlayMode, SidebarView, TabMeta};
use crate::theme::{Theme, ThemeId};
use jide_core::{DocId, Document};

/// Pure command dispatcher mutating [`AppState`] and returning [`Effect`]s.
#[allow(clippy::too_many_lines)]
pub fn dispatch(state: &mut AppState, cmd: Command) -> Vec<Effect> {
    match cmd {
        Command::Scroll { x_bits, y_bits } => {
            state.scroll_x = f32::from_bits(x_bits).max(0.0);
            state.scroll_y = f32::from_bits(y_bits).max(0.0);
            vec![]
        }
        Command::ToggleSidebar => {
            state.sidebar_visible = !state.sidebar_visible;
            vec![Effect::None]
        }
        Command::ActivateSidebar(view) => {
            if state.sidebar_visible && state.sidebar_view == view {
                state.sidebar_visible = false;
            } else {
                state.sidebar_visible = true;
                state.sidebar_view = view;
            }
            vec![Effect::None]
        }
        Command::OpenFile | Command::OpenFileDialog => vec![Effect::RequestOpenFileDialog],
        Command::OpenFolder => vec![Effect::RequestOpenFolderDialog],
        Command::OpenFolderPath(path) => {
            state.workspace_name = path.file_name().and_then(|n| n.to_str()).map(str::to_owned);
            state.recent_folders.retain(|p| p != &path);
            state.recent_folders.insert(0, path.clone());
            match jide_workspace::tree::FileTree::open(path.clone(), false) {
                Ok(tree) => {
                    state.open_folder = Some(path);
                    state.file_tree = Some(tree);
                    state.tree_generation = state.tree_generation.wrapping_add(1);
                }
                Err(err) => {
                    state.toast = Some(format!("Cannot open folder: {err}"));
                }
            }
            vec![Effect::UpdateWindowTitle(state.window_title())]
        }
        Command::ToggleFolderNode(path) => {
            if let Some(tree) = &mut state.file_tree {
                if let Err(err) = tree.toggle_dir(&path) {
                    state.toast = Some(format!("Cannot expand folder: {err}"));
                }
            }
            vec![]
        }
        Command::OpenFilePath {
            path,
            line,
            col,
            preview,
        } => {
            if let Some(pos) = state
                .documents
                .iter()
                .position(|d| d.path.as_deref().is_some_and(|p| paths_match(p, &path)))
            {
                if !preview {
                    if let Some(meta) = state.tab_states.get_mut(pos) {
                        meta.preview = false;
                    }
                }
                if line > 1 || col > 1 {
                    let doc = &mut state.documents[pos];
                    let prev_head = doc.selection.primary_range().head;
                    let did = doc.id;
                    let idx =
                        doc.char_idx_of_line_col(line.saturating_sub(1), col.saturating_sub(1));
                    doc.selection = jide_core::Selection::point(idx);
                    state.nav_history.record(did, prev_head);
                    state.nav_history.record(did, idx);
                }
                state.set_focused_doc(Some(pos));
                state.scroll_y = 0.0;
                state.scroll_x = 0.0;
                state.push_mru(pos);
                let mut effects = vec![Effect::UpdateWindowTitle(state.window_title())];
                effects.extend(super::search_dispatch::refresh_find_if_open(state));
                return effects;
            }
            vec![Effect::RequestOpenFilePath {
                path,
                line,
                col,
                preview,
            }]
        }
        Command::CopyPath(path) => {
            vec![Effect::CopyToClipboard(path.display().to_string())]
        }
        Command::Editor(cmd) => {
            let mut effects = editor::dispatch_editor(state, cmd);
            effects.extend(super::search_dispatch::refresh_find_if_open(state));
            effects
        }
        Command::SaveActive | Command::SaveAsDialog => {
            let Some(idx) = state.focused_doc_idx() else {
                return vec![];
            };
            if cmd == Command::SaveActive {
                let now_ms = state.now_ms;
                let format_on_save = state.format_on_save;
                let trim = state.trim_trailing_whitespace_on_save;
                let final_nl = state.insert_final_newline_on_save;
                super::m5_dispatch::prepare_doc_for_save(
                    &mut state.documents[idx],
                    format_on_save,
                    trim,
                    final_nl,
                    now_ms,
                );
            }
            let doc = &state.documents[idx];
            if let Some(path) = doc.path.clone().filter(|_| cmd == Command::SaveActive) {
                vec![Effect::RequestSaveDoc {
                    doc_id: doc.id,
                    path,
                }]
            } else {
                vec![Effect::RequestSaveAsDialog { doc_id: doc.id }]
            }
        }
        Command::CloseActiveTab => {
            if let Some(index) = state.focused_doc_idx() {
                return close_tab_indices(state, vec![index]);
            }
            vec![]
        }
        Command::CloseTab(index) => close_tab_indices(state, vec![index]),
        Command::CloseAllTabs => {
            let indices: Vec<usize> = (0..state.documents.len()).collect();
            close_tab_indices(state, indices)
        }
        Command::CloseOtherTabs(keep) => {
            let indices: Vec<usize> = (0..state.documents.len()).filter(|&i| i != keep).collect();
            close_tab_indices(state, indices)
        }
        Command::CloseTabsToRight(from) => {
            let indices: Vec<usize> = ((from + 1)..state.documents.len()).collect();
            close_tab_indices(state, indices)
        }
        Command::CloseAllSavedTabs => {
            let mut effects = Vec::new();
            let mut i = state.documents.len();
            while i > 0 {
                i -= 1;
                if !state.documents[i].is_dirty() {
                    let doc_id = state.documents[i].id;
                    state.remove_tab(i);
                    effects.push(Effect::CleanupHotExitBackup(doc_id));
                }
            }
            effects.push(Effect::UpdateWindowTitle(state.window_title()));
            effects
        }
        Command::ConfirmCloseCancel => {
            state.pending_close = None;
            vec![]
        }
        Command::ConfirmCloseDiscard => {
            let mut effects = Vec::new();
            if let Some(mut queue) = state.pending_close.take() {
                if !queue.is_empty() {
                    let doc_id = queue.remove(0);
                    state.remove_doc_by_id(doc_id);
                    effects.push(Effect::CleanupHotExitBackup(doc_id));
                }
                if !queue.is_empty() {
                    state.pending_close = Some(queue);
                }
            }
            effects.push(Effect::UpdateWindowTitle(state.window_title()));
            effects
        }
        Command::ConfirmCloseSave => {
            let mut effects = Vec::new();
            if let Some(mut queue) = state.pending_close.take() {
                if !queue.is_empty() {
                    let doc_id = queue.remove(0);
                    if let Some(doc) = state.documents.iter().find(|d| d.id == doc_id) {
                        state.close_after_save.insert(doc_id);
                        if let Some(path) = doc.path.clone() {
                            effects.push(Effect::RequestSaveDoc { doc_id, path });
                        } else {
                            effects.push(Effect::RequestSaveAsDialog { doc_id });
                        }
                    }
                }
                if !queue.is_empty() {
                    state.pending_close = Some(queue);
                }
            }
            effects
        }
        Command::PinTab(index) => {
            if let Some(meta) = state.tab_states.get_mut(index) {
                meta.pinned = true;
            }
            vec![Effect::None]
        }
        Command::UnpinTab(index) => {
            if let Some(meta) = state.tab_states.get_mut(index) {
                meta.pinned = false;
            }
            vec![Effect::None]
        }
        Command::SelectTab(index) => {
            if index < state.documents.len() {
                state.set_focused_doc(Some(index));
                state.scroll_y = 0.0;
                state.scroll_x = 0.0;
                state.push_mru(index);
                if let Some(meta) = state.tab_states.get_mut(index) {
                    meta.preview = false;
                }
            }
            let mut effects = vec![Effect::UpdateWindowTitle(state.window_title())];
            effects.extend(super::search_dispatch::refresh_find_if_open(state));
            effects
        }
        Command::MruNext => {
            if state.tab_mru.len() > 1 {
                if state.mru_switcher_open {
                    state.mru_switcher_idx = (state.mru_switcher_idx + 1) % state.tab_mru.len();
                } else {
                    state.mru_switcher_open = true;
                    state.mru_switcher_idx = 1;
                }
            }
            vec![]
        }
        Command::MruPrev => {
            if state.tab_mru.len() > 1 {
                if state.mru_switcher_open {
                    state.mru_switcher_idx = state
                        .mru_switcher_idx
                        .checked_sub(1)
                        .unwrap_or(state.tab_mru.len() - 1);
                } else {
                    state.mru_switcher_open = true;
                    state.mru_switcher_idx = state.tab_mru.len() - 1;
                }
            }
            vec![]
        }
        Command::MruCommit => {
            let target = state.tab_mru.get(state.mru_switcher_idx).copied();
            state.mru_switcher_open = false;
            state.mru_switcher_idx = 0;
            if let Some(index) = target {
                return dispatch(state, Command::SelectTab(index));
            }
            vec![]
        }
        Command::MruCancel => {
            state.mru_switcher_open = false;
            state.mru_switcher_idx = 0;
            vec![]
        }
        Command::ZoomIn => {
            state.zoom = (state.zoom + 0.1).min(3.0);
            vec![]
        }
        Command::ZoomOut => {
            state.zoom = (state.zoom - 0.1).max(0.5);
            vec![]
        }
        Command::ToggleWordWrap => {
            state.word_wrap = !state.word_wrap;
            state.scroll_x = 0.0;
            vec![]
        }
        Command::ForceEnableHighlighting => {
            if let Some(doc) = state.active_document() {
                let id = doc.id;
                state.force_highlight.insert(id);
            }
            vec![]
        }
        Command::SetLanguage(language) => {
            if let Some(index) = state.focused_doc_idx() {
                if let Some(doc) = state.documents.get_mut(index) {
                    doc.language = language;
                }
            }
            vec![]
        }
        Command::NewFile => {
            state.untitled_counter += 1;
            let id = state.allocate_doc_id();
            let mut doc = Document::new_untitled(id);
            doc.tab_size = state.default_tab_size;
            doc.insert_spaces = state.default_insert_spaces;
            state.documents.push(doc);
            state.tab_states.push(TabMeta::default());
            let new_idx = state.documents.len() - 1;
            state.set_focused_doc(Some(new_idx));
            state.scroll_y = 0.0;
            state.scroll_x = 0.0;
            state.push_mru(new_idx);
            vec![Effect::UpdateWindowTitle(state.window_title())]
        }
        Command::OpenQuickOpen => {
            state.overlay = Some(OverlayMode::QuickOpen);
            state.overlay_query = String::new();
            vec![]
        }
        Command::OpenCommandPalette => {
            state.overlay = Some(OverlayMode::CommandPalette);
            state.overlay_query = String::new();
            vec![]
        }
        Command::CloseOverlay => {
            state.overlay = None;
            state.overlay_query = String::new();
            vec![]
        }
        Command::SwitchTheme(id) => {
            state.theme = match &id {
                ThemeId::JustLight => Theme::builtin_light(),
                ThemeId::HighContrast => Theme::builtin_high_contrast(),
                ThemeId::JustDark => Theme::builtin_dark(),
                ThemeId::Custom(name) => state
                    .available_custom_themes
                    .iter()
                    .find(|t| t.id == id || t.name == *name)
                    .cloned()
                    .unwrap_or_else(Theme::builtin_dark),
            };
            state.user_theme_selected = true;
            vec![Effect::ThemeChanged, Effect::SaveSettings]
        }
        Command::ResetZoom => {
            state.zoom = 1.0;
            vec![Effect::None]
        }
        Command::RunById(id) => {
            if id == "__goto_line__" {
                state.overlay = Some(OverlayMode::GotoLine);
                state.overlay_query = String::new();
                return vec![];
            }
            if let Some(query) = id.strip_prefix("__overlay_query__:") {
                query.clone_into(&mut state.overlay_query);
                return vec![];
            }
            let concrete = match id.as_str() {
                "workbench.action.files.newUntitledFile" => Some(Command::NewFile),
                "workbench.action.files.openFile" => Some(Command::OpenFileDialog),
                "workbench.action.files.openFolder" => Some(Command::OpenFolder),
                "workbench.action.files.save" => Some(Command::SaveActive),
                "workbench.action.files.saveAs" => Some(Command::SaveAsDialog),
                "workbench.action.closeActiveEditor" => Some(Command::CloseActiveTab),
                "workbench.action.closeAllEditors" => Some(Command::CloseAllTabs),
                "workbench.action.quickOpen" => Some(Command::OpenQuickOpen),
                "workbench.action.showCommands" => Some(Command::OpenCommandPalette),
                "workbench.action.gotoLine" => Some(Command::RunById("__goto_line__".into())),
                "workbench.action.toggleSidebarVisibility" => Some(Command::ToggleSidebar),
                "workbench.action.zoomIn" => Some(Command::ZoomIn),
                "workbench.action.zoomOut" => Some(Command::ZoomOut),
                "workbench.action.zoomReset" => Some(Command::ResetZoom),
                "editor.action.toggleWordWrap" => Some(Command::ToggleWordWrap),
                "editor.action.forceHighlight" => Some(Command::ForceEnableHighlighting),
                "actions.find" => Some(Command::OpenFind { replace: false }),
                "editor.action.startFindReplaceAction" => Some(Command::OpenFind { replace: true }),
                "workbench.action.findInFiles" => Some(Command::OpenFindInFiles { replace: false }),
                "workbench.action.replaceInFiles" => {
                    Some(Command::OpenFindInFiles { replace: true })
                }
                "editor.action.nextMatchFindAction" => Some(Command::FindNext),
                "editor.action.previousMatchFindAction" => Some(Command::FindPrev),
                "workbench.action.togglePanel" => Some(Command::ToggleBottomSearchPanel),
                "editor.action.toggleLineComment" | "editor.action.commentLine" => {
                    Some(Command::Editor(editor::EditorCmd::ToggleLineComment))
                }
                "editor.action.blockComment" => {
                    Some(Command::Editor(editor::EditorCmd::ToggleBlockComment))
                }
                "editor.action.transformToUppercase" => {
                    Some(Command::Editor(editor::EditorCmd::TransformUpper))
                }
                "editor.action.transformToLowercase" => {
                    Some(Command::Editor(editor::EditorCmd::TransformLower))
                }
                "editor.action.trimTrailingWhitespace" => {
                    Some(Command::Editor(editor::EditorCmd::TrimTrailingWhitespace))
                }
                "editor.action.sortLinesAscending" => {
                    Some(Command::Editor(editor::EditorCmd::SortLines {
                        descending: false,
                        unique: false,
                    }))
                }
                "editor.action.sortLinesDescending" => {
                    Some(Command::Editor(editor::EditorCmd::SortLines {
                        descending: true,
                        unique: false,
                    }))
                }
                "editor.action.sortLinesUnique" => {
                    Some(Command::Editor(editor::EditorCmd::SortLines {
                        descending: false,
                        unique: true,
                    }))
                }
                "expandLineSelection" => {
                    Some(Command::Editor(editor::EditorCmd::ExpandLineSelection))
                }
                "editor.action.insertLineAfter" => {
                    Some(Command::Editor(editor::EditorCmd::InsertLineBelow))
                }
                "editor.action.insertLineBefore" => {
                    Some(Command::Editor(editor::EditorCmd::InsertLineAbove))
                }
                "editor.action.addSelectionToNextFindMatch" => {
                    Some(Command::Editor(editor::EditorCmd::AddNextOccurrence))
                }
                "editor.action.selectHighlights" => {
                    Some(Command::Editor(editor::EditorCmd::SelectAllOccurrences))
                }
                "editor.action.insertCursorAbove" => {
                    Some(Command::Editor(editor::EditorCmd::AddCursorAbove))
                }
                "editor.action.insertCursorBelow" => {
                    Some(Command::Editor(editor::EditorCmd::AddCursorBelow))
                }
                "editor.action.jumpToBracket" => {
                    Some(Command::Editor(editor::EditorCmd::JumpToMatchingBracket))
                }
                "editor.action.formatDocument" => Some(Command::FormatDocument),
                "editor.action.formatSelection" => Some(Command::FormatSelection),
                "editor.action.toggleFormatOnSave" => Some(Command::ToggleFormatOnSave),
                "editor.action.toggleBracketColorization" => {
                    Some(Command::ToggleBracketColorization)
                }
                "editor.action.toggleIndentGuides" => Some(Command::ToggleIndentGuides),
                "workbench.action.navigateBack" => Some(Command::NavBack),
                "workbench.action.navigateForward" => Some(Command::NavForward),
                "workbench.action.splitEditor" => Some(Command::ToggleSplitEditor),
                "workbench.action.focusFirstEditorGroup" => Some(Command::FocusGroup(0)),
                "workbench.action.focusSecondEditorGroup" => Some(Command::FocusGroup(1)),
                "workbench.action.openSettings" => Some(Command::OpenSettings),
                "workbench.action.openSettingsToml" => Some(Command::OpenSettingsToml),
                "workbench.action.selectTheme" => Some(Command::OpenThemePicker),
                "workbench.action.toggleZenMode" => Some(Command::ToggleZenMode),
                "workbench.action.reopenClosedEditor" => Some(Command::ReopenClosedTab),
                "workbench.action.nextEditor" => Some(Command::NextTab),
                "workbench.action.previousEditor" => Some(Command::PrevTab),
                "workbench.action.files.saveAll" => Some(Command::SaveAll),
                "workbench.view.explorer" => Some(Command::ActivateSidebar(SidebarView::Explorer)),
                "terminal.openHere" => Some(Command::OpenTerminalHere),
                "workbench.action.quit" => Some(Command::QuitApp),
                _ => None,
            };
            if let Some(cmd) = concrete {
                return dispatch(state, cmd);
            }
            vec![]
        }
        Command::RevealInFileManager(path) => {
            vec![Effect::RevealInFileManager(path)]
        }
        Command::GotoLine { line, col } => {
            if let Some(idx_pos) = state.focused_doc_idx() {
                let (did, old_head, new_idx) = {
                    let doc = &mut state.documents[idx_pos];
                    let old_head = doc.selection.primary_range().head;
                    let idx =
                        doc.char_idx_of_line_col(line.saturating_sub(1), col.saturating_sub(1));
                    doc.selection = jide_core::Selection::point(idx);
                    (doc.id, old_head, idx)
                };
                state.nav_history.record(did, old_head);
                state.nav_history.record(did, new_idx);
            }
            state.overlay = None;
            state.overlay_query = String::new();
            vec![]
        }
        cmd @ (Command::OpenFind { .. }
        | Command::CloseFind
        | Command::FindQueryChanged(_)
        | Command::ReplaceTextChanged(_)
        | Command::ToggleFindCase
        | Command::ToggleFindWholeWord
        | Command::ToggleFindRegex
        | Command::ToggleFindInSelection
        | Command::ToggleFindReplaceRow
        | Command::FindNext
        | Command::FindPrev
        | Command::FindSelectAllMatches
        | Command::ReplaceCurrentMatch
        | Command::ReplaceAllMatches) => super::search_dispatch::dispatch_find(state, cmd),
        cmd @ (Command::OpenFindInFiles { .. }
        | Command::ToggleBottomSearchPanel
        | Command::ProjectSearchQueryChanged(_)
        | Command::ProjectReplaceChanged(_)
        | Command::ProjectIncludeChanged(_)
        | Command::ProjectExcludeChanged(_)
        | Command::ToggleProjectCase
        | Command::ToggleProjectWholeWord
        | Command::ToggleProjectRegex
        | Command::ToggleProjectGitignore
        | Command::ToggleProjectReplaceRow
        | Command::ProjectSearchToggleFile(_)
        | Command::CancelProjectSearch
        | Command::ProjectReplacePreviewRequest
        | Command::ProjectReplaceConfirmExecute
        | Command::ProjectReplaceCancel
        | Command::AckSearchFocus) => super::search_dispatch::dispatch_project_search(state, cmd),
        cmd @ (Command::FormatDocument
        | Command::FormatSelection
        | Command::ToggleFormatOnSave
        | Command::ToggleBracketColorization
        | Command::ToggleIndentGuides
        | Command::ToggleAutoClosePairs
        | Command::SetRenderWhitespace(_)
        | Command::SetEditorFontSize(_)
        | Command::SetTabSize(_)
        | Command::ToggleInsertSpaces
        | Command::ConvertIndentToSpaces
        | Command::ConvertIndentToTabs
        | Command::ConvertEol(_)
        | Command::OpenSettings
        | Command::CloseSettings
        | Command::SettingsQueryChanged(_)
        | Command::OpenSettingsToml
        | Command::OpenThemePicker
        | Command::ToggleZenMode
        | Command::ToggleSplitEditor
        | Command::FocusGroup(_)
        | Command::NextTab
        | Command::PrevTab
        | Command::ReopenClosedTab
        | Command::SaveAll
        | Command::NavBack
        | Command::NavForward
        | Command::BeginChordCtrlK
        | Command::CancelChord
        | Command::OpenTerminalHere
        | Command::QuitApp
        | Command::ToggleFontLigatures
        | Command::ToggleLineNumbers
        | Command::ToggleHighlightActiveLine
        | Command::ToggleBreadcrumbs
        | Command::ToggleActivityBar
        | Command::ToggleStatusBar
        | Command::ToggleSidebarPosition
        | Command::ToggleTrimOnSave
        | Command::ToggleFinalNewlineOnSave
        | Command::SetAutoSaveMode(_)
        | Command::ToggleFileIcons
        | Command::ToggleTreeGuides
        | Command::ToggleSmartCase
        | Command::SetLineHeight(_)
        | Command::SetSettingsCategory(_)
        | Command::ResetSettingsDefaults
        | Command::DismissToast) => super::m5_dispatch::dispatch_m5(state, cmd),
    }
}

fn close_tab_indices(state: &mut AppState, mut indices: Vec<usize>) -> Vec<Effect> {
    indices.sort_unstable();
    indices.dedup();
    let mut dirty_ids: Vec<DocId> = Vec::new();
    let mut clean_indices: Vec<usize> = Vec::new();
    for idx in indices {
        if let Some(doc) = state.documents.get(idx) {
            if doc.is_dirty() {
                dirty_ids.push(doc.id);
            } else {
                clean_indices.push(idx);
            }
        }
    }
    let mut effects = Vec::new();
    for idx in clean_indices.into_iter().rev() {
        let doc_id = state.documents[idx].id;
        state.remove_tab(idx);
        effects.push(Effect::CleanupHotExitBackup(doc_id));
    }
    if !dirty_ids.is_empty() {
        state.pending_close = Some(dirty_ids);
    }
    effects.push(Effect::UpdateWindowTitle(state.window_title()));
    effects
}
