use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)]
pub struct EditorSettings {
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub tab_size: usize,
    pub insert_spaces: bool,
    pub word_wrap: bool,
    pub line_numbers: bool,
    pub render_whitespace: String,
    pub cursor_blink: bool,
    pub cursor_smooth_caret_animation: bool,
    pub smooth_scrolling: bool,
    pub minimap: bool,
    pub indent_guides: bool,
    pub bracket_pair_colorization: bool,
    pub format_on_save: bool,
    pub font_ligatures: bool,
    pub highlight_active_line: bool,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            font_family: "JetBrains Mono".to_string(),
            font_size: 14.0,
            line_height: 1.5,
            tab_size: 4,
            insert_spaces: true,
            word_wrap: false,
            line_numbers: true,
            render_whitespace: "selection".to_string(),
            cursor_blink: true,
            cursor_smooth_caret_animation: true,
            smooth_scrolling: true,
            minimap: false,
            indent_guides: true,
            bracket_pair_colorization: true,
            format_on_save: true,
            font_ligatures: false,
            highlight_active_line: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)]
pub struct UiSettings {
    pub theme: String,
    pub icon_theme: String,
    pub status_bar_visible: bool,
    pub activity_bar_visible: bool,
    pub side_bar_position: String,
    pub breadcrumbs_visible: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: "just-dark".to_string(),
            icon_theme: "default".to_string(),
            status_bar_visible: true,
            activity_bar_visible: true,
            side_bar_position: "left".to_string(),
            breadcrumbs_visible: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FilesSettings {
    pub auto_save: String,
    pub auto_save_delay: u64,
    pub eol: String,
    pub encoding: String,
    pub trim_trailing_whitespace: bool,
    pub insert_final_newline: bool,
    pub exclude: Vec<String>,
}

impl Default for FilesSettings {
    fn default() -> Self {
        Self {
            auto_save: "off".to_string(),
            auto_save_delay: 1000,
            eol: "auto".to_string(),
            encoding: "utf-8".to_string(),
            trim_trailing_whitespace: false,
            insert_final_newline: true,
            exclude: vec![
                "**/.git".to_string(),
                "**/.svn".to_string(),
                "**/.hg".to_string(),
                "**/CVS".to_string(),
                "**/.DS_Store".to_string(),
                "**/Thumbs.db".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchSettings {
    pub exclude: Vec<String>,
    pub use_ignore_files: bool,
    pub follow_symlinks: bool,
    pub smart_case: bool,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            exclude: vec![
                "**/node_modules".to_string(),
                "**/bower_components".to_string(),
                "**/*.code-search".to_string(),
            ],
            use_ignore_files: true,
            follow_symlinks: false,
            smart_case: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)]
pub struct ExplorerSettings {
    pub auto_reveal: bool,
    pub enable_drag_and_drop: bool,
    pub confirm_delete: bool,
    pub confirm_drag_and_drop: bool,
    pub sort_order: String,
    pub show_icons: bool,
    pub tree_guides: bool,
}

impl Default for ExplorerSettings {
    fn default() -> Self {
        Self {
            auto_reveal: true,
            enable_drag_and_drop: true,
            confirm_delete: true,
            confirm_drag_and_drop: true,
            sort_order: "default".to_string(),
            show_icons: true,
            tree_guides: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalSettings {
    pub default_profile: String,
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub cursor_blink: bool,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            default_profile: "default".to_string(),
            font_family: "Consolas, 'Courier New', monospace".to_string(),
            font_size: 14.0,
            line_height: 1.0,
            cursor_blink: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub editor: EditorSettings,
    pub ui: UiSettings,
    pub files: FilesSettings,
    pub search: SearchSettings,
    pub explorer: ExplorerSettings,
    pub terminal: TerminalSettings,
}

pub struct SettingsLoadResult {
    pub settings: Settings,
    pub warnings: Vec<String>,
}

impl Settings {
    #[must_use]
    pub fn load(user_path: &Path, workspace_path: Option<&Path>) -> SettingsLoadResult {
        let mut warnings = Vec::new();
        let mut merged = toml::Value::try_from(Settings::default())
            .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()));

        Self::apply_toml_layer("user", user_path, &mut merged, &mut warnings);
        if let Some(ws_path) = workspace_path {
            Self::apply_toml_layer("workspace", ws_path, &mut merged, &mut warnings);
        }

        let settings = merged.try_into::<Settings>().unwrap_or_else(|err| {
            warnings.push(format!("Invalid settings type, using defaults: {err}"));
            Settings::default()
        });

        SettingsLoadResult { settings, warnings }
    }

    /// Saves `Settings` atomically as TOML to `path`.
    pub fn save_toml(&self, path: &Path) -> std::io::Result<()> {
        let toml_text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp = path.with_extension("toml.tmp");
        std::fs::write(&temp, toml_text)?;
        std::fs::rename(&temp, path)?;
        Ok(())
    }

    /// Returns `true` if either settings layer explicitly specifies `[ui] theme`.
    #[must_use]
    pub fn has_explicit_theme(user_path: &Path, workspace_path: Option<&Path>) -> bool {
        let has_in = |p: &Path| -> bool {
            std::fs::read_to_string(p)
                .ok()
                .and_then(|s| toml::from_str::<toml::Value>(&s).ok())
                .and_then(|v| {
                    v.get("ui")
                        .and_then(toml::Value::as_table)
                        .and_then(|ui| ui.get("theme"))
                        .cloned()
                })
                .is_some()
        };
        has_in(user_path) || workspace_path.is_some_and(has_in)
    }

    fn apply_toml_layer(
        label: &str,
        path: &Path,
        base: &mut toml::Value,
        warnings: &mut Vec<String>,
    ) {
        if !path.exists() {
            return;
        }
        let Ok(content) = std::fs::read_to_string(path) else {
            warnings.push(format!(
                "Failed to read {label} settings: {}",
                path.display()
            ));
            return;
        };
        let layer = match toml::from_str::<toml::Value>(&content) {
            Ok(val) => val,
            Err(err) => {
                warnings.push(format!("Failed to parse {label} settings: {err}"));
                return;
            }
        };
        merge_known_toml("", base, layer, warnings);
    }
}

fn merge_known_toml(
    prefix: &str,
    base: &mut toml::Value,
    overlay: toml::Value,
    warnings: &mut Vec<String>,
) {
    match (base, overlay) {
        (toml::Value::Table(base_tbl), toml::Value::Table(overlay_tbl)) => {
            for (key, val) in overlay_tbl {
                let full_key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                if let Some(target) = base_tbl.get_mut(&key) {
                    merge_known_toml(&full_key, target, val, warnings);
                } else {
                    warnings.push(format!("Unknown setting key ignored: {full_key}"));
                }
            }
        }
        (target, replacement) => {
            if std::mem::discriminant(target) == std::mem::discriminant(&replacement) {
                *target = replacement;
            } else {
                warnings.push(format!(
                    "Type mismatch for setting '{prefix}', keeping default"
                ));
            }
        }
    }
}
