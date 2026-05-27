## Context

The workspace tree currently renders each row with `border-radius: var(--radius)` (6px) and `margin: 0 var(--space-1)` (4px inline gutters). Top-level workspace/repo rows additionally carry a `--ws-tint-*` background; child rows carry no fill. Selected rows compose a 2px `--accent` left border with an `--accent-tint` background — and, on tinted rows, the selection layers via `linear-gradient(var(--accent-tint), var(--accent-tint))` over the workspace tint.

The combined effect on screen is a column of rounded coloured pills separated by 4px gutters, with each selected pill carrying an additional indigo wash on top of its workspace colour. Two visual cues collapse into one surface treatment.

The rest of the app — Inter typography, outlined chips with 1px borders and uppercase letterspacing, monospace identifiers, dim opacity for missing rows — leans minimal and typographic. The pill-and-wash pattern doesn't share that vocabulary.

This change rewires the row grammar so workspace identity and selection sit on different visual channels: identity becomes an edge-to-edge tint band on the row's background; selection becomes a 2px accent bar on the inline-start edge with no fill. Both signals are still legible on the same row when they coexist.

## Goals / Non-Goals

**Goals:**

- Top-level tinted rows render edge-to-edge — no rounded corner, no side gutter.
- Child rows inherit the same flat geometry; visually unchanged because they carry no fill.
- Selection is a single visual signal: the 2px `--accent` left edge bar. No background change.
- Workspace tint remains visible on a selected row, without composition tricks.
- Hover state unchanged (`var(--surface-2)` on untinted; multiply-blend wash on tinted).
- Keyboard focus ring (`outline: 2px solid var(--accent)`) unchanged.
- Dim/missing-artifact treatment unchanged.

**Non-Goals:**

- Reworking the workspace tint palette (`--ws-tint-*` tokens stay as defined).
- Changing the workspace colour assignment UI in Settings.
- Introducing a divider line between adjacent top-level rows — out of scope; revisit only if the live result reads as busy.
- Touching any list surface outside the workspace tree (Settings rows continue to follow the row grammar via the visual-identity *Uniform Row Grammar* requirement, which inherits the change automatically when its tokens follow).

## Decisions

### Decision: Row geometry — edge-to-edge, no radius

Drop `border-radius: var(--radius)` and `margin: 0 var(--space-1)` from `.tree-row`. The 2px transparent left border slot that today reserves space for the selection bar stays as-is — selection still slides in without shifting label content horizontally.

**Why over alternatives:**

- *Keep radius, lose only the margin*: row corners would still curve against the sidebar edge, producing tab-like cutouts that read worse than a clean band.
- *Keep margin, lose only the radius*: tinted rows would float inside a transparent gutter, which is what we have today minus the rounded silhouette — the floating-card feel persists.
- *Edge-to-edge on tinted rows only*: requires conditional styling on `.tree-row--tinted` and breaks the uniform row grammar — child rows would have different geometry than their parent. Rejected for inconsistency.

### Decision: Selection — 2px left bar, no fill

`.tree-row.selected` keeps `border-left-color: var(--accent)` and drops `background: var(--accent-tint)`. `.tree-row--tinted.selected` drops its linear-gradient composition entirely; the workspace tint is already painted by `.tree-row--tinted`, and the selection bar sits in the existing border slot.

**Why over alternatives:**

- *Foreground-shift only (brighter label)*: pure typographic answer, but easy to miss in peripheral vision after scrolling away and back. The edge bar is glance-able from the corner of the eye.
- *Quieter fill (drop accent-tint opacity)*: keeps the dual-signal problem the change is trying to solve — selection still reads as a surface treatment, not a marker.
- *Saturated workspace tint on selection*: couples selection to workspace identity, breaks on child rows (no tint to saturate), and produces two selection languages.

### Decision: No new tokens

The change subtracts CSS rules; it does not add design tokens. `--accent`, `--accent-tint`, `--radius`, `--space-1` remain defined exactly as today — `--accent-tint` is still used by the markdown view link hover and other surfaces (verify in implementation), so the token stays. Only the *application* of `--accent-tint` to selected tree rows is removed.

### Decision: Spec changes go in `visual-identity`

The Tree Row Selection Model requirement lives in `visual-identity`; that's where the delta belongs. A new requirement codifies the flat row geometry (no `border-radius`, no inline gutter) so any future list surface that conforms to the *Uniform Row Grammar* requirement inherits it. The existing `spec-browser` requirement about tint-composes-with-selection still holds (the bar composes fine over the tint) — no delta there.

## Risks / Trade-offs

- **Adjacent tinted rows touch without separation.** If three workspaces stack with none expanded, their tint bands butt directly. Could read as a unified section list, or as jarring hue stripes depending on the chosen colours. → Verify in the running app. Mitigation if it reads badly: 1px hairline `--border` divider between adjacent top-level rows, or drop the dark-mode tint alpha from 0.55 to ~0.40.

- **Selection less obvious on the workspace row.** Today, a selected workspace row gets *two* signals: the accent left bar AND the accent-tint overlay. After the change it gets one. → The 2px bar plus the existing focus outline on keyboard focus should be sufficient. Watch for user feedback that selection is hard to spot when the workspace tint is a high-contrast colour.

- **`--accent-tint` may become orphaned.** If the markdown view link hover or another surface stops using `--accent-tint` independently, the token becomes dead. → Grep for `--accent-tint` callers during implementation and confirm it still has at least one non-tree-row consumer before deciding whether to keep the token definition.

## Migration Plan

Pure CSS change — no migration steps. The change ships in one commit; rollback is `git revert`. The on-disk OpenSpec format is untouched, so no workspace data migration. Users will see the visual change on the next app launch after the build is installed.
