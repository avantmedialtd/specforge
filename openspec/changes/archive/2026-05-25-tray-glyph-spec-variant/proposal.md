# Tray Glyph Variant for Active Spec Changes

## Why

The tray icon currently gives users one piece of at-a-glance information: the count of active changes. It does not differentiate *what kind* of work is in flight. When a user has changes that touch capability specs (the high-leverage, hardest-to-revert artifact in the OpenSpec workflow), they should be able to see that from the menu bar without opening the app.

## What Changes

- Add a second tray glyph variant, sourced from a new `crates/specforge/icons/tray-specs.svg`, displayed whenever any active change in any registered workspace has a non-empty `ArtifactStatus.specs`.
- Default to the existing `tray-icon.svg` glyph whenever the predicate is false or the cache state is unknown (no workspaces, pre-populate, transient parse error, etc.).
- Drive variant switching off the same `CacheEvent` broadcast that already feeds the badge updater — no new watcher coverage required.
- Preserve the existing scale-factor re-rasterization path so it re-rasterizes whichever variant is currently active.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `tray-indicator`: Adds a new requirement (peer to **Active-Change Badge**) covering glyph variant selection based on active-change spec activity, with scenarios for flip, flip-back, ANY-workspace aggregation, fail-safe default, and persistence across scale changes.

## Impact

- `crates/specforge/icons/`: adds `tray-specs.svg`; the existing `tray-icon.svg` continues to serve as the default variant.
- `crates/specforge/src/tray_icon.rs`: bundles the second SVG, introduces a glyph-variant enum, and parameterises `rasterize_glyph` by variant. The pure-black + alpha debug assertion continues to cover both SVGs.
- `crates/specforge/src/tray.rs`: adds a glyph-updater task analogous to `spawn_badge_updater`.
- `crates/specforge/src/lib.rs`: introduces shared variant state, wires the updater, and updates the `ScaleFactorChanged` handler to re-rasterize the current variant rather than always the default.
- `crates/openspec-core/src/watcher.rs`: small `any_change_touches_specs()` helper paralleling `total_active_count()`.
- No new dependencies. No IPC surface changes. No frontend changes. No filesystem layout changes.
