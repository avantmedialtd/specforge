# Design — Optimize the Aggregation Hot Path

## Measurements this design is calibrated against

Taken on the developer's real registry (12 repositories, 17 worktrees), best-of-5 unless noted:

| Operation | Measured |
|---|---|
| Per-spawn floor (any git command) | **25–33 ms** |
| `git config --get user.name` (trivial work) | 28 ms |
| `git status --porcelain -uall` (full tree scan) | 33 ms |
| Full recompute, serial (today) | **576 ms** |
| Same calls, concurrent | **179 ms** |
| `getDashboard()` git mining | 639 ms |

The first two rows are the whole design rationale: a command that reads one line of a config file costs the same as one that walks the entire working tree. **Spawn count is the cost model; bytes moved is noise.** Every decision below reduces spawn count, removes spawns from the critical path, or overlaps them.

## Decision 1 — Scope by resolving the edited workspace's repository

`handle_events` receives the `WorkspaceFolder` that changed. The registry already stores `repo_id` per entry, so the scope resolution is a registry lookup, not a git call.

```
handle_events(workspace, events)
  ├─ repo_id = registry.entry(workspace).repo_id
  ├─ Some(id) → refresh_aggregated_view_for(id)      ← 1 repo of git I/O
  └─ None     → refresh_aggregated_view()            ← flat (non-git) workspace
```

`refresh_aggregated_view_for` already falls back to the full recompute when the repo is absent from the snapshot (first appearance), so the new call site inherits correct first-run behaviour for free.

**Why this is low-risk:** the scoped path is not new code. `repo_monitor` has routed every git event through it since the *Status Freshness* scoping requirement landed, and the existing scenario "A git event in one repository does not recompute others" already tests it. This change adds a second caller.

## Decision 2 — Gather-then-compute, so no lock is held across a subprocess

Today:

```rust
let new_views = {
    let reg = registry.lock()?;          // ─┐
    let cache = self.cache.read()?;      //  ├─ held across ~576 ms of git
    compute_views(&reg, &cache, …)       // ─┘
};
```

The restructure splits `build_repo_snapshot` into three phases:

```
  ┌─ PHASE 1: gather (locks held, no I/O — microseconds) ─┐
  │  registry entries → owned Vec<RegistryEntry>          │
  │  cache changes    → owned Vec<ChangeData>             │
  │  default branches → owned Option<String>              │
  └───────────────── guards dropped here ─────────────────┘
  ┌─ PHASE 2: git I/O (no locks, parallel, off-runtime) ──┐
  │  per repo:     worktree_list                          │
  │  per worktree: status --porcelain=v2 --branch         │
  └───────────────────────────────────────────────────────┘
  ┌─ PHASE 3: merge (short write lock — microseconds) ────┐
  │  diff_views(last, next); store next                   │
  └───────────────────────────────────────────────────────┘
```

Phase 1 must clone the inputs it needs. That is a real cost — but it is bounded by the number of *registered entries*, not by git work, and it buys the removal of a 576 ms lock hold. The clone is on the order of tens of microseconds against hundreds of milliseconds saved.

**Consistency note:** dropping the guards means the registry could change between phase 1 and phase 3. This is already true of the current design in a weaker form (the watcher and registry mutate independently), and the merge in phase 3 is a whole-snapshot replace guarded by `replace_repo_view`, which returns `false` and triggers a full recompute when the repo is no longer present. A registry mutation mid-flight therefore degrades to a recompute, never to a corrupt snapshot.

## Decision 3 — Bounded concurrency via `std::thread::scope`

Per-worktree git calls are independent OS processes. They are issued from a scoped thread pool with a worker cap:

- **No new dependency.** `std::thread::scope` (stable since 1.63) borrows the input slice directly, so no `Arc`/`clone` of the gathered inputs is needed.
- **Why not rayon:** a work-stealing pool is overkill for ≤ 32 blocking subprocess waits, and the crate deliberately keeps its dependency surface thin.
- **Why not `spawn_blocking` per worktree:** the whole recompute already runs inside one `spawn_blocking`; nesting the runtime's blocking pool inside itself risks starving it, and `openspec-core` should not require a tokio runtime for what is pure sync logic (it is unit-tested without one).
- **Worker cap:** `min(available_parallelism, 8)`. Above 8 the measurement showed no further gain — the calls are I/O-bound on process creation, and an unbounded fan-out on a 20-worktree registry would spawn 40 processes at once.

**Determinism:** results are collected into a pre-sized `Vec` indexed by the worktree's registry position, never by completion order. The output is byte-identical to the serial path — which is the property the spec scenario asserts.

**WSL:** on Windows each spawn crosses the 9P boundary through `wsl.exe`, so parallelism helps *more* there, but an unbounded fan-out of `wsl.exe` processes would be pathological. The same cap applies; no WSL-specific branch is introduced, since everything routes through the existing `git_command` chokepoint.

## Decision 4 — One `status --porcelain=v2 --branch` instead of two spawns

`git status --porcelain=v2 --branch` emits branch headers and entry lines from one invocation:

```
# branch.oid <sha>
# branch.head <branch-name>        ← replaces `git branch --show-current`
1 .M N... <path>                   ← entries, as today
? <path>                           ← untracked
```

