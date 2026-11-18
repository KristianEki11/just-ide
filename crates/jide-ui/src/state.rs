use crate::theme::{Theme, ThemeId};
use jide_core::{DocId, Document, LanguageId};
use jide_workspace::tree::FileTree;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod dispatch;
mod editor;
mod m5_dispatch;
mod search_dispatch;
pub use dispatch::dispatch;
pub use editor::EditorCmd;
pub use search_dispatch::refresh_find_if_open;

/// Per-tab metadata stored parallel to `AppState::documents`.
#[derive(Clone, Debug, Default)]
pub struct TabMeta {
    /// True while this is a preview tab (single-click open; replaced on next single-click).
    pub preview: bool,
    /// True when the tab is pinned (no close button).
    pub pinned: bool,
}

/// Active view inside the left sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarView {
    /// File tree explorer.
    Explorer,
    /// Project-wide find in files.
    Search,
}

/// Which overlay is currently open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OverlayMode {
    /// Quick Open (Ctrl+P): fuzzy file search.
    QuickOpen,
    /// Command Palette (Ctrl+Shift+P / F1): command search + MRU.
    CommandPalette,
    /// Go to line (Ctrl+G).
    GotoLine,
    /// Interactive Theme Picker (Ctrl+K Ctrl+T).
    ThemePicker,
}

/// All user-triggerable actions in Just IDE (Architecture ADR-010).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Toggles sidebar visibility when clicking the active activity icon.
    ToggleSidebar,
    /// Activates the Explorer sidebar view (or toggles if already active).
    ActivateSidebar(SidebarView),
    /// Requests opening a file dialog.
    OpenFile,
    /// Requests opening a folder dialog.
    OpenFolder,
    /// Opens a specific folder path directly (from IPC / drag-and-drop).
    OpenFolderPath(PathBuf),
    /// Creates an untitled buffer.
    NewFile,
    /// Opens Quick Open overlay.
    OpenQuickOpen,
    /// Opens Command Palette overlay.
    OpenCommandPalette,
    /// Closes any open overlay.
    CloseOverlay,
    /// Switches the active UI/syntax theme.
    SwitchTheme(ThemeId),
    /// Resets zoom to 100%.
    ResetZoom,
    OpenFileDialog,
    SaveActive,
    SaveAsDialog,
    CloseActiveTab,
    CloseTab(usize),
    ConfirmCloseSave,
    ConfirmCloseDiscard,
    ConfirmCloseCancel,
    SelectTab(usize),
    PinTab(usize),
    UnpinTab(usize),
    CloseAllTabs,
    CloseOtherTabs(usize),
    CloseTabsToRight(usize),
    CloseAllSavedTabs,
    ZoomIn,
    ZoomOut,
    ToggleWordWrap,
    /// Overrides large-file highlighting suppression for the active document.
    ForceEnableHighlighting,
    /// Explicitly assigns a syntax language to the active document.
    SetLanguage(LanguageId),
    Scroll {
        x_bits: u32,
        y_bits: u32,
    },
    Editor(EditorCmd),
    /// Execute a command by its palette id string.
    RunById(String),
    /// Open a file path directly with optional line/column and preview flag.
    OpenFilePath {
        path: PathBuf,
        line: usize,
        col: usize,
        preview: bool,
    },
    /// Toggle directory expand/collapse in the workspace file tree.
    ToggleFolderNode(PathBuf),
    /// Copy a file or folder path to the system clipboard.
    CopyPath(PathBuf),
    /// Reveal path in system file manager.
    RevealInFileManager(PathBuf),
    /// Navigate to line (optionally column), 1-based.
    GotoLine {
        line: usize,
        col: usize,
    },
    /// Open or cycle forward through the MRU tab switcher (Ctrl+Tab).
    MruNext,
    /// Open or cycle backward through the MRU tab switcher (Ctrl+Shift+Tab).
    MruPrev,
    /// Confirm the selected tab in the MRU tab switcher.
    MruCommit,
    /// Cancel the MRU tab switcher without switching tabs.
    MruCancel,
    // --- M4 Search & Replace ---
    OpenFind {
        replace: bool,
    },
    CloseFind,
    FindQueryChanged(String),
    ReplaceTextChanged(String),
    ToggleFindCase,
    ToggleFindWholeWord,
    ToggleFindRegex,
    ToggleFindInSelection,
    ToggleFindReplaceRow,
    FindNext,
    FindPrev,
    FindSelectAllMatches,
    ReplaceCurrentMatch,
    ReplaceAllMatches,
    OpenFindInFiles {
        replace: bool,
    },
    ToggleBottomSearchPanel,
    ProjectSearchQueryChanged(String),
    ProjectReplaceChanged(String),
    ProjectIncludeChanged(String),
    ProjectExcludeChanged(String),
    ToggleProjectCase,
    ToggleProjectWholeWord,
    ToggleProjectRegex,
    ToggleProjectGitignore,
    ToggleProjectReplaceRow,
    ProjectSearchToggleFile(PathBuf),
    CancelProjectSearch,
    ProjectReplacePreviewRequest,
    ProjectReplaceConfirmExecute,
    ProjectReplaceCancel,
    AckSearchFocus,
    // --- M5 Power Editing, Settings, Built-in Prettier, Split & Workspace ---
    FormatDocument,
    FormatSelection,
    ToggleFormatOnSave,
    ToggleBracketColorization,
    ToggleIndentGuides,
    ToggleAutoClosePairs,
    SetRenderWhitespace(String),
    SetEditorFontSize(u32),
    SetTabSize(usize),
    ToggleInsertSpaces,
    ConvertIndentToSpaces,
    ConvertIndentToTabs,
    ConvertEol(jide_core::Eol),
    OpenSettings,
    CloseSettings,
    SettingsQueryChanged(String),
    OpenSettingsToml,
    OpenThemePicker,
    ToggleZenMode,
    ToggleSplitEditor,
    FocusGroup(usize),
    NextTab,
    PrevTab,
    ReopenClosedTab,
    SaveAll,
    NavBack,
    NavForward,
    BeginChordCtrlK,
    CancelChord,
    OpenTerminalHere,
    QuitApp,
    // --- Expanded Application Settings & Layout ---
    ToggleFontLigatures,
    ToggleLineNumbers,
    ToggleHighlightActiveLine,
    ToggleBreadcrumbs,
    ToggleActivityBar,
    ToggleStatusBar,
    ToggleSidebarPosition,
    ToggleTrimOnSave,
    ToggleFinalNewlineOnSave,
    SetAutoSaveMode(String),
    ToggleFileIcons,
    ToggleTreeGuides,
    ToggleSmartCase,
    SetLineHeight(u32),
    SetSettingsCategory(String),
    ResetSettingsDefaults,
    DismissToast,
}

