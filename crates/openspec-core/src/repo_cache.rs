//! Per-repository cache of a value mined from git history.
//!
//! Mining a repository — walking its history with `git log` — is expensive and
//! almost always redundant, because the derived values are functions of
//! append-only history: until a new commit lands, last fetch's answer is still
//! the right one. [`RepoCache`] mines a repository at most once per history
//! change rather than once per Dashboard fetch; the app layer
//! (`crates/openspec-app/src/service.rs`) invalidates a repository's entry when
//! `CacheEvent::GraphChanged` fires for it. See
//! `openspec/changes/cache-change-lifecycle/design.md` for the full rationale
//! and the alternatives considered.
//!
//! The cache is generic over the mined value because two derivations need
//! exactly these semantics and exactly this invalidation signal: change
//! lifecycles ([`LifecycleCache`]) and the year-long commit-activity walk that
//! backs the heatmap, streak and leaderboard ([`CommitActivityCache`]). They
//! share ONE implementation deliberately — the single-flight and
//! invalidated-while-in-flight handling below is subtle enough that a second
//! hand-rolled copy is a liability, not a convenience.

use crate::git::{ChangeLifecycle, RepoId};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::{Arc, Mutex, OnceLock};

/// A single lifecycle derivation's shared result cell: `None` once resolved
/// to a failure (never promoted to [`Slot::Done`]), `Some(v)` once resolved
/// successfully. Shared via `Arc` between the `InFlight` slot every
/// single-flighted caller joins and the `Done` slot the cache serves once
/// promoted — promotion relabels the slot rather than copying the mined
/// `Vec` into a second, independent allocation (see
/// [`LifecycleCache::get_or_compute`]).
type Derivation<T> = Arc<OnceLock<Option<T>>>;

/// One repository's cache entry: a resolved success, or a shared handle to an
/// in-flight derivation that concurrent callers collapse onto. `InFlight`
/// additionally carries the cache's generation *at the moment this slot was
/// installed* — every caller that joins it (not only the one that created
/// it) uses that stamped value for the post-resolution
/// invalidated-while-in-flight check, rather than independently sampling the
/// live generation at whatever later moment it happens to join. See
/// [`LifecycleCache::get_or_compute`] for why a per-joiner read is unsound.
enum Slot<T> {
    Done(Derivation<T>),
    InFlight(Derivation<T>, u64),
}

/// The cache's entire mutable state, behind one lock. `slots` and
/// `generation` are deliberately fields of the SAME locked struct rather than
/// a separate map lock plus an independent atomic counter: every place that
/// needs to compare "the generation when a derivation was claimed" against
/// "the generation now" must make both reads part of the same critical
/// section as the corresponding slot install/removal, or a concurrent
/// [`LifecycleCache::invalidate`] can interleave between them and be silently
/// undone. Concretely, that interleaving was a real bug here: a prior version
/// bumped an `AtomicU64` generation counter and removed the map entry as two
/// separate, unlocked steps, which let a caller that joined an about-to-be-
/// invalidated in-flight slot read the already-bumped generation and wrongly
/// promote pre-invalidation data to `Done`. Folding both into one `Mutex`
/// makes that ordering bug impossible to reintroduce by construction, rather
/// than relying on every call site remembering to lock in the right order —
/// mirroring `WatcherManager::invalidate_identity` in `watcher.rs`, which
/// locks its cache before bumping its own generation counter for the
/// identical reason.
struct Locked<T> {
    slots: HashMap<RepoId, Slot<T>>,
    generation: u64,
}

impl<T> Default for Locked<T> {
    fn default() -> Self {
        Self {
            slots: HashMap::new(),
            generation: 0,
        }
    }
}

struct Inner<T> {
    locked: Mutex<Locked<T>>,
}

impl<T> Default for Inner<T> {
    fn default() -> Self {
        Self {
            locked: Mutex::new(Locked::default()),
        }
    }
}

/// Per-repository cache of a git-mined value `T`. Cheap to clone — every clone
/// shares the same underlying state via `Arc`, mirroring
/// [`crate::watcher::WatcherManager`].
///
/// - **Closure-injected** ([`Self::get_or_compute`]), mirroring
///   [`crate::dashboard::compute_dashboard`]'s `lifecycle_for` closure, so the
///   cache is unit-testable with fixtures and no git.
/// - **Single-flight**: concurrent misses for the same repository collapse
///   into one `miner` invocation via `OnceLock::get_or_init`, which blocks
///   every other caller for that repository until the first completes.
/// - **Failures are never cached**: an `Err` from `miner` is handed back to
///   the caller as `T::default()` (matching the public, empty-on-error
///   contract of the git helpers it fronts) but is not stored, so the next
///   call retries instead of pinning a transient failure in as an empty
///   result for the rest of the session. The error is logged (to stderr)
///   before being discarded, so a persistently-failing repository is
///   diagnosable even though the cache's own return type can't carry it.
/// - **Invalidation is race-free against an in-flight derivation.** `slots`
///   and the invalidation `generation` live behind one lock ([`Locked`]), and
///   a derivation's claim generation is stamped onto its `InFlight` slot at
///   install time rather than sampled independently by each caller that
///   later joins it. See [`Locked`] and [`Self::get_or_compute`].
pub struct RepoCache<T> {
    inner: Arc<Inner<T>>,
}

