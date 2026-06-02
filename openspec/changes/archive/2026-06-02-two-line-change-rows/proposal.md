# Two-Line Change Rows

## Why

Every row in the workspace sidebar is a single horizontal flex line (`.tree-row` in `src/App.css`, composed in `src/components/WorkspaceTree.tsx`): `[chevron] [swatch] [.row-label flex:1, ellipsis] [.row-meta flex-shrink:0]`. For a **singleton change row** — the common case — `labelForInstance` renders the branch as an inline chip *inside* `.row-label`, after the change name:

```
<span class="row-label">          // flex:1, overflow:hidden, text-overflow:ellipsis
  <span>{changeName}</span>           // greedy, comes first
  <span class="row-branch">{branch}</span>   // the worktree label — tail position
</span>
```

The branch therefore sits after a greedy name inside one ellipsizing box. The moment the change name is even moderately long, the box fills with the name, the ellipsis fires, and the branch chip is pushed past the clip boundary and **disappears entirely** — not truncated, gone. The worktree/branch label is the first thing sacrificed, exactly the label the user reports being unable to see.

Two contrasts confirm the diagnosis: the Repo header row shows its branch fine because `defaultBranch` lives in the protected `.row-meta` slot, and a multi-instance child row shows its branch fine because there the branch *is* the whole label. Only the singleton (and the equivalent flat-workspace) change row crams name + worktree label + status onto one line, and the name always wins.

This matters more now that `worktree-dev-slots` shipped: a change increasingly lives across several worktrees, so the branch/worktree is load-bearing identity rather than a garnish. The single-line grammar was designed before that, and the data model has outgrown the row.

## What Changes

- **Sole change rows render across two lines.** A change row that is the only row for its change — a flattened git singleton instance row, or a flat-workspace change row — gets the change name on line 1 (full width, no longer truncated) and a subordinate detail line beneath it.
- **The change name anchors the row.** Line 1 shows the change name in a slightly heavier weight, so the change — not the branch — is what the eye lands on.
- **The detail line carries the branch + status.** Line 2 shows the branch (folder basename fallback for detached HEAD; `changeId` for non-git flat rows) on the leading edge and status on the trailing edge (progress meter or ✓, relative modification time, divergence label when present).
- **Colour ties each change to its workspace.** A workspace-colour **rail** runs down the row's inline-start border — the same slot the selection bar uses, which overrides it when the row is selected — and the **branch chip** is an outlined chip tinted to the same workspace palette colour (a contrast-safe shade). So a change reads as belonging to its workspace top-to-bottom, without colouring the name text.
- **The branch leaves the greedy label.** The root cause — branch rendered as an inline tail inside the flex `.row-label` — is removed; the branch can no longer be clipped by a long change name.
- **Scope is targeted; everything else is untouched.** Multi-instance child rows stay single-line (their branch is already their whole label and their meta is already protected). Repo/workspace headers, multi-instance disclosure parents, and all artifact/section/task rows stay single-line. Only the two sole-change-row types change.

## Capabilities

### Modified Capabilities

- `spec-browser`: *Instance Row Chrome* is re-scoped to multi-instance **child** rows (where branch-as-primary-label is the correct, existing behaviour), and a new *Two-Line Sole-Change-Row Layout* requirement defines the two-line presentation for flattened singleton instance rows and flat-workspace change rows.

## Impact

- **Spec:** one requirement modified and one added in `openspec/specs/spec-browser/spec.md`. The modification also corrects a latent inaccuracy: the old *Instance Row Chrome* claimed branch was every instance row's primary label, but the code already renders the change name as the primary label for singletons (`labelForInstance(instance, changeName)`), reserving branch-as-primary for multi-instance children.
- **Code:** `src/components/WorkspaceTree.tsx` (a two-line render path for sole change rows; `labelForInstance` reworked so branch leaves `.row-label`; the owning workspace's `PaletteColor` threaded from `RepoNode`/`FlatWorkspaceNode` down to the change rows for the rail + chip tint) and `src/App.css` (stacked two-line layout; line-2 meta tier; the workspace-colour rail classes; the workspace-tinted branch-chip classes; and contrast-safe `--ws-text-*` palette tokens). No multi-instance child, header, parent, or artifact-row markup changes.
- **No Rust, IPC, settings-schema, or persistence changes.** No type changes cross the boundary; `ChangeInstance` already carries `branch`/`worktreePath`, and the workspace `PaletteColor` (`RepoView.color` / flat workspace colour) already crosses for the top-level swatch.
- **Behaviour delta for users:** singleton and flat change rows become ~1.4–1.5× taller (line 2 is the smaller meta tier); the worktree label is always visible regardless of change-name length; and each change is colour-coded to its workspace via the rail + branch chip. Multi-instance trees and the rest of the sidebar are visually unchanged.

## Resolved / Out of Scope

- **Detail-line styling — RESOLVED through apply-time iteration** (see `design.md`): the branch is an outlined chip tinted to the workspace colour, the change name carries the only emphasis on line 1 (heavier weight, no colour), and a workspace-colour rail codes the row to its workspace. A plain muted subtitle and an accent-coloured chip were both tried and rejected (the latter pulled attention off the change name).
- **Line-1 label content is unchanged and not unified here.** A git singleton's line 1 keeps showing the logical change name; a flat-workspace row's line 1 keeps showing the proposal title (falling back to its directory name). Reconciling that pre-existing difference is explicitly out of scope.
