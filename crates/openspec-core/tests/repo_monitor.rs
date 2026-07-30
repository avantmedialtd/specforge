//! End-to-end coverage of [`openspec_core::repo_monitor::RepoMonitor`] +
//! the `WatcherManager::sync_repos` integration.
//!
//! These tests shell out to the real `git` binary to set up worktrees and
//! verify that the meta-watcher picks up runtime worktree additions and
//! removals without user action.

use openspec_core::git::invocation_log;
use openspec_core::{CacheEvent, RepoId, WatcherManager, WorkspaceRegistry, WorkspaceView};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::sync::broadcast;

/// Short debounce so the meta-watcher reacts within a test timeout.
const TEST_DEBOUNCE: Duration = Duration::from_millis(50);

/// Generous outer timeout for filesystem-event-driven assertions.
const EVENT_TIMEOUT: Duration = Duration::from_secs(5);

fn git(args: &[&str], cwd: &Path) {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git invocation");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn init_openspec_repo(root: &Path) -> PathBuf {
    fs::create_dir_all(root.join("openspec/changes")).unwrap();
    git(&["init", "-b", "main"], root);
    git(&["config", "user.email", "t@t"], root);
    git(&["config", "user.name", "t"], root);
    git(&["commit", "--allow-empty", "-m", "init"], root);
    root.canonicalize().unwrap()
}

fn add_worktree(root: &Path, branch: &str, path: &Path) {
    git(
        &["worktree", "add", "-b", branch, path.to_str().unwrap()],
        root,
    );
    fs::create_dir_all(path.join("openspec/changes")).unwrap();
}

async fn wait_until<F>(mut check: F)
where
    F: FnMut() -> bool,
{
    let start = Instant::now();
    while start.elapsed() < EVENT_TIMEOUT {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("condition never became true within {:?}", EVENT_TIMEOUT);
}

/// Drain the broadcast channel until `pred` matches an event or the timeout
/// elapses. Returns the count of events matching `pred` observed up to and
/// including the first match (so callers can assert at-most-once coalescing by
/// continuing to drain after the first match within a window).
async fn wait_for_event<F>(rx: &mut broadcast::Receiver<CacheEvent>, mut pred: F) -> bool
where
    F: FnMut(&CacheEvent) -> bool,
{
    let deadline = Instant::now() + EVENT_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok(ev)) => {
                if pred(&ev) {
                    return true;
                }
            }
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(broadcast::error::RecvError::Closed)) => return false,
            Err(_) => return false,
        }
    }
}

/// Build a watcher over a single registered repo, populate it, and install the
/// repo monitor. Returns the watcher and the repo's `RepoId`.
async fn watched_repo(root: &Path, registry: Arc<Mutex<WorkspaceRegistry>>) -> WatcherManager {
    {
        let mut reg = registry.lock().unwrap();
        reg.register(root.to_path_buf()).unwrap();
    }
    let watcher = WatcherManager::with_registry(TEST_DEBOUNCE, Some(registry.clone()));
    let folders = registry.lock().unwrap().folders();
    for folder in folders {
        watcher.add_workspace(folder).await.unwrap();
    }
    watcher.sync_repos();
    watcher
}

#[tokio::test]
async fn refs_change_emits_graph_changed() {
    let tmp = TempDir::new().unwrap();
    let root = init_openspec_repo(&tmp.path().join("repo"));
    let registry = Arc::new(Mutex::new(WorkspaceRegistry::new(
        tmp.path().join("ws.json"),
    )));
    let watcher = watched_repo(&root, registry.clone()).await;
    let repo_id = registry
        .lock()
        .unwrap()
        .entry(&root)
        .unwrap()
        .repo_id
        .clone()
        .unwrap();

    let mut rx = watcher.subscribe();
    // A commit moves refs/heads/main and writes logs/HEAD.
    git(&["commit", "--allow-empty", "-m", "second"], &root);

    assert!(
        wait_for_event(&mut rx, |ev| matches!(
            ev,
            CacheEvent::GraphChanged { repo_id: r } if r.as_path() == repo_id.as_path()
        ))
        .await,
        "expected GraphChanged after a commit moved the refs"
    );
}

