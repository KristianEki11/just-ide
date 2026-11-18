//! M4 search & replace command handlers (in-buffer Find/Replace + streaming Find/Replace in Files).

use super::{AppState, Effect, SidebarView};
use jide_core::Range;
use jide_search::in_buffer::{replace_all_in_doc, replace_one_in_doc, SearchQuery};
use std::path::PathBuf;

pub(super) const LARGE_DOC_FIND_BYTES: usize = 1_048_576;

/// Refreshes find matches if `state.find.open` is true and doc/version changed.
pub fn refresh_find_if_open(state: &mut AppState) -> Vec<Effect> {
    if !state.find.open {
        return vec![];
    }
    let Some(doc) = state.active_document().cloned() else {
        state.find.recompute_sync(None);
        return vec![];
    };
    if state.find.cached_doc_id == Some(doc.id) && state.find.cached_version == doc.version {
        return vec![];
    }
    recompute_active_find(state)
}

pub(super) fn recompute_active_find(state: &mut AppState) -> Vec<Effect> {
    let Some(doc) = state.active_doc.and_then(|idx| state.documents.get(idx)) else {
        state.find.recompute_sync(None);
        return vec![];
    };
    if !state.find.query.is_empty() && doc.rope.len_bytes() > LARGE_DOC_FIND_BYTES {
        state.find.cached_doc_id = Some(doc.id);
        state.find.cached_version = doc.version;
        state.find.searching = true;
        return vec![Effect::RunBufferFind {
            doc_id: doc.id,
            version: doc.version,
            query: state.find.to_query(),
        }];
    }
    state.find.recompute_sync(Some(doc));
    vec![]
}

pub(super) fn dispatch_find(state: &mut AppState, cmd: super::Command) -> Vec<Effect> {
    use super::Command;
    match cmd {
        Command::OpenFind { replace } => {
            let doc_clone = state.active_document().cloned();
            state.find.open_with_doc(doc_clone.as_ref(), replace);
            recompute_active_find(state)
        }
        Command::CloseFind => {
            state.find.open = false;
            state.find.searching = false;
            vec![]
        }
        Command::FindQueryChanged(query) => {
            state.find.query = query;
            recompute_active_find(state)
        }
        Command::ReplaceTextChanged(rep) => {
            state.find.replacement = rep;
            vec![]
        }
        Command::ToggleFindCase => {
            state.find.case_sensitive = !state.find.case_sensitive;
            recompute_active_find(state)
        }
        Command::ToggleFindWholeWord => {
            state.find.whole_word = !state.find.whole_word;
            recompute_active_find(state)
        }
        Command::ToggleFindRegex => {
            state.find.use_regex = !state.find.use_regex;
            recompute_active_find(state)
        }
        Command::ToggleFindInSelection => {
            state.find.in_selection = !state.find.in_selection;
            if state.find.in_selection && state.find.selection_bounds.is_none() {
                state.find.selection_bounds =
                    state.active_document().map(|d| d.selection.primary_range());
            }
            recompute_active_find(state)
        }
        Command::ToggleFindReplaceRow => {
            state.find.replace_open = !state.find.replace_open;
            vec![]
        }
        Command::FindNext | Command::FindPrev => {
            let forward = matches!(cmd, Command::FindNext);
            if !state.find.open {
                let doc_clone = state.active_document().cloned();
                state.find.open_with_doc(doc_clone.as_ref(), false);
                let effects = recompute_active_find(state);
                if state.find.matches.is_empty() {
                    return effects;
                }
            }
            let Some(idx) = state.active_doc else {
                return vec![];
            };
            let mut find = state.find.clone();
            if let Some(doc) = state.documents.get_mut(idx) {
                find.step_match(doc, forward);
            }
            state.find = find;
            vec![]
        }
        Command::FindSelectAllMatches => {
            let Some(idx) = state.active_doc else {
                return vec![];
            };
            let find = &state.find;
            if let Some(doc) = state.documents.get_mut(idx) {
                find.select_all_matches(doc);
            }
            vec![]
        }
        Command::ReplaceCurrentMatch => replace_current_match(state),
        Command::ReplaceAllMatches => replace_all_matches(state),
        _ => vec![],
    }
}

fn replace_current_match(state: &mut AppState) -> Vec<Effect> {
    let Some(idx) = state.active_doc else {
        return vec![];
    };
    let Some(active) = state.find.matches.get(state.find.active_match).cloned() else {
        return vec![];
    };
    let mut single_query = state.find.to_query();
    single_query.in_selection = Some(Range {
        anchor: active.start_char,
        head: active.end_char,
    });
    let replacement = state.find.replacement.clone();
    let now = state.now_ms;
    if let Some(doc) = state.documents.get_mut(idx) {
        if let Err(err) = replace_one_in_doc(doc, &single_query, &replacement, now) {
            state.toast = Some(err.to_string());
            return vec![];
        }
    }
    if let Some(meta) = state.tab_states.get_mut(idx) {
        meta.preview = false;
    }
    let mut effects = recompute_active_find(state);
    effects.push(Effect::UpdateWindowTitle(state.window_title()));
    effects
}

