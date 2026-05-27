## Context

The workspace tree's artifact subtree currently renders a leading `<Check />` (when the artifact's underlying file is present) or a `<DotOutline />` (when absent). Because an active change almost always has all four artifacts present, this collapses to "four checkmarks in a row that all say the file exists" — a near-zero-information signal that competes with the chevron for the same visual slot. The same `Check` glyph is reused in the *trailing* slot of a section row to mean "all tasks done," so the icon has two different meanings depending on position. The flat-change row also borrows the leading slot to display "all tasks done," whereas the instance row (the path git-worktree changes take) does not surface completion at all.

The visual-identity spec carries an aspirational `MISSING` chip rule for missing-artifact rows that was never wired into the React code (the code went with the inline `DotOutline` icon instead). Either treatment makes absence loud; we want absence quiet.

## Goals / Non-Goals

**Goals:**

- A single unifying rule: *leading slot = identity (chevron, tint); trailing slot = state (progress, completion, mtime, divergence).*
- Default silence — present, healthy artifacts carry no leading glyph.
- Missing artifacts read as a *muted slot* rather than as a loud chip or a competing icon — they remain visible in the tree (so the four-artifact mental model is preserved) but cannot be selected or clicked.
- Symmetric completion handling across the two change-row types (`FlatChangeNode` and `InstanceNode`).

**Non-Goals:**

- Introducing a `draft` / `outdated` workflow state for artifacts. The proposal considered it; we explicitly defer until there is a real signal to read (frontmatter, validator output, mtime heuristic). Today the only state we can compute cheaply is `present` / `absent`.
- Touching the leading `CheckSquare` / `Square` glyph on individual task rows. That glyph IS the task's primary visual identity — a checklist row should look like a checkbox.
- Touching the trailing `Check` on a completed section row. The section's auto-collapse already signals "done"; the trailing tick is the explanation when a user re-expands a completed section. Useful, kept.
- Adding a "create missing artifact" interaction. Dim rows are passive slot indicators only; artifact creation continues to happen outside the app (CLI / opsx workflows). The dim-row treatment intentionally does not promise interactivity that the read-only viewer cannot fulfil.

## Decisions

### Dim treatment: `opacity: 0.45` on the row, not desaturated text colour

Why opacity over a muted text-colour variable: the workspace tree includes tinted rows (red / purple / yellow / brown / etc. per `Top-Level Row Display Name and Tint`). A text-colour shift competes with the tint background and can land at low contrast inside a coloured row. Opacity uniformly mutes everything in the row (label, chevron-spacer, any meta) and composes predictably on top of any background — tint, hover, selection.

`0.45` because it is the lowest value where the row text remains legible at the workspace tree's 13px label size on standard contrast (the same threshold WCAG uses for "secondary text" patterns in dark themes). `0.4` starts to wash out against the macOS sidebar vibrancy; `0.5` reads as "deemphasised but still primary," which under-sells the absence. Reviewed against both tinted and untinted rows in light/dark mode before committing.

Alternative considered: hiding missing artifact rows entirely. Rejected because the four-row block is part of the OpenSpec mental model — seeing "Specs" greyed out reminds the user that specs *can* exist. Hiding makes new changes look like a different shape from completed ones.

### Non-interactive missing rows: `pointer-events: none` rather than a disabled-style click handler

Why: a dim row that responds to clicks would land on an empty detail pane and require either (a) a new "this artifact doesn't exist yet" empty state, or (b) silently showing nothing. Both add surface area for a passive indicator. `pointer-events: none` short-circuits the question — the row is visibly inert, no cursor change, no hover state, no selection ring. This matches the read-only viewer principle in the existing `Read-Only Viewer` spec requirement.

Side effect: the chevron-spacer remains in place so the visual rhythm of the four-row block doesn't collapse. We only suppress *interactivity*, not *layout*.

### One completion-glyph rule for both row types

Two paths to symmetry were considered:

1. Modify the existing `Instance Row Chrome` requirement to enumerate completion alongside progress/mtime.
2. Add a single new requirement covering completion glyph rendering on both `FlatChangeNode` and `InstanceNode`.

Picked (2). The rule is the same on both row types ("when every task is complete, render a trailing `Check` glyph alongside the progress count"); expressing it once keeps it from drifting between two requirements over time. The two existing chrome requirements continue to govern the *other* elements they describe.

### Where the trailing glyph sits in the meta cluster

Trailing meta on an instance row already orders as: `[active dot] [progress] [mtime] [divergence chip]`. The new completion glyph slots in *between progress and mtime* — adjacent to the count it relates to (`29/29 ✓`), separated from mtime (`1m ago`). The flat-change row's meta is simpler today (`[changeId]`); the completion glyph goes *before* the changeId, again to keep state-of-the-change ahead of identity-of-the-change.

## Risks / Trade-offs

- **Tinted rows + 0.45 opacity** → text legibility on saturated tints (purple, red). Mitigation: review at both 100% and 4K display scales on macOS sidebar vibrancy before merging. The tint applies only to top-level workspace rows; child rows (including artifact rows) sit on the default background, so this risk is mostly theoretical — but worth double-checking on macOS where vibrancy composes underneath.
- **Existing tree tests** → tests that assert on the presence of `<Check />` / `<DotOutline />` icons under artifact rows will fail. Mitigation: rewrite them to assert on the `.tree-row--dim` class (or the inverse: presence of *no* leading icon). Listed under tasks.
- **Aesthetic regression on missing-artifact discoverability** → some users may not notice a dim row at all and miss that "an artifact slot is empty." Mitigation: the row label still reads (just dimmer); the row's chevron-spacer keeps the slot visible; and the detail pane (already surfaces presence elsewhere through the workspace's progress counters) is unaffected. If the muting turns out to be too quiet in use, a future change can re-introduce a small marker — but it would not bring back the leading-`✓` confusion.
- **The retired `MISSING` chip rule** in `visual-identity` was example-only ("status badges *e.g.* MISSING, DIVERGED") and unused in code. Removing it from the spec carries near-zero migration risk; the only impact is that the example list shortens.