#[tokio::test]
async fn index_change_emits_a_status_update() {
    let tmp = TempDir::new().unwrap();
    let root = init_openspec_repo(&tmp.path().join("repo"));
    let registry = Arc::new(Mutex::new(WorkspaceRegistry::new(
        tmp.path().join("ws.json"),
    )));
    let watcher = watched_repo(&root, registry.clone()).await;

    let mut rx = watcher.subscribe();
    // Stage a NON-spec file at the repo root: writes `.git/index` but does not
    // touch the `openspec/` subtree, so the only source of an Updated is the
    // repo-monitor index watcher.
    fs::write(root.join("foo.txt"), "x").unwrap();
    git(&["add", "foo.txt"], &root);

    assert!(
        wait_for_event(&mut rx, |ev| matches!(ev, CacheEvent::Updated { .. })).await,
        "expected a status Updated after the index changed"
    );
}

#[tokio::test]
async fn sync_repos_installs_a_monitor_and_picks_up_a_new_worktree() {
    let tmp = TempDir::new().unwrap();
    let root = init_openspec_repo(&tmp.path().join("repo"));

    let cfg = tmp.path().join("workspaces.json");
    let registry = Arc::new(Mutex::new(WorkspaceRegistry::new(cfg)));
    {
        let mut reg = registry.lock().unwrap();
        reg.register(root.clone()).unwrap();
    }

    let watcher = WatcherManager::with_registry(TEST_DEBOUNCE, Some(registry.clone()));

    // Wire the main worktree (registered above) into the watcher and install
    // the monitor.
    {
        let folders = registry.lock().unwrap().folders();
        for folder in folders {
            watcher.add_workspace(folder).await.unwrap();
        }
    }
    watcher.sync_repos();

    // Add a new worktree at runtime — meta-watcher should detect it.
    let wt2 = tmp.path().join("wt2");
    add_worktree(&root, "feature", &wt2);
    let wt2_canonical = wt2.canonicalize().unwrap();

    wait_until(|| {
        let reg = registry.lock().unwrap();
        reg.entry(&wt2_canonical).is_some()
    })
    .await;

    // The newly-discovered worktree's openspec/changes/ should also be
    // watched by the per-workspace watcher.
    wait_until(|| watcher.is_watching(&wt2_canonical)).await;
}

#[tokio::test]
async fn meta_watcher_removes_a_worktree_whose_path_is_deleted() {
    let tmp = TempDir::new().unwrap();
    let root = init_openspec_repo(&tmp.path().join("repo"));
    let wt = tmp.path().join("ephemeral");
    add_worktree(&root, "ephemeral", &wt);
    let wt_canonical = wt.canonicalize().unwrap();

    let cfg = tmp.path().join("workspaces.json");
    let registry = Arc::new(Mutex::new(WorkspaceRegistry::new(cfg)));
    {
        let mut reg = registry.lock().unwrap();
        reg.register(root.clone()).unwrap();
    }

    let watcher = WatcherManager::with_registry(TEST_DEBOUNCE, Some(registry.clone()));
    {
        let folders = registry.lock().unwrap().folders();
        for folder in folders {
            watcher.add_workspace(folder).await.unwrap();
        }
    }
    watcher.sync_repos();

    // Sanity: the ephemeral worktree is currently tracked + watched.
    assert!(watcher.is_watching(&wt_canonical));

    // Simulate `rm -rf` (the harness's typical cleanup path).
    fs::remove_dir_all(&wt).unwrap();
    // Some filesystems do not fire FSEvents on a recursive remove of the
    // tracked subtree's parent directory if the parent goes away too. Touch
    // the `.git/worktrees/<name>` dir to force a meta-watcher fire.
    let _ = fs::remove_dir_all(root.join(".git/worktrees/ephemeral"));

    wait_until(|| {
        let reg = registry.lock().unwrap();
        reg.entry(&wt_canonical).is_none()
    })
    .await;
}

