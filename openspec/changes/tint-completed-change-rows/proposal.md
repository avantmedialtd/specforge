# Tint Completed Change Rows

## Why

The workspace tree already lights up completed work — a fully-done change earns an `--ok-strong` green left rail and a filled green completion disc — but the signal lives only at the row's edge and in its trailing glyph. When a tree mixes in-progress and finished changes, the *body* of a completed row is visually identical to an in-progress one, so "done" registers as a thin rail plus a small mark rather than as a highlight the eye lands on. A soft green background wash across the completed change row makes completion legible at a glance — the highlight the user asked for — without adding motion or noise.

## What Changes

- A completed two-line change row (the flattened git singleton `InstanceNode` and the flat-workspace `FlatChangeNode`) renders a soft `--ok`-family background tint across the whole row, **in addition to** its existing `--ok-strong` rail and completion disc.
- The tint is **additive**: the completion disc (the colour-independent "done" shape) and the `--ok-strong` rail both stay, so the accessibility story — shape carries the meaning, green reinforces — is unchanged. The wash never becomes the sole signal.
- **Selection continues to win.** A selected completed change shows the `--accent-tint` selection wash and `--accent` bar, exactly as selection already overrides the completion rail today. The green completion wash appears only on an *unselected* completed row.
- A **hover** over an unselected completed row deepens the green wash one notch (mirroring how `.tree-row.selected:hover` deepens the accent wash), rather than reverting to the neutral `--surface-2` hover.
- One token pair is added: **`--ok-tint`** and **`--ok-tint-strong`** — a low-alpha `--ok` wash mirroring the existing `--accent-tint` / `--accent-tint-strong` pair (same alphas, so text contrast on the wash matches the already-accepted selection wash).

## Capabilities

### Modified Capabilities

- `visual-identity`: two requirements are amended.
  - *Completed-State Styling*: its standing "a completed change SHALL NOT receive a full-row background wash" clause is reversed for the completed change row, which now renders an **additive** `--ok-tint` wash (deepening to `--ok-tint-strong` on hover). The completion disc and `--ok-strong` rail remain required, so the wash is reinforcement and the colour-independent shape signal survives. Selection still overrides the wash.
  - *Tree Row Selection Model*: its "hover on an unselected row renders `--surface-2` uniformly on every row" clause gains a single carve-out — an unselected completed change row deepens its green completion wash on hover instead of reverting to the neutral background. The core invariant is untouched: the `--accent-tint` wash still means *selected* and still overrides the completion wash whenever both apply.

## Impact

- `src/App.css` — add `--ok-tint` / `--ok-tint-strong` tokens (light + dark, alphas matched to the `--accent-tint` pair); add a `background` wash to the existing `.tree-row--complete` rule and a `.tree-row--complete:hover` deepened-wash rule, both placed so `.tree-row.selected` / `.tree-row.selected:hover` (higher specificity) still override them to the accent wash.
- `src/components/WorkspaceTree.tsx` — **likely no change.** `Row` already emits `tree-row--complete` for a completed two-line change row (the `complete` flag is set from `allTasksDone(...)` at both the singleton `InstanceNode` and `FlatChangeNode`), and the new CSS hooks onto that existing class. To be confirmed during implementation.
- **No Rust changes.** Completion is already derived (`completedTasks === totalTasks`); this is a pure presentation change.
- **Deliberate scope boundaries** (so nobody "fixes" these later as oversights):
  - **Change row only.** The wash applies to the two-line change row — *not* to Sections, the Tasks artifact node, multi-instance child rows, or leaf tasks. Those keep today's disc / green-struck-text treatment. A near-complete change must not turn its whole subtree green.
  - **Additive, not a replacement.** The completion disc and `--ok-strong` rail both stay; the wash is reinforcement layered on top, never a substitute for the shape-based signal.
  - **Selection keeps its exclusive claim on the accent wash.** The green wash yields to the `--accent-tint` selection wash; a selected row still reads unambiguously as selected.
  - **No motion / celebration.** No animation when a row flips to done — static colour only, consistent with the prior completion-styling change.
  - **No workspace-level roll-up.** The always-visible workspace/repo rows are untouched; a per-workspace "all changes done" indicator remains out of scope.
