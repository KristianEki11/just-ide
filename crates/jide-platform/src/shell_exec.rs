//! Shell execution helpers (reveal in file manager, open terminal).

use std::path::Path;
use std::process::Command;

/// Reveals the given path in the system file manager.
///
/// On Windows: `explorer /select,"<path>"`
/// On macOS: `open -R <path>`
/// On Linux: `xdg-open <parent_dir>`
pub fn reveal_in_file_manager(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        Command::new("explorer")
            .raw_arg(windows_explorer_select_arg(path))
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg("-R").arg(path).spawn()?;
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let parent = path.parent().unwrap_or(path);
        Command::new("xdg-open").arg(parent).spawn()?;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_explorer_select_arg(path: &Path) -> String {
    format!("/select,\"{}\"", path.display())
}

/// Opens a terminal at the given directory.
///
/// On Windows: tries `wt.exe -d <dir>`, falls back to `powershell.exe -NoExit -Command Set-Location -LiteralPath <dir>`
/// On macOS: `open -a Terminal <dir>`
/// On Linux: `x-terminal-emulator`
pub fn open_terminal_at(dir: &Path) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "path is not a directory",
        ));
    }

    #[cfg(target_os = "windows")]
    {
        if Command::new("wt.exe").arg("-d").arg(dir).spawn().is_ok() {
            return Ok(());
        }

        let escaped = dir.display().to_string().replace('\'', "''");
        Command::new("powershell.exe")
            .arg("-NoExit")
            .arg("-Command")
            .arg(format!("Set-Location -LiteralPath '{escaped}'"))
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-a")
            .arg("Terminal")
            .arg(dir)
            .spawn()?;
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        Command::new("x-terminal-emulator")
            .current_dir(dir)
            .spawn()?;
    }

    Ok(())
}
