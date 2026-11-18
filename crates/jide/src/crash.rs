//! Application panic hook flushing dirty buffers and writing `crash.log` (`05-ROADMAP` M6).

use jide_ui::state::AppState;
use jide_workspace::crash::{take_recovery_notice, write_crash_artifacts, CrashDocSnapshot};
use jide_workspace::paths::AppPaths;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

struct CrashState {
    logs_dir: PathBuf,
    backups_dir: PathBuf,
    snapshots: Vec<CrashDocSnapshot>,
}

static CRASH_REGISTRY: OnceLock<Arc<Mutex<CrashState>>> = OnceLock::new();
static HOOK_SET: OnceLock<()> = OnceLock::new();

/// Installs the global panic hook once, targeting `logs_dir` and `backups_dir`.
pub fn install_hook(logs_dir: PathBuf, backups_dir: PathBuf) {
    let cell = CRASH_REGISTRY.get_or_init(|| {
        Arc::new(Mutex::new(CrashState {
            logs_dir: logs_dir.clone(),
            backups_dir: backups_dir.clone(),
            snapshots: Vec::new(),
        }))
    });

    if let Ok(mut guard) = cell.lock() {
        guard.logs_dir = logs_dir;
        guard.backups_dir = backups_dir;
    }

    HOOK_SET.get_or_init(|| {
        let prev = std::panic::take_hook();
        let shared = Arc::clone(cell);
        std::panic::set_hook(Box::new(move |info| {
            let msg = format!(
                "Just IDE crash report\nVersion: {}\nLocation: {:?}\nPayload: {}\n",
                env!("CARGO_PKG_VERSION"),
                info.location()
                    .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())),
                panic_payload(info),
            );
            let guard = shared
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            write_crash_artifacts(&guard.logs_dir, &guard.backups_dir, &msg, &guard.snapshots);
            drop(guard);
            prev(info);
        }));
    });
}

/// Refreshes the in-memory snapshot list used if an unexpected panic occurs.
pub fn update_snapshots(state: &AppState) {
    let Some(shared) = CRASH_REGISTRY.get() else {
        return;
    };
    let mut snaps = Vec::new();
    for doc in &state.documents {
        if !doc.is_dirty() {
            continue;
        }
        let Ok(bytes) = doc.encode_bytes() else {
            continue;
        };
        let (line, col) = doc.line_col_of(doc.selection.primary_range().head);
        snaps.push(CrashDocSnapshot {
            backup_key: format!("doc_{}", doc.id.0),
            path: doc.path.clone(),
            untitled_name: doc.path.is_none().then(|| format!("Untitled-{}", doc.id.0)),
            encoding_name: doc.encoding.name().to_owned(),
            has_bom: doc.has_bom,
            eol: doc.eol.as_str().to_owned(),
            cursor: (line, col),
            bytes,
        });
    }
    if let Ok(mut guard) = shared.lock() {
        guard.snapshots = snaps;
    }
}

/// Returns a recovery notice banner if the previous session panicked cleanly via our hook.
#[must_use]
pub fn startup_recovery_notice(paths: Option<&AppPaths>) -> Option<String> {
    let p = paths?;
    take_recovery_notice(&p.logs_dir())
}

fn panic_payload(info: &std::panic::PanicHookInfo<'_>) -> String {
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}
