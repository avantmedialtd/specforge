## Context

Workspace identity in the sidebar is currently a full-row background tint applied via `.tree-row--tinted` plus a per-colour rule that resolves `--tree-row-tint` to one of eight `--ws-tint-*` tokens (light + dark scheme). In dark mode each tint resolves to roughly `hsl(<H> 45% 27% / 0.55)` — a saturated wash that fills the row edge-to-edge after the recent flatten-sidebar-rows change.

The signal works: at a glance the user can tell three workspaces apart. But the area it occupies — three full-width coloured stripes stacked vertically — dominates a sidebar whose other vocabulary (Inter labels, mono identifiers, outlined chips, 1px hairlines) is deliberately minimal. The earlier `c14f234` iteration already pulled the rows from "floating pill" to "edge-to-edge band" without addressing the underlying area-coverage problem.

This change moves the identity signal off the row background and onto a small inline glyph at the start of the row's content — the smallest workable surface for the colour to live on — and replaces the lost "section header" affordance with a 1px hairline divider between successive top-level rows.

## Goals / Non-Goals

**Goals:**

- Workspace identity is conveyed by an 8px filled circular swatch between chevron and label, on top-level rows only.
- The full-row background tint is removed entirely, not weakened. All `.tree-row--tinted*` rules and all `--ws-tint-*` tokens are deleted (orphan-clean).
- Successive top-level rows are visually grouped as sections via a 1px `var(--border)` top hairline.
- Hover state simplifies to uniform `var(--surface-2)` across every row; no more tint-composition exception.
- Selection signal unchanged: 2px `--accent` `border-left` in the existing reserved slot.
- No new design tokens are introduced. Existing `--ws-swatch-*` tokens (already used by the Settings palette picker) carry the swatch colour.

**Non-Goals:**

- Settings palette picker UI — unchanged; it already renders `--ws-swatch-*` dots.
- The eight-colour palette itself — `PaletteColor` enum, registry assignment logic, and the colour-roundtrip across the IPC boundary all stay as-is.
- Task checkbox / status-dot rendering — they live in `row-icon` and are not affected by the new `row-swatch` slot.
- Bringing back any form of row-background tint as a user-toggleable preference.
- Light-mode visual tuning — the deleted `--ws-tint-*` tokens defined light-scheme values too, but light-scheme rows resolved to `var(--surface)` underneath and continue to do so. No replacement needed.

## Decisions

### Decision: 8px filled swatch as the identity marker

A new inline element, an 8×8 filled circle in the configured palette colour, sits between the chevron and the label on top-level rows. Top-level only — child rows carry no identity marker.

**Why over alternatives:**

- *Reduced-intensity row tint*: the issue is *area*, not just *intensity*. A 0.30-alpha band is still a band — three of them still stack to dominate the sidebar.
- *Left-edge rail (3–4px strip)*: a strong second choice, with the bonus that the strip can extend through children so workspace context survives scrolling. Discussed and not chosen because it occupies the same 0–4px inline-start band already reserved for the 2px `--accent` selection bar; coexistence would require either nudging the selection bar inboard or rendering the rail at a different inset, complicating two visual signals to fix a problem the swatch solves with one.
- *Colour-text label*: dyeing the workspace name itself in the palette colour. Rejected because the typographic system already uses colour to mean "muted / strong / status", and recolouring workspace names would collide with that grammar.
- *Coloured chip on the right side of the row*: pushes identity into the meta region, which is the worst place for scanning. Rejected.

### Decision: 8px size, not 6px or 4px

The status-dot vocabulary already uses 4px for compact status indicators. The workspace swatch needs to be visible enough to scan from several rows away, distinct from the 4px status dots semantically and visually. 8px is the smallest size that reads as "identity glyph" rather than "status indicator" at the row's 5px-padded height.

**Why over alternatives:**

- *6px*: visually borderline with the 4px status dot; the semantic distinction blurs.
- *10–12px*: starts to dominate the chevron next to it and pushes label content further right than the existing layout absorbs gracefully.

### Decision: Filled circle, not square or vertical bar

The `Dot` SVG primitive already exists in `src/components/icons.tsx` (mandated by the visual-identity *Inline SVG Icon Set* requirement, filled and outlined variants). Filled circle is the obvious reuse.

**Why over alternatives:**

- *Square*: would harmonise marginally better with the engineered-minimal aesthetic (flat rows, hairline borders), but a square at 8px reads as a UI control (button, tag) rather than as an inline glyph.
- *3px vertical bar*: tighter footprint but visually ambiguous against the chevron next to it (both vertical strokes).

### Decision: New `row-swatch` slot in the `Row` primitive, not reuse of `row-icon`

