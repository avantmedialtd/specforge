# Cache Change-Lifecycle Mining

## Why

`change_lifecycle` walks a repository's **entire git history** on every Dashboard fetch:

```
git log --all --reverse --no-renames --diff-filter=A --name-status \
    --pretty=format:… -- openspec/changes
```

No `--since`, no `--max-count` — unlike every other log invocation in the app, which are all windowed (`14 days`, `371 days`, `-n 500`). It runs once per registered repository, per fetch.

Measured across the developer's 12 repositories: **482 ms on a quiet machine, 983 ms under load — roughly 75% of the Dashboard's total git cost.** The pathspec does not bound it: `meter-burn` takes 127–150 ms for only 105 commits touching `openspec/changes`, because the filter does not stop git walking the full history. The cost therefore tracks *total repository history* and grows without limit as these repos age.

The work is almost entirely redundant. A change's lifecycle — the commit that created it, the commit that archived it — is **append-only history**: once written it never changes. Yet every fetch re-derives all of it from scratch, for all 12 repositories, regardless of which (if any) repository's history actually moved.

The redundancy is already visible elsewhere in the code: `ActivityLog::reconcile_lifecycle`, which consumes this output, is explicitly idempotent — after the first pass it records zero new events. The app pays ~500 ms per fetch to compute an input whose downstream effect is usually nothing.

Two signals make a cache cheap and correct:

- **`GraphChanged` is already emitted and already repo-scoped.** `RepoMonitor` classifies `refs/`, `HEAD`, `logs/HEAD`, and `packed-refs` writes as the `graph` concern and emits `CacheEvent::GraphChanged { repo_id }`. Since `--all` reads exactly those refs, that event is precisely the invalidation signal — at zero additional cost.
- **Checking validity with git would cost more than it saves.** Measured, `git rev-parse --all` takes **469 ms** across the 12 repos — about half the recompute it would guard, and *more than recomputing* for 3 of them (`avantmedia` 136 ms to validate vs 68 ms to recompute). Validation is itself a spawn per repo, and spawn count is the cost model. Event-driven invalidation costs nothing.

## What Changes

- **Introduce a per-repository lifecycle cache** in `openspec-core`: a `LifecycleCache` keyed by `RepoId`, holding the mined `Vec<ChangeLifecycle>`, with a `get_or_compute(repo_id, miner)` accessor that takes the mining function as a closure — matching the closure-injection pattern `compute_dashboard` already uses, so it is unit-testable with no git and no runtime.
- **Invalidate per repository on `GraphChanged`.** The app layer subscribes to the existing event stream and drops only the affected repository's entry. No new watcher, no polling, no extra git call.
- **Single-flight the mining.** Concurrent Dashboard fetches for a cold repository wait on one in-flight computation rather than each spawning their own `git log`. This matters at startup, where the background warm and the user's first Dashboard open can otherwise race.
- **Do not cache failures.** `change_lifecycle` currently returns an empty `Vec` on any error, which is indistinguishable from "this repo genuinely has no changes". An internal fallible variant lets the cache store only successful results and retry after a failure, so a transient git error cannot pin an empty lifecycle in memory for the session. The public empty-on-error surface is unchanged, preserving the *Graceful Degradation Without Git* contract.
- **Warm the cache in the background at startup**, off the critical path, so the first Dashboard open finds it populated rather than paying the full mining pass.
- **Share the cache with the first-launch backfill.** `backfill_activity` mines the same data for the same repositories; routing it through the cache removes a duplicate full pass on first launch.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `dashboard`: extends *Change Lifecycle Metrics* with a derivation-freshness contract — lifecycle data is mined at most once per repository per history change rather than once per fetch; a fetch whose repositories' histories are unchanged issues no lifecycle mining invocation; when a repository's history moves, only that repository is re-mined and the next fetch reflects it within the debounce window. Extends *Graceful Degradation Without Git* so a failed mining is not cached, and a later fetch retries rather than serving a permanently empty lifecycle.

## Impact

- **Rust only.** No IPC type changes, no frontend changes, no schema changes. The `DashboardData` payload is byte-identical for identical history.
- `crates/openspec-core/src/`: new `lifecycle_cache.rs` (or a cache type beside the existing `cache.rs`), exported from `lib.rs`.
- `crates/openspec-core/src/git.rs`: an internal fallible `change_lifecycle` variant; the existing public empty-on-error signature is retained for callers that want it.
- `crates/openspec-app/src/service.rs`: `AppService` holds the cache; `dashboard()` (line ~849) and `backfill_activity` (line ~1143) route through it; the event-stream subscription invalidates on `GraphChanged`.
- **Expected effect on the Dashboard's 639 ms git cost:**

  | Situation | Today | After |
  |---|---|---|
  | Fetch, no history change (the common case) | ~500 ms | **0 ms** |
  | Fetch after a commit in one repository | ~500 ms | one repo re-mined (~40–150 ms) |
  | First fetch after app start | ~500 ms | warmed in background beforehand |

- **Deliberately not addressed:** the mining itself stays an unbounded history walk. Caching removes it from the hot path; bounding or incrementalising the walk (for example, scanning only commits newer than the last-seen ref tips) is a further optimization that only matters once a single repository's history is large enough that the post-commit re-mine is itself objectionable. Cache first, measure, then decide.
- **Interaction with `optimize-aggregation-hot-path`:** independent. That change addresses the aggregation recompute (`watcher.rs`, `repo_view.rs`, `repo_monitor.rs`, the frontend hooks); this one addresses Dashboard git mining (`service.rs`, `git.rs`). They touch disjoint functions and can land in either order.
