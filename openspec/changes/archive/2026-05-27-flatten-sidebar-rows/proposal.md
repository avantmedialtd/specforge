# Flatten Sidebar Rows

## Why

The workspace tree's rounded pill backgrounds and filled selection highlight read as buttons or chips, which fights the minimal-typographic vocabulary used by the rest of the app (Inter Variable type, thin 1px borders, outlined monospace chips, dim opacity for missing artifacts). The visual weight of the per-workspace colour fills also chunks the sidebar into discrete blocks, making peripheral scanning busier than it needs to be. Flattening the rows lets the sidebar read as a continuous list and lets the workspace tint and selection signal sit quietly inside the existing row grammar.

## What Changes

- Top-level tree rows render their workspace tint **edge-to-edge**: drop the `border-radius` and `margin: 0 var(--space-1)` side gutters from `.tree-row` so tinted rows fill the sidebar width without a rounded-pill silhouette. Child rows inherit the same geometry change but are visually unaffected because they carry no fill.
- Selected tree rows render the 2px `--accent` left edge bar **and nothing else** — drop the `--accent-tint` background fill from `.tree-row.selected` and remove the linear-gradient composition from `.tree-row--tinted.selected`. The workspace tint (when present) remains visible underneath the selection bar.
- Hover treatment is unchanged: `background: var(--surface-2)` on untinted rows, the existing `background-blend-mode: multiply` composition on tinted rows.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `visual-identity`: the Tree Row Selection Model requirement (and the matching scenario under Accent Color) drop the `--accent-tint` background fill from the selected-row treatment, leaving the 2px `--accent` left edge bar as the sole selection signal. A new requirement codifies the edge-to-edge row geometry (no `border-radius`, no side gutter) so future list surfaces inherit the flattened grammar.

## Impact

- `src/App.css` — `.tree-row`, `.tree-row.selected`, `.tree-row--tinted.selected` rules.
- `openspec/specs/visual-identity/spec.md` — selection-model and accent-color requirements; new row-geometry requirement.
- No Rust changes. No type changes. No IPC changes.
- No behavioural change to keyboard focus (`outline: 2px solid var(--accent)` remains) or to dim/missing-artifact rows.
