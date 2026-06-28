# Reduce Repo-Monitor Overhead

## Why

SpecForge's background CPU use scales with the number of registered repositories
and their git activity, not with what the user is doing. A live diagnosis of a
long-running instance (8 registered repos, ~62 worktrees) found it holding ~45
filesystem watchers and sustaining **~43% of a CPU core for two days** while
nominally idle.

Two design choices drive this, and neither is required by any user-visible
contract:

1. **Per-signal watchers.** `RepoMonitor::install` creates *four* independent
   `notify` debouncers per repository — `meta` (`.git/worktrees/`), `config`
   (`.git/config` + `origin/HEAD`), `refs` (`.git/refs` recursive), and `index`
   (`.git/index` + `.git/worktrees/` recursive). Eight repos = 32 git
   debouncers; add one `openspec/` watcher per workspace/worktree and the
   process runs ~45 FSEvents streams, each with its own debouncer thread and
   `FileIdMap`.

2. **Global recompute on every event.** Each repo's index/refs watcher calls
   `refresh_status_and_notify()` → `refresh_aggregated_view()` →
   `compute_views()`, which runs `git status --porcelain` for **every worktree
   of every registered repository** (`repo_view.rs`). So a single git write in
   *one* repo fans out to ~62 `git status` subprocesses across *all* repos. With
   normal multi-repo worktree churn this fires near-continuously.

The `Status Freshness` requirement only mandates that a repository's rollup
refresh within the debounce window after *its own* git events. The global fan-out
and the per-signal watcher multiplication are incidental implementation cost, not
contract.

## What Changes

- **Repo-scoped status recompute (#1 — the dominant win).** A git event observed
  for repository A recomputes only A's worktree status and merges the result into
  the cached aggregated view, instead of recomputing all repositories.
  Window-focus stays a full recompute.
- **One watcher per repository (#2).** Collapse the four per-repo debouncers into
  a single debouncer that watches all repo-level paths and dispatches by event
  path to the existing handlers (worktree reconcile, default-branch refresh,
  graph-changed, status refresh). Cuts ~45 watchers to ~21.
- **Coalesced refresh (#3).** A burst of git events for a repository (rebase,
  fetch, checkout) collapses to a single status recompute and a single
  commit-graph refresh per debounce window rather than one per raw event.
- **Read-only status (feedback-loop fix, found during implementation).** The
  status check is made read-only with `--no-optional-locks` so `git status` never
  rewrites `.git/index`. Without this, a status refresh writes the index, which
  the index watcher observes, triggering another status refresh — a tight loop
  (measured at ~128 reconciles for a single worktree add in tests). This is a
  prerequisite for the watcher consolidation to be safe.

No change to *what* the user sees — the commit-state chips, the per-repo dirty
dot, the commit-graph rail, and freshness timing are all preserved. This change
is purely about bounding the work performed per event and the number of OS
watchers held.

Out of scope: per-worktree (sub-repo) recompute precision; switching the bundled
binary to a release build (a packaging/runtime concern, not a code change here).

## Capabilities

- **Modified:** `working-tree-status` — `Status Freshness` gains a scoping clause
  that bounds per-event recompute to the originating repository.
- **Modified:** `workspace-registry` — new requirements bounding the repository
  watcher footprint to one watcher per repo and requiring coalesced refresh.

## Impact

- `crates/openspec-core`:
  - `repo_monitor.rs` — collapse the four debouncers into one, path-dispatched;
    keep idempotent install and clean teardown.
  - `watcher.rs` — add a repo-scoped `refresh_status_and_notify` /
    `refresh_aggregated_view` variant carrying a `repo_id`.
  - `repo_view.rs` — recompute a single repository's `RepoView` and merge it into
    the snapshot, preserving registration order.
  - `git.rs` — `worktree_status` runs with `--no-optional-locks` (read-only).
- No IPC/type changes expected; event names and payloads are unchanged.
- Tests: `repo_view.rs` (scoped recompute equals global recompute for the changed
  repo, and issues no `git status` for other repos), `repo_monitor.rs`/`watcher.rs`
  (one watcher per repo, idempotent install, coalescing).
