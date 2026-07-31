# Tasks — Cache Change-Lifecycle Mining

Groups 1–2 build the cache as pure, git-free core logic; group 3 wires it in; group 4
removes the duplicate first-launch pass. The cache is testable in full before it is
ever connected to a real repository.

## 1. Fallible mining, so failures are distinguishable

- [x] 1.1 Add an internal fallible variant of `change_lifecycle` in `crates/openspec-core/src/git.rs` returning `Result<Vec<ChangeLifecycle>, _>`, distinguishing "git ran and found nothing" from "git failed" — the current empty-vec-on-error return conflates them, which is safe for direct callers but would let a transient failure pin an empty lifecycle in the cache
- [x] 1.2 Reimplement the existing public `change_lifecycle` as a wrapper that maps `Err` to the empty vec, so every current caller and the *Graceful Degradation Without Git* contract are unchanged

## 2. The cache (pure core logic, no git, no runtime)

- [x] 2.1 Add `LifecycleCache` to `openspec-core` — keyed by `RepoId`, holding `Vec<ChangeLifecycle>` — with `get_or_compute(repo, miner)` taking the mining function as a closure, mirroring the closure injection `compute_dashboard` already uses so the cache is unit-testable with fixtures and no repository on disk. Export it from `lib.rs`
- [x] 2.2 Implement single-flight: concurrent `get_or_compute` calls for the same cold repo perform one mining invocation and both receive its result. The map lock is held only to claim or clone the per-repo slot — **never across the miner call** (holding it across a ~150 ms `git log` would reintroduce exactly the lock-across-subprocess defect the sibling change fixes)
- [x] 2.3 Retain only successful derivations: an `Err` from the miner is returned to the caller but not stored, so the next fetch retries. A successful empty result **is** stored (a repository with no changes is a real answer)
- [x] 2.4 Implement `invalidate(repo)` and `invalidate_all()`
- [x] 2.5 Unit-test the cache by counting closure invocations: hit/miss, scoped invalidation, `invalidate_all`, failure-not-cached (miner `Err` then `Ok` → second fetch succeeds), successful-empty-is-cached, and single-flight under concurrent access

## 3. Wire it into the Dashboard path

- [x] 3.1 Hold a `LifecycleCache` on `AppService` in `crates/openspec-app/src/service.rs`
- [x] 3.2 Route the per-repo `change_lifecycle` call in `dashboard()` (line ~849) through the cache; keep the `reconcile_lifecycle` call on the cached result — it is idempotent, so replaying it against cached data records nothing new, which is the intended behaviour
- [x] 3.3 Subscribe to the existing `CacheEvent` stream and invalidate the affected repository on `GraphChanged { repo_id }`. No new watcher and no polling — `RepoMonitor` already watches `refs/`, `HEAD`, `logs/HEAD`, and `packed-refs`, which is exactly the ref set `git log --all` reads
- [x] 3.4 Treat `RecvError::Lagged` on that subscription as `invalidate_all()`, not as a no-op — a dropped event is the only realistic staleness path, and a conservative full flush closes it
- [x] 3.5 Warm the cache in the background after the initial populate, on the blocking pool rather than a runtime worker, strictly best-effort: if the warm has not finished when the user opens the Dashboard, single-flight (2.2) makes the concurrent cold fetch safe rather than duplicative

## 4. Remove the duplicate first-launch pass

- [x] 4.1 Route `backfill_activity`'s `change_lifecycle` call (line ~1143) through the same cache, so first launch does not mine every repository twice — once to seed the activity log and again for the first Dashboard fetch

## 5. Verification

- [x] 5.1 Add an invocation-counting test at the service boundary: two consecutive dashboard fetches with no intervening event mine each repository exactly once
- [x] 5.2 Add a scoped-invalidation test: `GraphChanged` for repo A causes the next fetch to mine A and no other repository
- [x] 5.3 Add a payload-equivalence test: `DashboardData` computed through the cache is identical to the same computation without it, over a fixture registry
- [x] 5.4 `cargo test` (workspace) and `bun run build` both pass
- [x] 5.5 `cargo clippy --workspace --all-targets` clean
- [ ] 5.6 Manual check against the real 12-repository registry: open the Dashboard, close and reopen it with no commits in between, and confirm the second open issues no lifecycle mining; then commit in one repository and confirm only that repository is re-mined. Record observed figures against the 482 ms (quiet) / 983 ms (loaded) baselines
- [ ] 5.7 Confirm the lifecycle metrics and today's-ships relative times are unchanged for identical history, and that removing the `git` binary still degrades per the *Graceful Degradation Without Git* scenarios
