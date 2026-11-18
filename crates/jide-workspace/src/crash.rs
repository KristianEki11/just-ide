//! Crash artifact flushing and startup recovery detection built on [`HotExitStore`].

use crate::hot_exit::HotExitStore;
use std::path::{Path, PathBuf};

/// Plain snapshot of an open unsaved document cached for panic-time backup flushing.
#[derive(Debug, Clone)]
pub struct CrashDocSnapshot {
    pub backup_key: String,
    pub path: Option<PathBuf>,
    pub untitled_name: Option<String>,
    pub encoding_name: String,
    pub has_bom: bool,
    pub eol: String,
    pub cursor: (usize, usize),
    pub bytes: Vec<u8>,
}

/// Writes `crash.log`, drops `crash.pending`, and flushes dirty buffer snapshots into `backups_dir`.
pub fn write_crash_artifacts(
    logs_dir: &Path,
    backups_dir: &Path,
    report: &str,
    snapshots: &[CrashDocSnapshot],
) {
    let _ = std::fs::create_dir_all(logs_dir);
    let _ = std::fs::write(logs_dir.join("crash.log"), report);
    let _ = std::fs::write(logs_dir.join("crash.pending"), "1");

    let store = HotExitStore::new(backups_dir.to_path_buf());
    for snap in snapshots {
        let _ = store.backup_buffer(
            &snap.backup_key,
            snap.path.as_deref(),
            snap.untitled_name.as_deref(),
            &snap.encoding_name,
            snap.has_bom,
            &snap.eol,
            snap.cursor,
            "",
            &snap.bytes,
        );
    }
}

/// Checks for a `crash.pending` flag from a prior run, removes it, and returns a recovery notice.
#[must_use]
pub fn take_recovery_notice(logs_dir: &Path) -> Option<String> {
    let pending = logs_dir.join("crash.pending");
    if !pending.exists() {
        return None;
    }
    let _ = std::fs::remove_file(&pending);
    let crash_log = logs_dir.join("crash.log");
    Some(format!(
        "Recovered unsaved session after an unexpected exit. Log: {}",
        crash_log.display()
    ))
}