`Row` already has an `icon?: ReactNode` slot rendered between the chevron and the label, used by `TaskNode` for the checkbox glyph (`CheckSquare` / `Square`). Workspace rows currently use no icon. The swatch could squeeze into `icon` (workspace rows would pass `<ColorDot color={color} />` as the icon prop), but the semantic meaning is different: `icon` denotes per-row status, `swatch` denotes workspace identity. They're never co-present on the same row in practice (workspace rows are at depth 0, task rows at depth 4+), but the props read more clearly when separated.

**Why over alternatives:**

- *Reuse `icon`*: simpler — one slot. Rejected because the same slot would carry two semantically distinct concepts (status glyph vs identity glyph), making the `Row` interface less clear to future readers.
- *Inline `<span>` inside the label*: ties identity to the label's typography, complicates truncation and ellipsis on long workspace names. Rejected.

### Decision: Reuse `--ws-swatch-*`, delete `--ws-tint-*`

The 8px swatch needs the punchy 55–60% lightness palette values, not the translucent 0.55-alpha wash values. `--ws-swatch-*` already exist for the Settings picker dots — same surface, same size — and carry exactly the right values. Defining a new `--ws-glyph-*` would duplicate the swatch tokens for no reason.

The `--ws-tint-*` tokens lose their sole consumer (`.tree-row--tinted.tree-row--tint-*`) and become orphan. They are deleted from both `:root` and the dark-scheme override block. This contrasts with `--accent-tint`, which the flatten-sidebar-rows change left orphan because the visual-identity spec mandates it; `--ws-tint-*` are not mandated by any spec so they go cleanly.

### Decision: 1px `--border` top-divider on top-level rows except the first

Without the row tint, three workspace rows in a row would read as identical flat lines (modulo the small swatch). A 1px hairline above each workspace, except the first, restores the "section starts here" cue while staying within the minimal-typographic vocabulary.

**Why top-divider, not bottom-divider:**

- A top-divider on every workspace row except `:first-child` puts the rule directly above the row's start — natural reading order for "new section ahead."
- A bottom-divider on every workspace row except `:last-child` requires knowing which row is last (more complex selector in a tree where the last visible top-level row depends on what's expanded).
- The `:first-child` selector is stable: the first child of `.tree` is always the first top-level row.

**Why `--border`, not `--border-strong`:**

- `--border` matches the hairline grammar used between the split-pane edges and the footer.
- `--border-strong` is reserved for emphasised dividers (split-pane drag region hover state) and would re-introduce a heavier visual element.

### Decision: Hover simplifies to uniform `--surface-2`

The current rule has an exception:

```css
.tree-row:hover { background: var(--surface-2); }
.tree-row--tinted:hover {
  background: linear-gradient(var(--surface-2), var(--surface-2)),
              var(--tree-row-tint);
  background-blend-mode: multiply;
}
```

With `.tree-row--tinted` gone, the linear-gradient composition has no surface to compose with. The exception clause is deleted and hover becomes uniformly `var(--surface-2)` on every row.

## Risks / Trade-offs

- **[8px swatch is small enough to miss on a very wide sidebar at 4K @ 100%.]** → The Settings picker uses the same `--ws-swatch-*` tokens at a similar visible size and has not surfaced this complaint. Mitigation if it does: bump to 10px, which still fits inside the row's 5px vertical padding without affecting layout.

- **[The hairline divider plus the swatch may collectively feel busier than intended once seen in the running app.]** → The user already opted in to the divider variant after seeing flat-and-divider variants side by side. If it reads as too much, the cheap rollback is to drop the divider clause and keep only the swatch — that's a one-line CSS removal and a one-clause spec deletion.

- **[`--ws-tint-*` deletion is permanent in this change.]** If a future change wants to restore the row tint as a user-toggleable option, it would need to re-add the eight tokens for both schemes. → This is a deliberate trade — keeping orphan tokens around "in case" was rejected explicitly in the proposal. The git history preserves the values if they're needed again.

- **[The Settings picker and the in-tree swatch will now look visually identical at glance.]** Both are filled circles in `--ws-swatch-*`. → That's a feature, not a risk: choosing a colour in Settings predicts exactly what will appear in the tree. Today the Settings picker dot and the tree-row band are different surfaces with different intensities, and that disconnect is a small papercut.

## Migration Plan

Pure UI change. No on-disk data, no IPC, no Rust. Ship in one commit; rollback is `git revert`. The next app launch after install picks up the new visual.

Implementation order (informational — full task breakdown lives in `tasks.md`):

1. Update spec deltas (`spec-browser`, `visual-identity`) so the contract is in place.
2. Add the new `row-swatch` slot to `Row`, render an SVG `Dot` in the swatch slot for top-level rows.
3. Remove `.tree-row--tinted*` rules and `--ws-tint-*` tokens; add `.row-swatch` rules and the `:not(:first-child)` divider rule.
4. Switch `RepoNode` and `FlatWorkspaceNode` from `tint={color}` to `swatch={color}`; drop the `tint` prop from `Row`.
5. Visual check in `bun tauri dev`; verify with one workspace, three workspaces, expanded child rows, selected workspace, hover.
