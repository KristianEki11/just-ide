use crossbeam_channel::{Receiver, Sender};
use jide_core::{DocId, Document, SaveFault, Selection};
use jide_search::in_buffer::{find_in_rope, replace_all_in_doc, BufferMatch, SearchQuery};
use jide_search::in_files::{
    search_in_files_streaming, FileSearchResult, FilesSearchQuery, SearchStreamEvent,
};
use jide_search::replace_in_files::{apply_replace_in_files, ReplaceResult};
use jide_ui::state::{dispatch, paths_match, AppState, Command, EditorCmd, Effect};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub enum JobResult {
    FileOpened {
        doc: Box<Document>,
        preview: bool,
    },
    FileReloaded(Box<Document>),
    FileSaved {
        doc_id: DocId,
        version: u64,
        path: PathBuf,
    },
    DialogPicked {
        doc_id: Option<DocId>,
        path: PathBuf,
    },
    DialogCancelled,
    FolderPicked(PathBuf),
    CutCopied {
        doc_id: DocId,
        version: u64,
        selection: Selection,
        whole_line: bool,
    },
    Paste {
        doc_id: DocId,
        version: u64,
        selection: Selection,
        text: String,
    },
    ErrorToast {
        doc_id: Option<DocId>,
        message: String,
    },
    SyntaxSetLoaded(Arc<jide_syntax::SyntaxSet>),
    BufferFindCompleted {
        doc_id: DocId,
        version: u64,
        matches: Vec<BufferMatch>,
        regex_error: Option<String>,
    },
    FilesSearchBatch {
        search_id: u64,
        results: Vec<FileSearchResult>,
    },
    FilesSearchComplete {
        search_id: u64,
        elapsed_ms: u64,
    },
    FilesSearchError {
        search_id: u64,
        error: String,
    },
    FilesReplaceDone {
        result: ReplaceResult,
        query: FilesSearchQuery,
        replacement: String,
    },
}

pub struct Workers {
    sender: Sender<JobResult>,
    receiver: Receiver<JobResult>,
    saving: HashSet<DocId>,
    clipboard: Sender<Box<dyn FnOnce() + Send>>,
    buffer_find_cancel: Option<Arc<AtomicBool>>,
    files_search_cancel: Option<Arc<AtomicBool>>,
}

impl Default for Workers {
    fn default() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        let (clipboard, jobs) = crossbeam_channel::unbounded::<Box<dyn FnOnce() + Send>>();
        std::thread::spawn(move || {
            while let Ok(job) = jobs.recv() {
                job();
            }
        });
        Self {
            clipboard,
            sender,
            receiver,
            saving: HashSet::new(),
            buffer_find_cancel: None,
            files_search_cancel: None,
        }
    }
}

impl Workers {
    fn clipboard_job(&self, ctx: &egui::Context, job: impl FnOnce() -> JobResult + Send + 'static) {
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        let _ = self.clipboard.send(Box::new(move || {
            let _ = sender.send(job());
            ctx.request_repaint();
        }));
    }

