# Working-Tree Status Indicators

## Why

SpecForge already surfaces specs that live uncommitted in worktrees. Worktrees
are auto-discovered (`RepoMonitor` reconciles the registry against
`git worktree list`), and the parser reads the **working tree** off disk — not
git objects — so a brand-new `openspec/changes/<id>/` that has never been
committed already appears in the tree as a `ChangeInstance` under its repo.

What is missing is *git awareness of that state*. SpecForge has **zero**
working-tree status detection today (`git status --porcelain` appears nowhere in
the codebase; the only `--porcelain` call is `worktree list`). A committed,
pushed spec and a brand-new untracked one render identically. The user cannot
tell, at a glance, which repos and which worktrees carry uncommitted work.

This change gives SpecForge two complementary signals, both derived from git:

- **Per-change spec commit state** — for each change instance (a worktree's copy
  of a change), whether its `openspec/changes/<id>/` directory is committed,
  has uncommitted modifications, or is entirely untracked (the "new spec that
  only exists uncommitted in a worktree" case). This is on-domain for a spec
  browser and stays live on the `openspec/`-scoped watcher events we already
  receive.
- **Per-repo dirty rollup** — whether *any* worktree of the repo has *any*
  uncommitted change (the familiar source-control "dirty" dot), rolled up onto
  the repo node, plus a distinct mark when the dirt includes uncommitted specs.

It deliberately does **not** touch the Dashboard. The Dashboard's
created/archived/throughput surfaces are mined from `git log`
(`change_lifecycle`) and are blind to uncommitted changes by construction;
making them filesystem-aware is a separate, larger change and is out of scope.

## What Changes

- A new `git::worktree_status` helper runs a single
  `git status --porcelain --untracked-files=all` per worktree (through the
  existing `git_command` chokepoint, so WSL keeps working) and yields **both**
  signals from one call: the whole-tree dirty bit, and a per-`openspec/changes/<id>/`
  classification.
- `ChangeInstance` gains a `spec_commit_state: SpecCommitState`
  (`Committed` | `Modified` | `Untracked`).
- `RepoView` gains `dirty: bool`, `dirty_worktrees: Vec<PathBuf>`, and
  `has_uncommitted_specs: bool`.
- The tree renders a per-instance commit-state chip next to the existing
  divergence chip, and two rollup marks on the repo node: a whole-repo dirty dot
  and a distinct "specs uncommitted" mark.
- Freshness for the whole-repo dirty signal: `RepoMonitor` adds a `.git/index`
  watch (catches stage/unstage/commit/checkout) and the app recomputes on
  window focus. No broad working-tree watching and no background poll — the
  spec-scoped signal is already live on existing events.

## Capabilities

- **Added:** `working-tree-status` — the per-worktree status computation, the
  `ChangeInstance`/`RepoView` fields, and the freshness contract.
- **Modified:** `spec-browser` — the tree renders the two indicators.

## Impact

- `crates/openspec-core` — `git.rs` (status helper + `SpecCommitState`),
  `repo_view.rs` (new fields, per-worktree status gathering in `compute_views`),
  `repo_monitor.rs` (`.git/index` watch), `types.rs`.
- `crates/openspec-app` — IPC plumbing + a focus-triggered recompute path.
- `src/` — `types.ts` mirror, `WorkspaceTree.tsx` rendering, CSS for the
  chips/dots.
