# Just IDE — User Guide

A fast, offline code editor. No AI, no plugins, no network access.

## Install

**Installer (`JustIDE-<version>-x64-setup.exe`)** — per-user, no administrator prompt. Installs to `%LOCALAPPDATA%\Programs\JustIDE\`. Optional checkboxes:

- Add `jide` to your user `PATH`
- "Open with Just IDE" in the Explorer context menu (files, folders, folder background)
- Desktop shortcut
- Associate common file extensions (off by default)

The installer is unsigned, so Windows SmartScreen may warn on first run: choose *More info → Run anyway*. Verify downloads against `SHA256SUMS.txt`.

The uninstaller keeps `%APPDATA%\JustIDE` unless you tick **Remove user settings & session**.

**Portable (`JustIDE-<version>-x64-portable.zip`)** — unzip anywhere. The `portable.flag` file next to `jide.exe` switches on portable mode: settings, state and backups live in `.\data\` beside the exe, and nothing is written to the registry. Delete `portable.flag` to use the normal profile folders instead.

## Command line

```
jide [PATHS...] [OPTIONS]
```

| Option | Meaning |
|---|---|
| `path`, `path:line`, `path:line:col` | Open a file, optionally at a position, e.g. `jide src/main.rs:42:7` |
| `folder` | Open a folder in the Explorer |
| `--new-window` | Do not reuse a running instance |
| `--wait` | Block until the opened files are closed (use as a git editor) |
| `--self-check` | Headless smoke test; prints a JSON report, exits 0 on success and 1 on failure |
| `--bench-startup` | Print startup timings (ms) and exit |
| `--reset-state` | Clear session and backups (asks for confirmation) |
| `-V`, `--version` | Print the version |

By default a second `jide` launch hands its paths to the running window and exits. Use as your git editor:

```
git config --global core.editor "jide --wait"
```

## Where things are stored

| What | Location |
|---|---|
| Settings, keybindings, custom themes | `%APPDATA%\JustIDE\` |
| Session, recent files | `%LOCALAPPDATA%\JustIDE\state\` |
| Hot-exit backups | `%LOCALAPPDATA%\JustIDE\backups\` |
| Logs (7 days) | `%LOCALAPPDATA%\JustIDE\logs\` |

In portable mode everything is under `.\data\` next to `jide.exe`.

Unsaved work is backed up continuously and restored after a crash or restart. Saves are atomic.

## Keybindings

On macOS use Cmd in place of Ctrl.

### General

| Action | Key |
|---|---|
| Command palette | Ctrl+Shift+P / F1 |
| Quick open | Ctrl+P |
| New file / Open file | Ctrl+N / Ctrl+O |
| Open folder | Ctrl+K Ctrl+O |
| Save / Save as / Save all | Ctrl+S / Ctrl+Shift+S / Ctrl+K S |
| Close tab / Reopen closed tab | Ctrl+W / Ctrl+Shift+T |
| Next / previous tab | Ctrl+PageDown / Ctrl+PageUp |
| Most-recently-used tab switch | Ctrl+Tab |
| Toggle sidebar | Ctrl+B |
| Explorer / Search view | Ctrl+Shift+E / Ctrl+Shift+F |
| Toggle bottom panel | Ctrl+J |
| Split editor | Ctrl+\ |
| Focus group 1 / 2 | Ctrl+1 / Ctrl+2 |
| Settings | Ctrl+, |
| Zoom in / out / reset | Ctrl+= / Ctrl+- / Ctrl+0 |
| Zen mode | Ctrl+K Z |
| Toggle theme (dark / light) | Ctrl+K Ctrl+T |
| Quit | Ctrl+Q |

### Editing

| Action | Key |
|---|---|
| Undo / Redo | Ctrl+Z / Ctrl+Y, Ctrl+Shift+Z |
| Cut / Copy / Paste / Select all | Ctrl+X / Ctrl+C / Ctrl+V / Ctrl+A |
| Duplicate line | Shift+Alt+Down / Shift+Alt+Up |
| Move line | Alt+Up / Alt+Down |
| Delete line | Ctrl+Shift+K |
| Insert line below / above | Ctrl+Enter / Ctrl+Shift+Enter |
| Toggle line / block comment | Ctrl+/ / Shift+Alt+A |
| Indent / Outdent | Tab / Shift+Tab (or Ctrl+] / Ctrl+[) |
| Add next occurrence | Ctrl+D |
| Select all occurrences | Ctrl+Shift+L |
| Add cursor above / below | Ctrl+Alt+Up / Ctrl+Alt+Down |
| Add cursor at click | Alt+Click |
| Column selection | Shift+Alt+Drag |
| Expand line selection | Ctrl+L |
| Jump to matching bracket | Ctrl+Shift+\ |
| Toggle word wrap | Alt+Z |
| Upper / lower case | Ctrl+K Ctrl+U / Ctrl+K Ctrl+L |
| Trim trailing whitespace | Ctrl+K Ctrl+X |
| Sort lines (ascending) | F9 (descending and unique are in the palette) |

### Navigation and search

| Action | Key |
|---|---|
| Find / Replace | Ctrl+F / Ctrl+H |
| Next / previous match | F3 / Shift+F3 |
| Find / Replace in files | Ctrl+Shift+F / Ctrl+Shift+H |
| Go to line | Ctrl+G |
| Go back / forward | Alt+Left / Alt+Right |

## Settings

Press **Ctrl+,** for the Settings screen; changes apply instantly. For everything else, edit `settings.toml` (palette: *Preferences: Open settings.toml*). Edits are picked up on save. An invalid file shows a warning and keeps your previous settings.

Settings are layered, later wins: built-in defaults, then `%APPDATA%\JustIDE\settings.toml`, then an optional per-folder `.jide/settings.toml` in the opened folder.

```toml
[editor]
font_size = 14
tab_size = 4
insert_spaces = true
word_wrap = false
render_whitespace = "selection"   # none | selection | all
auto_close_brackets = true
cursor_blink = true