#[tokio::test]
async fn scoped_status_refresh_recomputes_only_the_target_repo() {
    let tmp = TempDir::new().unwrap();
    let a = init_openspec_repo(&tmp.path().join("a"));
    let b = init_openspec_repo(&tmp.path().join("b"));

    let cfg = tmp.path().join("workspaces.json");
    let registry = Arc::new(Mutex::new(WorkspaceRegistry::new(cfg)));
    {
        let mut reg = registry.lock().unwrap();
        reg.register(a.clone()).unwrap();
        reg.register(b.clone()).unwrap();
    }
    let watcher = WatcherManager::with_registry(TEST_DEBOUNCE, Some(registry.clone()));
    {
        let folders = registry.lock().unwrap().folders();
        for folder in folders {
            watcher.add_workspace(folder).await.unwrap();
        }
    }
    // Seed last_views with both repos clean.
    watcher.aggregate_and_emit();

    let repo_id = |path: &Path| -> RepoId {
        registry
            .lock()
            .unwrap()
            .entry(&path.canonicalize().unwrap())
            .unwrap()
            .repo_id
            .clone()
            .unwrap()
    };
    let repo_id_a = repo_id(&a);
    let repo_id_b = repo_id(&b);

    let dirty_of = |views: &[WorkspaceView], id: &RepoId| -> bool {
        views
            .iter()
            .find_map(|v| match v {
                WorkspaceView::Repo(r) if r.repo_id.as_path() == id.as_path() => Some(r.dirty),
                _ => None,
            })
            .expect("repo present in views")
    };

    let before = watcher.workspace_views();
    assert!(!dirty_of(&before, &repo_id_a));
    assert!(!dirty_of(&before, &repo_id_b));

    // Dirty BOTH repos on disk with an untracked spec file.
    for root in [&a, &b] {
        let d = root.join("openspec/changes/foo");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("proposal.md"), "x").unwrap();
    }

    // Scoped refresh of A only.
    watcher.refresh_aggregated_view_for(&repo_id_a);

    let after = watcher.workspace_views();
    assert!(dirty_of(&after, &repo_id_a), "A was recomputed → now dirty");
    assert!(
        !dirty_of(&after, &repo_id_b),
        "B must NOT be recomputed by a scoped refresh of A (a full recompute would mark it dirty)"
    );
}

#[tokio::test]
async fn sync_repos_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    let root = init_openspec_repo(&tmp.path().join("repo"));

    let cfg = tmp.path().join("workspaces.json");
    let registry = Arc::new(Mutex::new(WorkspaceRegistry::new(cfg)));
    registry.lock().unwrap().register(root.clone()).unwrap();

    let watcher = WatcherManager::with_registry(TEST_DEBOUNCE, Some(registry));
    watcher.sync_repos();
    let count_after_first = watcher.watched_count();
    watcher.sync_repos();
    let count_after_second = watcher.watched_count();
    assert_eq!(count_after_first, count_after_second);
}

#[tokio::test]
async fn one_repo_monitor_per_repo_and_idempotent() {
    // Two distinct repos → exactly two repo monitors (one watcher each), and a
    // repeated sync must not add more.
    let tmp = TempDir::new().unwrap();
    let a = init_openspec_repo(&tmp.path().join("a"));
    let b = init_openspec_repo(&tmp.path().join("b"));

    let cfg = tmp.path().join("workspaces.json");
    let registry = Arc::new(Mutex::new(WorkspaceRegistry::new(cfg)));
    {
        let mut reg = registry.lock().unwrap();
        reg.register(a).unwrap();
        reg.register(b).unwrap();
    }

    let watcher = WatcherManager::with_registry(TEST_DEBOUNCE, Some(registry));
    watcher.sync_repos();
    assert_eq!(watcher.repo_monitor_count(), 2);
    watcher.sync_repos();
    assert_eq!(
        watcher.repo_monitor_count(),
        2,
        "re-syncing must not install additional monitors"
    );
}

// -------------------------------------------------------------------------
// Invocation counting, non-blocking, and determinism coverage for the
// aggregation-hot-path optimizations (scoping, coalescing, lock release,
// concurrency, identity memoization).
// -------------------------------------------------------------------------

