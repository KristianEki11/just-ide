//! Workspace synchronization: hot-exit store, file watcher, session, and recents.

use jide_core::{Document, Selection};
use jide_platform::os_theme::OsAppearance;
use jide_ui::state::{paths_match, AppState, TabMeta};
use jide_ui::theme::{Theme, ThemeId};
use jide_workspace::hot_exit::HotExitStore;
use jide_workspace::keybindings::KeybindingMap;
use jide_workspace::paths::AppPaths;
use jide_workspace::session::{RecentList, SessionState, TabState};
use jide_workspace::settings::Settings;
use jide_workspace::tree::FileTree;
use jide_workspace::watcher::{WatchEvent, WorkspaceWatcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub struct WorkspaceSync {
    pub paths: Option<AppPaths>,
    pub watcher: Option<WorkspaceWatcher>,
    pub config_watcher: Option<WorkspaceWatcher>,
    pub last_hot_exit: Instant,
    pub last_watched_folder: Option<PathBuf>,
}

impl Default for WorkspaceSync {
    fn default() -> Self {
        let paths = AppPaths::new().ok();
        if let Some(ref p) = paths {
            let _ = p.ensure_dirs();
        }
        Self {
            paths,
            watcher: None,
            config_watcher: None,
            last_hot_exit: Instant::now(),
            last_watched_folder: None,
        }
    }
}

impl WorkspaceSync {
    /// Loads saved session, hot-exit backups, recent list, custom themes, and settings into `AppState`.
    pub fn load_state(&self, state: &mut AppState, reset_state: bool) {
        let Some(paths) = &self.paths else {
            apply_os_theme_fallback(state);
            return;
        };
        if reset_state {
            let _ = std::fs::remove_file(paths.session_file());
            let _ = std::fs::remove_dir_all(paths.backups_dir());
            apply_os_theme_fallback(state);
            return;
        }

        // Load recents.
        if let Ok(recents) = RecentList::load(&paths.recent_file()) {
            state.recent_files = recents.recent_files;
            state.recent_folders = recents.recent_folders;
        }

        restore_session(paths, state);
        restore_hot_exit(paths, state);
        self.load_custom_themes(state);
        self.apply_settings(state, true);

        if !state.documents.is_empty() && state.active_doc.is_none() {
            state.active_doc = Some(0);
        }
    }

    /// Scans `themes_dir` for `.toml` custom theme definitions.
    pub fn load_custom_themes(&self, state: &mut AppState) {
        let Some(paths) = &self.paths else {
            return;
        };
        let themes_dir = paths.themes_dir();
        let Ok(entries) = std::fs::read_dir(&themes_dir) else {
            return;
        };
        let mut custom = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                if let Ok(raw) = std::fs::read_to_string(&path) {
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("custom")
                        .to_owned();
                    match Theme::from_toml_str(ThemeId::Custom(stem), &raw) {
                        Ok(t) => custom.push(t),
                        Err(err) => {
                            state.toast = Some(format!(
                                "Invalid theme {}: {err}",
                                path.file_name().unwrap_or_default().to_string_lossy()
                            ));
                        }
                    }
                }
            }
        }
        state.available_custom_themes = custom;
    }

    /// Loads user + workspace `settings.toml` into `AppState`, auto-detecting OS theme on first launch.
    pub fn apply_settings(&self, state: &mut AppState, detect_os_if_unset: bool) {
        let Some(paths) = &self.paths else {
            if detect_os_if_unset && !state.user_theme_selected {
                apply_os_theme_fallback(state);
            }
            return;
        };
        let user_path = paths.settings_file();
        let ws_path = state
            .open_folder
            .as_ref()
            .map(|f| f.join(".jide").join("settings.toml"));
        let res = Settings::load(&user_path, ws_path.as_deref());
        if let Some(kb_warn) = KeybindingMap::validate_toml_file(&paths.keybindings_file()) {
            state.toast = Some(kb_warn);
        } else if let Some(first_warn) = res.warnings.first() {
            state.toast = Some(format!("Settings warning: {first_warn}"));
        }
        let s = &res.settings;
        state.editor_font_size = s.editor.font_size.clamp(8.0, 48.0);
        state.editor_line_height = s.editor.line_height.clamp(1.1, 2.2);
        state.font_ligatures = s.editor.font_ligatures;
        state.line_numbers = s.editor.line_numbers;
        state.highlight_active_line = s.editor.highlight_active_line;
        state.default_tab_size = s.editor.tab_size.clamp(1, 16);
        state.default_insert_spaces = s.editor.insert_spaces;
        state.word_wrap = s.editor.word_wrap;
        state.indent_guides = s.editor.indent_guides;
        state.bracket_pair_colorization = s.editor.bracket_pair_colorization;
        state.format_on_save = s.editor.format_on_save;
        state
            .render_whitespace
            .clone_from(&s.editor.render_whitespace);
        state.breadcrumbs_visible = s.ui.breadcrumbs_visible;
        state.activity_bar_visible = s.ui.activity_bar_visible;
        state.status_bar_visible = s.ui.status_bar_visible;
        state.side_bar_right = s.ui.side_bar_position.eq_ignore_ascii_case("right");
        state.auto_save_mode.clone_from(&s.files.auto_save);
        state.trim_trailing_whitespace_on_save = s.files.trim_trailing_whitespace;
        state.insert_final_newline_on_save = s.files.insert_final_newline;
        state.show_file_icons = s.explorer.show_icons;
        state.show_tree_guides = s.explorer.tree_guides;
        state.project_search.use_gitignore = s.search.use_ignore_files;
        state.smart_case_search = s.search.smart_case;

        let explicit_theme = Settings::has_explicit_theme(&user_path, ws_path.as_deref());
        if explicit_theme {
            state.theme = resolve_theme_by_name(&s.ui.theme, &state.available_custom_themes);
            state.user_theme_selected = true;
        } else if detect_os_if_unset && !state.user_theme_selected {
            apply_os_theme_fallback(state);
        }
    }

    /// Saves current UI-controlled preferences into user `settings.toml`.
    pub fn save_settings_from_state(&self, state: &AppState) {
        let Some(paths) = &self.paths else {
            return;
        };
        let user_path = paths.settings_file();
        let mut settings = Settings::load(&user_path, None).settings;
        settings.editor.font_size = state.editor_font_size;
        settings.editor.line_height = state.editor_line_height;
        settings.editor.font_ligatures = state.font_ligatures;
        settings.editor.line_numbers = state.line_numbers;
        settings.editor.highlight_active_line = state.highlight_active_line;
        settings.editor.tab_size = state.default_tab_size;
        settings.editor.insert_spaces = state.default_insert_spaces;
        settings.editor.word_wrap = state.word_wrap;
        settings
            .editor
            .render_whitespace
            .clone_from(&state.render_whitespace);
        settings.editor.indent_guides = state.indent_guides;
        settings.editor.bracket_pair_colorization = state.bracket_pair_colorization;
        settings.editor.format_on_save = state.format_on_save;
        settings.ui.breadcrumbs_visible = state.breadcrumbs_visible;
        settings.ui.activity_bar_visible = state.activity_bar_visible;
        settings.ui.status_bar_visible = state.status_bar_visible;
        if state.side_bar_right {
            "right"
        } else {
            "left"
        }
        .clone_into(&mut settings.ui.side_bar_position);
        settings.files.auto_save.clone_from(&state.auto_save_mode);
        settings.files.trim_trailing_whitespace = state.trim_trailing_whitespace_on_save;
        settings.files.insert_final_newline = state.insert_final_newline_on_save;
        settings.explorer.show_icons = state.show_file_icons;
        settings.explorer.tree_guides = state.show_tree_guides;
        settings.search.use_ignore_files = state.project_search.use_gitignore;
        settings.search.smart_case = state.smart_case_search;
        settings.ui.theme = match &state.theme.id {
            ThemeId::JustLight => "just-light".to_owned(),
            ThemeId::HighContrast => "high-contrast".to_owned(),
            ThemeId::JustDark => "just-dark".to_owned(),
            ThemeId::Custom(name) => name.clone(),
        };
        let _ = settings.save_toml(&user_path);
    }

    /// Ensures user `settings.toml` exists on disk and returns its path.
    #[must_use]
    pub fn ensure_user_settings_file(&self) -> Option<PathBuf> {
        let paths = self.paths.as_ref()?;
        let p = paths.settings_file();
        if !p.exists() {
            let _ = Settings::default().save_toml(&p);
        }
        Some(p)
    }

    /// Persists session and recents to disk.
    pub fn save_state(&self, state: &AppState) {
        let Some(paths) = &self.paths else {
            return;
        };

        // Save recents.
        let recents = RecentList {
            recent_folders: state.recent_folders.clone(),
            recent_files: state.recent_files.clone(),
        };
        let _ = recents.save(&paths.recent_file());

        // Save session.
        let tabs = state
            .documents
            .iter()
            .enumerate()
            .map(|(i, doc)| {
                let meta = state.tab_states.get(i);
                let (line, col) = doc.line_col_of(doc.selection.primary_range().head);
                TabState {
                    path: doc.path.clone(),
                    untitled_id: doc
                        .path
                        .is_none()
                        .then(|| usize::try_from(doc.id.0).ok())
                        .flatten(),
                    pinned: meta.is_some_and(|m| m.pinned),
                    preview: meta.is_some_and(|m| m.preview),
                    cursor: (line, col),
                    scroll: state.scroll_y,
                }
            })
            .collect();

        let session = SessionState {
            open_folder: state.open_folder.clone(),
            tabs,
            active_group: state.active_group,
            split_ratio: state.split_ratio,
        };
        let _ = session.save(&paths.session_file());
    }

    /// Flushes all dirty buffers (and untitled) to the hot-exit store, and removes stale backups.
    pub fn flush_hot_exit(&self, state: &AppState) {
        let Some(paths) = &self.paths else {
            return;
        };
        let store = HotExitStore::new(paths.backups_dir());

        let mut active_dirty_ids = HashSet::new();
        for doc in &state.documents {
            let key = format!("doc_{}", doc.id.0);
            if doc.is_dirty() {
                let Ok(bytes) = doc.encode_bytes() else {
                    continue;
                };
                active_dirty_ids.insert(key.clone());
                let (line, col) = doc.line_col_of(doc.selection.primary_range().head);
                let _ = store.backup_buffer(
                    &key,
                    doc.path.as_deref(),
                    doc.path
                        .is_none()
                        .then(|| format!("Untitled-{}", doc.id.0))
                        .as_deref(),
                    doc.encoding.name(),
                    doc.has_bom,
                    doc.eol.as_str(),
                    (line, col),
                    "",
                    &bytes,
                );
            } else {
                let _ = store.cleanup_backup(&key);
            }
        }

        if let Ok(manifest) = store.load_manifest() {
            for entry in manifest.entries {
                if !active_dirty_ids.contains(&entry.doc_id) {
                    let _ = store.cleanup_backup(&entry.doc_id);
                }
            }
        }
    }

    /// Removes the hot-exit backup for a discarded or closed document immediately.
    pub fn cleanup_doc_backup(&self, doc_id: jide_core::DocId) {
        if let Some(paths) = &self.paths {
            let store = HotExitStore::new(paths.backups_dir());
            let _ = store.cleanup_backup(&format!("doc_{}", doc_id.0));
        }
    }

    /// Checks the watcher and updates watcher paths if open folder changed.
    pub fn poll(
        &mut self,
        state: &mut AppState,
        workers: &mut crate::workers::Workers,
        ctx: &egui::Context,
    ) {
        if self.config_watcher.is_none() {
            if let Some(ref paths) = self.paths {
                let mut watch_dirs = Vec::new();
                if let Some(cfg_root) = paths.settings_file().parent() {
                    watch_dirs.push(cfg_root.to_path_buf());
                }
                if !watch_dirs.is_empty() {
                    let wakeup_ctx = ctx.clone();
                    self.config_watcher =
                        WorkspaceWatcher::new_with_wakeup(&watch_dirs, move || {
                            wakeup_ctx.request_repaint();
                        })
                        .ok();
                }
            }
        }

        // Update watcher if folder changed.
        if state.open_folder != self.last_watched_folder {
            let prev_lig = state.font_ligatures;
            self.last_watched_folder.clone_from(&state.open_folder);
            self.apply_settings(state, false);
            state.theme.apply_to_egui(ctx);
            if state.font_ligatures != prev_lig {
                jide_ui::fonts::configure_fonts(ctx, state.font_ligatures);
                jide_ui::fonts::spawn_lazy_font_loader(ctx, state.font_ligatures);
            }
            if let Some(ref folder) = state.open_folder {
                let wakeup_ctx = ctx.clone();
                self.watcher =
                    WorkspaceWatcher::new_with_wakeup(std::slice::from_ref(folder), move || {
                        wakeup_ctx.request_repaint();
                    })
                    .ok();
            } else {
                self.watcher = None;
            }
        }

        // Drain config watcher events (.toml hot reload).
        let mut config_dirty = false;
        if let Some(ref cfg_watcher) = self.config_watcher {
            while let Ok(ev) = cfg_watcher.events().try_recv() {
                let path = match ev {
                    WatchEvent::Created(p) | WatchEvent::Modified(p) | WatchEvent::Removed(p) => p,
                    WatchEvent::Renamed { to, .. } => to,
                };
                if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                    config_dirty = true;
                }
            }
        }

        // Drain watcher events.
        let mut tree_dirty = false;
        if let Some(ref watcher) = self.watcher {
            while let Ok(event) = watcher.events().try_recv() {
                match event {
                    WatchEvent::Modified(path) => {
                        if path.ends_with(".jide/settings.toml")
                            || path.ends_with(r".jide\settings.toml")
                        {
                            config_dirty = true;
                        }
                        handle_modified_file(&path, state, workers, ctx);
                    }
                    WatchEvent::Removed(path) => {
                        handle_removed_file(&path, state);
                        tree_dirty = true;
                    }
                    WatchEvent::Created(_) | WatchEvent::Renamed { .. } => {
                        tree_dirty = true;
                    }
                }
            }
        }

        if config_dirty {
            let prev_lig = state.font_ligatures;
            self.load_custom_themes(state);
            self.apply_settings(state, false);
            state.theme.apply_to_egui(ctx);
            if state.font_ligatures != prev_lig {
                jide_ui::fonts::configure_fonts(ctx, state.font_ligatures);
                jide_ui::fonts::spawn_lazy_font_loader(ctx, state.font_ligatures);
            }
        }

        if tree_dirty {
            if let Some(ref folder) = state.open_folder {
                if let Ok(tree) = FileTree::open(folder.clone(), false) {
                    state.file_tree = Some(tree);
                    state.tree_generation = state.tree_generation.wrapping_add(1);
                }
            }
        }

        // Auto hot-exit every 2 seconds.
        if self.last_hot_exit.elapsed().as_secs() >= 2 {
            self.last_hot_exit = Instant::now();
            self.flush_hot_exit(state);
        }
    }
}

