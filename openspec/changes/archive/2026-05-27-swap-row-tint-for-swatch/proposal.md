# Swap Row Tint For Swatch

## Why

The full-row workspace tint (introduced in `601b739` and iterated in `c14f234` from "floating pill" to "edge-to-edge band") still reads as overdone. Three full-width coloured bands stack up in the sidebar and dominate the visual field. The colours themselves work for identity — distinguishing one workspace from another at a glance is what they're there for — so the goal is to preserve that signal while shrinking the area it occupies. An 8px filled dot is the minimal viable identity marker, and a 1px hairline between workspaces restores the "section break" affordance that the row-band gave us for free.

## What Changes

- Top-level tree rows (flat workspace nodes and repository group nodes) render their configured palette colour as an **8px filled circular swatch** between the chevron and the label, replacing the previous full-row tint background. Child rows still carry no identity marker.
- Swatch colour resolves from the existing `--ws-swatch-*` tokens (the punchy 55–60% lightness variants that already drive the Settings palette picker). No new tokens are introduced.
- The `.tree-row--tinted` rule and every `.tree-row--tinted.tree-row--tint-*` colour-resolution rule are deleted from `src/App.css`. The `--ws-tint-*` tokens (8 light + 8 dark-scheme) are deleted from `:root` — orphan-clean, like the recently-orphaned `--accent-tint` is not.
- A 1px `var(--border)` `border-top` is rendered on every top-level row except the first, so successive workspaces remain visually grouped as sections without a heavy fill.
- Hover treatment becomes uniformly `background: var(--surface-2)` on every row. The current exception clause for tinted rows composing a `background-blend-mode: multiply` wash over the tint disappears with the tint.
- Selection is unchanged: the 2px `--accent` `border-left` slot composes cleanly with the swatch (different position) and the divider (different axis).

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `spec-browser`: the Top-Level Row Display Name and Tint requirement is renamed to **Display Name and Swatch** and rewritten so that the identity channel is an 8px filled swatch in the row content, not a row background tint. A clause is added requiring a 1px `--border` top-divider between successive top-level rows.
- `visual-identity`: the Tree Row Selection Model requirement drops the "tinted top-level rows SHALL compose the hover wash over the tint" exception from the hover clause. Hover becomes uniformly `background: var(--surface-2)` on every row, regardless of depth.

## Impact

- `src/App.css` — delete `.tree-row--tinted` and the eight `.tree-row--tinted.tree-row--tint-*` rules; delete `--ws-tint-*` tokens from both schemes; add `.row-swatch` rules and per-colour `--ws-swatch-*` mappings; add `border-top: var(--border-width) solid var(--border)` on `.tree-row[data-top-level]:not(:first-child)` (or equivalent selector).
- `src/components/WorkspaceTree.tsx` — `Row` primitive gains a `swatch?: PaletteColor | null` slot rendered between chevron and label; `RepoNode` and `FlatWorkspaceNode` switch from `tint={color}` to `swatch={color}`; the `tint` prop is removed from `Row` along with its `tintClass` composition.
- `openspec/specs/spec-browser/spec.md` — rename and rewrite the Top-Level Row Display Name and Tint requirement; add the inter-workspace divider clause.
- `openspec/specs/visual-identity/spec.md` — drop the tint-composition exception from the Tree Row Selection Model hover clause.
- No Rust changes. No IPC changes. No `PaletteColor` enum changes (the eight palette colours stay; only their rendering pathway changes).
- No behavioural change to keyboard focus, dim/missing-artifact rows, the Settings palette picker, or any non-top-level row.