// Derived `Clone`/`Default` would demand `T: Clone`/`T: Default` on the struct
// itself; the state behind the `Arc` needs neither, so both are written out.
impl<T> Clone for RepoCache<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> Default for RepoCache<T> {
    fn default() -> Self {
        Self {
            inner: Arc::new(Inner::default()),
        }
    }
}

/// Per-repository cache of mined [`ChangeLifecycle`] data — the original
/// consumer of [`RepoCache`], kept as a named alias so call sites read the same
/// as before the cache was generalized.
pub type LifecycleCache = RepoCache<Vec<ChangeLifecycle>>;

/// Per-repository cache of the year-long `(author-date, author)` commit walk
/// that backs the Dashboard's heatmap, streak and per-author leaderboard.
///
/// This walk used to sit behind the gamification opt-in, which defaulted to
/// off, so a default install never paid it. Making the progress layer
/// unconditional made it run once per registered repository on EVERY Dashboard
/// fetch — measured at ~30-40ms per repo, so a dozen repos cost a third of a
/// second of git per refresh. It is a pure function of history invalidated by
/// exactly the same `GraphChanged` signal as the lifecycle mine, so it belongs
/// in the same cache rather than a second hand-rolled one.
pub type CommitActivityCache = RepoCache<Vec<(String, crate::identity::Author)>>;

