#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Just IDE (`jide`) application binary entry point.

mod cli;
mod crash;
mod self_check;
mod workers;
mod workspace_sync;

use clap::Parser;
use cli::Cli;
use jide_ui::fonts::{configure_fonts, configure_initial_fonts, spawn_lazy_font_loader};
use jide_ui::palette::PaletteState;
use jide_ui::shell::render_shell;
use jide_ui::state::{dispatch, AppState, Command};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing_subscriber::EnvFilter;
use workspace_sync::WorkspaceSync;

fn main() -> anyhow::Result<()> {
    let process_start = Instant::now();
    jide_platform::attach_parent_console();

    #[cfg(target_os = "windows")]
    if std::env::var_os("WGPU_BACKEND").is_none() {
        std::env::set_var("WGPU_BACKEND", "gl");
    }

    let cli = Cli::parse();

    if cli.self_check {
        let report = self_check::run(process_start);
        println!("{}", serde_json::to_string(&report)?);
        if !report.ok {
            std::process::exit(1);
        }
        return Ok(());
    }

    init_logging();

    let run_mode = if cli.bench_startup {
        RunMode::BenchStartup
    } else if let Some(secs) = cli.smoke_idle {
        RunMode::SmokeIdle {
            duration: Duration::from_secs(secs),
        }
    } else {
        RunMode::Interactive
    };

    // Single-instance check: if not new_window and interactive, try to forward args to existing instance.
    let pipe_name = jide_platform::ipc::default_pipe_name();
    if run_mode == RunMode::Interactive && !cli.new_window && !cli.paths.is_empty() {
        let payload = jide_platform::ipc::IpcPayload {
            args: cli.paths.clone(),
            cwd: std::env::current_dir().ok(),
            request_focus: true,
        };
        if jide_platform::ipc::try_send_to_existing(&pipe_name, &payload) {
            return Ok(());
        }
    }

    let ipc_server = if run_mode == RunMode::Interactive && !cli.new_window {
        jide_platform::ipc::IpcServer::start(&pipe_name)
    } else {
        None
    };

    run_native_app(cli, run_mode, ipc_server, process_start)
}

fn run_native_app(
    cli: Cli,
    run_mode: RunMode,
    ipc_server: Option<jide_platform::ipc::IpcServer>,
    process_start: Instant,
) -> anyhow::Result<()> {
    let startup_ms = Arc::new(AtomicU64::new(0));
    let frame_count = Arc::new(AtomicU64::new(0));
    let done_flag = Arc::new(AtomicBool::new(false));

    let startup_ms_ref = Arc::clone(&startup_ms);
    let frame_count_ref = Arc::clone(&frame_count);
    let done_flag_ref = Arc::clone(&done_flag);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Just IDE")
            .with_inner_size(jide_ui::default_window_size())
            .with_min_inner_size([640.0, 480.0]),
        renderer: eframe::Renderer::Wgpu,
        centered: true,
        persist_window: false,
        ..Default::default()
    };

    let result = eframe::run_native(
        "Just IDE",
        native_options,
        Box::new(move |cc| {
            configure_initial_fonts(&cc.egui_ctx);
            let mut state = AppState::default();
            let workers = workers::Workers::default();
            let sync = WorkspaceSync::default();
            if let Some(ref p) = sync.paths {
                crash::install_hook(p.logs_dir(), p.backups_dir());
            }
            if run_mode != RunMode::BenchStartup {
                sync.load_state(&mut state, cli.reset_state);
                if state.font_ligatures {
                    configure_fonts(&cc.egui_ctx, true);
                }
                if let Some(notice) = crash::startup_recovery_notice(sync.paths.as_ref()) {
                    state.toast = Some(notice);
                }
            }

            let palette = PaletteState::default();

            for argument in &cli.paths {
                let (path, line, col) = workers::parse_location(argument);
                if path.is_dir() {
                    let _ = dispatch(&mut state, Command::OpenFolderPath(path));
                } else {
                    let id = state.allocate_doc_id();
                    workers.open(&cc.egui_ctx, id, path, line, col, false);
                }
            }
            state.theme.apply_to_egui(&cc.egui_ctx);

            if run_mode == RunMode::Interactive {
                spawn_lazy_font_loader(&cc.egui_ctx, state.font_ligatures);
            }

            if let RunMode::SmokeIdle { duration } = run_mode {
                let ctx = cc.egui_ctx.clone();
                let done = Arc::clone(&done_flag_ref);
                let _ = std::thread::Builder::new()
                    .name("jide-smoke-idle-timer".to_owned())
                    .spawn(move || {
                        std::thread::sleep(duration);
                        done.store(true, Ordering::Release);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    });
            }

            Ok(Box::new(JustIdeApp {
                state,
                workers,
                editor: jide_ui::editor_view::EditorView::default(),
                palette,
                sync,
                ipc_server,
                process_start,
                run_mode,
                startup_ms: startup_ms_ref,
                frame_count: frame_count_ref,
                done_flag: done_flag_ref,
                first_frame_recorded: false,
                syntax_set_requested: false,
                last_cache_eviction: Instant::now(),
                last_crash_snapshot: Instant::now(),
                was_focused: true,
                last_auto_save: Instant::now(),
            }))
        }),
    );

    if let Err(err) = result {
        anyhow::bail!("eframe exited with error: {err}");
    }

    match run_mode {
        RunMode::BenchStartup => {
            #[allow(clippy::cast_precision_loss)]
            let ms = (startup_ms.load(Ordering::Acquire) as f64) / 1000.0;
            println!("{{\"startup_ms\":{ms:.2}}}");
        }
        RunMode::SmokeIdle { duration } => {
            let frames = frame_count.load(Ordering::Acquire);
            println!("{{\"frames\":{frames},\"seconds\":{}}}", duration.as_secs());
        }
        RunMode::Interactive => {}
    }

    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RunMode {
    Interactive,
    BenchStartup,
    SmokeIdle { duration: Duration },
}

