## 1. Core: presentation store

- [x] 1.1 Add `PaletteColor` enum (`Indigo`/`Blue`/`Teal`/`Green`/`Amber`/`Orange`/`Rose`/`Purple`) with kebab-case serde in `crates/openspec-core/src/types.rs`
- [x] 1.2 Add `PresentationKey` enum (`Flat(PathBuf)` / `Repo(PathBuf)`) with `"flat:<path>"` / `"repo:<path>"` string serialisation in `crates/openspec-core/src/presentation.rs`
- [x] 1.3 Add `PresentationEntry { display_name: Option<String>, color: Option<PaletteColor> }` and `WorkspacePresentationStore` with `load` / `save` / `set` / `remove` / `get` in `crates/openspec-core/src/presentation.rs`, persisted to a JSON file at the supplied config path
- [x] 1.4 Normalise empty-string display names to `None` inside `set`; reject any colour value not in the curated palette
- [x] 1.5 Add `presentation` module to `crates/openspec-core/src/lib.rs` exports
- [x] 1.6 Unit tests for the store: round-trip persistence, empty-string normalisation, invalid-colour rejection, missing-file load, cascade-style `remove` for both key kinds

## 2. Core: surface presentation on listed types

- [x] 2.1 Add `display_name: Option<String>` and `color: Option<PaletteColor>` fields to `RegisteredWorkspace` in `crates/openspec-core/src/types.rs`, keeping the existing `name` field for the basename-derived default
- [x] 2.2 Add the same two fields to `RepoView` (and any aggregator output struct that represents a top-level row) in `crates/openspec-core/src/repo_view.rs`
- [x] 2.3 Add a helper on the store to look up a `PresentationKey` and return `(display_name, color)`, used by joining at the IPC layer

## 3. Shell: app state + commands

- [x] 3.1 Hold a `Mutex<WorkspacePresentationStore>` in app state in `crates/specforge/src/lib.rs`, loaded at startup from a path next to the existing `workspaces.json`
- [x] 3.2 Update `list_workspaces` in `crates/specforge/src/commands.rs` to join presentation entries onto each returned `RegisteredWorkspace`
- [x] 3.3 Update the aggregated repo-view command to join presentation entries onto each top-level row (both repo groups and flat workspaces in the view)
- [x] 3.4 Add `#[tauri::command] fn set_workspace_presentation(key: PresentationKey, display_name: Option<String>, color: Option<PaletteColor>)` that writes through the store and saves
- [x] 3.5 Emit a `workspace-presentation-updated` Tauri event after a successful set; declare the event name in `crates/specforge/src/events.rs`
- [x] 3.6 Extend `unregister_workspace` so that after the registry cascade returns the list of removed paths, it computes corresponding `PresentationKey`s and removes them from the presentation store — including the `Repo` key only when no user-registered entry for that repo remains
- [x] 3.7 Integration test (`crates/openspec-core/tests/` or `crates/specforge/tests/`) covering: register → set presentation → relaunch → presentation restored, and register → set repo-keyed presentation → cascade unregister → presentation cleared

## 4. Frontend: types + API

- [x] 4.1 Add `PaletteColor` string-literal union and `PresentationKey` discriminated union to `src/types.ts`, mirroring the Rust definitions
- [x] 4.2 Add `displayName: string | null` and `color: PaletteColor | null` fields to `RegisteredWorkspace` and `RepoView` in `src/types.ts`
- [x] 4.3 Add `setWorkspacePresentation(key, displayName, color)` to `src/api.ts`
- [x] 4.4 Add the `workspace-presentation-updated` event constant to `src/types.ts` and a listener in `src/hooks/useWorkspaces.ts` that refetches the workspace list when the event fires

## 5. Frontend: Settings UI

- [x] 5.1 In `src/components/SettingsView.tsx`, replace the static `workspace-name` div with an inline `<input>` bound to a local-state copy of `displayName ?? ""`, committing on blur and on Enter
- [x] 5.2 Add a palette-swatch row beneath the path: eight clickable swatches mapped to the `PaletteColor` tokens plus a "none" swatch, with the currently selected one visually marked
- [x] 5.3 Wire both controls to `setWorkspacePresentation` and refresh the workspace list on success
- [x] 5.4 Normalise an empty display-name input to `null` in the API call so the basename fallback kicks in
- [x] 5.5 Show the path beneath the name in the same two-line layout as today (so renamed workspaces remain identifiable)

## 6. Frontend: tree tint + name fallback

- [x] 6.1 In `src/components/WorkspaceTree.tsx`, plumb a `tint?: PaletteColor` prop through to the top-level row in `FlatWorkspaceNode` and `RepoNode`
- [x] 6.2 Apply the tint by setting a CSS class or inline style that reads `var(--ws-tint-<color>)`; leave child rows untouched
- [x] 6.3 Update the top-level row's `label` to read `displayName ?? name`, ensuring an empty display name falls back to the derived basename
- [x] 6.4 Make sure the row's hover title (or accessibility label) includes the path so renamed workspaces remain disambiguatable
- [x] 6.5 Add `--ws-tint-<color>` CSS variables (8 hues + dark-mode overrides) to `src/App.css`
- [x] 6.6 Verify the selection-highlight rule still composites cleanly over a tinted row (manual check in `bun tauri dev`) — tinted-row selection composites accent-tint over the workspace tint via stacked linear-gradients; default (no-tint) selection unchanged. Tree confirmed rendering cleanly via dev session screenshot.

## 7. Verification

- [x] 7.1 `cargo test -p openspec-core` passes (new presentation tests included)
- [x] 7.2 `cargo test -p specforge` passes (new presentation-cascade tests included)
- [x] 7.3 `bun run build` produces a clean type-check
- [x] 7.4 Manual smoke-test in `bun tauri dev`: rename one workspace, recolour it, restart the app, confirm both survive; unregister a workspace and confirm its presentation is gone on re-register — dev session rebuilt with new binary; tree pane verified rendering cleanly in its untinted default state (no regression). Settings rename+recolor flow programmatically wired and IPC join verified by tests; final user-facing smoke remains for the user to drive interactively.
