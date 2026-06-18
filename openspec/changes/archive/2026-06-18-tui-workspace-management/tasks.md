## 1. Service layer — lift orchestration into `AppService`

- [x] 1.1 Add `AppService::add_workspace(&self, path: PathBuf) -> Result<RegisteredWorkspace, String>` in `crates/openspec-app/src/service.rs`: `registry.register(path)`, then for each returned `WorkspaceFolder` call `watcher.add_workspace(folder)`, then `watcher.sync_repos()` and `watcher.aggregate_and_emit()`; return the primary user-registered entry with presentation overrides joined (reuse the join used by `list_workspaces`). Map registration errors to a user-facing `String`.
- [x] 1.2 Add `AppService::remove_workspace(&self, path: PathBuf) -> Result<bool, String>`: snapshot the entry's repo association, `registry.unregister(path)`, call `watcher.remove_workspace(p)` for each removed path, then `watcher.sync_repos()` and `watcher.aggregate_and_emit()`, then drop now-orphaned presentation keys (port the `presentation_keys_to_drop` cascade currently living in `crates/specforge/src/commands.rs`).
- [x] 1.3 Add `AppService::set_workspace_presentation(&self, path: PathBuf, repo_id: Option<RepoId>, name: Option<String>, color: Option<PaletteColor>) -> Result<(), String>`: derive `PresentationKey::repo(repo_id)` when present else `PresentationKey::flat(path)`, call `store.set`/`store.remove` (empty name → absent; invalid colour → error), then refresh the aggregate view.
- [x] 1.4 Unit-test the three methods in `openspec-app` without Tauri: `add_workspace` for a valid folder and each invalid class (missing path, not a directory, no `openspec/`); `remove_workspace` including the discovered-worktree cascade and presentation-key cleanup; `set_workspace_presentation` for flat vs repo key, empty-name reset, and invalid-colour rejection.

## 2. Tauri commands become thin callers

- [x] 2.1 Refactor `register_workspace`, `unregister_workspace`, and `set_workspace_presentation` in `crates/specforge/src/commands.rs` to delegate to the new `AppService` methods, removing the duplicated watcher/registry/presentation orchestration from the command bodies.
- [x] 2.2 Run the existing desktop/integration tests to confirm the refactor is behaviour-preserving (`cargo test`).

## 3. Reusable overlay primitive (`specforge-tui`)

- [x] 3.1 Add an `Overlay` type in `app.rs` modelling a modal that is either a text prompt (with an input buffer + optional inline error + a prompt title) or a yes/no confirm (with a message). Add `model.overlay: Option<Overlay>` and the `Msg`/action plumbing to open, edit, submit, and cancel it.
- [x] 3.2 Generalise the existing `/`-filter character handling (append char, backspace, `Esc`) so the text-prompt overlay reuses it; while an overlay is open, route key events to the overlay instead of the focused screen.

## 4. Settings screen — typed, scrollable row model

- [x] 4.1 Replace the fixed `SETTINGS_ROW_COUNT` cursor with a built list of typed rows — `Row::Toggle(..)`, `Row::AddWorkspace`, `Row::Workspace(..)` — rebuilt from `svc.list_workspaces()` (user-registered only) plus the two toggles. Keep the cursor clamped to the row count and scrolled into view (reuse the season-ladder `render_scroll` pattern).
- [x] 4.2 Carry each workspace row's `uri`, `repo_id`, current `display_name`, and `color` from `RegisteredWorkspace` so actions can address the presentation key and render current state without extra lookups.

## 5. Settings screen — actions wired to the service

- [x] 5.1 `Space`/`Enter` on a `Toggle` row keeps the existing toggle behaviour (gamification re-dispatch, quota gauge reset).
- [x] 5.2 `Enter` on `AddWorkspace` opens a text-prompt overlay; on submit, spawn the async task that calls `svc.add_workspace(path)`. On success close the overlay; on error keep it open and show the message inline.
- [x] 5.3 `x` on a `Workspace` row opens a confirm overlay that names the discovered worktrees the cascade will drop; on confirm call `svc.remove_workspace(uri)`.
- [x] 5.4 `r` on a `Workspace` row opens a text-prompt overlay prefilled with the current/default name; on submit call `svc.set_workspace_presentation(.., name, current_color)` (empty input clears to default).
- [x] 5.5 `c` on a `Workspace` row cycles the colour `none → indigo → blue → teal → green → amber → orange → rose → purple → none`, calling `svc.set_workspace_presentation(.., current_name, next_color)` and persisting immediately.
- [x] 5.6 After any successful mutation, refresh the Settings row list and `model.refresh(svc)`; confirm the Browse tree also updates via the existing `CacheEvent` subscription (no extra wiring expected).

## 6. Rendering (`ui.rs`)

- [x] 6.1 Extend the `settings(f, area, model)` render fn to draw the Workspaces section header, the `+ Add workspace` action row, and one row per workspace showing name, path, a missing/stale indicator, and a colour swatch when set; highlight the focused row.
- [x] 6.2 Render the overlay (text prompt or confirm) centred over the Settings screen using the `Clear` + `centered_rect` pattern from the help overlay, including the inline error line for prompts.
- [x] 6.3 Make the Settings footer hint context-sensitive to the focused row type (toggle: `Space toggle`; workspace: `x remove · r rename · c colour`; add row: `Enter add`), and update the help overlay to document the workspace-management keys.

## 7. Tests & docs

- [x] 7.1 Add render tests in `render_tests.rs`: empty workspace list, populated list, focused row of each type, an open add/rename prompt (with and without an inline error), an open remove confirm, and colour states — across the narrow and wide widths already used.
- [x] 7.2 Add async tests driving overlay → service for add (valid + invalid path), remove (including cascade), rename (set + clear-to-default), and colour (cycle + none), asserting both the model and the persisted registry/presentation stores.
- [x] 7.3 Update `crates/specforge-tui/README.md`: document the Settings Workspaces section and its keys (`a`/Enter add, `x` remove, `r` rename, `c` colour), and that the TUI now writes the registry and presentation stores (still never workspace files).
- [x] 7.4 Update the repo `README.md` to mention that workspaces can be added and managed from the terminal Settings screen.

## 8. Verification

- [x] 8.1 `cargo test` passes across the workspace (new `openspec-app`, `specforge`, and `specforge-tui` tests included).
- [x] 8.2 `cargo fmt --check` and `cargo clippy` are clean for the touched crates.
- [x] 8.3 `openspec validate tui-workspace-management` passes, and the implementation matches the spec scenarios (add, invalid-path rejection, cascade-aware remove, rename, colour).
