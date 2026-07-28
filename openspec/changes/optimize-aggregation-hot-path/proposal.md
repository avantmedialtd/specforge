# Optimize the Aggregation Hot Path

## Why

Every file save under `openspec/changes/` triggers a **full cross-repository git sweep**. On a real 12-repository / 17-worktree registry this was measured at **576 ms**, and it runs in the worst possible place: synchronously on a tokio worker thread, with the registry `Mutex` **and** the workspace cache `RwLock` held for the entire duration.

The cost is not git's work — it is process spawn. Measured on this machine, `git config --get user.name` (which reads one line of a text file) costs the same ~25–30 ms as a full `git status --porcelain -uall`. The optimization lever is therefore *the number of spawns*, not the bytes each one moves.

The aggregation itself is `R × worktree_list + W × (current_branch + worktree_status)` — for R=12, W=17 that is 12 + 34 = **46 sequential git processes per file save**.

Three separate defects stack in that one call:

1. **It is unscoped.** Editing a file in repository A issues `git status` for repositories B…L as well. The repo-scoped path already exists (`refresh_aggregated_view_for`) and the `working-tree-status` *Status Freshness* requirement already mandates scoping — but only for **git** events. File-change events were never specified, so they default to the expensive path.
2. **It holds two locks across subprocess I/O.** `Inner::refresh_aggregated_view` keeps the registry guard and cache read guard alive for all 576 ms, so any concurrent cache write (another workspace's watcher) or command-handler read blocks behind it. A third lock (`repo_monitors`) is acquired *inside* that scope.
3. **It is not on the blocking pool.** `handle_events` is `tokio::spawn`'d, so the sweep stalls a runtime worker. The window-focus handler wraps the identical call in `spawn_blocking` with a comment explaining exactly why — the technique is present in the codebase and simply did not propagate to this sibling call site.

A fourth multiplier sits above all of it: **the frontend does not coalesce.** `useWorkspaces` registers 8 listeners and `useDashboard` 4, each unconditionally refetching. One debounced batch legitimately emits several events (`ChangeArchived` + `Updated` + `LogicalChangeArchived` + …), so a single archive fires `getDashboard()` twice. The hooks' comments state "the backend already debounces, so this isn't a hot loop" — that conflates debouncing *filesystem events into one batch* with emitting *one event per batch*, which the backend deliberately does not do.

Compounded, on the measured registry:

| Interaction (Dashboard open) | Git subprocess work |
|---|---|
| Save a spec file | ≈ 1.2 s |
| `git commit` | ≈ 1.4 s |
| Archive a change | ≈ 1.85 s |

Finally, `reconcile` calls the **full** recompute inside its per-worktree loop, so a batch that adds N worktrees performs N full sweeps rather than one.

## What Changes

- **Scope the file-change path.** `handle_events` resolves the edited workspace's `RepoId` and uses the existing repo-scoped recompute, falling back to the full recompute only when the repo is absent from the snapshot (the first-appearance case the scoped path already handles). This extends the *Status Freshness* scoping guarantee from git events to file-change events — the same optimization, applied to its sibling path.
- **Release locks before git I/O.** `compute_views` / `compute_repo_view` are restructured so the registry and cache guards are used to *gather inputs* (entries, cached changes, default branches) into owned values, then dropped before any subprocess runs. Git I/O executes lock-free; results are merged back under a short write lock.
- **Move the recompute off the runtime.** The recompute in `handle_events` and in `repo_monitor`'s reconcile task runs on `spawn_blocking`, matching the window-focus path.
- **Hoist the recompute out of `reconcile`'s loop.** Watcher add/remove for every added and removed worktree happens first; a single recompute and a single derived-event emission follow. N worktree changes produce one sweep.
- **Run per-worktree git concurrently.** The per-worktree `current_branch` + `worktree_status` calls are independent processes; they are issued in parallel with a bounded worker count. Result ordering stays registry-deterministic, so output is byte-identical to the serial path. Measured on the real registry: 576 ms → **179 ms (3.2×)**.
- **Merge two spawns into one.** `current_branch` + `worktree_status` are replaced by a single `git status --porcelain=v2 --branch` per worktree, which returns branch and status together — halving W's coefficient.
- **Cache the local git identity.** `git_identity` (2 spawns, uncached, on every file-edit batch) is memoized per repository and invalidated by the existing `.git/config` watch, which `RepoMonitor` already classifies as a `default_branch` concern.
- **Coalesce frontend refetches.** `useWorkspaces` and `useDashboard` collapse all events arriving in the same microtask tick into a single refetch, with the in-flight request de-duplicated so overlapping batches cannot stack.

Out of scope: the unbounded `change_lifecycle` history walk (482 ms of the Dashboard's 639 ms). It is an append-only-data caching problem with its own invalidation design, and lands as a separate change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `working-tree-status`: extends the *Status Freshness* scoping guarantee to cover `openspec/`-scoped file-change events (previously specified for git events only); adds a non-blocking contract stating the recompute MUST NOT serialize concurrent cache reads or writes for its duration; and sanctions concurrent per-worktree status invocations while requiring the result be identical to a serial recompute.
- `workspace-registry`: extends *Coalesced Repository Refresh* so a single debounced batch that adds or removes multiple worktrees produces at most one aggregated recompute, rather than one per affected worktree.
- `dashboard`: bounds *Reactive Dashboard Updates* to **at most one** refresh per debounced batch — the existing requirement mandates that a refresh happens, but never that it happens once.

## Impact

- **Rust + frontend.** No IPC type changes, no schema changes, no user-visible behaviour change other than latency.
- `crates/openspec-core/src/watcher.rs`: `handle_events` scoping + `spawn_blocking`; `Inner::refresh_aggregated_view*` lock restructuring.
- `crates/openspec-core/src/repo_view.rs`: `build_repo_snapshot` gathers inputs before dropping guards; per-worktree git parallelized; `compute_views` / `compute_repo_view` signatures take owned inputs.
- `crates/openspec-core/src/git.rs`: new combined branch+status invocation (`--porcelain=v2 --branch`) and its parser; `git_identity` memoization.
- `crates/openspec-core/src/repo_monitor.rs`: reconcile loop hoisted to a single recompute; git I/O off the runtime thread.
- `src/hooks/useWorkspaces.ts`, `src/hooks/useDashboard.ts`: microtask coalescing + in-flight de-duplication.
- **Risk concentrates in the `porcelain=v2` parser swap** — it changes the format the dirty rollup and per-change classification are derived from. The existing `working-tree-status` scenarios plus new parser unit tests cover it; the v1 parser is removed only once v2 passes them.
- **Windows/WSL**: all new git calls route through the existing `git_command(GitAnchor, …)` chokepoint, so the `wsl.exe` backend applies unchanged. Bounded concurrency matters more there (each spawn crosses the 9P boundary); the worker cap keeps the fan-out from multiplying `wsl.exe` processes.
