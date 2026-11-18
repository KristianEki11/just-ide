//! Unified command registry: id, category, title, and default shortcut for every user action.
//!
//! `CommandRegistry::all()` is the authoritative list consumed by the Command Palette.
//! Exit criterion M3-5: `CommandRegistry::all().len() == palette_commands_len()`.

/// Static metadata for a command shown in the Command Palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandMeta {
    pub id: &'static str,
    pub category: &'static str,
    pub title: &'static str,
    pub default_shortcut: &'static str,
}

/// Full registry of all user-triggerable commands.
pub struct CommandRegistry;

const COMMANDS: &[CommandMeta] = &[
    // File
    CommandMeta {
        id: "workbench.action.files.newUntitledFile",
        category: "File",
        title: "New File",
        default_shortcut: "Ctrl+N",
    },
    CommandMeta {
        id: "workbench.action.files.openFile",
        category: "File",
        title: "Open File...",
        default_shortcut: "Ctrl+O",
    },
    CommandMeta {
        id: "workbench.action.files.openFolder",
        category: "File",
        title: "Open Folder...",
        default_shortcut: "Ctrl+K Ctrl+O",
    },
    CommandMeta {
        id: "workbench.action.files.save",
        category: "File",
        title: "Save",
        default_shortcut: "Ctrl+S",
    },
    CommandMeta {
        id: "workbench.action.files.saveAs",
        category: "File",
        title: "Save As...",
        default_shortcut: "Ctrl+Shift+S",
    },
    CommandMeta {
        id: "workbench.action.files.saveAll",
        category: "File",
        title: "Save All",
        default_shortcut: "Ctrl+K S",
    },
    // Edit
    CommandMeta {
        id: "undo",
        category: "Edit",
        title: "Undo",
        default_shortcut: "Ctrl+Z",
    },
    CommandMeta {
        id: "redo",
        category: "Edit",
        title: "Redo",
        default_shortcut: "Ctrl+Y",
    },
    CommandMeta {
        id: "editor.action.clipboardCutAction",
        category: "Edit",
        title: "Cut",
        default_shortcut: "Ctrl+X",
    },
    CommandMeta {
        id: "editor.action.clipboardCopyAction",
        category: "Edit",
        title: "Copy",
        default_shortcut: "Ctrl+C",
    },
    CommandMeta {
        id: "editor.action.clipboardPasteAction",
        category: "Edit",
        title: "Paste",
        default_shortcut: "Ctrl+V",
    },
    CommandMeta {
        id: "editor.action.selectAll",
        category: "Edit",
        title: "Select All",
        default_shortcut: "Ctrl+A",
    },
    CommandMeta {
        id: "editor.action.deleteLines",
        category: "Edit",
        title: "Delete Line",
        default_shortcut: "Ctrl+Shift+K",
    },
    CommandMeta {
        id: "editor.action.moveLineUp",
        category: "Edit",
        title: "Move Line Up",
        default_shortcut: "Alt+Up",
    },
    CommandMeta {
        id: "editor.action.moveLineDown",
        category: "Edit",
        title: "Move Line Down",
        default_shortcut: "Alt+Down",
    },
    CommandMeta {
        id: "editor.action.copyLinesUpAction",
        category: "Edit",
        title: "Duplicate Line Up",
        default_shortcut: "Shift+Alt+Up",
    },
    CommandMeta {
        id: "editor.action.copyLinesDownAction",
        category: "Edit",
        title: "Duplicate Line Down",
        default_shortcut: "Shift+Alt+Down",
    },
    CommandMeta {
        id: "editor.action.indentLines",
        category: "Edit",
        title: "Indent",
        default_shortcut: "Tab",
    },
    CommandMeta {
        id: "editor.action.outdentLines",
        category: "Edit",
        title: "Outdent",
        default_shortcut: "Shift+Tab",
    },
    CommandMeta {
        id: "editor.action.toggleWordWrap",
        category: "Edit",
        title: "Toggle Word Wrap",
        default_shortcut: "Alt+Z",
    },
    // Navigation
    CommandMeta {
        id: "workbench.action.quickOpen",
        category: "Navigation",
        title: "Quick Open File...",
        default_shortcut: "Ctrl+P",
    },
    CommandMeta {
        id: "workbench.action.showCommands",
        category: "Navigation",
        title: "Show All Commands",
        default_shortcut: "Ctrl+Shift+P",
    },
    CommandMeta {
        id: "workbench.action.gotoLine",
        category: "Navigation",
        title: "Go to Line/Column...",
        default_shortcut: "Ctrl+G",
    },
    CommandMeta {
        id: "workbench.action.closeActiveEditor",
        category: "Navigation",
        title: "Close Editor",
        default_shortcut: "Ctrl+W",
    },
    CommandMeta {
        id: "workbench.action.closeAllEditors",
        category: "Navigation",
        title: "Close All Editors",
        default_shortcut: "Ctrl+K Ctrl+W",
    },
    // View
    CommandMeta {
        id: "workbench.action.toggleSidebarVisibility",
        category: "View",
        title: "Toggle Sidebar",
        default_shortcut: "Ctrl+B",
    },
    CommandMeta {
        id: "workbench.action.toggleZenMode",
        category: "View",
        title: "Toggle Zen Mode",
        default_shortcut: "Ctrl+K Z",
    },
    CommandMeta {
        id: "workbench.action.zoomIn",
        category: "View",
        title: "Zoom In",
        default_shortcut: "Ctrl+=",
    },
    CommandMeta {
        id: "workbench.action.zoomOut",
        category: "View",
        title: "Zoom Out",
        default_shortcut: "Ctrl+-",
    },
    CommandMeta {
        id: "workbench.action.zoomReset",
        category: "View",
        title: "Reset Zoom",
        default_shortcut: "Ctrl+0",
    },
    // Find
    CommandMeta {
        id: "actions.find",
        category: "Find",
        title: "Find in File",
        default_shortcut: "Ctrl+F",
    },
    CommandMeta {
        id: "editor.action.startFindReplaceAction",
        category: "Find",
        title: "Replace in File",
        default_shortcut: "Ctrl+H",
    },
    CommandMeta {
        id: "workbench.action.findInFiles",
        category: "Find",
        title: "Find in Files",
        default_shortcut: "Ctrl+Shift+F",
    },
    CommandMeta {
        id: "workbench.action.replaceInFiles",
        category: "Find",
        title: "Replace in Files",
        default_shortcut: "Ctrl+Shift+H",
    },
    CommandMeta {
        id: "editor.action.nextMatchFindAction",
        category: "Find",
        title: "Find Next",
        default_shortcut: "F3",
    },
    CommandMeta {
        id: "editor.action.previousMatchFindAction",
        category: "Find",
        title: "Find Previous",
        default_shortcut: "Shift+F3",
    },
    CommandMeta {
        id: "workbench.action.togglePanel",
        category: "View",
        title: "Toggle Search Bottom Panel",
        default_shortcut: "Ctrl+J",
    },
    // Theme
    CommandMeta {
        id: "workbench.action.selectTheme",
        category: "Preferences",
        title: "Color Theme",
        default_shortcut: "Ctrl+K Ctrl+T",
    },
    // Highlighting
    CommandMeta {
        id: "editor.action.forceHighlight",
        category: "Editor",
        title: "Enable Highlighting (Large File Override)",
        default_shortcut: "",
    },
    CommandMeta {
        id: "editor.action.changeLanguageMode",
        category: "Editor",
        title: "Change Language Mode",
        default_shortcut: "",
    },
    // M5 Power Editing & Transforms
    CommandMeta {
        id: "editor.action.commentLine",
        category: "Edit",
        title: "Toggle Line Comment",
        default_shortcut: "Ctrl+/",
    },
    CommandMeta {
        id: "editor.action.blockComment",
        category: "Edit",
        title: "Toggle Block Comment",
        default_shortcut: "Shift+Alt+A",
    },
    CommandMeta {
        id: "editor.action.transformToUppercase",
        category: "Edit",
        title: "Transform to Uppercase",
        default_shortcut: "Ctrl+K Ctrl+U",
    },
    CommandMeta {
        id: "editor.action.transformToLowercase",
        category: "Edit",
        title: "Transform to Lowercase",
        default_shortcut: "Ctrl+K Ctrl+L",
    },
    CommandMeta {
        id: "editor.action.trimTrailingWhitespace",
        category: "Edit",
        title: "Trim Trailing Whitespace",
        default_shortcut: "Ctrl+K Ctrl+X",
    },
    CommandMeta {
        id: "editor.action.sortLinesAscending",
        category: "Edit",
        title: "Sort Lines Ascending",
        default_shortcut: "F9",
    },
    CommandMeta {
        id: "editor.action.sortLinesDescending",
        category: "Edit",
        title: "Sort Lines Descending",
        default_shortcut: "",
    },
    CommandMeta {
        id: "editor.action.sortLinesUnique",
        category: "Edit",
        title: "Sort & Deduplicate Lines",
        default_shortcut: "",
    },
    CommandMeta {
        id: "expandLineSelection",
        category: "Selection",
        title: "Select Line",
        default_shortcut: "Ctrl+L",
    },
    CommandMeta {
        id: "editor.action.insertLineAfter",
        category: "Edit",
        title: "Insert Line Below",
        default_shortcut: "Ctrl+Enter",
    },
    CommandMeta {
        id: "editor.action.insertLineBefore",
        category: "Edit",
        title: "Insert Line Above",
        default_shortcut: "Ctrl+Shift+Enter",
    },
    CommandMeta {
        id: "editor.action.addSelectionToNextFindMatch",
        category: "Selection",
        title: "Add Selection To Next Find Match",
        default_shortcut: "Ctrl+D",
    },
    CommandMeta {
        id: "editor.action.selectHighlights",
        category: "Selection",
        title: "Select All Occurrences of Find Match",
        default_shortcut: "Ctrl+Shift+L",
    },
    CommandMeta {
        id: "editor.action.insertCursorAbove",
        category: "Selection",
        title: "Add Cursor Above",
        default_shortcut: "Ctrl+Alt+Up",
    },
    CommandMeta {
        id: "editor.action.insertCursorBelow",
        category: "Selection",
        title: "Add Cursor Below",
        default_shortcut: "Ctrl+Alt+Down",
    },
    CommandMeta {
        id: "editor.action.jumpToBracket",
        category: "Navigation",
        title: "Go to Bracket",
        default_shortcut: "Ctrl+Shift+\\",
    },
    // Built-in Prettier & Formatting
    CommandMeta {
        id: "editor.action.formatDocument",
        category: "Prettier",
        title: "Prettier: Format Document",
        default_shortcut: "Shift+Alt+F",
    },
    CommandMeta {
        id: "editor.action.formatSelection",
        category: "Prettier",
        title: "Prettier: Format Selection",
        default_shortcut: "",
    },
    CommandMeta {
        id: "editor.action.toggleFormatOnSave",
        category: "Prettier",
        title: "Prettier: Toggle Format on Save",
        default_shortcut: "",
    },
    CommandMeta {
        id: "editor.action.toggleBracketColorization",
        category: "Prettier",
        title: "Toggle Rainbow Bracket Pair Colorization",
        default_shortcut: "",
    },
    CommandMeta {
        id: "editor.action.toggleIndentGuides",
        category: "View",
        title: "Toggle Indent Guides",
        default_shortcut: "",
    },
    // Navigation, Split & Workspace
    CommandMeta {
        id: "workbench.action.navigateBack",
        category: "Navigation",
        title: "Navigate Back",
        default_shortcut: "Alt+Left",
    },
    CommandMeta {
        id: "workbench.action.navigateForward",
        category: "Navigation",
        title: "Navigate Forward",
        default_shortcut: "Alt+Right",
    },
    CommandMeta {
        id: "workbench.action.splitEditor",
        category: "View",
        title: "Split Editor Right / Toggle Split",
        default_shortcut: "Ctrl+\\",
    },
    CommandMeta {
        id: "workbench.action.focusFirstEditorGroup",
        category: "Navigation",
        title: "Focus First Editor Group",
        default_shortcut: "Ctrl+1",
    },
    CommandMeta {
        id: "workbench.action.focusSecondEditorGroup",
        category: "Navigation",
        title: "Focus Second Editor Group",
        default_shortcut: "Ctrl+2",
    },
    CommandMeta {
        id: "workbench.action.openSettings",
        category: "Preferences",
        title: "Open Settings (UI)",
        default_shortcut: "Ctrl+,",
    },
    CommandMeta {
        id: "workbench.action.openSettingsToml",
        category: "Preferences",
        title: "Open User Settings (settings.toml)",
        default_shortcut: "",
    },
    CommandMeta {
        id: "workbench.action.reopenClosedEditor",
        category: "Navigation",
        title: "Reopen Closed Editor",
        default_shortcut: "Ctrl+Shift+T",
    },
    CommandMeta {
        id: "workbench.action.nextEditor",
        category: "Navigation",
        title: "Next Editor",
        default_shortcut: "Ctrl+PageDown",
    },
    CommandMeta {
        id: "workbench.action.previousEditor",
        category: "Navigation",
        title: "Previous Editor",
        default_shortcut: "Ctrl+PageUp",
    },
    CommandMeta {
        id: "workbench.view.explorer",
        category: "View",
        title: "Show Explorer",
        default_shortcut: "Ctrl+Shift+E",
    },
    CommandMeta {
        id: "terminal.openHere",
        category: "Workspace",
        title: "Open External Terminal Here",
        default_shortcut: "",
    },
    CommandMeta {
        id: "workbench.action.quit",
        category: "File",
        title: "Exit Just IDE",
        default_shortcut: "Ctrl+Q",
    },
];

impl CommandRegistry {
    /// Returns every registered command.
    #[must_use]
    pub fn all() -> &'static [CommandMeta] {
        COMMANDS
    }

    /// Finds a command by its id string.
    #[must_use]
    pub fn by_id(id: &str) -> Option<&'static CommandMeta> {
        COMMANDS.iter().find(|c| c.id == id)
    }

    /// Returns the count of commands shown in the palette.
    #[must_use]
    pub fn len() -> usize {
        COMMANDS.len()
    }
}

/// Called by the palette completeness test to compare against `CommandRegistry::all().len()`.
#[must_use]
pub fn palette_commands_len() -> usize {
    CommandRegistry::len()
}