/// Side effects produced by [`dispatch`] for the application shell to execute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// No external side effect required.
    None,
    /// Re-apply updated theme visuals to `egui::Context`.
    ThemeChanged,
    /// Re-apply font definitions (e.g., ligatures toggle).
    FontsChanged,
    /// Informational notice (temporary placeholder until M1/M3 wire full handlers).
    Notice(String),
    RequestOpenFileDialog,
    RequestOpenFolderDialog,
    RequestOpenFilePath {
        path: PathBuf,
        line: usize,
        col: usize,
        preview: bool,
    },
    RequestSaveAsDialog {
        doc_id: DocId,
    },
    RequestSaveDoc {
        doc_id: DocId,
        path: PathBuf,
    },
    CleanupHotExitBackup(DocId),
    CopyToClipboard(String),
    CutToClipboard {
        text: String,
        doc_id: DocId,
        version: u64,
        selection: jide_core::Selection,
        whole_line: bool,
    },
    RequestPaste,
    UpdateWindowTitle(String),
    RevealInFileManager(PathBuf),
    RunBufferFind {
        doc_id: DocId,
        version: u64,
        query: jide_search::in_buffer::SearchQuery,
    },
    RunFilesSearch {
        search_id: u64,
        query: jide_search::in_files::FilesSearchQuery,
    },
    CancelFilesSearch,
    RunReplaceInFiles {
        query: jide_search::in_files::FilesSearchQuery,
        replacement: String,
    },
    OpenSettingsToml,
    SaveSettings,
    OpenTerminalHere(PathBuf),
    RequestQuit,
    ReloadThemeList,
}