struct JustIdeApp {
    state: AppState,
    workers: workers::Workers,
    editor: jide_ui::editor_view::EditorView,
    palette: PaletteState,
    sync: WorkspaceSync,
    ipc_server: Option<jide_platform::ipc::IpcServer>,
    process_start: Instant,
    run_mode: RunMode,
    startup_ms: Arc<AtomicU64>,
    frame_count: Arc<AtomicU64>,
    done_flag: Arc<AtomicBool>,
    first_frame_recorded: bool,
    syntax_set_requested: bool,
    last_cache_eviction: Instant,
    last_crash_snapshot: Instant,
    was_focused: bool,
    last_auto_save: Instant,
}

impl JustIdeApp {
    fn maintain_caches_and_snapshots(&mut self) {
        if self.last_crash_snapshot.elapsed() >= Duration::from_secs(1) {
            self.last_crash_snapshot = Instant::now();
            crash::update_snapshots(&self.state);
        }

        if self.last_cache_eviction.elapsed() >= Duration::from_secs(60) {
            self.last_cache_eviction = Instant::now();
            let mut active_ids = Vec::new();
            if let Some(doc) = self
                .state
                .active_doc
                .and_then(|i| self.state.documents.get(i))
            {
                active_ids.push(doc.id);
            }
            if self.state.split_open {
                if let Some(doc) = self
                    .state
                    .group2_doc
                    .and_then(|i| self.state.documents.get(i))
                {
                    active_ids.push(doc.id);
                }
            }
            self.editor.evict_inactive_docs(&active_ids, 4000);
        }
    }

    fn process_auto_save(&mut self, ctx: &egui::Context) {
        let focused = ctx.input(|i| i.focused);
        let focus_lost = self.was_focused && !focused;
        self.was_focused = focused;

        let due = match self.state.auto_save_mode.as_str() {
            "onFocusChange" => focus_lost,
            "afterDelay" => self.last_auto_save.elapsed() >= Duration::from_secs(2),
            _ => false,
        };
        if !due {
            return;
        }
        self.last_auto_save = Instant::now();
        let pending: Vec<(jide_core::DocId, std::path::PathBuf)> = self
            .state
            .documents
            .iter()
            .filter(|doc| doc.is_dirty())
            .filter_map(|doc| doc.path.clone().map(|path| (doc.id, path)))
            .collect();
        for (doc_id, path) in pending {
            self.handle_effect(ctx, jide_ui::state::Effect::RequestSaveDoc { doc_id, path });
        }
    }