#[tokio::test]
async fn file_edit_in_one_repo_issues_no_status_invocations_for_another_repo() {
    invocation_log::enable();
    let tmp = TempDir::new().unwrap();
    let a = init_openspec_repo(&tmp.path().join("a"));
    let b = init_openspec_repo(&tmp.path().join("b"));

    let cfg = tmp.path().join("workspaces.json");
    let registry = Arc::new(Mutex::new(WorkspaceRegistry::new(cfg)));
    {
        let mut reg = registry.lock().unwrap();
        reg.register(a.clone()).unwrap();
        reg.register(b.clone()).unwrap();
    }
    let watcher = WatcherManager::with_registry(TEST_DEBOUNCE, Some(registry.clone()));
    {
        let folders = registry.lock().unwrap().folders();
        for folder in folders {
            watcher.add_workspace(folder).await.unwrap();
        }
    }
    watcher.sync_repos();
    // Seed last_views so both repos are already present in the snapshot —
    // otherwise the scoped path's own first-appearance fallback would (ic
    // correctly) perform a full recompute, which isn't what this test means
    // to exercise.
    watcher.aggregate_and_emit();

    let mut rx = watcher.subscribe();
    let mark = invocation_log::mark();

    // Edit a spec file in repo A only — the scoped file-change path this
    // change adds (group 4) should bound the resulting recompute to A.
    let change_dir = a.join("openspec/changes/foo");
    fs::create_dir_all(&change_dir).unwrap();
    fs::write(change_dir.join("proposal.md"), "x").unwrap();

    assert!(
        wait_for_event(&mut rx, |ev| matches!(
            ev,
            CacheEvent::ChangeAdded { workspace, change_id }
                if workspace == &a && change_id == "foo"
        ))
        .await,
        "expected ChangeAdded for repo A after the file edit"
    );

    let invocations = invocation_log::recorded_since(mark);
    let status_calls_for_b: Vec<_> = invocations
        .iter()
        .filter(|inv| inv.anchor.starts_with(&b) && inv.args.iter().any(|a| a == "status"))
        .collect();
    assert!(
        status_calls_for_b.is_empty(),
        "file edit in repo A must not issue `git status` for repo B: {status_calls_for_b:?}"
    );
}

#[tokio::test]
async fn second_file_edit_batch_reuses_the_memoized_git_identity() {
    invocation_log::enable();
    let tmp = TempDir::new().unwrap();
    let root = init_openspec_repo(&tmp.path().join("repo"));
    // A configured identity is required for `git_identity` to have anything
    // to spawn for in the first place (an unconfigured repo short-circuits
    // to `None` — see `init_openspec_repo`, which already sets both).

    let cfg = tmp.path().join("workspaces.json");
    let registry = Arc::new(Mutex::new(WorkspaceRegistry::new(cfg)));
    registry.lock().unwrap().register(root.clone()).unwrap();
    let watcher = WatcherManager::with_registry(TEST_DEBOUNCE, Some(registry.clone()));
    {
        let folders = registry.lock().unwrap().folders();
        for folder in folders {
            watcher.add_workspace(folder).await.unwrap();
        }
    }
    watcher.sync_repos();

    let mut rx = watcher.subscribe();

    // First batch: identity cache miss, spawns `git config --get user.*`.
    let change_dir1 = root.join("openspec/changes/foo");
    fs::create_dir_all(&change_dir1).unwrap();
    fs::write(change_dir1.join("proposal.md"), "x").unwrap();
    assert!(
        wait_for_event(&mut rx, |ev| matches!(
            ev,
            CacheEvent::ChangeAdded { change_id, .. } if change_id == "foo"
        ))
        .await,
        "expected ChangeAdded for the first batch"
    );

    // Second batch: the memo from the first batch should be reused.
    let mark = invocation_log::mark();
    let change_dir2 = root.join("openspec/changes/bar");
    fs::create_dir_all(&change_dir2).unwrap();
    fs::write(change_dir2.join("proposal.md"), "y").unwrap();
    assert!(
        wait_for_event(&mut rx, |ev| matches!(
            ev,
            CacheEvent::ChangeAdded { change_id, .. } if change_id == "bar"
        ))
        .await,
        "expected ChangeAdded for the second batch"
    );

    let invocations = invocation_log::recorded_since(mark);
    // Filtered to this test's own repo path — the invocation log is
    // process-global and shared with concurrently-running tests (see the
    // `invocation_log` module doc), so an unfiltered check could pick up an
    // unrelated test's own first-time `git config` read.
    let config_spawns: Vec<_> = invocations
        .iter()
        .filter(|inv| inv.anchor.starts_with(&root) && inv.args.iter().any(|a| a == "config"))
        .collect();
    assert!(
        config_spawns.is_empty(),
        "second batch must reuse the memoized identity, not re-spawn `git config`: {config_spawns:?}"
    );
}

