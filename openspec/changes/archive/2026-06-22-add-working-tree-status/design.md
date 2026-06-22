# Design — Working-Tree Status Indicators

## Context

The aggregation pipeline already produces, per git repository, a `RepoView`
holding `LogicalChange`s, each with one `ChangeInstance` per worktree that
contains the change. `compute_views` (in `repo_view.rs`) is where the per-worktree
git I/O lives today — it already calls `git::current_branch` once per worktree and
`git::worktree_list` once per repo. The pure `aggregate`/`build_repo_view`
functions stay I/O-free; divergence's filesystem reads are the one exception and
set precedent for content I/O during the build.

Two facts shape the design:

1. The per-workspace file watcher is **scoped to `openspec/`** (recursive). It
   fires on spec edits but never on non-spec source edits.
2. `RepoMonitor` already watches `.git/refs`, `.git/HEAD`, `.git/config`, and
   `.git/worktrees/`, emitting `GraphChanged` / reconciling worktrees. It does
   **not** watch `.git/index`.

## Two signals, two levels

The signals live at different levels because a worktree is only visible in the
tree *when it contains a change*. A dirty worktree with no specs has nowhere to
render except the repo rollup.

```
RepoView                      ← signal 1: whole-repo dirty (rollup)
  dirty: bool                    any worktree has any uncommitted file
  dirty_worktrees: [PathBuf]     which worktrees (tooltip / future per-wt row)
  has_uncommitted_specs: bool    any instance's spec_commit_state != Committed
  active: [LogicalChange]
    instances: [ChangeInstance]
      spec_commit_state         ← signal 2: this worktree's copy of THIS change
        Committed | Modified | Untracked
```

`SpecCommitState::Untracked` is the headline clause-1 case: a spec directory git
has never seen, existing only as uncommitted files in a worktree.

## One call, both signals

Per worktree, a single command yields everything:

```
git -C <worktree> status --porcelain --untracked-files=all
```

Routed through `git::git_command(GitAnchor::Worktree(path), &[...])` so the WSL
backend (`wsl.exe -d <distro> git …`) is used transparently on Windows.

Parsing rules:

- **whole-repo dirty** = output has at least one line.
- For each change instance in the worktree, take the lines whose path is under
  `openspec/changes/<id>/` (porcelain path field; handle the rename `->` form
  and quoted paths) and classify with this precedence:
  1. any line with a tracked status code (`M`, `A`, `D`, `R`, `C`, in index or
     worktree column) → **Modified**
  2. else any `??` line → **Untracked**
  3. else → **Committed**

  Precedence matters because a directory can hold both a tracked-but-modified
  file and a new untracked file; "Modified" is the stronger, more actionable
  signal.

A worktree with no openspec changes contributes only to signal 1.

## Where it runs

`compute_views` gathers one `worktree_status` per worktree alongside the existing
`current_branch` call, builds a `PathBuf -> WorktreeStatus` map, and threads it
into `WorktreeSnapshot`. The pure `build_repo_view` consumes the precomputed
status to set `spec_commit_state` per instance and the three `RepoView` rollup
fields — keeping the aggregator I/O-free and unit-testable exactly as it is now.

Cost: one `git status` per worktree per aggregation, on top of the
`current_branch` call already made there. `git status` on a small repo is cheap
and bounded by worktree count. If profiling shows pressure, the per-worktree
status can be memoized against the worktree's `.git/index` mtime; not part of
v1.

## Freshness (whole-repo signal)

The spec signal is already live: spec edits fire the `openspec/` watcher →
re-aggregate → recompute. The whole-repo signal needs more, because non-spec
edits are invisible to that watcher.

Chosen mechanism (cheapest adequate):

- **`.git/index` watch** added to `RepoMonitor` (a fourth debounced watcher
  beside meta/config/refs). Catches `git add`/reset, commit, checkout, merge —
  i.e. every staging and history move. On a debounced batch it triggers a
  re-aggregation (emits a cache event the IPC layer already turns into a view
  refresh).
- **Window-focus recompute**: the app recomputes views when the main window
  regains focus, covering the "edited a file in my editor, tabbed back" path
  that touches neither the index nor any spec file.

Explicitly rejected:

- **Watching the whole working tree** — defeats the `openspec/`-scoped design,
  noisy, and pulls in `target/`, `node_modules/`, etc.
- **Background poll** — the WSL backend already polls for its own reasons, but a
  general timer is unnecessary background work given index-watch + focus covers
  real workflows. (Left as a future option if users report a stale dot.)

Consequence to document in the spec: a purely-unstaged non-spec edit may not flip
the repo dot until the next focus or git event. This is an accepted, stated
limitation, not a bug.

## Visual language

- **Repo node** (`RepoNode`): today renders `swatch · name · branch · count`.
  Add, after the count, a whole-repo **dirty dot** (one accent) and — when
  `has_uncommitted_specs` — a distinct **specs-uncommitted mark** (e.g. an
  asterisk/pencil glyph) so "dirty source file" reads differently from
  "uncommitted spec". Both suppressed when clean. Tooltip lists
  `dirty_worktrees`.
- **Instance row**: already carries a status line with the branch chip and the
  divergence chip. Add a commit-state chip — shown only for `Modified` /
  `Untracked` (a `Committed` instance shows nothing, to avoid chip noise).
- **Flat (non-git) workspaces**: no `RepoId`, so no dots — unchanged.

## Edge cases

- **Archived instances** (`is_archived_here`): status is still computed but the
  archive lives under `openspec/changes/archive/<id>/`; the prefix match keys on
  that path. Archived rows are browsed in the Archive view, not the active tree,
  so chips there are low-value; v1 may skip the chip on archived instances.
- **Detached HEAD / no branch**: status is independent of branch; works.
- **Missing/!git or git binary absent**: `worktree_status` returns a
  "clean/unknown" value; no dots, no chips — graceful degradation, matching how
  the codebase already treats absent git.
- **Submodules / nested git**: `git status` from the worktree root reports
  submodule summary lines; treat any such line as whole-repo dirty but never as
  a spec change (paths won't be under `openspec/changes/`).
- **Renames** (`R old -> new`): use the destination path for the prefix test.

## Types (mirrored by hand in `src/types.ts`)

```rust
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SpecCommitState { Committed, Modified, Untracked }
```

```ts
export type SpecCommitState = "committed" | "modified" | "untracked"
```

`RepoView` and `ChangeInstance` gain the fields above with
`#[serde(rename_all = "camelCase")]` already in force on those structs.
