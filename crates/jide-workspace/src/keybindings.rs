use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyChord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeySequence(pub Vec<KeyChord>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WhenContext {
    Any,
    EditorFocus,
    TreeFocus,
    PaletteOpen,
    FindOpen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key: String,
    pub command: String,
    #[serde(default = "default_when")]
    pub when: WhenContext,
}

fn default_when() -> WhenContext {
    WhenContext::Any
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct KeybindingsConfig {
    #[serde(default)]
    pub bind: Vec<KeyBinding>,
}

pub struct KeybindingMap {
    bindings: Vec<KeyBinding>,
}

impl KeybindingMap {
    #[must_use]
    pub fn new() -> Self {
        Self {
            bindings: Self::default_bindings(),
        }
    }

    #[must_use]
    pub fn load(path: &Path) -> Self {
        let mut map = Self::new();

        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(config) = toml::from_str::<KeybindingsConfig>(&content) {
                    for custom in config.bind {
                        // Override existing or add new
                        if let Some(pos) = map
                            .bindings
                            .iter()
                            .position(|b| b.key == custom.key && b.when == custom.when)
                        {
                            map.bindings[pos] = custom;
                        } else {
                            map.bindings.push(custom);
                        }
                    }
                }
            }
        }

        map
    }

    /// Returns a human-readable warning string if `path` exists and has invalid keybindings TOML.
    #[must_use]
    pub fn validate_toml_file(path: &Path) -> Option<String> {
        if !path.exists() {
            return None;
        }
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(err) => return Some(format!("Failed to read keybindings.toml: {err}")),
        };
        toml::from_str::<KeybindingsConfig>(&content)
            .err()
            .map(|err| format!("Invalid keybindings.toml: {err}"))
    }

    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn default_bindings() -> Vec<KeyBinding> {
        vec![
            // File operations
            KeyBinding {
                key: "Ctrl+N".into(),
                command: "workbench.action.files.newUntitledFile".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+O".into(),
                command: "workbench.action.files.openFile".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+K Ctrl+O".into(),
                command: "workbench.action.files.openFolder".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+S".into(),
                command: "workbench.action.files.save".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+Shift+S".into(),
                command: "workbench.action.files.saveAs".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+K S".into(),
                command: "workbench.action.files.saveAll".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+W".into(),
                command: "workbench.action.closeActiveEditor".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+K Ctrl+W".into(),
                command: "workbench.action.closeAllEditors".into(),
                when: WhenContext::Any,
            },
            // Edit operations
            KeyBinding {
                key: "Ctrl+Z".into(),
                command: "undo".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+Y".into(),
                command: "redo".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+Shift+Z".into(),
                command: "redo".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+X".into(),
                command: "editor.action.clipboardCutAction".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+C".into(),
                command: "editor.action.clipboardCopyAction".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+V".into(),
                command: "editor.action.clipboardPasteAction".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+F".into(),
                command: "actions.find".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+H".into(),
                command: "editor.action.startFindReplaceAction".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+Shift+F".into(),
                command: "workbench.action.findInFiles".into(),
                when: WhenContext::Any,
            },
            // Navigation
            KeyBinding {
                key: "Ctrl+P".into(),
                command: "workbench.action.quickOpen".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+Shift+P".into(),
                command: "workbench.action.showCommands".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+G".into(),
                command: "workbench.action.gotoLine".into(),
                when: WhenContext::EditorFocus,
            },
            // View operations
            KeyBinding {
                key: "Ctrl+B".into(),
                command: "workbench.action.toggleSidebarVisibility".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+J".into(),
                command: "workbench.action.togglePanel".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+K Z".into(),
                command: "workbench.action.toggleZenMode".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+K Ctrl+T".into(),
                command: "workbench.action.selectTheme".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "F1".into(),
                command: "workbench.action.showCommands".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+Shift+T".into(),
                command: "workbench.action.reopenClosedEditor".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+PageDown".into(),
                command: "workbench.action.nextEditor".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+PageUp".into(),
                command: "workbench.action.previousEditor".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+Tab".into(),
                command: "workbench.action.openNextRecentlyUsedEditor".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+Shift+E".into(),
                command: "workbench.view.explorer".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+\\".into(),
                command: "workbench.action.splitEditor".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+1".into(),
                command: "workbench.action.focusFirstEditorGroup".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+2".into(),
                command: "workbench.action.focusSecondEditorGroup".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+,".into(),
                command: "workbench.action.openSettings".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+=".into(),
                command: "workbench.action.zoomIn".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+-".into(),
                command: "workbench.action.zoomOut".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+0".into(),
                command: "workbench.action.zoomReset".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+Q".into(),
                command: "workbench.action.quit".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Shift+Alt+Down".into(),
                command: "editor.action.copyLinesDownAction".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Shift+Alt+Up".into(),
                command: "editor.action.copyLinesUpAction".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Alt+Up".into(),
                command: "editor.action.moveLineUp".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Alt+Down".into(),
                command: "editor.action.moveLineDown".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+Shift+K".into(),
                command: "editor.action.deleteLines".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+Enter".into(),
                command: "editor.action.insertLineAfter".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+Shift+Enter".into(),
                command: "editor.action.insertLineBefore".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+/".into(),
                command: "editor.action.commentLine".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Shift+Alt+A".into(),
                command: "editor.action.blockComment".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Tab".into(),
                command: "editor.action.indentLines".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Shift+Tab".into(),
                command: "editor.action.outdentLines".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+]".into(),
                command: "editor.action.indentLines".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+[".into(),
                command: "editor.action.outdentLines".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+D".into(),
                command: "editor.action.addSelectionToNextFindMatch".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+Shift+L".into(),
                command: "editor.action.selectHighlights".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+Alt+Up".into(),
                command: "editor.action.insertCursorAbove".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+Alt+Down".into(),
                command: "editor.action.insertCursorBelow".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Alt+Click".into(),
                command: "editor.action.addCursorAtClick".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Shift+Alt+Drag".into(),
                command: "editor.action.columnSelection".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+L".into(),
                command: "expandLineSelection".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+Shift+\\".into(),
                command: "editor.action.jumpToBracket".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Alt+Z".into(),
                command: "editor.action.toggleWordWrap".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+K Ctrl+U".into(),
                command: "editor.action.transformToUppercase".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+K Ctrl+L".into(),
                command: "editor.action.transformToLowercase".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+K Ctrl+X".into(),
                command: "editor.action.trimTrailingWhitespace".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "F9".into(),
                command: "editor.action.sortLinesAscending".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "F3".into(),
                command: "editor.action.nextMatchFindAction".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Shift+F3".into(),
                command: "editor.action.previousMatchFindAction".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+Shift+H".into(),
                command: "workbench.action.replaceInFiles".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Ctrl+Shift+O".into(),
                command: "workbench.action.gotoSymbol".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Alt+Left".into(),
                command: "workbench.action.navigateBack".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Alt+Right".into(),
                command: "workbench.action.navigateForward".into(),
                when: WhenContext::Any,
            },
            KeyBinding {
                key: "Shift+Alt+F".into(),
                command: "editor.action.formatDocument".into(),
                when: WhenContext::EditorFocus,
            },
            KeyBinding {
                key: "Ctrl+K Ctrl+F".into(),
                command: "editor.action.formatSelection".into(),
                when: WhenContext::EditorFocus,
            },
            // Context-specific
            KeyBinding {
                key: "Escape".into(),
                command: "closeFind".into(),
                when: WhenContext::FindOpen,
            },
            KeyBinding {
                key: "Escape".into(),
                command: "closePalette".into(),
                when: WhenContext::PaletteOpen,
            },
        ]
    }

    #[must_use]
    pub fn lookup(&self, key: &str, context: WhenContext) -> Option<&str> {
        self.bindings
            .iter()
            .find(|b| b.key == key && (b.when == context || b.when == WhenContext::Any))
            .map(|b| b.command.as_str())
    }
}

impl Default for KeybindingMap {
    fn default() -> Self {
        Self::new()
    }
}