/// Plain application state consumed by the UI shell.
#[derive(Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct AppState {
    /// Active theme.
    pub theme: Theme,
    /// Whether the sidebar panel is expanded.
    pub sidebar_visible: bool,
    /// Active view shown in the sidebar.
    pub sidebar_view: SidebarView,
    /// Default sidebar width in logical pixels.
    pub sidebar_width: f32,
    /// Active UI zoom factor.
    pub zoom: f32,
    /// Current workspace display name.
    pub workspace_name: Option<String>,
    /// Status message shown briefly in the shell.
    pub status_hint: Option<String>,
    /// Number of untitled buffers created in this stub session.
    pub untitled_counter: usize,
    pub documents: Vec<Document>,
    pub active_doc: Option<usize>,
    pub word_wrap: bool,
    pub editor_font_size: f32,
    pub scroll_y: f32,
    pub scroll_x: f32,
    pub whole_line_clipboard: Option<String>,
    pub toast: Option<String>,
    pub next_doc_id: u64,
    pub now_ms: u64,
    /// Lazy-loaded syntax set (None until Frame 0+ completes).
    pub syntax_set: Option<Arc<jide_syntax::SyntaxSet>>,
    /// Documents where the user explicitly enabled highlighting despite large-file mode.
    pub force_highlight: HashSet<DocId>,
    // --- M3 workspace fields ---
    /// Currently open folder root (if any).
    pub open_folder: Option<PathBuf>,
    /// Loaded file tree (Some when a folder is open).
    pub file_tree: Option<FileTree>,
    /// Which overlay is shown (Quick Open / Command Palette / Goto Line).
    pub overlay: Option<OverlayMode>,
    /// The text the user has typed in the overlay input.
    pub overlay_query: String,
    /// Tab metadata (preview flag, pinned flag) indexed by document position.
    pub tab_states: Vec<TabMeta>,
    /// MRU tab visit history (indices into `documents`).
    pub tab_mru: Vec<usize>,
    /// Whether the Ctrl+Tab MRU switcher is open.
    pub mru_switcher_open: bool,
    /// Index within mru list while Ctrl+Tab is held.
    pub mru_switcher_idx: usize,
    /// Recent-file list for empty state.
    pub recent_files: Vec<PathBuf>,
    /// Recent-folder list for empty state.
    pub recent_folders: Vec<PathBuf>,
    /// Dirty document ids awaiting `[Don't Save] [Cancel] [Save]` confirmation.
    pub pending_close: Option<Vec<DocId>>,
    /// Documents that should close once their async save completes.
    pub close_after_save: HashSet<DocId>,
    /// Monotonic counter bumped whenever the workspace tree refreshes or changes.
    pub tree_generation: u64,
    /// In-buffer Find & Replace widget state (`M4`).
    pub find: crate::find_widget::FindState,
    /// Project-wide streaming Find & Replace in Files state (`M4`).
    pub project_search: crate::search_panel::ProjectSearchState,
    // --- M5 + Built-in Prettier fields ---
    pub format_on_save: bool,
    pub bracket_pair_colorization: bool,
    pub indent_guides: bool,
    pub auto_close_pairs: bool,
    pub render_whitespace: String,
    pub default_tab_size: usize,
    pub default_insert_spaces: bool,
    pub zen_mode: bool,
    pub settings_open: bool,
    pub settings_query: String,
    pub split_open: bool,
    pub active_group: usize,
    pub group2_doc: Option<usize>,
    pub split_ratio: f32,
    pub closed_tabs_history: Vec<(PathBuf, TabMeta)>,
    pub nav_history: jide_core::NavigationHistory,
    pub chord_ctrl_k: bool,
    pub available_custom_themes: Vec<Theme>,
    pub user_theme_selected: bool,
    // --- Expanded App Settings ---
    pub editor_line_height: f32,
    pub font_ligatures: bool,
    pub line_numbers: bool,
    pub highlight_active_line: bool,
    pub breadcrumbs_visible: bool,
    pub activity_bar_visible: bool,
    pub status_bar_visible: bool,
    pub side_bar_right: bool,
    pub trim_trailing_whitespace_on_save: bool,
    pub insert_final_newline_on_save: bool,
    pub auto_save_mode: String,
    pub show_file_icons: bool,
    pub show_tree_guides: bool,
    pub smart_case_search: bool,
    pub settings_category: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            theme: Theme::builtin_dark(),
            sidebar_visible: true,
            sidebar_view: SidebarView::Explorer,
            sidebar_width: 260.0,
            zoom: 1.0,
            workspace_name: None,
            status_hint: None,
            untitled_counter: 0,
            documents: Vec::new(),
            active_doc: None,
            word_wrap: false,
            editor_font_size: 14.0,
            scroll_y: 0.0,
            scroll_x: 0.0,
            whole_line_clipboard: None,
            toast: None,
            next_doc_id: 1,
            now_ms: 0,
            syntax_set: None,
            force_highlight: HashSet::new(),
            open_folder: None,
            file_tree: None,
            overlay: None,
            overlay_query: String::new(),
            tab_states: Vec::new(),
            tab_mru: Vec::new(),
            mru_switcher_open: false,
            mru_switcher_idx: 0,
            recent_files: Vec::new(),
            recent_folders: Vec::new(),
            pending_close: None,
            close_after_save: HashSet::new(),
            tree_generation: 0,
            find: crate::find_widget::FindState::default(),
            project_search: crate::search_panel::ProjectSearchState::default(),
            format_on_save: true,
            bracket_pair_colorization: true,
            indent_guides: true,
            auto_close_pairs: true,
            render_whitespace: "selection".into(),
            default_tab_size: 4,
            default_insert_spaces: true,
            zen_mode: false,
            settings_open: false,
            settings_query: String::new(),
            split_open: false,
            active_group: 0,
            group2_doc: None,
            split_ratio: 0.5,
            closed_tabs_history: Vec::new(),
            nav_history: jide_core::NavigationHistory::default(),
            chord_ctrl_k: false,
            available_custom_themes: Vec::new(),
            user_theme_selected: false,
            editor_line_height: 1.5,
            font_ligatures: false,
            line_numbers: true,
            highlight_active_line: true,
            breadcrumbs_visible: true,
            activity_bar_visible: true,
            status_bar_visible: true,
            side_bar_right: false,
            trim_trailing_whitespace_on_save: false,
            insert_final_newline_on_save: true,
            auto_save_mode: "off".into(),
            show_file_icons: true,
            show_tree_guides: true,
            smart_case_search: true,
            settings_category: "All".into(),
        }
    }
}