    fn spawn(&self, ctx: &egui::Context, job: impl FnOnce() -> JobResult + Send + 'static) {
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(job());
            ctx.request_repaint();
        });
    }

    pub fn spawn_syntax_set_loader(&self, ctx: &egui::Context) {
        self.spawn(ctx, || {
            let syntax_set = Arc::new(jide_syntax::load_syntax_set());
            JobResult::SyntaxSetLoaded(syntax_set)
        });
    }

    pub fn open(
        &self,
        ctx: &egui::Context,
        id: DocId,
        path: PathBuf,
        line: usize,
        col: usize,
        preview: bool,
    ) {
        self.spawn(ctx, move || match load_doc_at(id, &path, line, col) {
            Ok(doc) => JobResult::FileOpened {
                doc: Box::new(doc),
                preview,
            },
            Err(error) => JobResult::ErrorToast {
                doc_id: None,
                message: format!("Opening {}: {error}", path.display()),
            },
        });
    }

    pub fn reload(&self, ctx: &egui::Context, id: DocId, path: PathBuf, line: usize, col: usize) {
        self.spawn(ctx, move || match load_doc_at(id, &path, line, col) {
            Ok(doc) => JobResult::FileReloaded(Box::new(doc)),
            Err(error) => JobResult::ErrorToast {
                doc_id: None,
                message: format!("Reloading {}: {error}", path.display()),
            },
        });
    }

    #[allow(clippy::too_many_lines)]
    pub fn effect(&mut self, ctx: &egui::Context, state: &mut AppState, effect: Effect) {
        match effect {
            Effect::RequestOpenFileDialog => self.spawn(ctx, || {
                rfd::FileDialog::new()
                    .pick_file()
                    .map_or(JobResult::DialogCancelled, |path| JobResult::DialogPicked {
                        doc_id: None,
                        path,
                    })
            }),
            Effect::RequestOpenFolderDialog => self.spawn(ctx, || {
                rfd::FileDialog::new()
                    .pick_folder()
                    .map_or(JobResult::DialogCancelled, JobResult::FolderPicked)
            }),
            Effect::RequestOpenFilePath {
                path,
                line,
                col,
                preview,
            } => {
                let id = state.allocate_doc_id();
                self.open(ctx, id, path, line, col, preview);
            }
            Effect::RevealInFileManager(path) => {
                let _ = jide_platform::shell_exec::reveal_in_file_manager(&path);
            }
            Effect::RequestSaveAsDialog { doc_id } => self.spawn(ctx, move || {
                rfd::FileDialog::new()
                    .save_file()
                    .map_or(JobResult::DialogCancelled, |path| JobResult::DialogPicked {
                        doc_id: Some(doc_id),
                        path,
                    })
            }),
            Effect::RequestSaveDoc { doc_id, path } => {
                if !self.saving.is_empty() {
                    state.toast = Some("A save is already in progress.".into());
                    return;
                }
                let Some(source) = state.documents.iter().find(|doc| doc.id == doc_id) else {
                    return;
                };
                // Avoid cloning potentially large undo histories on the UI thread.
                let mut snapshot = Document::new_untitled(doc_id);
                snapshot.rope = source.rope.clone();
                snapshot.encoding = source.encoding;
                snapshot.has_bom = source.has_bom;
                snapshot.eol = source.eol;
                snapshot.version = source.version;
                snapshot.read_only = source.read_only;
                self.saving.insert(doc_id);
                self.spawn(ctx, move || {
                    match snapshot.save_to(&path, SaveFault::None) {
                        Ok(()) => JobResult::FileSaved {
                            doc_id,
                            version: snapshot.version,
                            path,
                        },
                        Err(error) => JobResult::ErrorToast {
                            doc_id: Some(doc_id),
                            message: format!("Saving {}: {error}", path.display()),
                        },
                    }
                });
            }
            Effect::CutToClipboard {
                text,
                doc_id,
                version,
                selection,
                whole_line,
            } => {
                self.clipboard_job(ctx, move || {
                    match arboard::Clipboard::new()
                        .and_then(|mut clipboard| clipboard.set_text(text))
                    {
                        Ok(()) => JobResult::CutCopied {
                            doc_id,
                            version,
                            selection,
                            whole_line,
                        },
                        Err(error) => JobResult::ErrorToast {
                            doc_id: None,
                            message: error.to_string(),
                        },
                    }
                });
            }
            Effect::CopyToClipboard(text) => {
                self.clipboard_job(ctx, move || {
                    match arboard::Clipboard::new()
                        .and_then(|mut clipboard| clipboard.set_text(text))
                    {
                        Ok(()) => JobResult::DialogCancelled,
                        Err(error) => JobResult::ErrorToast {
                            doc_id: None,
                            message: error.to_string(),
                        },
                    }
                });
            }
            Effect::RequestPaste => {
                let Some(doc) = state.active_document() else {
                    return;
                };
                let (doc_id, version, selection) = (doc.id, doc.version, doc.selection.clone());
                self.clipboard_job(ctx, move || {
                    match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
                        Ok(text) => JobResult::Paste {
                            doc_id,
                            version,
                            selection,
                            text,
                        },
                        Err(error) => JobResult::ErrorToast {
                            doc_id: None,
                            message: error.to_string(),
                        },
                    }
                });
            }
            Effect::UpdateWindowTitle(title) => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
            }
            Effect::ThemeChanged => state.theme.apply_to_egui(ctx),
            Effect::Notice(message) => state.toast = Some(message),
            Effect::CleanupHotExitBackup(_)
            | Effect::OpenSettingsToml
            | Effect::SaveSettings
            | Effect::RequestQuit
            | Effect::ReloadThemeList
            | Effect::FontsChanged
            | Effect::None => {}
            Effect::OpenTerminalHere(dir) => {
                self.spawn(
                    ctx,
                    move || match jide_platform::shell_exec::open_terminal_at(&dir) {
                        Ok(()) => JobResult::DialogCancelled,
                        Err(err) => JobResult::ErrorToast {
                            doc_id: None,
                            message: format!("Open terminal failed: {err}"),
                        },
                    },
                );
            }
            Effect::RunBufferFind {
                doc_id,
                version,
                query,
            } => {
                if let Some(prev) = self.buffer_find_cancel.take() {
                    prev.store(true, Ordering::Release);
                }
                let Some(doc) = state
                    .documents
                    .iter()
                    .find(|d| d.id == doc_id && d.version == version)
                else {
                    return;
                };
                let rope = doc.rope.clone();
                let cancel = Arc::new(AtomicBool::new(false));
                self.buffer_find_cancel = Some(Arc::clone(&cancel));
                self.spawn(ctx, move || {
                    match find_in_rope(&rope, &query, Some(&cancel)) {
                        Ok(matches) => JobResult::BufferFindCompleted {
                            doc_id,
                            version,
                            matches,
                            regex_error: None,
                        },
                        Err(jide_search::SearchError::Cancelled) => JobResult::DialogCancelled,
                        Err(err) => JobResult::BufferFindCompleted {
                            doc_id,
                            version,
                            matches: Vec::new(),
                            regex_error: Some(err.to_string()),
                        },
                    }
                });
            }
            Effect::RunFilesSearch { search_id, query } => {
                self.spawn_project_search(ctx, search_id, query);
            }
            Effect::CancelFilesSearch => {
                if let Some(prev) = self.files_search_cancel.take() {
                    prev.store(true, Ordering::Release);
                }
            }
            Effect::RunReplaceInFiles { query, replacement } => {
                let open_paths: Vec<PathBuf> = state
                    .documents
                    .iter()
                    .filter_map(|d| d.path.clone())
                    .collect();
                let now_ms = state.now_ms;
                self.spawn(ctx, move || {
                    let open_refs: Vec<&Path> = open_paths.iter().map(PathBuf::as_path).collect();
                    match apply_replace_in_files(query.clone(), &replacement, &open_refs, now_ms) {
                        Ok(result) => JobResult::FilesReplaceDone {
                            result,
                            query,
                            replacement,
                        },
                        Err(err) => JobResult::ErrorToast {
                            doc_id: None,
                            message: err.to_string(),
                        },
                    }
                });
            }
        }
    }

    fn spawn_project_search(
        &mut self,
        ctx: &egui::Context,
        search_id: u64,
        query: FilesSearchQuery,
    ) {
        if let Some(prev) = self.files_search_cancel.take() {
            prev.store(true, Ordering::Release);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.files_search_cancel = Some(Arc::clone(&cancel));

        let (batch_tx, batch_rx) = crossbeam_channel::unbounded::<SearchStreamEvent>();
        let out_tx = self.sender.clone();
        let ui_ctx = ctx.clone();
        let started = Instant::now();
        let cancel_worker = Arc::clone(&cancel);

        std::thread::spawn(move || {
            let err_tx = out_tx.clone();
            let err_ctx = ui_ctx.clone();
            let worker_handle = std::thread::spawn(move || {
                if let Err(err) = search_in_files_streaming(query, cancel_worker, batch_tx) {
                    if !matches!(err, jide_search::SearchError::Cancelled) {
                        let _ = err_tx.send(JobResult::FilesSearchError {
                            search_id,
                            error: err.to_string(),
                        });
                        err_ctx.request_repaint();
                    }
                }
            });

            while let Ok(ev) = batch_rx.recv() {
                match ev {
                    SearchStreamEvent::Results(results) => {
                        let _ = out_tx.send(JobResult::FilesSearchBatch { search_id, results });
                        ui_ctx.request_repaint();
                    }
                    SearchStreamEvent::Complete { .. } => {
                        let elapsed_ms =
                            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                        let _ = out_tx.send(JobResult::FilesSearchComplete {
                            search_id,
                            elapsed_ms,
                        });
                        ui_ctx.request_repaint();
                    }
                    SearchStreamEvent::FileError { .. } => {}
                }
            }
            let _ = worker_handle.join();
        });
    }

    pub fn drain(&mut self, ctx: &egui::Context, state: &mut AppState) {
        while let Ok(result) = self.receiver.try_recv() {
            self.handle_job_result(ctx, state, result);
            for eff in jide_ui::state::refresh_find_if_open(state) {
                self.effect(ctx, state, eff);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(state.window_title()));
        }
    }

    fn handle_job_result(&mut self, ctx: &egui::Context, state: &mut AppState, result: JobResult) {
        match result {
            JobResult::CutCopied {
                doc_id,
                version,
                selection,
                whole_line,
            } => {
                if let Some(doc) = state.documents.iter_mut().find(|doc| {
                    doc.id == doc_id && doc.version == version && doc.selection == selection
                }) {
                    let result = if whole_line {
                        doc.delete_lines(state.now_ms)
                    } else {
                        doc.delete_forward(state.now_ms)
                    };
                    if let Err(error) = result {
                        state.toast = Some(error.to_string());
                    }
                } else {
                    state.toast =
                        Some("Copied, but cut cancelled because the selection changed.".into());
                }
            }
            JobResult::FileOpened { doc, preview } => {
                if let Some(path) = &doc.path {
                    state.recent_files.retain(|p| p != path);
                    state.recent_files.insert(0, path.clone());
                }
                state.open_file_tab(*doc, preview);
            }
            JobResult::FileReloaded(doc) => {
                state.reload_file_tab(*doc);
            }
            JobResult::FolderPicked(path) => {
                let _ = dispatch(state, Command::OpenFolderPath(path));
            }
            JobResult::FileSaved {
                doc_id,
                version,
                path,
            } => {
                self.saving.remove(&doc_id);
                if let Some(doc) = state.documents.iter_mut().find(|doc| doc.id == doc_id) {
                    if doc.language == jide_core::LanguageId::default() {
                        let first_line =
                            (doc.rope.len_lines() > 0).then(|| doc.rope.line(0).to_string());
                        doc.language = jide_core::LanguageId::new(
                            jide_syntax::detect_language(Some(&path), first_line.as_deref()).id,
                        );
                    }
                    doc.path = Some(path);
                    doc.saved_version = version;
                }
                if state.close_after_save.remove(&doc_id) {
                    state.remove_doc_by_id(doc_id);
                }
            }
            JobResult::DialogPicked {
                doc_id: Some(doc_id),
                path,
            } => self.effect(ctx, state, Effect::RequestSaveDoc { doc_id, path }),
            JobResult::DialogPicked { doc_id: None, path } => {
                let id = state.allocate_doc_id();
                self.open(ctx, id, path, 1, 1, false);
            }
            JobResult::Paste {
                doc_id,
                version,
                selection,
                text,
            } => {
                if state.active_document().is_some_and(|doc| {
                    doc.id == doc_id && doc.version == version && doc.selection == selection
                }) {
                    dispatch(state, Command::Editor(EditorCmd::PasteText(text)));
                } else {
                    state.toast =
                        Some("Paste cancelled because the document or selection changed.".into());
                }
            }
            JobResult::ErrorToast { doc_id, message } => {
                if let Some(id) = doc_id {
                    self.saving.remove(&id);
                }
                state.toast = Some(message);
            }
            JobResult::DialogCancelled => {}
            JobResult::SyntaxSetLoaded(syntax_set) => {
                state.syntax_set = Some(syntax_set);
            }
            other => handle_search_job_result(state, other),
        }
    }
}

