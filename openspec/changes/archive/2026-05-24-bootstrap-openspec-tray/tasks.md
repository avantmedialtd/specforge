# Tasks

## 1. Project Scaffold

- [x] 1.1 Initialise Tauri 2 project with React + TypeScript + Vite frontend in the repo root
- [x] 1.2 Convert the Rust side to a Cargo workspace with `crates/openspec-core` and `crates/openspec-tray` member crates
- [x] 1.3 Add Rust runtime dependencies to the relevant crates: `tauri` v2, `tauri-plugin-notification`, `tauri-plugin-autostart`, `tauri-plugin-window-state`, `notify`, `notify-debouncer-full`, `serde`, `serde_json`, `tokio`
- [x] 1.4 Add frontend dependencies: `@tauri-apps/api`, `react-markdown`, `remark-gfm`, `rehype-highlight`
- [ ] 1.5 Verify `cargo tauri dev` boots, shows an empty window, and hot-reloads on frontend edits

## 2. `openspec-core`: Workspace Registry

- [x] 2.1 Define `WorkspaceFolder` struct and the registered-workspaces in-memory store
- [x] 2.2 Implement config-file persistence (load on startup, save on mutation) using Tauri's path resolver for the per-OS data directory
- [x] 2.3 Implement `register_workspace(path)` that validates the presence of an `openspec/` subdirectory before accepting
- [x] 2.4 Implement `unregister_workspace(path)` that disposes any watcher tied to the workspace
- [x] 2.5 Implement `list_workspaces()` returning the current registered list with each workspace's missing-folder state
- [x] 2.6 Unit tests for register / unregister / list and round-trip persistence

## 3. `openspec-core`: OpenSpec Parser

- [x] 3.1 Port `ChangeData`, `Section`, `Task`, `ArtifactStatus`, `WorkspaceFolderRef` types from `../artifex/vscode-extension/src/types.ts`
- [x] 3.2 Port `parse_tasks_md(path) -> Vec<Section>` from `taskParser.ts`, preserving task completion state and source line numbers
- [x] 3.3 Port `parse_proposal_title(path) -> Option<String>` from `titleExtractor.ts`
- [x] 3.4 Implement `parse_artifact_status(change_dir) -> ArtifactStatus` (presence of proposal.md, design.md, tasks.md, and any `specs/<capability>/spec.md` files)
- [x] 3.5 Implement `parse_change(change_dir, workspace) -> ChangeData` aggregating the above
- [x] 3.6 Implement `parse_all_changes(workspace) -> Vec<ChangeData>` listing every direct child of `openspec/changes/` whose immediate parent is not `archive/`
- [x] 3.7 Add fixture-based unit tests under `crates/openspec-core/tests/fixtures/` mirroring representative change shapes (no tasks.md, empty tasks.md, sectioned tasks with mixed completion, multiple capability specs)

## 4. `openspec-core`: Cache and Watcher

- [x] 4.1 Implement in-memory cache keyed by workspace path → `Vec<ChangeData>`
- [x] 4.2 Populate the cache for a workspace on registration via `parse_all_changes`
- [x] 4.3 Establish a debounced filesystem watcher per workspace using `notify` + `notify-debouncer-full` on the workspace's `openspec/changes/` directory
- [x] 4.4 On watcher events, refresh the affected workspace's cache entry and emit a Tauri event identifying the workspace
- [x] 4.5 Detect structural transitions (new change directory appears, existing change moves into `archive/`) and emit dedicated `change-added` / `change-archived` events with workspace and change identifier
- [x] 4.6 Track recently self-written file paths with a short TTL so future writes from the app do not trigger their own watcher events (infrastructure landed now, not exercised in v1)
- [x] 4.7 Handle deletion of a registered workspace folder: mark workspace as missing in the registry, dispose its watcher, do not crash
- [x] 4.8 Unit tests covering: initial populate, single edit triggers single coalesced update, new change directory triggers `change-added`, archive move triggers `change-archived`, missing-folder graceful handling

## 5. `openspec-core`: Tauri Command Surface

- [x] 5.1 Expose `register_workspace`, `unregister_workspace`, `list_workspaces` as Tauri commands
- [x] 5.2 Expose `get_changes(workspace)` reading from cache
- [x] 5.3 Expose `read_artifact(workspace, change_id, artifact_kind, capability?)` returning raw markdown text for the detail pane
- [x] 5.4 Expose `get_active_count()` returning total non-archived count summed across all registered workspaces
- [x] 5.5 Expose `set_launch_on_login(enabled)` and `set_notifications_enabled(enabled)` settings commands persisting to the config file
- [x] 5.6 Document event names and payload schemas (`cache-updated`, `change-added`, `change-archived`) in a single Rust module so the frontend can mirror the types

## 6. `openspec-tray`: Tray Icon and Badge

