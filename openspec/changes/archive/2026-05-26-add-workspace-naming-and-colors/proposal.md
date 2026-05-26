# Add Workspace Naming and Colors

## Why

The tree pane shows each top-level workspace by its folder basename, with no way to distinguish two workspaces that share a name or to visually group personal favourites. Users with several registered workspaces want a custom display name and a tinted background on the parent row so they can identify a workspace at a glance from the sidebar.

## What Changes

- New persistent store for per-workspace presentation (display name + colour token), keyed separately from the workspace registry so identity stays decoupled from cosmetics.
- Settings view gains an inline rename field and a curated colour-swatch row (8 tokens + "none") for every listed workspace.
- Tree pane tints the parent workspace/repo row background using the selected colour token; child rows remain unchanged.
- When the colour is "none" or no presentation entry exists, rendering matches today exactly (no visual regression for unconfigured workspaces).
- Presentation entries cascade-delete when their last user-registered workspace is unregistered, mirroring the existing registry cascade.

## Capabilities

### New Capabilities
<!-- None. This change adds presentation persistence to an existing capability rather than introducing a separable concept. -->

### Modified Capabilities
- `workspace-registry`: persist per-workspace presentation (display name + colour) alongside the registry, surface those fields on `RegisteredWorkspace` and aggregated repo views, and extend the Settings View with rename + recolour affordances.
- `spec-browser`: tint the top-level workspace/repo row in the tree pane using the configured colour token and surface the configured display name in place of the derived basename.

## Impact

- **Rust core** (`crates/openspec-core/`): new `WorkspacePresentationStore` (parallel to `WorkspaceRegistry`), new key type covering flat workspaces and repo groups, new `PaletteColor` enum, `RegisteredWorkspace` and `RepoView` extended with `displayName` and `color`, cascade hook on unregister.
- **Tauri shell** (`crates/specforge/`): new `set_workspace_presentation` command, store loaded at startup and joined into the existing `list_workspaces` / repo-view command results.
- **Frontend** (`src/`): `RegisteredWorkspace` / `RepoView` TypeScript types updated, Settings rename + palette UI added, tree CSS adds `--ws-tint-<colour>` variables for light + dark themes and applies the tint on parent rows only.
- **Config files**: a new `presentation.json` lives next to `workspaces.json` in the app config directory; missing file means empty store (no migration needed).
- **No breaking changes** to existing IPC payloads — every new field is optional and absent means today's behaviour.
