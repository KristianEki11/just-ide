# Just IDE (`jide`)

> A fast, native, offline code editor written in Rust.  
> Lighter than VS Code, more capable than Notepad++. **No AI. No plugins. No telemetry. Zero network calls.**

**Version:** `0.1.0`  
**Owner:** King (kidev)  
**Primary platform:** Windows 10/11 x64 (MSVC) · Secondary: Linux, macOS

---

## Features

- **Native Rust Performance:** Built on `eframe`/`egui` (`wgpu`) and `ropey` (`O(log n)` edits across 100 MB+ files in large-file mode).
- **Zero-Runtime Dependencies & Strict Privacy:** Compiles to one native binary. No LLM calls, no plugin runtime, no cloud sync, no network imports.
- **Built-in Prettier & Syntax Coloring:** Pure-Rust offline formatter (`Shift+Alt+F`, selection format, format-on-save, clickable `Prettier ✓` status menu), 33 P0 languages highlighted via `syntect` + `two-face`, and 4-depth rainbow bracket pair colorization.
- **Workspace & Navigation:** Lazy `.gitignore`-aware Explorer tree, pinned/preview/dirty tabs, Quick Open (`Ctrl+P`), Command Palette (`Ctrl+Shift+P` / `F1`), Go to Line (`Ctrl+G`), MRU `Ctrl+Tab`, single-instance IPC (`--wait`), and drag-and-drop.
- **Project-Wide Search & Replace:** Streaming Find in Files (`Ctrl+Shift+F`), Replace in Files with preview modal, in-buffer regex/case/word search with `$1` capture groups, and scrollbar tick markers.
- **Power Editing & Settings:** Multi-cursor (`Ctrl+D`, `Ctrl+Shift+L`, `Ctrl+Alt+Up/Down`, `Alt+Click`, `Shift+Alt+Drag` column box), auto-close pairs, line/block comments, transforms, sort/trim, indent guides, 2-group vertical split (`Ctrl+\`), Zen mode (`Ctrl+K Z`), live-reloaded `settings.toml` / `keybindings.toml`, interactive Settings UI (`Ctrl+,`), and custom `.toml` themes (`Ctrl+K Ctrl+T`).
- **Crash-Proof Storage:** Atomic saves (temp -> fsync -> rename), continuous Hot Exit backups, and panic-hook recovery reports.

---

## Build & Package (Windows x64 MSVC)

```powershell
cargo build --release -p jide
.\target\release\jide.exe --self-check
.\target\release\jide.exe --bench-startup
.\scripts\package-windows.ps1
```

Outputs under `dist/`:
- `JustIDE-0.1.0-x64-portable.zip` (portable mode enabled via `portable.flag`)
- `JustIDE-0.1.0-x64-setup.exe` (offline per-user NSIS installer)
- `SHA256SUMS.txt`

---

## Documentation & Licensing

- User guide & keybindings: [`docs/USER-GUIDE.md`](./docs/USER-GUIDE.md)
- Release notes & benchmarks: [`CHANGELOG.md`](./CHANGELOG.md)
- Third-party attributions: [`THIRD_PARTY_NOTICES.md`](./THIRD_PARTY_NOTICES.md)
- License: MIT ([`LICENSE`](./LICENSE))