fn replace_all_matches(state: &mut AppState) -> Vec<Effect> {
    let Some(idx) = state.active_doc else {
        return vec![];
    };
    let query: SearchQuery = state.find.to_query();
    let replacement = state.find.replacement.clone();
    let now = state.now_ms;
    if let Some(doc) = state.documents.get_mut(idx) {
        match replace_all_in_doc(doc, &query, &replacement, now) {
            Ok(count) => {
                state.toast = Some(format!("Replaced {count} occurrences."));
            }
            Err(err) => {
                state.toast = Some(err.to_string());
                return vec![];
            }
        }
    }
    if let Some(meta) = state.tab_states.get_mut(idx) {
        meta.preview = false;
    }
    let mut effects = recompute_active_find(state);
    effects.push(Effect::UpdateWindowTitle(state.window_title()));
    effects
}

pub(super) fn dispatch_project_search(state: &mut AppState, cmd: super::Command) -> Vec<Effect> {
    use super::Command;
    match cmd {
        Command::OpenFindInFiles { replace } => {
            if !state.project_search.in_bottom_panel {
                state.sidebar_visible = true;
                state.sidebar_view = SidebarView::Search;
            }
            state.project_search.replace_expanded = replace;
            state.project_search.request_focus = true;
            if let Some(doc) = state.active_document() {
                if !doc.selection.primary_range().is_empty() {
                    let sel = doc.selected_text();
                    if !sel.contains('\n') && !sel.is_empty() {
                        state.project_search.query = sel;
                        return trigger_project_search(state);
                    }
                }
            }
            vec![]
        }
        Command::ToggleBottomSearchPanel => {
            state.project_search.in_bottom_panel = !state.project_search.in_bottom_panel;
            if state.project_search.in_bottom_panel {
                state.project_search.request_focus = true;
            }
            vec![]
        }
        Command::ProjectSearchQueryChanged(q) => {
            state.project_search.query = q;
            trigger_project_search(state)
        }
        Command::ProjectReplaceChanged(rep) => {
            state.project_search.replacement = rep;
            vec![]
        }
        Command::ProjectIncludeChanged(inc) => {
            state.project_search.include_glob = inc;
            trigger_project_search(state)
        }
        Command::ProjectExcludeChanged(exc) => {
            state.project_search.exclude_glob = exc;
            trigger_project_search(state)
        }
        Command::ToggleProjectCase => {
            state.project_search.case_sensitive = !state.project_search.case_sensitive;
            trigger_project_search(state)
        }
        Command::ToggleProjectWholeWord => {
            state.project_search.whole_word = !state.project_search.whole_word;
            trigger_project_search(state)
        }
        Command::ToggleProjectRegex => {
            state.project_search.use_regex = !state.project_search.use_regex;
            trigger_project_search(state)
        }
        Command::ToggleProjectGitignore => {
            state.project_search.use_gitignore = !state.project_search.use_gitignore;
            trigger_project_search(state)
        }
        Command::ToggleProjectReplaceRow => {
            state.project_search.replace_expanded = !state.project_search.replace_expanded;
            vec![]
        }
        Command::ProjectSearchToggleFile(path) => {
            toggle_file_collapse(state, &path);
            vec![]
        }
        Command::CancelProjectSearch => {
            state.project_search.searching = false;
            vec![Effect::CancelFilesSearch]
        }
        Command::ProjectReplacePreviewRequest => {
            if !state.project_search.query.is_empty() && state.project_search.total_matches > 0 {
                state.project_search.confirm_replace = Some((
                    state.project_search.results.len(),
                    state.project_search.total_matches,
                ));
            }
            vec![]
        }
        Command::ProjectReplaceCancel => {
            state.project_search.confirm_replace = None;
            vec![]
        }
        Command::ProjectReplaceConfirmExecute => {
            state.project_search.confirm_replace = None;
            let Some(root) = state.open_folder.clone() else {
                return vec![];
            };
            let query = state.project_search.to_files_query(root);
            let replacement = state.project_search.replacement.clone();
            vec![Effect::RunReplaceInFiles { query, replacement }]
        }
        Command::AckSearchFocus => {
            state.find.request_focus = false;
            state.project_search.request_focus = false;
            vec![]
        }
        _ => vec![],
    }
}

fn toggle_file_collapse(state: &mut AppState, path: &PathBuf) {
    if !state.project_search.collapsed_files.remove(path) {
        state.project_search.collapsed_files.insert(path.clone());
    }
}

fn trigger_project_search(state: &mut AppState) -> Vec<Effect> {
    let search_id = state.project_search.begin_new_search();
    let Some(root) = state.open_folder.clone() else {
        state.project_search.searching = false;
        return vec![Effect::CancelFilesSearch];
    };
    if state.project_search.query.is_empty() {
        return vec![Effect::CancelFilesSearch];
    }
    let query = state.project_search.to_files_query(root);
    vec![Effect::RunFilesSearch { search_id, query }]
}
