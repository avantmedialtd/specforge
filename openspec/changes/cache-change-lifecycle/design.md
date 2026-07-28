# Design — Cache Change-Lifecycle Mining

## The measurement that picks the design

Two numbers decide everything here. Across the developer's 12 repositories:

| | Measured |
|---|---|
| `change_lifecycle` (the recompute) | 482 ms quiet · 983 ms under load |
| `git rev-parse --all` (a validity check) | **469 ms** |

A validity check that costs half of what it guards is not a cache — it is a slightly cheaper recompute. And for 3 of the 12 repositories it is not even that: `avantmedia` takes 136 ms to enumerate refs and 68 ms to re-mine outright. The reason is the same one that governs the sibling change: **a git spawn costs ~25–30 ms regardless of the work it does**, so any per-repo verification spawn is already most of the price.

That eliminates the whole family of "persist the cache, verify it on load" designs. What remains must invalidate **without asking git anything** — which is exactly what the existing event stream provides for free.

## Decision 1 — Invalidate on `GraphChanged`, which already exists and is already scoped

`git log --all` reads all refs. `RepoMonitor` already watches `refs/` (recursive), `HEAD`, `logs/HEAD`, and `packed-refs`, classifies them as the `graph` concern, and emits:

```rust
CacheEvent::GraphChanged { repo_id: PathBuf }
```

The watched set and the read set coincide, so this event is a sound invalidation signal — and it is repo-scoped, which is the second win. Today a commit in one repository causes all twelve to be re-mined on the next fetch; after this change it invalidates one.

```
        TODAY                              AFTER
  ┌──────────────────┐              ┌──────────────────┐
  │ every fetch      │              │ GraphChanged(A)  │
  │   ↓              │              │   ↓              │
  │ mine all 12 repos│              │ drop A's entry   │
  │   ~500 ms        │              │   ↓              │
  └──────────────────┘              │ next fetch:      │
                                    │   mine A only    │
                                    │   B…L from cache │
                                    └──────────────────┘
```

The subscription lives in the app layer (`service.rs` already consumes this stream); the cache type itself lives in `openspec-core`, per the repository convention that watchers, registries, and derivation logic stay out of the Tauri crate so they remain testable from `cargo test`.

**Missed-event risk.** If a `GraphChanged` were ever dropped, the cache would serve stale lifecycle data until the next one. The broadcast channel does drop for lagging subscribers (`RecvError::Lagged`). The invalidation subscriber must therefore treat `Lagged` as "invalidate everything" rather than ignoring it — a one-line guard that converts the only realistic staleness path into a conservative full flush.

## Decision 2 — Closure-injected cache, so it tests without git

```rust
impl LifecycleCache {
    fn get_or_compute(
        &self,
        repo: &RepoId,
        miner: impl FnOnce(&RepoId) -> Result<Vec<ChangeLifecycle>, LifecycleError>,
    ) -> Vec<ChangeLifecycle>;

    fn invalidate(&self, repo: &RepoId);
    fn invalidate_all(&self);
}
```

Taking the miner as a closure mirrors `compute_dashboard`, which already receives `commit_activity` and `lifecycle_for` as injected closures precisely so the dashboard is unit-testable with fixtures and no git. The cache inherits that property: hit/miss/invalidate/single-flight semantics are all testable by counting closure invocations, with no repository on disk.

## Decision 3 — Single-flight, because startup races the first open

Without it, the background warm and a user opening the Dashboard immediately would each start a full mining pass for the same repositories. The accessor holds a per-repo slot that concurrent callers wait on:

```
caller A ─▶ miss ─▶ claims slot ─▶ mines ─┐
caller B ─▶ miss ─▶ waits on slot ────────┴─▶ both get one result
```

Implemented with a `Mutex<HashMap<RepoId, Arc<OnceLock<…>>>>` (or an equivalent per-key guard): the map lock is held only long enough to claim or clone the slot, never across the mining itself — the same gather-then-compute discipline the sibling change applies to the aggregation path. Holding the map lock across a 150 ms `git log` would reintroduce exactly the defect that change is fixing.

