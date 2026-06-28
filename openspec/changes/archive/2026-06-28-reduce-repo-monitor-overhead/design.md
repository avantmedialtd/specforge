# Design — Reduce Repo-Monitor Overhead

## Context

The relevant pipeline today:

```
A git write in ANY one repo (.git/index, .git/worktrees/, .git/refs, .git/config)
        │
        │   45 notify watchers feed this: 4 per repo × 8 repos (meta+config+refs+index)
        │   + ~13 workspace/worktree openspec/ watchers
        ▼
  refresh_status_and_notify()              ← fired by EACH repo's index/refs watcher
        ▼
  refresh_aggregated_view() → compute_views()
        ▼
  git status --porcelain  ×  EVERY worktree of EVERY repo   ← ~62 subprocesses
        ▼                     (repo_view.rs, loop over all entries)
  emit cache-updated → frontend refetches ALL views
```

Key facts that constrain the design:

- `compute_views(&reg, &cache, default_branch_fn)` (in `repo_view.rs`) is the only
  place per-worktree git I/O happens during a build. It calls `git::worktree_status`
  once per worktree and `git::current_branch`/`worktree_list` per repo. The pure
  `aggregate`/`build_repo_view` functions stay I/O-free.
- `WatcherManager::refresh_aggregated_view()` recomputes the **entire**
  `last_views` snapshot, diffs against the prior snapshot (`diff_views`), and
  returns the diff events. Callers emit those after the raw cache events to
  preserve ordering (`Updated → LogicalChangeAdded/InstanceAdded/…`).
- `RepoMonitor` already keys monitors by `RepoId` in `WatcherManager::sync_repos`,
  with idempotent install (`monitors.contains_key`) and `Drop`-based teardown.
- `CacheEvent::GraphChanged { repo_id }` is already repo-scoped; only the
  working-tree status path fans out globally.

## #1 — Repo-scoped status recompute

The fix is to make the recompute proportional to the repository that changed.

`refresh_aggregated_view` becomes two paths:

- `refresh_aggregated_view()` — unchanged full recompute. Still used by startup
  populate and the window-focus refresh (focus legitimately wants everything
  re-checked).
- `refresh_aggregated_view_for(repo_id)` — recompute only `repo_id`'s `RepoView`
  and splice it into the snapshot:

```
fn refresh_aggregated_view_for(&self, repo_id) -> Vec<CacheEvent>:
    reg + cache locked as today
    new_repo_view = compute_repo_view(repo_id, &reg, &cache, default_branch_fn)
        # runs git status ONLY for repo_id's worktrees
    let mut next = last_views.clone()
    replace the WorkspaceView::Repo entry whose repo_id matches (same index →
        registration order preserved); if absent, fall back to a full recompute
    events = diff_views(&last_views, &next)
    *last_views = next
    events
```

`compute_repo_view` is a narrowed `compute_views`: filter the registry/cache to
`repo_id`'s worktrees, run the same per-worktree status gathering, and build the
one `RepoView` via the existing pure `build_repo_view`. Factor the shared body so
the scoped and global paths cannot drift.

`refresh_status_and_notify` gains a scoped sibling `refresh_status_for(repo_id)`
that calls `refresh_aggregated_view_for(repo_id)` and emits the single `Updated`
carrier (same as today, but the carrier is `repo_id`'s main worktree). The
repo-monitor index/refs handlers pass their `repo_id`; the window-focus path
keeps calling the full `refresh_status_and_notify`.

**Equivalence is the correctness bar:** the merged snapshot after a scoped
recompute of A MUST equal what a full recompute would have produced (A’s view
re-derived, every other view byte-identical because nothing else changed). This
is directly testable.

## #2 — One watcher per repository

Today `RepoMonitor` holds four `Option<Debouncer>` + four `JoinHandle`s. Collapse
to one debouncer + one task. The single debouncer registers every repo-level path
that the four currently watch:

| Path                                   | Recursive | Concern it serves            |
|----------------------------------------|-----------|------------------------------|
| `.git/worktrees/`                      | recursive | reconcile (entries) + status (per-wt index/HEAD) |
| `.git/config`                          | no        | default branch               |
| `.git/refs/remotes/origin/`            | no        | default branch               |
| `.git/refs`                            | recursive | graph-changed                |
| `.git/HEAD`, `logs/HEAD`, `packed-refs`| no        | graph-changed                |
| `.git/index`                           | no        | status                       |

One callback → one channel → one task. The task classifies each debounced batch
by path prefix into a set of affected concerns and runs each **at most once** per
batch:

```
for each debounced batch:
    concerns = {}
    for path in batch.paths:
        if under .git/worktrees/ and (top-level entry add/remove) → concerns += Reconcile
        if under .git/worktrees/ and (index|HEAD inside a worktree) → concerns += Status
        if == .git/config or under .git/refs/remotes/origin/        → concerns += DefaultBranch
        if under .git/refs or in {HEAD, logs/HEAD, packed-refs}      → concerns += Graph
        if == .git/index                                            → concerns += Status
    if Reconcile     in concerns: reconcile(repo_id, registry, watcher).await
    if DefaultBranch in concerns: default_branch refresh
    if Graph         in concerns: watcher.emit(GraphChanged{repo_id})
    if Status        in concerns: watcher.refresh_status_for(repo_id)
```

The path predicates are exactly the ones that select each watch root today, so no
concern is lost. `.git/worktrees/` is watched once (recursively) instead of by
both the meta and index watchers — a worktree entry appearing triggers reconcile;
an index/HEAD write inside an existing worktree triggers status.

Install stays idempotent (`sync_repos` already dedups by `RepoId`); `Drop` aborts
the single task and drops the single debouncer. `.git/worktrees/` is still
force-created so the first `git worktree add` is caught.

This takes the per-repo debouncer count 4 → 1 (32 → 8 for the 8-repo case), so
total watchers fall ~45 → ~21. Fewer FSEvents streams, debouncer threads, and
`FileIdMap`s — lower idle floor.

## #3 — Coalesced refresh

Coalescing falls out of #2: because one debouncer now produces one batch covering
all of a repo's git paths, the "run each concern at most once per batch" rule
means a rebase/fetch that writes many index + ref entries yields exactly one
status recompute and one `GraphChanged` for that window — instead of the previous
behaviour where four independent debouncers each fired their own batch. If a
single logical git operation still spans multiple debounce windows in practice,
lengthen the index/refs debounce (currently `DEFAULT_DEBOUNCE = 200ms`); this is
a constant tweak, gated on observation rather than assumed.

## Ordering & event-contract preservation

- The scoped path uses the same `diff_views` + "emit after raw events" discipline,
  so subscribers still observe `Updated → LogicalChange…/Instance…` in the
  established order.
- `refresh_status_for` emits the same single `Updated` carrier that
  `refresh_status_and_notify` does today; `cache-updated` consumers refetch all
  views, so a per-repo carrier is sufficient.
- `GraphChanged { repo_id }` is unchanged.

## Risks & mitigations

- **Dropped concern from path mis-classification.** Mitigation: predicates are
  lifted verbatim from the current per-watcher roots; a test asserts each path
  class still triggers its handler through the single watcher.
- **Scoped/global drift.** Mitigation: both paths share one `build_repo_view`
  body; a test asserts scoped recompute of A == full recompute restricted to A.
- **Snapshot ordering when splicing.** Mitigation: replace the existing entry
  in place by index; fall back to a full recompute if the repo isn't present yet
  (first appearance).