impl AppState {
    #[must_use]
    pub fn focused_doc_idx(&self) -> Option<usize> {
        if self.split_open && self.active_group == 1 {
            self.group2_doc.or(self.active_doc)
        } else {
            self.active_doc
        }
    }

    pub fn set_focused_doc(&mut self, idx: Option<usize>) {
        if self.split_open && self.active_group == 1 {
            self.group2_doc = idx;
        } else {
            self.active_doc = idx;
        }
    }

    #[must_use]
    pub fn active_document(&self) -> Option<&Document> {
        self.focused_doc_idx()
            .and_then(|index| self.documents.get(index))
    }

    /// Returns `true` when syntax highlighting should run for `doc`.
    #[must_use]
    pub fn should_highlight(&self, doc: &Document) -> bool {
        !doc.is_binary && (!doc.large_file || self.force_highlight.contains(&doc.id))
    }

    pub fn allocate_doc_id(&mut self) -> DocId {
        let id = DocId(self.next_doc_id);
        self.next_doc_id += 1;
        id
    }

    #[must_use]
    pub fn window_title(&self) -> String {
        self.active_document().map_or_else(
            || "Just IDE".into(),
            |doc| {
                format!(
                    "{}{} — Just IDE",
                    document_name(doc),
                    if doc.is_dirty() { " ●" } else { "" }
                )
            },
        )
    }

    /// Remove tab at `index`, preserving `active_doc`, `group2_doc`, `tab_states`, and `tab_mru`.
    pub fn remove_tab(&mut self, index: usize) {
        if index >= self.documents.len() {
            return;
        }
        let removed = self.documents.remove(index);
        self.force_highlight.remove(&removed.id);
        let removed_meta = if self.tab_states.len() > index {
            self.tab_states.remove(index)
        } else {
            TabMeta::default()
        };
        if let Some(path) = removed.path {
            self.closed_tabs_history
                .retain(|(p, _)| !paths_match(p, &path));
            self.closed_tabs_history.push((path, removed_meta));
            if self.closed_tabs_history.len() > 20 {
                self.closed_tabs_history.remove(0);
            }
        }
        // Fix mru indices.
        self.tab_mru.retain(|&i| i != index);
        for mru_idx in &mut self.tab_mru {
            if *mru_idx > index {
                *mru_idx -= 1;
            }
        }
        // Fix active_doc.
        self.active_doc = if self.documents.is_empty() {
            None
        } else {
            match self.active_doc {
                Some(ai) if ai == index => self
                    .tab_mru
                    .first()
                    .copied()
                    .or_else(|| Some(index.min(self.documents.len() - 1))),
                Some(ai) if ai > index => Some(ai - 1),
                ai => ai,
            }
        };
        // Fix group2_doc.
        self.group2_doc = if self.documents.is_empty() {
            self.split_open = false;
            self.active_group = 0;
            None
        } else {
            match self.group2_doc {
                Some(g) if g == index => self.tab_mru.first().copied().or(self.active_doc),
                Some(g) if g > index => Some(g - 1),
                other => other,
            }
        };
        self.scroll_y = 0.0;
        self.scroll_x = 0.0;
    }

