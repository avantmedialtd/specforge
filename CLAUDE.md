# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Product vs format naming

This project has two distinct names that must not collapse:

- **SpecForge** — the product (this desktop app). Used for the app name, window title, tray menu, bundle ID `com.avantmedia.specforge`, the Tauri crate `crates/specforge/`, and the npm package.
- **OpenSpec** — the *format* the app reads. Used for the on-disk layout (`openspec/`, `openspec/changes/`, `openspec/changes/archive/`), the `openspec-core` crate, workspace-validation errors, and file-picker prompts that ask the user for an OpenSpec workspace.

When editing user-visible copy, errors, dialogs, or path segments, pick deliberately. The `product-identity` spec at `openspec/specs/product-identity/spec.md` is the source of truth.

## Commands

Package manager is **bun**. Tauri dev/build commands are invoked through bun scripts (the Tauri config calls `bun run dev` / `bun run build`).

| Action | Command |
|---|---|
| Run the desktop app in dev mode | `bun tauri dev` |
| Run dev with WebView devtools auto-opened | `bun run tauri:devtools` (or set `SPECFORGE_OPEN_DEVTOOLS=1`) |
| Type-check + build frontend bundle | `bun run build` |
| Build production app bundle | `bun tauri build` |
| Run Rust tests (workspace) | `cargo test` |
| Run a single Rust integration test | `cargo test -p openspec-core --test parser` (replace `parser` with `cache` / `registry` / `watcher` / `self_write`) |
| Run a single test by name | `cargo test -p openspec-core <name_substring>` |

Frontend-only `vite` dev (`bun run dev`) exists but Tauri commands won't work without the Rust shell — use `bun tauri dev` for anything beyond CSS/markup tweaks.

TypeScript is strict with `noUnusedLocals` and `noUnusedParameters`. `bun run build` runs `tsc --noEmit` first; type errors block the bundle.

## Architecture

Two-layer split: a headless Rust core (`openspec-core`) owns all state and filesystem logic; the Tauri shell (`specforge`) wraps it with a tray icon, commands, events, settings, and notifications. The React frontend is a pure consumer of those commands and events.

### Rust workspace

- **`crates/openspec-core/`** — no Tauri dependency. Cleanly testable from `cargo test` without the GUI.
  - `registry.rs` — `WorkspaceRegistry` persists registered workspace folders to a JSON file. Registration canonicalises the path and requires an `openspec/` subdirectory.
  - `parser.rs` — parses `tasks.md`, `proposal.md` titles, capability spec directories, and the four-artifact status (`proposal`/`design`/`tasks`/`specs[]`). Task-line format mirrors the artifex VSCode extension.
  - `cache.rs` — `WorkspaceCache`: a plain `HashMap<PathBuf, Vec<ChangeData>>`, passive (the watcher mutates it).
  - `watcher.rs` — `WatcherManager` runs one `notify` + `notify-debouncer-full` watcher per registered workspace, scoped to that workspace's `openspec/` subtree (recursive). On debounced batches it re-parses the workspace and emits `CacheEvent::{Updated, ChangeAdded, ChangeArchived}` over a tokio broadcast channel. The manager is cheaply cloneable; clones share state via `Arc`. The processing task holds a `Weak<Inner>` so the manager can be dropped cleanly.
  - `self_write.rs` — `SelfWriteTracker` records paths the app just wrote so the watcher can ignore the resulting events. v1 is read-only so the tracker is effectively idle; it exists so the pipeline is ready for future interactive editing (e.g. checkbox toggling).
  - `types.rs` — `serde(rename_all = "camelCase")` on every type that crosses the IPC boundary. The TypeScript types in `src/types.ts` mirror these by hand — keep them in sync.

- **`crates/specforge/`** — Tauri shell.
  - `lib.rs` — the `run()` entry point. Order matters: install the event forwarder *before* populating the cache (initial `add_workspace` calls don't emit `Updated`, but later edits do — so the forwarder needs to be live first). Cache is populated synchronously via `block_on` so the frontend's first `get_changes` doesn't race a watcher startup.
  - `commands.rs` — `#[tauri::command]` handlers. `read_artifact` canonicalises the resolved path and rejects anything outside the workspace's `openspec/changes/` subtree.
  - `events.rs` — bridges `CacheEvent` → named Tauri events (`cache-updated`, `change-added`, `change-archived`). Event names and payload shapes are constants here; the frontend imports the same names from `src/types.ts`.
  - `tray.rs` + `tray_icon.rs` — system tray. The badge updater subscribes to the same `CacheEvent` stream as the forwarder; on macOS the badge is set via `set_title`. The tray glyph is an SVG rasterized per-monitor at the active scale factor and re-rasterized when the window moves between displays with different scales. macOS template rendering requires **pure black + alpha** — `rasterize` debug-asserts every output pixel has R=G=B=0. Edit `crates/specforge/icons/tray-icon.svg` keeping that invariant.
  - `notifications.rs` — desktop notifications fire only on `ChangeAdded` / `ChangeArchived`. `Updated` (plain file edits) never notifies, by design. Gated by `settings.notifications_enabled`.
  - `settings.rs` — file-backed `AppSettings`. **Launch-on-login is not stored here** — it lives in the OS via `tauri-plugin-autostart` and is queried fresh each time.

### Frontend

- **`src/api.ts`** — every Tauri command is wrapped in `invokeLogged`, which logs args + results when `import.meta.env.DEV` is true. Add new commands here.
- **`src/hooks/useWorkspaces.ts`** — owns the workspace list and per-workspace change map. Listens for all three cache events and refetches the affected workspace.
- **`src/App.tsx`** — master-detail (`SplitPane`) wrapping `WorkspaceTree` + `DetailPane`, with a settings toggle that swaps the right pane.
- The tree-selection contract is the `TreeSelection` discriminated union in `src/types.ts` — adding a new selectable node type means extending that union and the `handleSelect` switch in `App.tsx`.

### Window-lifecycle quirks

- The main window's **close button hides** rather than destroys (the watcher and tray must keep running). Cmd-Q, the "Quit SpecForge" tray item, or `app.exit(0)` are the only exit paths.
- On macOS, clicking the Dock icon when no windows are visible re-shows the main window (handled in the `RunEvent::Reopen` branch of `run()`).

## OpenSpec workflow

This repo dogfoods OpenSpec: proposed/in-flight work lives in `openspec/changes/<change-id>/`, archived work moves to `openspec/changes/archive/`, and capability specs live in `openspec/specs/<capability>/spec.md`. Each active change directory may contain `proposal.md`, `design.md`, `tasks.md`, and a `specs/<capability>/spec.md` subtree — the same four-artifact structure SpecForge browses for any registered workspace. When adding or modifying behaviour, check the relevant capability spec for the contract you're working against.

## Conventions

- Avoid `cd` in shell commands; use absolute paths or `cargo` / `bun` flags that target the right crate/package.
- Rust types crossing the IPC boundary use `#[serde(rename_all = "camelCase")]`. TypeScript mirrors live in `src/types.ts` — there's no codegen, so keep both sides matched.
- Don't introduce file watchers, registries, or parsers in the Tauri crate — that logic belongs in `openspec-core` so it stays testable from `cargo test`.
- For UI changes that need visual verification, start `bun tauri dev` yourself rather than asking the user to run it.
- After every `git push` to upstream, monitor the GitHub build (`gh run watch` or `gh run list --branch <branch>` then `gh run view <id> --log-failed` on failure) and report the outcome.
