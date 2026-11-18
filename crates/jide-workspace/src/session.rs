use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabState {
    pub path: Option<PathBuf>,
    pub untitled_id: Option<usize>,
    pub pinned: bool,
    pub preview: bool,
    pub cursor: (usize, usize),
    pub scroll: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionState {
    pub open_folder: Option<PathBuf>,
    pub tabs: Vec<TabState>,
    pub active_group: usize,
    pub split_ratio: f32,
}

impl SessionState {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let session = serde_json::from_str(&content)?;
        Ok(session)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecentList {
    pub recent_folders: Vec<PathBuf>,
    pub recent_files: Vec<PathBuf>,
}

impl RecentList {
    const MAX_ITEMS: usize = 16;

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let recent = serde_json::from_str(&content)?;
        Ok(recent)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn add_folder(&mut self, folder: PathBuf) {
        self.recent_folders.retain(|f| f != &folder);
        self.recent_folders.insert(0, folder);
        self.recent_folders.truncate(Self::MAX_ITEMS);
    }

    pub fn add_file(&mut self, file: PathBuf) {
        self.recent_files.retain(|f| f != &file);
        self.recent_files.insert(0, file);
        self.recent_files.truncate(Self::MAX_ITEMS);
    }
}