    /// Push `index` to the front of the MRU tab list.
    pub fn push_mru(&mut self, index: usize) {
        self.tab_mru.retain(|&i| i != index);
        self.tab_mru.insert(0, index);
    }

    /// Remove the tab owning `doc_id`, if present.
    pub fn remove_doc_by_id(&mut self, doc_id: DocId) {
        if let Some(pos) = self.documents.iter().position(|d| d.id == doc_id) {
            self.remove_tab(pos);
        }
    }

    /// Silently reload an already-open document in-place without stealing focus or duplicating tabs.
    pub fn reload_file_tab(&mut self, mut doc: Document) {
        let existing_pos = self.documents.iter().position(|d| {
            d.id == doc.id
                || (doc.path.is_some()
                    && d.path
                        .as_deref()
                        .is_some_and(|p| doc.path.as_deref().is_some_and(|q| paths_match(p, q))))
        });
        let Some(pos) = existing_pos else {
            self.open_file_tab(doc, false);
            return;
        };
        let (old_line, old_col) =
            self.documents[pos].line_col_of(self.documents[pos].selection.primary_range().head);
        let idx = doc.char_idx_of_line_col(old_line.saturating_sub(1), old_col.saturating_sub(1));
        doc.selection = jide_core::Selection::point(idx);
        doc.id = self.documents[pos].id;
        self.documents[pos] = doc;
    }

    /// Open a file as a tab; deduplicates open paths and replaces any current preview tab.
    pub fn open_file_tab(&mut self, doc: Document, preview: bool) {
        // 1. Check if this document or path is already open.
        if let Some(pos) = self.documents.iter().position(|d| {
            d.id == doc.id
                || (doc.path.is_some()
                    && d.path
                        .as_deref()
                        .is_some_and(|p| doc.path.as_deref().is_some_and(|q| paths_match(p, q))))
        }) {
            self.documents[pos] = doc;
            if !preview {
                if let Some(meta) = self.tab_states.get_mut(pos) {
                    meta.preview = false;
                }
            }
            self.set_focused_doc(Some(pos));
            self.scroll_y = 0.0;
            self.scroll_x = 0.0;
            self.push_mru(pos);
            return;
        }

        // 2. If opening a preview tab and another preview tab exists, replace it.
        if preview {
            if let Some(prev_preview_idx) = self.tab_states.iter().position(|m| m.preview) {
                self.force_highlight
                    .remove(&self.documents[prev_preview_idx].id);
                self.documents[prev_preview_idx] = doc;
                self.tab_states[prev_preview_idx] = TabMeta {
                    preview: true,
                    pinned: false,
                };
                self.set_focused_doc(Some(prev_preview_idx));
                self.scroll_y = 0.0;
                self.scroll_x = 0.0;
                self.push_mru(prev_preview_idx);
                return;
            }
        }
        self.documents.push(doc);
        self.tab_states.push(TabMeta {
            preview,
            pinned: false,
        });
        let idx = self.documents.len() - 1;
        self.set_focused_doc(Some(idx));
        self.scroll_y = 0.0;
        self.scroll_x = 0.0;
        self.push_mru(idx);
    }
}

/// Platform-aware path equivalence check (`\\?\` verbatim prefix and case on Windows).
#[must_use]
pub fn paths_match(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    let norm = |p: &Path| {
        let s = p.to_string_lossy().replace('\\', "/");
        s.strip_prefix("//?/").unwrap_or(&s).to_owned()
    };
    let na = norm(a);
    let nb = norm(b);
    if cfg!(windows) {
        na.eq_ignore_ascii_case(&nb)
    } else {
        na == nb
    }
}

#[must_use]
pub fn document_name(doc: &Document) -> String {
    doc.path
        .as_ref()
        .and_then(|path| path.file_name())
        .map_or_else(
            || format!("Untitled-{}", doc.id.0),
            |name| name.to_string_lossy().into_owned(),
        )
}
