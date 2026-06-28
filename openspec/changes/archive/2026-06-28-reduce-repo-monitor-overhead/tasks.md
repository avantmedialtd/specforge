# Tasks — Reduce Repo-Monitor Overhead

## 1. Repo-scoped status recompute (#1)
- [x] 1.1 Factor `compute_views` so a single repository's `RepoView` can be built
  in isolation — `compute_repo_view(repo_id, &reg, &cache, default_branch_fn)` —
  reusing the existing per-worktree `worktree_status` gathering and the pure
  `build_repo_view`. (Extracted `build_repo_snapshot`; added pure
  `replace_repo_view` splice.)
- [x] 1.2 Add `WatcherManager::refresh_aggregated_view_for(repo_id)` in
  `watcher.rs`: recompute only that repo's view, splice it into `last_views` in
  place (preserving registration order), diff against the prior snapshot, return
  the events; fall back to the full recompute if the repo isn't in the snapshot
  yet.
- [x] 1.3 Add `refresh_status_for(repo_id)` (scoped sibling of
  `refresh_status_and_notify`) that emits the single `Updated` carrier for that
  repo. Keep the window-focus path on the full `refresh_status_and_notify`.
  (Repo-monitor wired to it in 2.2.)
- [x] 1.4 Tests: scoped recompute of repo A yields the same `RepoView` and the
  same `diff_views` output as a full recompute for A; scoped recompute of A issues
  **no** `git status` for B's/C's worktrees. (`compute_repo_view_*` unit tests +
  `scoped_status_refresh_recomputes_only_the_target_repo` integration test, which
  dirties both repos and proves B is left clean.)

## 2. One watcher per repository (#2)
- [x] 2.1 In `repo_monitor.rs`, replaced the four per-repo debouncers + tasks with
  a single `Debouncer` watching every repo-level path (`.git/worktrees/`
  recursive, `.git/config`, `.git/refs` recursive — which also covers
  `refs/remotes/origin/`, `HEAD`/`logs/HEAD`/`packed-refs`, `.git/index`) feeding
  one channel and one task. `RepoMonitor` now holds one `_debouncer` + one `task`.
- [x] 2.2 The processing task classifies each debounced batch by path
  (`RepoPaths::classify`) into the set of affected `Concerns` and dispatches —
  worktree reconcile, default-branch refresh, `GraphChanged`, **scoped** status
  refresh — each at most once per batch. Reconcile keys on the whole
  `.git/worktrees/` subtree (not the bare entry) so a `git worktree add` whose
  directory event arrives before `git worktree list` would report it is still
  reconciled by a later settled batch.
- [x] 2.3 Idempotent install preserved (`sync_repos` dedups by `RepoId`); `Drop`
  aborts the one task and drops the one debouncer; `.git/worktrees/` force-create
  kept.
- [x] 2.4 Tests: `one_repo_monitor_per_repo_and_idempotent` (two repos → two
  monitors, re-sync stays two) via new `repo_monitor_count()`; pure
  `RepoPaths::classify` unit tests cover every path class; integration
  `refs_change_emits_graph_changed` and `index_change_emits_a_status_update`
  confirm end-to-end routing through the single watcher.

## 3. Coalesced refresh (#3) + feedback-loop fix
- [x] 3.1 Coalescing asserted purely: `commit_burst_coalesces_to_one_status_and_one_graph`
  merges index+refs+HEAD+logs paths and checks the result is a single status +
  single graph concern (one dispatch each), independent of FSEvents timing.
- [x] 3.2 **Feedback-loop fix (discovered during 2.x):** the status check ran
  `git status`, which rewrote `.git/index` (and linked worktrees' indexes),
  re-triggering the index watcher → status → … in a tight loop (observed: 128
  reconciles for one worktree add). Fixed by adding `--no-optional-locks` to
  `git::worktree_status` so a read-only status never writes the index. Guarded by
  `worktree_status_does_not_write_the_index` (verified red without the flag). With
  the loop gone, `DEFAULT_DEBOUNCE` (200ms) needs no change.

## 4. Verify
- [x] 4.1 `cargo test -p openspec-core` green — 290 tests (201 lib + 89
  integration), including the previously parallel-flaky worktree-pickup test,
  which now passes under parallel load because 4→1 watchers cut FSEvents pressure.
  `cargo fmt --all --check` clean; `cargo clippy -p openspec-core` clean; full
  `cargo build --workspace` succeeds. (`specforge-web` clippy needs a built
  frontend `dist/`, unrelated to this change.)
- [ ] 4.2 Manual (recommended on a **release** build): register several repos,
  perform git ops in one, confirm only that repo recomputes (no `git status`
  storm) and idle CPU is materially lower than the pre-change baseline. Best run
  by the user against their real multi-repo setup that exhibited the issue.