This halves W's coefficient (58 ms → ~33 ms per worktree). Detached HEAD reports `# branch.head (detached)`, which maps to the existing `branch: None`, matching `current_branch`'s current contract.

**This is the highest-risk item in the change** — it swaps the format underlying both the dirty rollup and the per-change classification. Mitigation: the v2 parser lands with its own unit tests over recorded fixtures (clean, dirty-staged, dirty-unstaged, untracked, detached HEAD, renamed-disabled, quoted paths) *before* the v1 parser is removed, and the existing `working-tree-status` scenarios run against it.

`--no-optional-locks` and `core.quotepath=false` carry over unchanged — the former is spec-mandated (a status that writes the index would re-trigger the index watcher and loop).

## Decision 5 — Memoize `git_identity` against the existing config watch

`git_identity` costs 2 spawns per file-edit batch to read values that change approximately never. It is memoized per `RepoId` in the `WatcherManager`'s inner state.

Invalidation is free: `RepoMonitor` already watches `.git/config` and classifies it as the `default_branch` concern. That dispatch clears the identity entry for the repo alongside refreshing the branch. Global (`~/.gitconfig`) changes are not watched — a stale global identity resolves on next app start, which matches the existing tolerance for `default_branch` staleness.

## Decision 6 — Frontend coalescing at a zero-delay task boundary

Both hooks funnel every listener through one coalescing refetch:

```
event ─┐
event ─┼─▶ schedule (setTimeout(fn, 0), idempotent) ─▶ one refetch
event ─┘
```

- **`setTimeout(fn, 0)`, not `queueMicrotask`/rAF.** The original design called for a microtask leading edge on the premise that "Tauri delivers all events from one backend batch in the same task." **That premise is false, verified against Tauri 2.11.2**: each event listener callback arrives via its own `evaluateJavaScript` injection with its own microtask checkpoint, so event #1's `queueMicrotask` runs to completion *before* event #2's script even starts — a microtask leading edge does not coalesce same-batch events at all (an N-event batch still triggers up to N refetches). A macrotask boundary does: every event from one backend-debounced batch lands within the same task-queue window well inside a `setTimeout(fn, 0)` delay, so it reliably catches the whole batch, at the cost of a sub-millisecond-to-a-few-ms delay (browsers clamp nested zero-delay timers, but this is never deeply nested) that is not observable against the freshness contract below. `rAF` would stall refetches entirely when the window is hidden — bad for a tray-driven app whose window is often closed.
- **In-flight de-duplication.** A refetch that is already running sets a "refetch again when done" flag rather than starting a second concurrent request, so overlapping batches cannot stack `getDashboard()` calls. The *first* call into a hook's lifecycle must also go through this same scheduler, not a direct call — otherwise an event arriving during that first call's round trip starts a second, genuinely concurrent request outside the in-flight tracking, and a stale response can resolve after (and overwrite) a fresher one.
- **The debounce stays in the backend.** No meaningful timer is introduced on the frontend; the freshness contract (`within the debounce window`) is unaffected by a delay several orders of magnitude below the debounce window itself.

## Alternatives considered

**Replace shelling-out with `gix`/`libgit2`.** This would eliminate the spawn floor entirely — the single largest cost — rather than working around it. Rejected *for this change*, not on merit: the WSL backend fundamentally depends on invoking the distribution's own `git` through `wsl.exe`, so an in-process implementation would need a second code path for Windows/WSL, and `git status` semantics (gitignore, worktree config, submodules) are subtle enough that a reimplementation is its own multi-change project. Worth revisiting once this change establishes the concurrency and gather-then-compute structure that would host it.

**Cache `worktree_status` and serve stale on file edits.** Rejected: it directly contradicts the *Status Freshness* contract, and the freshness is the feature. The fix is to make the recompute cheap, not to skip it.

**Lengthen the debounce window.** Rejected: it delays the cost without reducing it, and degrades the responsiveness the app is built around.

**Emit one coalesced event from the backend instead of coalescing on the frontend.** Tempting, but the distinct events (`ChangeAdded`, `LogicalChangeArchived`, `InstanceRemoved`, …) are a deliberate part of the IPC contract that other consumers — the tray badge, notifications — discriminate on. Collapsing them backend-side would break those. Coalescing belongs in the consumer that refetches everything regardless of event kind.

## Verification strategy

The wins are latency, so the tests assert **structure**, not wall-clock (which is unstable in CI):

- **Invocation counting.** A test git-command recorder counts spawns per event. Assertions: a file edit in repo A issues zero `status` invocations for repos B/C; a batch adding 3 worktrees issues one recompute's worth, not three; two `git_identity` reads in one batch issue at most 2 config spawns on first call and 0 after.
- **Non-blocking.** A test acquires the cache write lock while a recompute is in flight and asserts it is not blocked for the recompute's duration.
- **Determinism.** Parallel and serial recomputes over the same fixture registry produce byte-identical `Vec<WorkspaceView>`.
- **Parser equivalence.** v1 and v2 parsers produce identical `WorktreeStatus` across the fixture corpus, until v1 is removed.
- **Manual.** Run the app against the real 12-repo registry and confirm the interaction latencies in the proposal's table drop as predicted.
