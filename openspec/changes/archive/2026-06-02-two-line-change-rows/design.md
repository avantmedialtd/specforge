# Design — Two-Line Change Rows

## Context

The workspace tree composes every row through one `Row` component (`src/components/WorkspaceTree.tsx:247`), styled by `.tree-row` (`src/App.css:393`):

```
.tree-row { display:flex; align-items:center; white-space:nowrap; overflow:hidden }
  [chevron 14px] [swatch 8px] [.row-label flex:1, overflow:hidden, text-overflow:ellipsis] [.row-meta flex-shrink:0]
```

`.row-label` is the single greedy, ellipsizing box. For a singleton change row, `labelForInstance(instance, changeName)` (`WorkspaceTree.tsx:618`) packs **two** inline children into it — the change name, then a `.row-branch` chip — so a long name evicts the branch past the clip boundary. For a multi-instance child, `labelForInstance(instance, null)` returns just the branch, and the child's progress/mtime live in the protected `.row-meta`, so children never clip their branch. Flat-workspace change rows (`FlatChangeNode`, `WorkspaceTree.tsx:748`) have the same single-line shape, with title in `.row-label` and meta in `.row-meta`.

The decision (captured via explore): **layout B, targeted scope** — give sole change rows two lines (change label on line 1, worktree identity + status on line 2); leave multi-instance children and every container/artifact/section/task row single-line.

## Decisions

### Decision 1 — "Sole change row" is the unit that goes two-line

The two-line treatment applies to exactly the rows that are the *only* row for their change and that today cram label + worktree label + status onto one line: the **flattened git singleton instance row** and the **flat-workspace change row**. These are the rows with the collision. Multi-instance child rows are excluded because their label is already nothing but the branch (no greedy name to fight) and their meta is already protected — a second line there would be density with no payoff. This is the user-selected "Targeted" scope.

### Decision 2 — Name owns line 1; worktree identity + status drop to line 2

Line 1 carries only the change's existing primary label (git singleton → change name; flat row → title ?? directory name), at the standard `--text-sm` tier, full width. This removes the ellipsis pressure that was clipping the branch — the name truncates only against the row edge, and only when it alone is too long. Line 2, at the dense `--text-2xs` muted meta tier, carries worktree identity on the leading edge and status on the trailing edge:

```
▾ collapse-tasks-node-by-default
     feat/collapse               ▰▰▱  2h  diverged
     └ branch ┘                   └ meter · age · divergence ┘
```

Identity-left / status-right mirrors the existing row instinct (label on the left, meta on the right), just stacked. This is layout **B** (name alone on line 1), chosen over layout **A** (subtitle under name, meta staying on line 1) because giving the name the whole top line is what makes long change names readable at all.

### Decision 3 — The branch leaves `.row-label`

The root-cause fix: the branch chip is removed from inside the greedy ellipsizing `.row-label` and rendered on line 2. `labelForInstance` for the singleton case stops appending the `.row-branch` chip; the singleton + flat render paths instead emit a two-line structure. Because line 2 is its own box, the worktree label no longer competes with the change name for space; it shows the branch (falling back to the worktree folder basename when there is no branch).

### Decision 4 — One row, two lines: a stacked content block, not two rows

The two lines remain a single interaction unit. The implementation keeps the chevron/swatch in a leading gutter and makes the row's content area a vertical stack (line 1 over line 2), so:

- the existing `paddingLeft: depth*12+4` indent and the `.tree-row--top-level` hairline are unaffected (sole change rows are never top-level);
- line 2 is indented to line 1's text origin (past the chevron + any swatch) by living inside the same content column the chevron precedes;
- selection (the 2px `--accent` `border-left` + `--accent-tint` wash) and `:hover` already target `.tree-row`, so they span both lines for free once both lines are children of the same `.tree-row`;
- the click handler stays on `.tree-row`, and the chevron's `stopPropagation` toggle is unchanged.

Concretely this is a small extension to `Row` (an optional line-2 `detail` slot that, when present, switches the content area from a single label to a stacked `label` over `detail`) rather than a parallel component, keeping the chevron/swatch/selection plumbing in one place. The exact element/grid shape is left to implementation; the contract is "two lines, one selectable row, line 2 indented to text origin."

### Decision 5 — The active indicator stays a child-row element

The active-instance dot renders only on `isPrimary && !isSingleton` (`WorkspaceTree.tsx:556`) — i.e. the primary of a *multi-instance* change, never a singleton or flat row. So sole change rows never carry it, and the new requirement omits it. This also cleanly scopes *Instance Row Chrome* (which keeps the active indicator) to the multi-instance child rows it actually describes.

