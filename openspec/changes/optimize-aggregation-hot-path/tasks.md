# Tasks — Optimize the Aggregation Hot Path

Ordered so each group is independently landable and verifiable. Groups 1–3 are
near-trivial and remove most of the *compounding*; groups 4–7 attack the 576 ms
recompute itself. Group 7 (the `porcelain=v2` swap) is the highest-risk item and
deliberately lands last, behind its own equivalence tests.

## 1. Frontend refetch coalescing

- [ ] 1.1 Add a shared coalescing helper for the hooks: a `useCoalescedRefetch(fn)` that returns a `schedule()` which is idempotent within one microtask tick (`queueMicrotask`, guarded by a "scheduled" flag), tracks the in-flight promise, and on completion runs exactly one follow-up if `schedule()` was called while the request was outstanding. Document why microtask and not `setTimeout`/`rAF` (all events from one backend batch arrive in the same task; `rAF` would stall while the window is hidden, and this app's window is often closed to the tray)
- [ ] 1.2 Route all 8 listeners in `src/hooks/useWorkspaces.ts` through `schedule()` instead of calling `refreshViews()` directly; keep `onWorkspacePresentationUpdated` on the full `refresh()` path but coalesced the same way
- [ ] 1.3 Route all 4 listeners in `src/hooks/useDashboard.ts` through `schedule()` instead of calling `load()` directly; preserve the `cancelled` guard on unmount so a settled follow-up cannot `setState` after teardown
- [ ] 1.4 Correct the stale comments in both hooks ("The backend already debounces these events, so this isn't a hot loop") — the backend debounces *filesystem events into one batch*, it does not emit one event per batch; state what the coalescing actually guarantees

## 2. Coalesce the reconcile batch

- [ ] 2.1 Restructure `reconcile()` in `crates/openspec-core/src/repo_monitor.rs`: perform every `remove_workspace` / `add_workspace` first (emitting `WorkspaceRemoved` / `Updated` per affected worktree, as today), then call the aggregated recompute **once** after the loops and emit its derived events — replacing the two in-loop `refresh_aggregated_view()` calls at the current lines 302 and 319
- [ ] 2.2 Preserve the documented ordering guarantee: the aggregated snapshot is refreshed before any subscriber observes the derived events, and derived events follow the per-worktree raw events

## 3. Move recomputes off the async runtime

- [ ] 3.1 Wrap the recompute in `Inner::handle_events` (`crates/openspec-core/src/watcher.rs`) in `spawn_blocking`, matching the window-focus path in `crates/specforge/src/lib.rs:169` — carry over that call site's explanatory comment
- [ ] 3.2 Do the same for the git work in `repo_monitor`'s processing task: `git::default_branch` on the `default_branch` concern and the recompute inside `reconcile`, both of which currently run subprocesses directly on a tokio worker

## 4. Scope the file-change path to one repository

- [ ] 4.1 In `Inner::handle_events`, resolve the changed `WorkspaceFolder`'s `repo_id` from the registry (a lookup, not a git call) and dispatch to `refresh_aggregated_view_for(repo_id)`; fall back to the full `refresh_aggregated_view()` only when the workspace has no `repo_id` (flat, non-git workspace). The scoped path's existing not-in-snapshot fallback covers first appearance
- [ ] 4.2 Add a doc comment recording that this is the same bound `repo_monitor` already applies to git events, now extended to file-change events per the amended *Status Freshness* requirement

## 5. Gather-then-compute: no locks across git I/O

- [ ] 5.1 Split `build_repo_snapshot` in `crates/openspec-core/src/repo_view.rs` into a gather phase returning owned inputs (registry entries, per-worktree cached `Vec<ChangeData>`, archived stubs, resolved default branch) and a compute phase that takes those owned inputs and performs the git I/O
- [ ] 5.2 Restructure `Inner::refresh_aggregated_view` and `refresh_aggregated_view_for` in `watcher.rs` so the registry `MutexGuard` and cache `RwLockReadGuard` are dropped after the gather phase and before any git invocation; resolve `default_branch` during gather so `repo_monitors` is no longer locked underneath the other two
- [ ] 5.3 Merge under a short write lock, keeping `replace_repo_view`'s `false` → full-recompute fallback as the guard against a registry mutation landing mid-flight

## 6. Concurrent per-worktree git

- [ ] 6.1 Issue the per-worktree git calls concurrently in the compute phase using `std::thread::scope` (no new dependency), with the worker count capped at `min(available_parallelism, 8)`
- [ ] 6.2 Collect results into a pre-sized `Vec` indexed by registry position, never by completion order, so the output is byte-identical to the serial path
- [ ] 6.3 Confirm the concurrency sits *inside* the single outer `spawn_blocking` and does not nest tokio's blocking pool within itself; `openspec-core` must still compute views with no tokio runtime present (it is unit-tested that way)

## 7. One status invocation per worktree

- [ ] 7.1 Add a `worktree_branch_and_status` to `crates/openspec-core/src/git.rs` issuing a single `git status --porcelain=v2 --branch` (keeping `--no-optional-locks`, `core.quotepath=false`, `--untracked-files=all`, `--no-renames`) through the existing `git_command(GitAnchor, …)` chokepoint so the WSL backend applies unchanged
- [ ] 7.2 Write the v2 parser: `# branch.head <name>` → branch (`(detached)` → `None`, matching `current_branch`'s contract), `1`/`2`/`u`/`?` entry lines → the existing `WorktreeStatus` shape
- [ ] 7.3 Unit-test the v2 parser over recorded fixtures — clean, staged-dirty, unstaged-dirty, untracked, detached HEAD, quoted/non-ASCII paths — and assert equivalence with the v1 parser's output on each before removing v1
- [ ] 7.4 Replace the `current_branch` + `worktree_status` pair in the compute phase with the single call; remove the now-dead v1 status parser only once 7.3 passes

## 8. Memoize the local git identity

- [ ] 8.1 Memoize `git_identity` per `RepoId` in the `WatcherManager`'s inner state, replacing the 2 uncached `git config` spawns currently issued on every file-edit batch in `handle_events`
- [ ] 8.2 Invalidate the memo from `RepoMonitor`'s existing `default_branch` concern dispatch (which already fires on `.git/config` writes); document that global `~/.gitconfig` changes resolve on next app start, matching the existing `default_branch` staleness tolerance

## 9. Verification

- [ ] 9.1 Add a test git-command recorder that counts invocations per event, and assert: a file edit in repo A issues zero `status` invocations for repos B/C; a batch adding 3 worktrees performs exactly one recompute; a second `git_identity` read in the same batch issues zero config spawns
- [ ] 9.2 Add a non-blocking test: hold the cache write lock while a recompute is in flight and assert it is not blocked for the duration of the recompute's git I/O
- [ ] 9.3 Add a determinism test: concurrent and serial recomputes over the same fixture registry produce identical `Vec<WorkspaceView>`, worktree ordering included
- [ ] 9.4 `cargo test` (workspace) and `bun run build` (strict `tsc --noEmit` + vite bundle) both pass
- [ ] 9.5 `cargo clippy --workspace --all-targets` clean
- [ ] 9.6 Run the app against the real 12-repository / 17-worktree registry and confirm the proposal's latency table improves as predicted: save a spec file, `git commit`, and archive a change, each with the Dashboard open. Record the observed figures against the 576 ms / 639 ms baselines
- [ ] 9.7 Confirm no behaviour regression in the tray badge, notifications, and commit-graph refresh — all subscribe to the same event stream whose per-event emission is deliberately unchanged by this work