    fn handle_effect(&mut self, ctx: &egui::Context, effect: jide_ui::state::Effect) {
        match &effect {
            jide_ui::state::Effect::CleanupHotExitBackup(doc_id) => {
                self.sync.cleanup_doc_backup(*doc_id);
            }
            jide_ui::state::Effect::SaveSettings => {
                self.sync.save_settings_from_state(&self.state);
            }
            jide_ui::state::Effect::ReloadThemeList => {
                self.sync.load_custom_themes(&mut self.state);
            }
            jide_ui::state::Effect::OpenSettingsToml => {
                if let Some(path) = self.sync.ensure_user_settings_file() {
                    let id = self.state.allocate_doc_id();
                    self.workers.open(ctx, id, path, 1, 1, false);
                }
            }
            jide_ui::state::Effect::FontsChanged => {
                configure_fonts(ctx, self.state.font_ligatures);
                spawn_lazy_font_loader(ctx, self.state.font_ligatures);
            }
            jide_ui::state::Effect::RequestQuit => {
                self.sync.flush_hot_exit(&self.state);
                self.sync.save_settings_from_state(&self.state);
                self.sync.save_state(&self.state);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            _ => {}
        }
        self.workers.effect(ctx, &mut self.state, effect);
    }
}

impl eframe::App for JustIdeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let count = self.frame_count.fetch_add(1, Ordering::AcqRel) + 1;
        tracing::debug!(frame = count, "ui frame painted");

        self.state.now_ms =
            u64::try_from(self.process_start.elapsed().as_millis()).unwrap_or(u64::MAX);

        // 1. Drain workers & workspace sync (defer watcher setup until Frame 2+).
        self.workers.drain(ui.ctx(), &mut self.state);
        if self.first_frame_recorded {
            self.sync.poll(&mut self.state, &mut self.workers, ui.ctx());
            self.maintain_caches_and_snapshots();
        }

        // 2. Poll IPC server for incoming requests.
        if let Some(ref srv) = self.ipc_server {
            while let Ok(payload) = srv.receiver().try_recv() {
                for arg in payload.args {
                    let (path, line, col) = workers::parse_location(&arg);
                    let path = if path.is_relative() {
                        payload.cwd.as_ref().map_or(path.clone(), |c| c.join(&path))
                    } else {
                        path
                    };
                    if path.is_dir() {
                        let _ = dispatch(&mut self.state, Command::OpenFolderPath(path));
                    } else {
                        let id = self.state.allocate_doc_id();
                        self.workers.open(ui.ctx(), id, path, line, col, false);
                    }
                }
                if payload.request_focus {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
        }

        // 3. Handle dropped files (drag-and-drop).
        let dropped_files = ui.ctx().input(|i| i.raw.dropped_files.clone());
        for dropped in dropped_files {
            let path = dropped.path().to_path_buf();
            if path.is_dir() {
                let _ = dispatch(&mut self.state, Command::OpenFolderPath(path));
            } else {
                let id = self.state.allocate_doc_id();
                self.workers.open(ui.ctx(), id, path, 1, 1, false);
            }
        }

        // 4. Handle close request.
        if ui.ctx().input(|input| input.viewport().close_requested()) {
            if self
                .state
                .documents
                .iter()
                .any(jide_core::Document::is_dirty)
            {
                // Auto-flush hot-exit so work is never lost, then permit exit.
                self.sync.flush_hot_exit(&self.state);
                self.sync.save_state(&self.state);
            } else {
                self.sync.save_state(&self.state);
            }
        }

        // 5. Render shell.
        let commands = render_shell(ui, &self.state, &mut self.editor, &mut self.palette);
        for cmd in commands {
            let effects = dispatch(&mut self.state, cmd);
            for effect in effects {
                self.handle_effect(ui.ctx(), effect);
            }
        }
        self.process_auto_save(ui.ctx());

        if !self.first_frame_recorded {
            self.first_frame_recorded = true;
            let micros =
                u64::try_from(self.process_start.elapsed().as_micros()).unwrap_or(u64::MAX);
            self.startup_ms.store(micros, Ordering::Release);
            if self.run_mode == RunMode::BenchStartup {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            } else if !self.syntax_set_requested {
                self.syntax_set_requested = true;
                self.workers.spawn_syntax_set_loader(ui.ctx());
            }
        }

        if matches!(self.run_mode, RunMode::SmokeIdle { .. })
            && self.done_flag.load(Ordering::Acquire)
        {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
