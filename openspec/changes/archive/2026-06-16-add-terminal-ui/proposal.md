# Add a Terminal UI (TUI) Frontend

## Why

SpecForge today is desktop-only: browsing OpenSpec workspaces, the gamified dashboard, and the season/battle-pass loop all require a GUI and a WebView. That excludes the terminal-native workflows where this work actually happens — reviewing OpenSpec changes over SSH on a headless box, glancing at activity from a tmux pane beside the editor, and running without the weight of a desktop window. The headless `openspec-core` crate already owns all the state, parsing, watching, git, and the dashboard/season/graph math, so a second, terminal-native frontend is within reach: the gap is not computation but a presentation surface plus a small amount of orchestration currently trapped in the Tauri shell.

## What Changes

- **New `specforge-tui` binary** — a terminal frontend on `ratatui` + `crossterm` (immediate mode, organized as an Elm/TEA loop) offering a full interactive master-detail browser, the gamified dashboard, and the season ladder, with live updates driven by the same watcher broadcast the desktop app consumes.
- **Three run modes from one binary** — `specforge-tui` (full interactive TUI), `--status` (print a computed snapshot and exit; pipeable/CI-friendly), and `--line` (a single ambient status line for a shell prompt or tmux status bar — the terminal twin of the desktop tray badge).
- **New headless `openspec-app` crate (enabler, not user-facing)** — extract the orchestration currently locked inside the Tauri shell (the file-backed settings store, the ~270-line dashboard *assembly* in `get_dashboard`, first-launch backfill/seeding, and config-dir resolution) into a Tauri-free `AppService` that both frontends consume in-process. This makes the dashboard assembly unit-testable from `cargo test` for the first time and guarantees the two frontends compute identically. The Tauri `commands.rs` shrinks to thin delegates; desktop behavior is unchanged.
- **No change to `openspec-core`** — parsing, cache, watcher, git, and the dashboard/season/graph math are reused as-is.

## Capabilities

### New Capabilities

- `terminal-ui`: a terminal-native SpecForge frontend that browses OpenSpec workspaces, renders artifact markdown, and presents the gamified dashboard and season ladder in a TTY — with live updates, graceful color/width degradation, and snapshot/status-line run modes.

### Modified Capabilities

<!-- None. Existing capabilities' requirements are unchanged: the TUI mirrors the
     desktop's spec-browser / dashboard / seasons behavior on a new surface, and the
     openspec-app extraction is an internal refactor with no behavioral delta. -->

## Impact

- **New crate `crates/openspec-app/`** — depends on `openspec-core`; owns `AppService`, the settings store (moved from `crates/specforge/src/settings.rs`), the dashboard assembly (moved from `get_dashboard`), backfill/seeding (moved from `lib.rs`), and the single config-dir resolver.
- **New crate `crates/specforge-tui/`** — depends on `openspec-app`; adds `ratatui`, `crossterm` (with `event-stream`), `tui-tree-widget`, `pulldown-cmark`. Workspace members grow from 2 to 4.
- **`crates/specforge/` (Tauri shell)** — `commands.rs` becomes thin delegates to `AppService`; `settings.rs`, the `get_dashboard` body, and the `lib.rs` setup/backfill block move into `openspec-app`. Tray, notifications, dock badge, menu, and launch-on-login stay shell-only (OS integration). Desktop behavior unchanged; new `cargo test` coverage lands on the moved dashboard assembly.
- **No IPC / no serde on the TUI path** — the TUI holds `AppService` in-process and calls functions directly.
- **Read-only v1** — the TUI does not write to workspaces; checkbox toggling via the existing `self_write` tracker is a deliberate v2.
