# Quieten Tree Icons

## Why

Every present artifact row currently wears a leading `✓` whose only meaning is "this file exists on disk" — a near-zero-information signal for active changes that almost always have all four artifacts present, and one that directly contradicts the same `✓` glyph used in the *trailing* slot of a section row to mean "all tasks complete." The result: a sidebar dense with redundant ticks that drowns out the signals that actually carry state (progress counts, section completion, divergence).

## What Changes

- **Drop the "artifact exists" leading icon.** Present artifact rows (Proposal, Specs, Design, Tasks) render with no leading glyph.
- **Dim missing artifact rows.** When the underlying file does not exist, the artifact row renders at `opacity: 0.45`, is non-interactive (pointer events disabled, click is a no-op, can't be selected). The row remains in the tree as a slot indicator.
- **Move the flat-change "all done" glyph from leading to trailing.** `FlatChangeNode`'s leading `✓` (rendered today when all tasks complete) relocates to the trailing meta slot alongside progress and mtime.
- **Add a trailing "all done" glyph to instance rows.** `InstanceNode` does not currently surface task completion anywhere; add a trailing `✓` in the meta slot so flat-change and instance rows behave consistently.
- **Retire the "missing-artifact chip"** visual-identity requirement — dim-row supersedes the chip treatment.

The rule that crystallises: *leading slot = identity (chevron, tint); trailing slot = state (progress, completion, mtime, divergence).*

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `spec-browser`: artifact subtree rendering — present rows shed their leading icon; missing rows become dim + non-interactive; trailing completion glyph is added to both flat-change and instance rows.
- `visual-identity`: the "Missing-artifact chip is outlined" scenario under "Outlined Chip Badges" is replaced by a "Missing-artifact row is dimmed" scenario expressing the new treatment.

## Impact

- Frontend only: `src/components/WorkspaceTree.tsx` (`ArtifactNode`, `FlatChangeNode`, `InstanceNode`) and `src/App.css` (new `.tree-row--dim` rule; the `.icon-present` / `.icon-absent` rules become dead and can be removed).
- No Rust, IPC, or `ChangeData` shape changes — the booleans on `change.artifacts.*` already carry every signal we need.
- Existing tree tests that assert on leading-icon presence will need to flip to assert on row class / trailing-meta presence instead.
