use crate::error::{Result, WorkspaceError};
use directories::{BaseDirs, ProjectDirs};
use std::path::PathBuf;

/// Application paths for settings, state, backups, and logs.
/// Supports portable mode via `portable.flag` next to executable.
pub struct AppPaths {
    root: PathBuf,
    local_root: PathBuf,
    portable: bool,
}

impl AppPaths {
    /// Create paths using system directories or portable mode.
    pub fn new() -> Result<Self> {
        let exe_path = std::env::current_exe().map_err(WorkspaceError::Io)?;
        let exe_dir = exe_path
            .parent()
            .ok_or_else(|| WorkspaceError::InvalidPath(exe_path.clone()))?;

        let portable_flag = exe_dir.join("portable.flag");
        let portable = portable_flag.exists();

        if portable {
            let root = exe_dir.join("data");
            Ok(Self {
                local_root: root.clone(),
                root,
                portable: true,
            })
        } else {
            let proj = ProjectDirs::from("", "", crate::app_dir_name())
                .ok_or_else(|| WorkspaceError::InvalidPath(PathBuf::from("no home directory")))?;
            let local_root = BaseDirs::new().map_or_else(
                || proj.data_local_dir().to_path_buf(),
                |base| base.data_local_dir().join(crate::app_dir_name()),
            );
            Ok(Self {
                root: proj.config_dir().to_path_buf(),
                local_root,
                portable: false,
            })
        }
    }

    #[must_use]
    pub const fn is_portable(&self) -> bool {
        self.portable
    }

    #[must_use]
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.toml")
    }

    #[must_use]
    pub fn keybindings_file(&self) -> PathBuf {
        self.root.join("keybindings.toml")
    }

    #[must_use]
    pub fn themes_dir(&self) -> PathBuf {
        self.root.join("themes")
    }

    #[must_use]
    pub fn session_file(&self) -> PathBuf {
        self.root.join("session.json")
    }

    #[must_use]
    pub fn recent_file(&self) -> PathBuf {
        self.root.join("recent.json")
    }

    #[must_use]
    pub fn backups_dir(&self) -> PathBuf {
        self.local_root.join("backups")
    }

    #[must_use]
    pub fn logs_dir(&self) -> PathBuf {
        self.local_root.join("logs")
    }

    /// Ensure all directories exist.
    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(self.themes_dir())?;
        std::fs::create_dir_all(self.backups_dir())?;
        std::fs::create_dir_all(self.logs_dir())?;
        Ok(())
    }
}

impl Default for AppPaths {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            let root = std::env::temp_dir().join(crate::app_dir_name());
            Self {
                local_root: root.clone(),
                root,
                portable: false,
            }
        })
    }
}