[ui]
theme = "just-dark"               # just-dark | just-light | high-contrast
zoom = 1.0
sidebar_visible = true

[files]
default_encoding = "utf-8"
default_eol = "auto"              # auto | lf | crlf
trim_trailing_whitespace = false
insert_final_newline = true
exclude = ["**/node_modules", "**/target", "**/.git"]

[search]
max_file_size_mb = 20
```

A project that wants two-space indentation only needs `.jide/settings.toml`:

```toml
[editor]
tab_size = 2
```

## Custom keybindings

Use the palette command *Preferences: Open keybindings.toml*. Each `[[bind]]` entry overrides or adds a binding:

```toml
[[bind]]
key = "ctrl+shift+d"
command = "editor.duplicate_line_down"
when = "editor_focus"   # optional: editor_focus | tree_focus | palette_open | find_open
```

## Custom themes

A theme is a plain TOML file (data only, no code runs). Drop it into `%APPDATA%\JustIDE\themes\` and set `theme` in `settings.toml` to its file name without the extension. Tokens you leave out fall back to the built-in theme of the same `kind`.

```toml
name = "My Theme"
kind = "dark"   # dark | light | high-contrast

[colors]
"bg.base" = "#16171B"
"bg.surface" = "#1C1D22"

[syntax]
keyword = { fg = "#C58AF9" }
comment = { fg = "#7A8290", italic = true }
heading = { fg = "#7AB8FF", bold = true }
```

The full token list is in the built-in themes (`assets/themes/just-dark.toml`, `just-light.toml`, `high-contrast.toml`). High Contrast is selected automatically on first launch if Windows High Contrast is on.

## Troubleshooting

- **Something looks wrong after an upgrade:** run `jide --self-check`; it prints a JSON report and exits non-zero on failure.
- **Start from a clean state:** `jide --reset-state` clears session and backups, never your settings or files.
- **Logs:** see the storage table above.