fn handle_modified_file(
    path: &Path,
    state: &mut AppState,
    workers: &mut crate::workers::Workers,
    ctx: &egui::Context,
) {
    for doc in &state.documents {
        if doc.path.as_deref().is_some_and(|p| paths_match(p, path)) {
            if doc.is_dirty() {
                state.toast = Some(format!(
                    "External change: {} was modified on disk.",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
            } else {
                let (line, col) = doc.line_col_of(doc.selection.primary_range().head);
                let id = doc.id;
                let target_path = doc.path.clone().unwrap_or_else(|| path.to_path_buf());
                workers.reload(ctx, id, target_path, line, col);
            }
            break;
        }
    }
}

fn handle_removed_file(path: &Path, state: &mut AppState) {
    for doc in &state.documents {
        if doc.path.as_deref().is_some_and(|p| paths_match(p, path)) {
            state.toast = Some(format!(
                "{} was deleted from disk.",
                path.file_name().unwrap_or_default().to_string_lossy()
            ));
            break;
        }
    }
}

fn restore_session(paths: &AppPaths, state: &mut AppState) {
    let Ok(session) = SessionState::load(&paths.session_file()) else {
        return;
    };
    if let Some(folder) = session.open_folder.filter(|f| f.exists()) {
        state.workspace_name = folder
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_owned);
        if let Ok(tree) = FileTree::open(folder.clone(), false) {
            state.open_folder = Some(folder);
            state.file_tree = Some(tree);
            state.tree_generation = state.tree_generation.wrapping_add(1);
        }
    }
    for tab in session.tabs {
        let Some(path) = tab.path.filter(|p| p.exists()) else {
            continue;
        };
        let id = state.allocate_doc_id();
        let Ok(mut doc) = Document::open(id, &path) else {
            continue;
        };
        let first_line = (doc.rope.len_lines() > 0).then(|| doc.rope.line(0).to_string());
        doc.language = jide_core::LanguageId::new(
            jide_syntax::detect_language(Some(&path), first_line.as_deref()).id,
        );
        let idx = doc.char_idx_of_line_col(
            tab.cursor.0.saturating_sub(1),
            tab.cursor.1.saturating_sub(1),
        );
        doc.selection = Selection::point(idx);
        state.documents.push(doc);
        state.tab_states.push(TabMeta {
            preview: tab.preview,
            pinned: tab.pinned,
        });
        state.push_mru(state.documents.len() - 1);
    }
}

fn restore_hot_exit(paths: &AppPaths, state: &mut AppState) {
    let store = HotExitStore::new(paths.backups_dir());
    let Ok(manifest) = store.load_manifest() else {
        return;
    };
    for entry in manifest.entries {
        let Ok(bytes) = store.read_backup(&entry.doc_id) else {
            continue;
        };
        if let Some(ref entry_path) = entry.path {
            if let Some(existing) = state.documents.iter_mut().find(|d| {
                d.path
                    .as_deref()
                    .is_some_and(|p| paths_match(p, entry_path))
            }) {
                let mut restored =
                    build_dirty_restored_doc(existing.id, Some(entry_path), &bytes, entry.cursor);
                restored.id = existing.id;
                *existing = restored;
                continue;
            }
        }

        let id = state.allocate_doc_id();
        let doc = build_dirty_restored_doc(id, entry.path.as_ref(), &bytes, entry.cursor);
        state.documents.push(doc);
        state.tab_states.push(TabMeta {
            preview: false,
            pinned: false,
        });
        state.push_mru(state.documents.len() - 1);
    }
}

fn build_dirty_restored_doc(
    id: jide_core::DocId,
    path: Option<&PathBuf>,
    bytes: &[u8],
    cursor: (usize, usize),
) -> Document {
    let mut doc = if let Some(p) = path {
        let mut d = Document::from_bytes(id, Some(p.clone()), bytes)
            .unwrap_or_else(|_| Document::from_text(id, &String::from_utf8_lossy(bytes)));
        let first_line = (d.rope.len_lines() > 0).then(|| d.rope.line(0).to_string());
        d.language = jide_core::LanguageId::new(
            jide_syntax::detect_language(Some(p), first_line.as_deref()).id,
        );
        d
    } else {
        Document::from_text(id, &String::from_utf8_lossy(bytes))
    };
    doc.version = 1;
    doc.saved_version = 0;
    doc.selection = Selection::point(
        doc.char_idx_of_line_col(cursor.0.saturating_sub(1), cursor.1.saturating_sub(1)),
    );
    doc
}

fn apply_os_theme_fallback(state: &mut AppState) {
    let os = OsAppearance::detect();
    state.theme = if os.high_contrast {
        Theme::builtin_high_contrast()
    } else if !os.dark_mode {
        Theme::builtin_light()
    } else {
        Theme::builtin_dark()
    };
}

fn resolve_theme_by_name(name: &str, custom: &[Theme]) -> Theme {
    match name.to_ascii_lowercase().as_str() {
        "just-light" | "light" | "just light" => Theme::builtin_light(),
        "high-contrast" | "high contrast" => Theme::builtin_high_contrast(),
        "just-dark" | "dark" | "just dark" => Theme::builtin_dark(),
        other => custom
            .iter()
            .find(|t| {
                t.name.eq_ignore_ascii_case(other)
                    || matches!(&t.id, ThemeId::Custom(id) if id.eq_ignore_ascii_case(other))
            })
            .cloned()
            .unwrap_or_else(Theme::builtin_dark),
    }
}
