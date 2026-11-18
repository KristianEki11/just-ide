use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifestEntry {
    pub doc_id: String,
    pub path: Option<PathBuf>,
    pub untitled_name: Option<String>,
    pub encoding_name: String,
    pub has_bom: bool,
    pub eol: String,
    pub cursor: (usize, usize),
    pub saved_hash: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BackupManifest {
    pub entries: Vec<BackupManifestEntry>,
}

pub struct HotExitStore {
    backups_dir: PathBuf,
}

impl HotExitStore {
    #[must_use]
    pub fn new(backups_dir: PathBuf) -> Self {
        Self { backups_dir }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn backup_buffer(
        &self,
        doc_id: &str,
        path: Option<&Path>,
        untitled_name: Option<&str>,
        encoding_name: &str,
        has_bom: bool,
        eol: &str,
        cursor: (usize, usize),
        saved_hash: &str,
        content: &[u8],
    ) -> Result<()> {
        std::fs::create_dir_all(&self.backups_dir)?;

        // Write content atomically
        let backup_file = self.backups_dir.join(format!("{doc_id}.bak"));
        let temp_file = self.backups_dir.join(format!("{doc_id}.tmp"));
        std::fs::write(&temp_file, content)?;
        std::fs::rename(&temp_file, &backup_file)?;

        // Update manifest
        let manifest_path = self.backups_dir.join("manifest.json");
        let mut manifest = self.load_manifest()?;

        // Remove existing entry if any
        manifest.entries.retain(|e| e.doc_id != doc_id);

        manifest.entries.push(BackupManifestEntry {
            doc_id: doc_id.to_string(),
            path: path.map(Path::to_path_buf),
            untitled_name: untitled_name.map(ToString::to_string),
            encoding_name: encoding_name.to_string(),
            has_bom,
            eol: eol.to_string(),
            cursor,
            saved_hash: saved_hash.to_string(),
        });

        let manifest_content = serde_json::to_string_pretty(&manifest)?;
        let temp_manifest = self.backups_dir.join("manifest.tmp");
        std::fs::write(&temp_manifest, manifest_content)?;
        std::fs::rename(&temp_manifest, &manifest_path)?;

        Ok(())
    }

    pub fn load_manifest(&self) -> Result<BackupManifest> {
        let manifest_path = self.backups_dir.join("manifest.json");
        if !manifest_path.exists() {
            return Ok(BackupManifest::default());
        }
        let content = std::fs::read_to_string(manifest_path)?;
        let manifest = serde_json::from_str(&content)?;
        Ok(manifest)
    }

    pub fn read_backup(&self, doc_id: &str) -> Result<Vec<u8>> {
        let backup_file = self.backups_dir.join(format!("{doc_id}.bak"));
        let content = std::fs::read(backup_file)?;
        Ok(content)
    }

    pub fn cleanup_backup(&self, doc_id: &str) -> Result<()> {
        let backup_file = self.backups_dir.join(format!("{doc_id}.bak"));
        if backup_file.exists() {
            std::fs::remove_file(backup_file)?;
        }

        // Update manifest
        let manifest_path = self.backups_dir.join("manifest.json");
        if manifest_path.exists() {
            let mut manifest = self.load_manifest()?;
            manifest.entries.retain(|e| e.doc_id != doc_id);
            let content = serde_json::to_string_pretty(&manifest)?;
            std::fs::write(manifest_path, content)?;
        }

        Ok(())
    }
}
