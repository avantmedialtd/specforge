# Design — Terminal UI Workspace Management

## Context

`specforge-tui` is a thin presentation layer over the headless `openspec-app::AppService`. It can already read the workspace set (`AppService::list_workspaces`, `AppService::workspace_views`) and subscribes to the watcher's `CacheEvent` broadcast for live tree refresh. It cannot mutate the set.

The mutation primitives already exist in `openspec-core` and are reachable through public `AppService` fields:

- `AppService.registry: Arc<Mutex<WorkspaceRegistry>>` — `register(path) -> Vec<WorkspaceFolder>` (validates `openspec/` subdir, canonicalises, auto-discovers sibling worktrees) and `unregister(path) -> Vec<PathBuf>` (cascades discovered worktrees).
- `AppService.presentation: Arc<Mutex<WorkspacePresentationStore>>` — `set(key, name, color)` (normalises empty name to absent, rejects non-curated colours) and `remove(key)`, keyed by `PresentationKey::flat(path)` or `PresentationKey::repo(repo_id)`.

What is missing is the *orchestration* that ties a registry mutation to the watcher and the cached aggregate view. Today that orchestration lives in the Tauri command layer (`register_workspace`, `unregister_workspace`, `set_workspace_presentation` in `crates/specforge/src/commands.rs`), not in the shared service — so the terminal cannot reuse it without duplicating it.

## Goals

- Add / remove / rename / recolour workspaces from the terminal Settings screen.
- One tested orchestration path shared by both frontends; no watcher or registry logic duplicated into a frontend.
- Effects visible in the running terminal without a restart.

## Non-Goals

- A filesystem browser or fuzzy picker for the add flow (path is typed/pasted; a browser is a possible follow-up).
- Cross-process coordination of concurrent desktop + terminal registry writes (remains last-writer-wins, as today for `activity.json`).
- Managing auto-discovered worktrees directly — only user-registered workspaces are manageable.
- Any change to the desktop Settings view UX.

## Decisions

### 1. Lift the orchestration into `AppService` (the load-bearing change)

Introduce on `AppService`:

- `add_workspace(path) -> Result<RegisteredWorkspace, String>` — `registry.register`, then for each returned folder `watcher.add_workspace`, then `watcher.sync_repos()`, then `watcher.aggregate_and_emit()`; return the primary user-registered entry (presentation overrides joined).
- `remove_workspace(path) -> Result<bool, String>` — snapshot repo association, `registry.unregister`, `watcher.remove_workspace` for each removed path, `watcher.sync_repos()`, `watcher.aggregate_and_emit()`, then drop the now-orphaned presentation keys (mirroring the existing `presentation_keys_to_drop` cascade).
- `set_workspace_presentation(path, repo_id, name, color) -> Result<(), String>` — derive the `PresentationKey` (repo-keyed when `repo_id` is present, else flat), call `store.set`/`store.remove`, refresh the aggregate.

The three Tauri commands collapse to thin wrappers over these methods. The terminal calls the same methods in-process. This satisfies the terminal-ui *In-Process Shared Application Service* requirement ("the service owns watcher lifecycle") and the project rule that watcher/registry orchestration must not live in a frontend crate, and it makes the path unit-testable in `openspec-app` without a Tauri app or a TTY.

Because `add_workspace`/`remove_workspace` end in `aggregate_and_emit()`, the terminal's existing `CacheEvent` subscription refreshes the Browse tree with no extra wiring — the frontend write path is "call the method; let the subscription redraw."

### 2. Generalise the Settings screen from a fixed row count to a typed row list

The Settings screen today is a fixed two-row cursor (`SETTINGS_ROW_COUNT = 2`). Replace it with a built list of typed rows:

```
Row::Toggle(ToggleId)        // gamification, quota
Row::AddWorkspace            // the "+ Add workspace" action
Row::Workspace(WorkspaceRef) // one per user-registered workspace
```

Key handling dispatches on the focused row's type: `Space`/`Enter` toggles a `Toggle`; `Enter` on `AddWorkspace` opens the add prompt; on a `Workspace` row, `x` removes (confirm), `r` renames (prompt prefilled with current/default name), `c` cycles colour. The footer hint is context-sensitive to the focused row type. The list is scrollable, keeping the cursor in view — reuse the season-ladder scroll pattern (`render_scroll`).

The workspace rows are built from `list_workspaces()` (user-registered only), so every row shown is removable/renamable/recolourable; discovered worktrees never appear here, which dissolves the "removable ≠ displayed" hazard.

### 3. A small reusable overlay primitive

Add one modal type used three ways — a text prompt (add, rename) and a yes/no confirm (remove). It generalises the existing hand-rolled `/`-filter input (char append, backspace, Esc) into a reusable `Overlay` with an input buffer and a result channel. The add/rename prompts surface the registry/presentation validation errors inline and stay open on failure; the remove confirm names the discovered worktrees the cascade will drop.

### 4. Colour interaction: cycle-on-`c`

Pressing `c` on a workspace row advances its colour through `none → indigo → blue → teal → green → amber → orange → rose → purple → none`, persisting immediately and re-tinting the row live. This avoids a second overlay; the eight tokens already map to terminal colours via `theme.rs`. (A swatch-strip overlay is a possible later refinement.)

### 5. Presentation keying mirrors the desktop

Rename/colour write `PresentationKey::repo(repo_id)` when the workspace is inside a git repo, else `PresentationKey::flat(path)`. Two user-registered workspaces in the same repo therefore share one presentation entry — identical to the desktop's model. `RegisteredWorkspace` already carries `repo_id`, `display_name`, and `color`, so the row renders current state and derives the key without extra lookups.

## Risks / Trade-offs

- **Concurrent registry writes.** Desktop + terminal running together can both write `workspaces.json` (last-writer-wins, no cross-process notify). Pre-existing for other shared state; documented as a known limitation, not addressed here.
- **Settings screen complexity.** The screen grows from two toggles to a heterogeneous form. Mitigated by the typed-row model and context-sensitive key handling; render tests cover empty and populated states and each row type.
- **Destructive remove.** Mitigated by a mandatory confirm that names the cascade.

## Test Plan

- `openspec-app`: unit tests for `add_workspace` (valid + each invalid path class), `remove_workspace` (including the discovered-worktree cascade and presentation-key cleanup), and `set_workspace_presentation` (flat vs repo key, empty-name reset, invalid-colour rejection), all without Tauri.
- `specforge-tui`: render tests for the Settings screen — empty workspace list, populated list, focused row of each type, open add/rename prompt, open remove confirm, and colour states across widths. Async tests driving the overlay → service path for add/remove/rename/colour and asserting the model and persisted stores.
