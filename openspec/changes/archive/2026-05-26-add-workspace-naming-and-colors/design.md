## Context

`openspec-core` persists user-registered workspaces in `workspaces.json` via `WorkspaceRegistry` (`crates/openspec-core/src/registry.rs`). The registry tracks two origins — `UserRegistered` (persisted) and `Discovered` (re-derived from `git worktree list` on startup and at runtime). The tree pane shows two kinds of top-level rows: a flat workspace node (a non-git registered folder) or a repo group (a `RepoView` aggregating all worktrees of one git repository, keyed by the canonicalised git common directory).

Display names today are derived: `WorkspaceFolder::from_path` uses the path's final component, and `RepoView::name` uses the main worktree's basename. There is no editing surface and no way to differentiate two workspaces that share a basename.

The Settings view (`src/components/SettingsView.tsx`) already iterates the registered list and exposes add/remove plus two app-wide toggles. The tree (`src/components/WorkspaceTree.tsx`) renders top-level rows via `FlatWorkspaceNode` and `RepoNode`, both of which read `name` directly.

## Goals / Non-Goals

**Goals:**
- Persist a per-top-level-row display name and colour token, survivable across restarts.
- Surface both fields wherever a top-level row is rendered (tree pane parent row, Settings list).
- Render the colour as a dim tinted background on the parent row only, with a curated palette + an explicit "none" option.
- Edit both fields from Settings: inline rename input + palette swatch row, single source of truth.
- Cascade-clean presentation entries when their underlying registration is removed.
- Preserve current behaviour exactly when no presentation entry exists.

**Non-Goals:**
- Per-worktree (sub-row) colouring. Worktrees inside a repo do not get individually colourable.
- Right-click context menu, command palette, or any non-Settings editing surface.
- Freeform hex colour input. Only curated palette tokens are persisted.
- Tinting the macOS tray icon or notification text (template-icon constraint + low value).
- Migrating today's `WorkspaceFolder.name` field — it remains the basename-derived default; presentation overrides it at render time.

## Decisions

### Separate `presentation.json` store, keyed by row identity

Add a `WorkspacePresentationStore` in `openspec-core/src/presentation.rs`, persisted to `presentation.json` in the same config directory as `workspaces.json`. The store is a thin `HashMap<PresentationKey, PresentationEntry>` with the same `load` / `save` / mutating-method shape as `WorkspaceRegistry`.

`PresentationKey` is an enum that tags the two top-level row kinds:

```rust
pub enum PresentationKey {
    Flat(PathBuf),  // canonical workspace path
    Repo(RepoId),   // canonical git common directory
}
```

`PresentationKey` serialises to a stringly form (`"flat:<path>"` / `"repo:<path>"`) so the JSON file is human-inspectable and the key survives round-tripping through Tauri commands.

**Alternative considered**: extend `WorkspaceFolder` with `display_name` + `color` and persist inside `workspaces.json`. Rejected because (a) repo groups have multiple workspace rows but only one row in the tree, so the "which workspace's colour wins" question becomes load-bearing; (b) discovered workspaces are not persisted, so cosmetics on them would either need promotion or get lost; (c) it conflates identity (the registry's job) with presentation (the new store's job). Keeping the two stores separate lets the registry cascade trigger a parallel cleanup on the presentation side without entangling the data models.

### Curated palette of 8 named tokens + null

The persisted `color` field is `Option<PaletteColor>` where the enum has 8 variants (`Indigo`, `Blue`, `Teal`, `Green`, `Amber`, `Orange`, `Rose`, `Purple`). `None` means "no tint" (the explicit "none" choice from the picker is identical to having no presentation entry, render-wise).

Serialised as kebab-case strings (`"indigo"`, `"teal"`, …) for IPC + on-disk consistency with the rest of the project's serde conventions.

**Why named tokens not hex**: light + dark mode parity becomes trivial (each token maps to two CSS variables, picked from a designed palette), accessibility is solved once at design time rather than every time the user picks a colour, and the visual rhythm of the app stays coherent.

### Cascade cleanup on unregister

`WorkspaceRegistry::unregister` already returns the list of paths it removed (the user-registered entry plus, when applicable, every cascaded discovered worktree of the same repo). The Tauri shell's `unregister_workspace` command will, after a successful unregister, compute the presentation keys those removals correspond to (every flat path becomes a `Flat(...)` key; the repo's common-dir becomes a `Repo(...)` key if the cascade fired, i.e. if no user-registered entry for the repo remains) and drop those entries from the presentation store.

Importantly, the presentation store does not get a reference to the registry — the shell mediates between the two. This keeps `openspec-core` modules independently testable.

### Tinted background on the parent row only

The tint is rendered as a CSS background-colour on the workspace/repo `Row`. Child rows do not inherit the tint and do not get a left-edge gutter. Selection highlight composites cleanly over the tint because both layers use semi-transparent fills against the row's base background.

**CSS token shape:**

```css
:root {
    --ws-tint-indigo: hsl(243 75% 95%);
    --ws-tint-teal:   hsl(174 65% 92%);
    /* …8 variants */
}
[data-theme="dark"] {
    --ws-tint-indigo: hsl(243 50% 22% / 0.5);
    --ws-tint-teal:   hsl(174 45% 22% / 0.5);
    /* …8 variants */
}
```

The Row component receives a `tint?: PaletteColor` prop. When set, the row gets `style={{ backgroundColor: 'var(--ws-tint-' + color + ')' }}` (or a class, depending on what reads cleaner in the existing CSS). When unset or null, no inline style is applied — the default row background is used, identical to today.

### Display-name precedence

The frontend reads `displayName ?? name` for any top-level row. `name` keeps coming from the registry (the basename-derived value); `displayName` comes from the presentation store. An empty-string `displayName` saved from Settings is normalised to `null` server-side, so clearing the field always falls back to the default rather than rendering an empty label.

### New Tauri command

```rust
#[tauri::command]
fn set_workspace_presentation(
    key: PresentationKey,
    display_name: Option<String>,
    color: Option<PaletteColor>,
) -> Result<(), AppError>;
```

Reads + writes through a `Mutex<WorkspacePresentationStore>` held in app state alongside the registry. After saving, the command emits a `workspace-presentation-updated` event so the frontend refetches and the tree re-tints.

`list_workspaces` and the aggregated repo-view command are extended to join in the presentation entries before returning — no new fetch round-trips on the frontend.

## Risks / Trade-offs

- **Two files to keep in sync** → cascade behaviour is centralised in the Tauri command that wraps unregister; tests cover registry + presentation jointly so drift is caught.
- **Renaming hides the path** → the path is still shown beneath the editable name in Settings (today's two-line `workspace-info` block); the tree row's hover title can include the path to keep it discoverable.
- **Curated palette feels limiting** → 8 hues is enough to make adjacent rows distinct; if it proves not to be, the enum can grow without breaking persistence (older entries keep their tokens, new tokens just become legal).
- **Migration of existing config** → none required: a missing `presentation.json` is a valid empty store, and `RegisteredWorkspace.displayName` / `.color` are `Option` fields that absent means today's behaviour.
- **Two workspaces sharing the same renamed display name** → the Settings list still shows the path, so the user can disambiguate; the tree shows the colour, which is the recognition signal. No collision detection.