impl<T: Clone + Default> RepoCache<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the cached value for `repo`, deriving it with `miner` on a miss.
    /// See the type doc for the single-flight, race-free invalidation, and
    /// failure-is-not-cached guarantees.
    pub fn get_or_compute<E: Display>(
        &self,
        repo: &RepoId,
        miner: impl FnOnce(&RepoId) -> Result<T, E>,
    ) -> T {
        // Claim or join this repo's slot. The lock is held only long enough
        // to read or install it — never across the miner call below, which
        // may shell out to `git log` for ~100ms or more. A freshly-installed
        // slot is stamped with the CURRENT generation while still holding
        // the lock, so the stamp is atomic with respect to any concurrent
        // `invalidate`/`invalidate_all` (which mutate under this same lock —
        // see `Locked`'s doc comment for the race this closes). A caller
        // that instead JOINS an already-installed slot reads the stamp
        // already recorded on it rather than re-sampling the live
        // generation itself, so every caller sharing one derivation agrees
        // on its vintage regardless of when each of them happened to join.
        // `claim_generation` is `None` for a fast-path `Done` hit — there is
        // nothing left to decide for an already-promoted slot.
        let (once, claim_generation): (Derivation<T>, Option<u64>) = {
            let mut locked = self.inner.locked.lock().unwrap_or_else(|e| e.into_inner());
            match locked.slots.get(repo) {
                Some(Slot::Done(once)) => (once.clone(), None),
                Some(Slot::InFlight(once, gen)) => (once.clone(), Some(*gen)),
                None => {
                    let once = Arc::new(OnceLock::new());
                    let gen = locked.generation;
                    locked
                        .slots
                        .insert(repo.clone(), Slot::InFlight(once.clone(), gen));
                    (once, Some(gen))
                }
            }
        };

        // `get_or_init` runs the closure for exactly one caller per
        // derivation (the single-flight owner) — or, for a `Done` hit,
        // returns the already-resolved value immediately without invoking
        // it at all. Every other caller sharing this `once` blocks here and
        // observes the identical result. A miner failure is logged here
        // (once, only by the owner) and collapsed to `None` — never cached,
        // regardless of generation; see the promote/evict decision below.
        let resolved: &Option<T> = once.get_or_init(|| match miner(repo) {
            Ok(v) => Some(v),
            Err(err) => {
                eprintln!(
                    "repo cache: mining failed for {}: {err}",
                    repo.as_path().display()
                );
                None
            }
        });

        if let Some(claim_generation) = claim_generation {
            let mut locked = self.inner.locked.lock().unwrap_or_else(|e| e.into_inner());
            match resolved {
                Some(_) if locked.generation == claim_generation => {
                    // Success, and nothing invalidated this repo (or the
                    // whole cache) since this derivation was claimed:
                    // promote by relabeling the slot — the same `Arc`, no
                    // second copy of the mined `Vec`.
                    locked.slots.insert(repo.clone(), Slot::Done(once.clone()));
                }
                _ => {
                    // Either the miner failed (never cached, at any
                    // generation), or it succeeded but an invalidation
                    // landed while it was in flight (don't resurrect a
                    // stale success). Evict this exhausted slot — but only
                    // if it's still exactly the one this caller
                    // created/joined; a concurrent invalidate or a fresh
                    // resolution may already have replaced it with newer
                    // state that must not be undone.
                    if let Some(Slot::InFlight(current, _)) = locked.slots.get(repo) {
                        if Arc::ptr_eq(current, &once) {
                            locked.slots.remove(repo);
                        }
                    }
                }
            }
        }

        resolved.clone().unwrap_or_default()
    }

    /// Drop the cached (or in-flight) entry for `repo`. The next
    /// [`Self::get_or_compute`] call for it starts a fresh derivation.
    ///
    /// Takes the lock BEFORE bumping `generation`, and mutates `slots` under
    /// that same critical section — see [`Locked`]'s doc comment for the
    /// race this ordering closes.
    pub fn invalidate(&self, repo: &RepoId) {
        let mut locked = self.inner.locked.lock().unwrap_or_else(|e| e.into_inner());
        locked.generation += 1;
        locked.slots.remove(repo);
    }

    /// Drop every cached entry. Used when a `RecvError::Lagged` on the
    /// `CacheEvent` subscription means an unknown number of `GraphChanged`
    /// events were dropped — a conservative full flush, per design.md's
    /// "Missed-event risk". Same lock-before-bump ordering as
    /// [`Self::invalidate`].
    pub fn invalidate_all(&self) {
        let mut locked = self.inner.locked.lock().unwrap_or_else(|e| e.into_inner());
        locked.generation += 1;
        locked.slots.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::LifecycleError;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Barrier};
    use std::thread;

    fn repo(name: &str) -> RepoId {
        RepoId(std::path::PathBuf::from(format!("/{name}/.git")))
    }

    fn lc(name: &str) -> ChangeLifecycle {
        ChangeLifecycle {
            change_name: name.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn miss_then_hit_invokes_miner_once() {
        let cache = LifecycleCache::new();
        let calls = AtomicUsize::new(0);
        let mine = |_: &RepoId| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<_, LifecycleError>(vec![lc("a")])
        };

        let first = cache.get_or_compute(&repo("x"), mine);
        let second = cache.get_or_compute(&repo("x"), mine);

        assert_eq!(first, vec![lc("a")]);
        assert_eq!(second, vec![lc("a")]);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "second fetch must hit the cache"
        );
    }

    #[test]
    fn scoped_invalidation_only_remines_the_invalidated_repo() {
        let cache = LifecycleCache::new();
        let calls_a = AtomicUsize::new(0);
        let calls_b = AtomicUsize::new(0);

        cache.get_or_compute(&repo("a"), |_| {
            calls_a.fetch_add(1, Ordering::SeqCst);
            Ok::<_, LifecycleError>(vec![])
        });
        cache.get_or_compute(&repo("b"), |_| {
            calls_b.fetch_add(1, Ordering::SeqCst);
            Ok::<_, LifecycleError>(vec![])
        });

        cache.invalidate(&repo("a"));

        cache.get_or_compute(&repo("a"), |_| {
            calls_a.fetch_add(1, Ordering::SeqCst);
            Ok::<_, LifecycleError>(vec![])
        });
        cache.get_or_compute(&repo("b"), |_| {
            calls_b.fetch_add(1, Ordering::SeqCst);
            Ok::<_, LifecycleError>(vec![])
        });

        assert_eq!(
            calls_a.load(Ordering::SeqCst),
            2,
            "invalidated repo must be re-mined"
        );
        assert_eq!(
            calls_b.load(Ordering::SeqCst),
            1,
            "untouched repo must still be cached"
        );
    }

    #[test]
    fn invalidate_all_clears_every_repo() {
        let cache = LifecycleCache::new();
        let calls = AtomicUsize::new(0);
        let mine = |_: &RepoId| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<_, LifecycleError>(vec![])
        };
        cache.get_or_compute(&repo("a"), mine);
        cache.get_or_compute(&repo("b"), mine);
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        cache.invalidate_all();

        cache.get_or_compute(&repo("a"), mine);
        cache.get_or_compute(&repo("b"), mine);
        assert_eq!(calls.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn failure_is_not_cached_and_is_retried() {
        let cache = LifecycleCache::new();
        let calls = AtomicUsize::new(0);
        let r = cache.get_or_compute(&repo("x"), |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(LifecycleError::CommandFailed)
        });
        assert!(r.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let r2 = cache.get_or_compute(&repo("x"), |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<_, LifecycleError>(vec![lc("recovered")])
        });
        assert_eq!(r2, vec![lc("recovered")]);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "the failure must not have been cached"
        );

        // And the recovered success IS now cached.
        let r3 = cache.get_or_compute(&repo("x"), |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<_, LifecycleError>(vec![lc("should not run")])
        });
        assert_eq!(r3, vec![lc("recovered")]);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "the successful result must now be cached"
        );
    }

    #[test]
    fn successful_empty_result_is_cached() {
        let cache = LifecycleCache::new();
        let calls = AtomicUsize::new(0);
        let mine = |_: &RepoId| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<_, LifecycleError>(Vec::new())
        };
        let r1 = cache.get_or_compute(&repo("x"), mine);
        let r2 = cache.get_or_compute(&repo("x"), mine);
        assert!(r1.is_empty());
        assert!(r2.is_empty());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a genuinely empty result (no changes) is a real answer and must be cached"
        );
    }

    #[test]
    fn concurrent_misses_for_the_same_repo_single_flight_to_one_invocation() {
        let cache = LifecycleCache::new();
        let calls = Arc::new(AtomicUsize::new(0));
        const N: usize = 8;
        let barrier = Arc::new(Barrier::new(N));

        let handles: Vec<_> = (0..N)
            .map(|_| {
                let cache = cache.clone();
                let calls = calls.clone();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    cache.get_or_compute(&repo("x"), |_| {
                        calls.fetch_add(1, Ordering::SeqCst);
                        // Give the other threads time to join this in-flight
                        // window rather than each starting their own.
                        thread::sleep(std::time::Duration::from_millis(30));
                        Ok::<_, LifecycleError>(vec![lc("a")])
                    })
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        for r in &results {
            assert_eq!(r, &vec![lc("a")]);
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "8 concurrent misses for one repo must collapse into a single mining invocation"
        );
    }

    /// Behavioural test: an invalidation landing while a derivation is in
    /// flight must prevent that derivation's result from being cached.
    /// Deterministic: driven entirely by explicit channel handshakes, never
    /// a sleep, so "invalidate lands strictly between claim and resolve" is
    /// provable by construction rather than a timing race.
    ///
    /// NOTE: this is *not* a regression test for the joiner-side generation
    /// race an adversarial review found in the original design (a caller
    /// joining an in-flight slot could sample an already-bumped generation
    /// while `invalidate`'s bump and map removal were two separate, unlocked
    /// steps, and wrongly promote pre-invalidation data to `Done`). This
    /// test only exercises the owner path, where the original design was
    /// already correct — it stays green even with that bug reintroduced.
    /// The race is closed structurally, not by test coverage: `invalidate`
    /// now bumps and removes atomically under the same lock that installs
    /// and reads slots, and joiners inherit the claim generation stamped
    /// into `Slot::InFlight` instead of sampling the live counter (see
    /// `Locked`'s doc comment). Anyone refactoring either property — e.g.
    /// moving the generation back to a separate atomic for contention
    /// reasons — must know that NO test will catch the reintroduced race.
    /// Covering it deterministically would require test-only
    /// instrumentation inside `get_or_compute` (a signal fired the instant
    /// a joiner attaches, before it blocks on `get_or_init`), which is more
    /// production-code surface than this one race warrants — the
    /// `concurrent_misses_for_the_same_repo_single_flight_to_one_invocation`
    /// test above already covers ordinary (non-invalidated) joining.
    #[test]
    fn invalidated_mid_flight_derivation_is_not_cached() {
        let cache = LifecycleCache::new();
        let calls = AtomicUsize::new(0);
        let (claimed_tx, claimed_rx) = mpsc::channel::<()>();
        let (proceed_tx, proceed_rx) = mpsc::channel::<()>();

        let owner_cache = cache.clone();
        let owner = thread::spawn(move || {
            owner_cache.get_or_compute(&repo("x"), |_| {
                // Prove the derivation is genuinely in flight before this
                // test invalidates it.
                claimed_tx.send(()).unwrap();
                proceed_rx.recv().unwrap();
                Ok::<_, LifecycleError>(vec![lc("pre-invalidation")])
            })
        });

        claimed_rx.recv().unwrap();
        // Invalidate strictly *during* the in-flight window: the miner is
        // still blocked on `proceed_rx`, guaranteed by the handshake above.
        cache.invalidate(&repo("x"));
        proceed_tx.send(()).unwrap();

        let result = owner.join().unwrap();
        assert_eq!(
            result,
            vec![lc("pre-invalidation")],
            "the in-flight caller still receives its own derivation's result"
        );

        // The next call must re-mine — proving the invalidated-mid-flight
        // result was not cached.
        let after = cache.get_or_compute(&repo("x"), |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<_, LifecycleError>(vec![lc("post-invalidation")])
        });
        assert_eq!(after, vec![lc("post-invalidation")]);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a derivation invalidated while in flight must not be cached"
        );
    }
}
