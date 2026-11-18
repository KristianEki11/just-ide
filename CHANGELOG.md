# Changelog

All notable changes to Just IDE (`jide`) are documented in this file.

## [0.1.0] - 2026-11-18

### Added
- **Core Buffer & File I/O (`jide-core`):** Rope buffer (`ropey`), grapheme-aware cursor navigation, multi-cursor transaction batching, BOM/encoding detection (`UTF-8`, `UTF-16 LE/BE`, `Windows-1252`, `Shift_JIS`, `GBK`), majority EOL handling (`LF`/`CRLF`/`CR`), and atomic temp-write + fsync + rename saves.
- **Built-in Prettier & Rich Syntax Coloring:** Offline pure-Rust code formatter (`Shift+Alt+F`, selection format, format-on-save, `Prettier ✓` status menu) for JSON/JSONC, TOML, HTML/XML/SVG, CSS/SCSS/LESS, JS/TS/Rust/C-style languages; 4-depth rainbow bracket pair colorization; expanded scope styling.
- **Syntax Highlighting (`jide-syntax`):** Lazy `two-face`/`syntect` syntax loading for all 33 P0 languages with 64-line incremental checkpoints and large-file safety override.
- **Workspace & IPC (`jide-workspace`, `jide-platform`):** Lazy `.gitignore`-aware file tree, tabs with pin/preview/dirty-close confirmation, Quick Open (`Ctrl+P`), Command Palette (`Ctrl+Shift+P`), MRU switcher (`Ctrl+Tab`), file watcher clean reload, single-instance IPC + `--wait`, and portable mode (`portable.flag`).
- **Search & Replace (`jide-search`):** Streaming Find & Replace in Files with `.gitignore` support and preview confirmation modal; in-buffer literal/regex search (`$1` capture groups) and scrollbar tick markers.
- **Power Editing, Settings & Themes:** Multi-cursor (`Ctrl+D`, `Ctrl+Shift+L`, `Ctrl+Alt+Up/Down`, `Alt+Click`, `Shift+Alt+Drag`), auto-close pairs, comment toggling, case/sort/trim transforms, indent guides, visible whitespace, 2-group split editor (`Ctrl+\`), Zen mode (`Ctrl+K Z`), interactive Settings screen (`Ctrl+,`), layered/live-reloaded `settings.toml` & `keybindings.toml`, OS theme auto-detect, and custom `.toml` themes (`Ctrl+K Ctrl+T`).
- **Polish & Release Hardening:** Windows PE icon/version/PerMonitorV2+UTF-8 manifest (`winres`), `panic = "abort"` with panic hook flushing `HotExitStore` backups and `crash.log`, `WGPU_BACKEND=gl` default on Windows (`~37 MB` idle RAM, `~74 ms` startup), 60-second background tab galley eviction, `xtask notices`, portable `.zip` + NSIS `.exe` installer, and `SHA256SUMS.txt`.

### Verified PRD §8 & Performance Budgets (Windows 11 x64 MSVC, Release)

| Metric | Budget | Measured |
|---|---|---|
| Cold start → first frame (P1, median of 5) | `< 250 ms` | **74.31 ms** |
| Load 100 MB buffer (P3) | `< 2 s` | **219.61 ms** (`from_bytes`) / **285.28 ms** (`open`) |
| 100 MB mid-file insert+undo | interactive | **1.64 µs** |
| 1,000 multi-cursor insert+undo (M5) | `< 5 ms` | **378.69 µs** |
| Full-screen warm frame layout (P5) | `< 4 ms` | **36.68 µs** |
| Idle RAM, 1 small file (P6 Private Working Set) | `< 60 MB` | **37.29 MB** (`51.15 MB` private commit) |
| RAM, 20 typical source tabs (P7 Private Working Set) | `< 150 MB` | **37.93 MB** (`52.15 MB` private commit) |
| Find in Files 20k files / 500 MB (P8) | `< 1.5 s` | **261.78 ms** (`replace_all_10mb_100k`: **190.95 ms**) |
| Quick Open 50k files (P9) | `< 16 ms` filter | **4.10 ms** filter (`34.22 ms` index build) |
| Idle repaints over 30s (P10) | `≈ 0%` (0 spurious) | **6 total frames** (init + exit only) |
| Release binary size (P11, stripped) | `< 20 MB` | **16.48 MB** (`17,276,416` bytes) |
| Self-check (33/33 P0 languages) | exit `0` | **`"ok":true`**, `33` passed, `0` failed (`1142 ms`) |
| Supply chain (`cargo deny` + `cargo audit`) | 0 advisories | Clean across **523** crate dependencies |