fn handle_search_job_result(state: &mut AppState, result: JobResult) {
    match result {
        JobResult::BufferFindCompleted {
            doc_id,
            version,
            matches,
            regex_error,
        } => {
            if state
                .active_document()
                .is_some_and(|d| d.id == doc_id && d.version == version)
            {
                state.find.matches = matches;
                state.find.regex_error = regex_error;
                state.find.searching = false;
                state.find.active_match = state
                    .find
                    .active_match
                    .min(state.find.matches.len().saturating_sub(1));
            }
        }
        JobResult::FilesSearchBatch { search_id, results } => {
            state.project_search.push_batch(search_id, results);
        }
        JobResult::FilesSearchComplete {
            search_id,
            elapsed_ms,
        } => {
            if state.project_search.search_id == search_id {
                state.project_search.searching = false;
                state.project_search.elapsed_ms = elapsed_ms;
            }
        }
        JobResult::FilesSearchError { search_id, error } => {
            if state.project_search.search_id == search_id {
                state.project_search.searching = false;
                state.project_search.regex_error = Some(error);
            }
        }
        JobResult::FilesReplaceDone {
            result,
            query,
            replacement,
        } => apply_files_replace_done(state, &result, query, &replacement),
        _ => {}
    }
}

fn apply_files_replace_done(
    state: &mut AppState,
    result: &ReplaceResult,
    query: FilesSearchQuery,
    replacement: &str,
) {
    let mut open_replaced = 0usize;
    let buf_query = SearchQuery {
        pattern: query.pattern,
        case_sensitive: query.case_sensitive,
        whole_word: query.whole_word,
        is_regex: query.is_regex,
        in_selection: None,
    };
    for skipped in &result.skipped_open_files {
        if let Some(doc) = state
            .documents
            .iter_mut()
            .find(|d| d.path.as_deref().is_some_and(|p| paths_match(p, skipped)))
        {
            if let Ok(n) = replace_all_in_doc(doc, &buf_query, replacement, state.now_ms) {
                if n > 0 {
                    open_replaced += 1;
                }
            }
        }
    }
    let total_files = result.modified_files.len() + open_replaced;
    if result.errors.is_empty() {
        state.toast = Some(format!("Replaced occurrences across {total_files} files."));
    } else {
        state.toast = Some(format!(
            "Replaced in {total_files} files ({} errors).",
            result.errors.len()
        ));
    }
}

fn load_doc_at(
    id: DocId,
    path: &std::path::Path,
    line: usize,
    col: usize,
) -> Result<Document, jide_core::CoreError> {
    let mut doc = Document::open(id, path)?;
    let first_line = (doc.rope.len_lines() > 0).then(|| doc.rope.line(0).to_string());
    doc.language = jide_core::LanguageId::new(
        jide_syntax::detect_language(Some(path), first_line.as_deref()).id,
    );
    doc.selection =
        Selection::point(doc.char_idx_of_line_col(line.saturating_sub(1), col.saturating_sub(1)));
    Ok(doc)
}

pub fn parse_location(argument: &str) -> (PathBuf, usize, usize) {
    let Some((prefix, last)) = argument.rsplit_once(':') else {
        return (argument.into(), 1, 1);
    };
    let Ok(number) = last.parse::<usize>() else {
        return (argument.into(), 1, 1);
    };
    if let Some((path, line)) = prefix.rsplit_once(':') {
        if let Ok(line) = line.parse::<usize>() {
            return (path.into(), line.max(1), number.max(1));
        }
    }
    (prefix.into(), number.max(1), 1)
}