### Decision 6 — No data, IPC, or persistence changes

`ChangeInstance` already carries `branch` and `worktreePath`; `ChangeData` already carries title/changeId. Nothing new crosses the IPC boundary, no settings key changes, and expansion/persistence is untouched (the chevron still toggles the artifact subtree via the same `toggle(nodeId, true)` path). This is a pure presentation change in `WorkspaceTree.tsx` + `App.css`.

## Spec surgery

- ***Instance Row Chrome*** (MODIFIED) — re-scoped to multi-instance **child** rows: branch-as-primary-label, single line, progress + mtime + divergence + active indicator in the trailing meta slot. A clause defers flattened singletons and flat rows to the new requirement, and corrects the old text's claim that branch is *every* instance row's primary label (it is the child behaviour; a singleton's primary label is its change name).
- ***Two-Line Sole-Change-Row Layout*** (ADDED) — defines the two sole-row types, the line-1 primary label (full width, unchanged content), the line-2 detail line (worktree identity leading / status trailing, muted dense tier, indented to text origin), the branch label (folder basename as fallback), the meta-only line 2 for flat rows, the exclusion of multi-instance children, and the single-interaction-unit / spanning-selection contract.

## Alternatives considered

- **Layout A — subtitle under name, meta stays on line 1.** Rejected per the explore decision: keeping meta on line 1 leaves the name sharing line 1 with the status cluster, so very long names still ellipsize. B gives the name the full top line.
- **Single-line priority inversion** (protect the branch in `.row-meta`, let the *name* ellipsize). Cheaper, no height change, but caps the worktree label at a narrow fixed width and sacrifices the name. Rejected: the goal is to *see* the worktree label without it fighting the name, which the stacked line achieves without truncating either.
- **Uniform scope** (every change/instance row, including multi-instance children, goes two-line) and **git-only scope** (exclude flat rows). Rejected in favour of Targeted: children have no collision to fix, and flat rows share the exact single-line cram that motivated the change.

## Risks / trade-offs

- **Vertical density.** Sole change rows grow ~1.4–1.5× taller (line 2 is the smaller `--text-2xs` tier, not a full second row). Bounded to leaf change rows; headers, parents, children, and artifact/section/task rows are unchanged, so a deep tree's structural rows don't grow.
- **Two label-content sources persist.** Git singleton line 1 shows the change name; flat line 1 shows the title. Not unified here (out of scope), so the two row types read slightly differently on line 1.
- **Selection/click must cover both lines.** Handled by keeping both lines inside one `.tree-row` so the existing selection/hover/click plumbing applies unchanged — but worth an explicit verification pass.

## Apply-time visual iteration (resolved)

The detail-line and emphasis treatment was settled live during apply, across several rounds:

1. **Plain muted subtitle** — the branch as plain `--text-muted` mono, no border. Too faint.
2. **Folder dropped as noise** — an earlier line showed `branch · folder`; the folder added no signal beyond the branch and just lengthened the line, so line 2 shows the branch alone (folder basename kept only as the no-branch fallback).
3. **Accent-indigo chip** — the branch as an outlined accent chip. Legible and on-grammar, but it pulled attention off the change name (the row's actual subject).
4. **Workspace-colour dot + tinted name** — moved colour onto the change name (a palette swatch dot plus tinting the name text in a contrast-safe shade). Rejected: coloured body text read as "off" and the contrast-safe shades went muddy (amber → brown).
5. **Final — workspace-colour rail + neutral-bold name + workspace-tinted chip.** A 2px rail in the workspace's palette colour runs down the inline-start border (the selection slot, overridden by selection); the change name carries the only emphasis on line 1 (heavier weight, plain high-contrast ink, no colour); and the branch is an outlined chip tinted to the same workspace colour. Colour codes the row to its workspace top-to-bottom (rail + chip) while the name stays the legible anchor.

**Contrast.** The palette swatch tokens are tuned as fills and run too light for ink, so chip text + border use new `--ws-text-*` variants — darkened for light scheme, lightened for dark — each computed to ≥4.6:1 on its background.

**Workspace-colour plumbing.** The owning `PaletteColor` is threaded from `RepoNode` / `FlatWorkspaceNode` through `LogicalChangeRow` to the change rows (it previously stopped at the top-level swatch). The rail reuses the `.tree-row` inline-start border; `.tree-row.selected` wins by specificity, so selection still shows the accent bar and the rail returns on deselect.
