#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

//! OS-specific platform helpers for Just IDE.
//!
//! Only this crate may contain `unsafe` blocks, and each must carry a `// SAFETY:` comment.

pub mod ipc;
pub mod os_theme;
pub mod shell_exec;

/// Attaches the process to its parent console on Windows if stdout is not already attached.
///
/// This enables GUI-subsystem binaries (`#![windows_subsystem = "windows"]`) to output
/// `--version`, `--self-check`, and `--bench-startup` results when invoked from a terminal.
pub fn attach_parent_console() {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
        use windows_sys::Win32::System::Console::{
            AttachConsole, GetStdHandle, ATTACH_PARENT_PROCESS, STD_OUTPUT_HANDLE,
        };

        // SAFETY: `GetStdHandle` is a standard Win32 call that reads the process's stdout handle
        // with no pointer dereferences or memory lifetime assumptions.
        let stdout_handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
        if stdout_handle.is_null() || stdout_handle == INVALID_HANDLE_VALUE {
            // SAFETY: `AttachConsole(ATTACH_PARENT_PROCESS)` safely attaches the current process to
            // its parent console if one exists; failure (e.g. launched from Explorer) is ignored.
            let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
        }
    }
}
