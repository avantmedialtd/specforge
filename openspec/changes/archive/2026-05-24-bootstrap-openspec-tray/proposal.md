# Bootstrap OpenSpec Tray

## Why

Peeking at OpenSpec state across multiple workspaces today requires opening VSCode and navigating away from the version-control view to the OpenSpec panel. That context switch is heavy enough that the state goes unchecked between deliberate visits, defeating the goal of ambient awareness. A dedicated menu-bar app surfaces active-change count at a glance and lets a developer drill into any registered workspace's change tree without bouncing through an IDE.

## What Changes

- New cross-platform desktop application built on Tauri 2 (Rust core, React + TypeScript + Vite frontend), distributed as a signed macOS app first and Windows / Linux follow.
- Two-crate Rust workspace: `openspec-core` (workspace registration, filesystem watching, OpenSpec parsing, in-memory cache, IPC commands) and `openspec-tray` (Tauri shell, tray icon, window, notifications, autostart wiring).
- System tray icon present whenever the app is running, with a badge showing the total count of non-archived changes across all registered workspaces.
- Main application window with normal app behaviour — Dock icon when running, regular window chrome, resizable, position and size persisted across restarts. Not a slide-down popover.
- Master-detail view inside the main window: left pane is a tree (workspace → change → artifact → section/task), right pane renders the selected artifact's markdown. The tree mirrors the existing artifex VSCode extension's structure.
- Manual workspace registration via an in-window settings view; the list of registered folders is persisted to a config file managed by `openspec-core`.
- Desktop notifications fire only on new changes and state transitions — never per file edit.
- Launch-on-login wiring via `tauri-plugin-autostart`.
- Read-only viewer for v1: no spec editing, no interactive task checkboxes, no state transitions initiated from the UI.

## Capabilities

### New Capabilities

- `tray-indicator`: System tray icon, badge reflecting count of non-archived changes across all registered workspaces, click-to-focus behaviour bringing the main window forward, and OS desktop notifications for new changes and state transitions.
- `spec-browser`: Main window master-detail viewer — workspace → change → artifact → section/task tree, rendered-markdown detail pane for selected leaf artifacts, scroll-to-anchor for section and individual task nodes, click matrix that defers interaction on workspace / change / specs-artifact nodes, window state persistence.
- `workspace-registry`: Manual workspace registration (add and remove), persistence of the registered folder list to a config file, filesystem watching of each registered folder, in-memory cache of parsed OpenSpec state, and settings UI surfacing workspace management plus launch-on-login and notification toggles.

### Modified Capabilities

(none — `openspec/specs/` is empty, this is the first change in a greenfield project)

## Impact

- New repository contents (currently the repo holds only the `openspec/` scaffold and `LICENSE`):
  - `crates/openspec-core/` — Rust crate. Filesystem watching via `notify` + `notify-debouncer-full`. OpenSpec parsing ported from `../artifex/vscode-extension/src/taskParser.ts` (~500 lines of TypeScript). In-memory cache. Tauri commands and events expose state to the frontend.
  - `crates/openspec-tray/` — Tauri 2 application binary. Tray icon, window management, plugin wiring (notifications, autostart, window-state).
  - `src/` — React + TypeScript + Vite frontend. Master-detail UI, settings view, markdown rendering.
- New runtime dependencies:
  - Rust: `tauri` (v2), `tauri-plugin-notification`, `tauri-plugin-autostart`, `tauri-plugin-window-state`, `notify`, `notify-debouncer-full`, `serde`, `serde_json`, `tokio`.
  - Frontend: `react`, `typescript`, `vite`, `@tauri-apps/api`, `react-markdown`, `remark-gfm`, `rehype-highlight`.
- Distribution: `cargo-packager` configuration for cross-platform builds (.app / .dmg, MSI / .exe, .deb / AppImage / .rpm). macOS notarised under the Avant Media Ltd Apple Developer account.
- No existing code is modified; the `openspec/config.yaml` may grow project-context entries describing the Rust + Tauri + TypeScript stack so future changes inherit them.