// `reconcile()`'s "one recompute per batch, not one per added worktree"
// coalescing is covered by `reconcile_adding_three_worktrees_performs_
// exactly_one_recompute` in `repo_monitor.rs`'s own unit test module, which
// calls `reconcile()` directly — bypassing the debouncer entirely — rather
// than driving it through real filesystem events and waiting for the
// watcher to settle. An earlier version of this coverage lived here as
// exactly that kind of event-driven test; it proved flaky under
// `cargo test --workspace` once recomputes serialize on a dedicated lock
// (fixing the lost-update race the lock exists for), because the gap
// between the `reconcile`-triggered recompute finishing and the
// independent, pre-existing `status`-concern recompute starting could
// stretch past any reasonable fixed "quiet" window under heavy scheduling
// contention. Calling `reconcile` directly and awaiting it to completion
// has zero dependency on that timing.

#[tokio::test]
async fn concurrent_cache_write_is_not_blocked_by_an_in_flight_recompute() {
    // `Non-Blocking Aggregated Recompute`: a concurrent reader/writer of the
    // cache must not be blocked for the duration of a recompute's git I/O.
    // Raced rather than timed: a `add_workspace` for an unrelated, tiny,
    // non-git workspace (a stand-in for "another workspace's watcher")
    // finishing before a many-worktree recompute completes is a genuine
    // ordering proof, not a machine-speed-dependent timing threshold — it
    // can only happen if the recompute isn't holding the cache lock across
    // its git I/O.
    let tmp = TempDir::new().unwrap();
    let root = init_openspec_repo(&tmp.path().join("repo"));
    // Well beyond the real 12-repo/17-worktree registry scale (per the
    // proposal) — deliberately so. `add_workspace`'s own cost (parsing an
    // empty dir, a cache write, and standing up a real OS-level filesystem
    // watcher) is itself somewhat variable under load, so the margin needs
    // to be wide enough that the recompute's ~N/8 rounds of ~25-30ms git
    // spawns reliably dominates it, not just usually.
    const WORKTREES: usize = 60;
    for i in 0..WORKTREES {
        let wt = tmp.path().join(format!("wt{i}"));
        add_worktree(&root, &format!("b{i}"), &wt);
    }

    let cfg = tmp.path().join("workspaces.json");
    let registry = Arc::new(Mutex::new(WorkspaceRegistry::new(cfg)));
    registry.lock().unwrap().register(root.clone()).unwrap();
    let watcher = WatcherManager::with_registry(TEST_DEBOUNCE, Some(registry.clone()));
    {
        let folders = registry.lock().unwrap().folders();
        for folder in folders {
            watcher.add_workspace(folder).await.unwrap();
        }
    }

    let recompute_done = Arc::new(AtomicBool::new(false));
    let watcher_for_recompute = watcher.clone();
    let done_flag = recompute_done.clone();
    let recompute_handle = std::thread::spawn(move || {
        // Sync call on a plain OS thread — `aggregate_and_emit` /
        // `refresh_aggregated_view` are fully synchronous (the concurrency
        // inside is `std::thread::scope`, not tokio), so this needs no
        // runtime context.
        watcher_for_recompute.aggregate_and_emit();
        done_flag.store(true, Ordering::SeqCst);
    });
    // Give the recompute thread a head start so it is past its
    // (microsecond-scale) gather phase and actively into its git I/O before
    // the concurrent write below is attempted — not correctness-critical
    // (the assertion is a genuine ordering check either way, and a too-long
    // head start only makes the race harder to win, never invalidates a
    // pass), just biases the timing favourably against "the recompute
    // thread simply hadn't been scheduled yet" as a false explanation for
    // `add_workspace` finishing first.
    tokio::time::sleep(Duration::from_millis(10)).await;

    let other = tmp.path().join("other-flat");
    fs::create_dir_all(other.join("openspec/changes")).unwrap();
    let other_ws = openspec_core::WorkspaceFolder::from_path(other.canonicalize().unwrap());
    watcher.add_workspace(other_ws).await.unwrap();

    assert!(
        !recompute_done.load(Ordering::SeqCst),
        "the concurrent add_workspace finished only after the {WORKTREES}-worktree recompute \
         had already completed — the two operations were serialized, so the cache lock \
         appears to still be held across the recompute's git I/O"
    );

    recompute_handle.join().unwrap();
}
