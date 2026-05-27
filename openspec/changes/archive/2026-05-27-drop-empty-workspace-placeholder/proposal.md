# Drop empty workspace placeholder

## Why

Every top-level workspace/repo row in the tree already carries a count badge that displays the active-change count, including `0`. Underneath, when that count is zero, we additionally render an italic faint child row reading "no active changes" — the same signal said twice, the verbose copy being the less honest one. Right next to it sits a `{totalActiveInstances === 0 && null}` block that has rendered nothing since it was written; the comment in-source already admits the badge does the job. The empty state should rest on the badge alone, and an empty top-level row should not pretend to be expandable.

## What Changes

- Remove the `"no active changes"` placeholder child row from `RepoNode` in `src/components/WorkspaceTree.tsx`.
- Remove the matching placeholder child row from `FlatWorkspaceNode` in the same file.
- Remove the adjacent dead `{totalActiveInstances === 0 && null}` block and the now-unused `totalActiveInstances` local.
- When a top-level row (a `RepoNode` or a `FlatWorkspaceNode`) has no active changes, render it as a leaf row (no disclosure chevron, no `onToggle`) so the chrome matches the content. The row remains selectable and continues to display its count badge at `0`.

## Capabilities

### New Capabilities
<!-- None. -->

### Modified Capabilities
- `spec-browser`: Adds a scenario under "Workspace Tree Hierarchy" that pins down how a top-level workspace or Repo group renders when it has no active changes (leaf row, `0` badge, no placeholder child).

## Impact

- Frontend: `src/components/WorkspaceTree.tsx` only.
- No Rust changes. No IPC contract changes. No CSS changes (the existing `.row-empty` style becomes unused; safe to leave for now — no other rendering path uses it and removing it is a follow-up if any).
- No tests touch the placeholder string today, so no test updates are forced. A small assertion that empty rows render without a chevron would be a nice-to-have but the codebase has no React test infra.
- Pre-existing wrinkle untouched: a `collapsed` override for a now-empty node lingers in the persisted set; behaviour is identical to today's and is out of scope.