## Decision 4 — Never cache a failure

`change_lifecycle` returns `Vec::new()` on *any* error — a missing git binary, a corrupt repository, a killed subprocess. That is correct for its callers (the *Graceful Degradation Without Git* requirement mandates an empty state, not an error), but it is fatal to a cache: an empty vec from a transient failure is indistinguishable from a repository that genuinely has no changes, and caching it would pin the Dashboard's lifecycle metrics empty for the rest of the session.

An internal fallible variant separates the two:

```
git ok, no changes  → Ok(vec![])    → cached  (a real, stable answer)
git failed          → Err(_)        → NOT cached, retried next fetch
```

The public `change_lifecycle` keeps its current empty-on-error signature by wrapping the fallible one, so no existing caller changes and the degradation contract is preserved verbatim.

## Decision 5 — Warm at startup, in the background

The first Dashboard open otherwise pays the whole uncached pass. A background warm after the initial cache populate — on the blocking pool, not a runtime worker — moves that cost off the user's critical path. It must be strictly best-effort: a warm that fails or is still running when the user opens the Dashboard simply results in a normal cold fetch, and single-flight (Decision 3) makes that safe rather than duplicative.

## Alternatives considered

**Persist the cache to disk and validate on load.** Rejected on the measurement above: `git rev-parse --all` costs 469 ms to guard a 482–983 ms recompute, and exceeds the recompute for 3 of 12 repositories. It also introduces a disk format to version and migrate, for a saving that only applies once per app launch.

**Derive lifecycle metrics from the activity log instead of git.** Genuinely attractive: the log is already persisted (1.08 MB, 3,739 events), already carries `changeCreated` / `changeArchived` with timestamps and change ids, and `reconcile_lifecycle` already backfills it from exactly this git data. Metrics could come from the log, and git mining would shrink to a reconcile step. Rejected *for this change* because it inverts the source of truth: git currently authoritative, log derived. If the log were deleted or partially written, metrics would silently degrade rather than self-correct, and the *Change Lifecycle Metrics* requirement explicitly specifies the metrics as "derived from git history" with dates recovered from commits. Worth revisiting as a follow-on once the cache has removed the latency pressure — at which point it is a correctness/architecture decision on its own merits, not a performance workaround.

**Bound the walk with `--since` or `--max-count`.** Rejected: time-to-archive needs a change's *creation* commit, which may be arbitrarily old. Any window silently drops long-lived changes from the average and makes the metric depend on the window rather than the history. The unbounded walk is correct; it just should not run on every fetch.

**Incrementally scan only new commits** (`git log --all --not <previous-tips>`). Deferred, not rejected. It would make the post-commit re-mine proportional to new commits rather than total history — the right end state if a single repository's history ever grows large enough that re-mining it after each commit is objectionable. It needs the previous tip set persisted and careful handling of rewritten history (rebase, amend, branch deletion), which is a meaningful design in its own right. Cache first, measure, then decide.

## Verification strategy

Wall-clock is unstable in CI, so the tests assert **invocation counts and cache semantics**, not timings:

- **Hit/miss.** Two consecutive fetches with no intervening event invoke the miner closure exactly once.
- **Scoped invalidation.** A `GraphChanged` for repo A invalidates A only; a subsequent fetch invokes the miner for A and for no other repository.
- **Lagged flush.** A subscriber observing `RecvError::Lagged` invalidates every entry.
- **Failure is not cached.** A miner returning `Err` on the first call and `Ok` on the second yields the successful result on the second fetch — proving the failure was not stored.
- **Single-flight.** Concurrent `get_or_compute` calls for one cold repo invoke the miner once and both receive the same result.
- **Payload equivalence.** `DashboardData` computed with the cache is identical to the same computation without it, over a fixture registry.
- **Manual.** Against the real 12-repository registry: open the Dashboard twice with no commits in between and confirm no `git log` mining runs on the second open; then commit in one repository and confirm only that repository is re-mined.