- [x] 6.1 Register a tray icon at startup via Tauri 2 `TrayIconBuilder`
- [x] 6.2 Implement a single `set_badge(count: Option<u32>)` abstraction so platform branches live in one place
- [x] 6.3 macOS implementation of `set_badge` using `with_title()` to place the count next to the icon
- [x] 6.4 Windows and Linux implementation of `set_badge` by swapping pre-rendered numbered icons (or compositing at runtime if simpler)
- [x] 6.5 On startup and on every `cache-updated` / `change-added` / `change-archived` event, recompute the active count and call `set_badge`
- [x] 6.6 Implement tray click handler that shows and focuses the main window (creating it if it has been hidden)

## 7. `openspec-tray`: Main Window and Lifecycle

- [x] 7.1 Configure the main window: resizable, default size ~900×650, normal chrome, Dock icon visible (do not set `LSUIElement: true`)
- [x] 7.2 Integrate `tauri-plugin-window-state` so window position, size, and maximised state persist across restarts
- [x] 7.3 Implement close-button-hides-window behaviour: closing the window keeps the app process alive so the tray icon and watcher continue; Cmd-Q (or platform equivalent) is the only exit
- [x] 7.4 Configure `tauri-plugin-autostart` and wire it to the `set_launch_on_login` command
- [x] 7.5 Configure `tauri-plugin-notification`, subscribe to `change-added` and `change-archived` events, and dispatch desktop notifications gated by the notifications-enabled setting

## 8. Frontend: Master-Detail Layout

- [x] 8.1 Implement the two-pane layout with a draggable divider; initial split ~30% / 70%
- [x] 8.2 Implement the workspace tree component: workspace → change → four artifact nodes (Proposal, Specs, Design, Tasks) in fixed order → sections → tasks
- [x] 8.3 Fetch initial tree data via the `get_changes` Tauri command for each registered workspace
- [x] 8.4 Subscribe to `cache-updated` events and refresh the affected workspace's subtree
- [x] 8.5 Preserve tree expand / collapse state across re-renders so cache updates do not collapse the user's view
- [x] 8.6 Empty states: "no workspaces registered" (with a CTA to settings); "workspace has no active changes"

## 9. Frontend: Detail Pane and Click Behaviour

- [x] 9.1 Implement the detail pane with `react-markdown` + `remark-gfm` + `rehype-highlight`, including a basic style sheet matching the app's chrome
- [x] 9.2 Implement leaf-artifact click handlers (Proposal, Design, Tasks, individual capability spec) that fetch markdown via `read_artifact` and render it
- [x] 9.3 Implement section / individual-task click handlers that ensure `tasks.md` is rendered and scroll the pane to a slugified heading anchor (sections) or to a line-based anchor (tasks)
- [x] 9.4 Implement workspace / change / specs-artifact click handlers as explicit no-ops (no detail pane change)
- [x] 9.5 Suppress interactive behaviour on markdown task checkboxes — render as visual indicators only, no event handlers wired to disk

## 10. Frontend: Settings View

- [x] 10.1 Reach settings via a gear icon top-right of the main window; settings replaces the detail pane while the master pane stays visible
- [x] 10.2 List registered workspaces with their folder paths, a "missing" indicator where applicable, and a per-row remove control
- [x] 10.3 Add-workspace flow: open a native folder picker, call `register_workspace`, surface the validation error message if the folder lacks `openspec/`
- [x] 10.4 Launch-on-login toggle wired to `set_launch_on_login`
- [x] 10.5 Notifications-enabled toggle wired to `set_notifications_enabled`
- [x] 10.6 Render the missing-folder indicator distinctly so it is obvious at a glance which registrations are broken

## 11. Packaging and Distribution

- [x] 11.1 Configure `cargo-packager` for macOS (.app and .dmg), Windows (MSI and .exe), and Linux (.deb, AppImage, .rpm) from a single config
- [ ] 11.2 Register the Apple Developer account under Avant Media Ltd and obtain a Developer ID Application certificate
- [ ] 11.3 Configure macOS signing in `tauri.conf.json` referencing the Avant Media certificate
- [ ] 11.4 Run the notarisation flow end-to-end and verify the resulting `.dmg` opens on a clean macOS install without Gatekeeper warnings
- [ ] 11.5 Smoke-test the notarised binary: launch, tray icon appears, main window opens, workspace registration persists, badge updates after archiving a change in a registered workspace

## 12. Verification

- [ ] 12.1 Manually verify: register two workspaces with differing active-change counts, badge equals the sum
- [ ] 12.2 Manually verify: archive a change on disk, badge decrements, desktop notification fires
- [ ] 12.3 Manually verify: edit `proposal.md` of an active change while its detail pane is open, pane re-renders without user action
- [ ] 12.4 Manually verify: resize and reposition the window, quit, relaunch, window restores to saved geometry
- [ ] 12.5 Walk through each scenario in `tray-indicator`, `spec-browser`, `workspace-registry` and confirm observed behaviour matches
- [x] 12.6 Run `openspec validate bootstrap-openspec-tray` and confirm zero validation errors
