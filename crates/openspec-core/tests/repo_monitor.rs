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

/// Wait until git invocations stop arriving, then return.
///
/// Several assertions below have the shape "no `git <verb>` anchored at X was
/// recorded since this mark". Seeding a watcher — `add_workspace`,
/// `sync_repos`, `aggregate_and_emit` — issues git work on background threads,
/// so on a loaded machine some of that seeding is still in flight when the mark
/// is taken, and is then misread as having been provoked by the edit under
/// test. That is a property of the *test*, not of the scoping being asserted:
/// the same run passes on an idle machine and fails under `cargo mutants`,
/// which builds in a temp tree and runs the binary's tests concurrently.
///
/// Marking only once the log has been quiet for several consecutive polls
/// removes the misattribution, and cannot weaken any assertion: waiting longer
/// can only move invocations *before* the mark, never after it. Deliberately
/// best-effort rather than a `wait_until` — if the log never settles we fall
/// through and behave exactly as before instead of introducing a new panic.
async fn wait_for_git_quiescence() {
    const STABLE_POLLS: u32 = 3;
    let start = Instant::now();
    let mut last = invocation_log::recorded_since(0).len();
    let mut stable = 0;
    while start.elapsed() < EVENT_TIMEOUT && stable < STABLE_POLLS {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let now = invocation_log::recorded_since(0).len();
        if now == last {
            stable += 1;
        } else {
            last = now;
            stable = 0;
        }
    }
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
    // The seeding above issues `git status` for both repos on background
    // threads; a straggler for B landing after the mark would be read as
    // provoked by the edit below. See `wait_for_git_quiescence`.
    wait_for_git_quiescence().await;
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
    // Quiesce first for the same reason as above — the first batch's trailing
    // git work must land before the mark, not after it.
    wait_for_git_quiescence().await;
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
//
// `Non-Blocking Aggregated Recompute` — that a concurrent cache writer is not
// blocked for the duration of a recompute's git I/O — also used to live here,
// as a race between a 60-worktree `aggregate_and_emit` and an `add_workspace`.
// It went the same way, and for the same reason: `add_workspace` stands up a
// real OS-level filesystem watcher, which on macOS is slow and highly
// variable, so on fast hardware it lost a race it is supposed to win and the
// test failed deterministically. It now lives in
// `tests/recompute_concurrency.rs`, which parks the recompute at the phase
// boundary via `watcher::recompute_gate` instead of racing it.

// ---------------------------------------------------------------------------
// Remotes: read lazily, remembered, forgotten on a `.git/config` change
// (`pull-request-worktree-links`: *Repository Remote Identities*).
// ---------------------------------------------------------------------------

/// `git remote -v` invocations anchored under `root` since `mark`. Filtered to
/// this test's repository because the invocation log is process-global.
fn remote_reads_since(mark: usize, root: &Path) -> usize {
    invocation_log::recorded_since(mark)
        .iter()
        .filter(|inv| {
            inv.anchor.starts_with(root)
                && inv.args.iter().any(|a| a == "remote")
                && inv.args.iter().any(|a| a == "-v")
        })
        .count()
}

fn origin_url(remotes: &[openspec_core::Remote]) -> Option<String> {
    remotes
        .iter()
        .find(|r| r.name == "origin")
        .map(|r| r.url.clone())
}

fn git_stdout(args: &[&str], cwd: &Path) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git invocation");
    assert!(out.status.success(), "git {args:?} failed");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Read once and remembered; a `git remote set-url` forgets the memo and
/// announces a refresh, and the next read sees the new URL. Done twice in a
/// row on purpose: git rewrites `.git/config` by renaming `config.lock` over
/// it, and the second round is what proves the config watch survives that
/// rename — the Linux CI runner is where it would fail (tasks.md 1.8).
#[tokio::test]
async fn remotes_are_read_once_and_reread_after_each_config_change() {
    invocation_log::enable();
    let tmp = TempDir::new().unwrap();
    let root = init_openspec_repo(&tmp.path().join("repo"));
    git(
        &["remote", "add", "origin", "git@github.com:acme/api.git"],
        &root,
    );
    let registry = Arc::new(Mutex::new(WorkspaceRegistry::new(
        tmp.path().join("workspaces.json"),
    )));
    let watcher = watched_repo(&root, registry).await;
    let repo_id = openspec_core::git_common_dir(&root).expect("a repository");
    wait_for_git_quiescence().await;

    let mark = invocation_log::mark();
    let first = watcher.remotes(&repo_id);
    let second = watcher.remotes(&repo_id);
    assert_eq!(first, second);
    assert_eq!(
        origin_url(&first).as_deref(),
        Some("git@github.com:acme/api.git")
    );
    assert_eq!(
        remote_reads_since(mark, &root),
        1,
        "read once, then remembered"
    );

    for next in [
        "git@github.com:acme/other.git",
        "https://github.com/acme/third.git",
    ] {
        wait_for_git_quiescence().await;
        let mut rx = watcher.subscribe();
        git(&["remote", "set-url", "origin", next], &root);
        assert!(
            wait_for_event(&mut rx, |ev| matches!(ev, CacheEvent::Updated { .. })).await,
            "the config change to {next} is announced as a refresh"
        );
        let mark = invocation_log::mark();
        let reread = watcher.remotes(&repo_id);
        assert_eq!(origin_url(&reread).as_deref(), Some(next));
        assert_eq!(
            remote_reads_since(mark, &root),
            1,
            "the memo was forgotten, so exactly one re-read"
        );
    }
}

/// A fetch rewrites remote-tracking refs, never the config: the remembered
/// remotes stay remembered.
#[tokio::test]
async fn a_remote_tracking_ref_update_keeps_the_remembered_remotes() {
    invocation_log::enable();
    let tmp = TempDir::new().unwrap();
    let root = init_openspec_repo(&tmp.path().join("repo"));
    git(
        &["remote", "add", "origin", "git@github.com:acme/api.git"],
        &root,
    );
    let registry = Arc::new(Mutex::new(WorkspaceRegistry::new(
        tmp.path().join("workspaces.json"),
    )));
    let watcher = watched_repo(&root, registry).await;
    let repo_id = openspec_core::git_common_dir(&root).expect("a repository");
    let _ = watcher.remotes(&repo_id);
    wait_for_git_quiescence().await;

    let mut rx = watcher.subscribe();
    let sha = git_stdout(&["rev-parse", "HEAD"], &root);
    let refs = root.join(".git/refs/remotes/origin");
    fs::create_dir_all(&refs).unwrap();
    fs::write(refs.join("main"), format!("{sha}\n")).unwrap();
    assert!(
        wait_for_event(&mut rx, |ev| matches!(ev, CacheEvent::GraphChanged { .. })).await,
        "the ref update is seen"
    );
    wait_for_git_quiescence().await;

    let mark = invocation_log::mark();
    let remotes = watcher.remotes(&repo_id);
    assert_eq!(
        origin_url(&remotes).as_deref(),
        Some("git@github.com:acme/api.git")
    );
    assert_eq!(
        remote_reads_since(mark, &root),
        0,
        "a ref update must not forget the remembered remotes"
    );
}

/// A disabled repository's config change still refreshes its (cold) row, but
/// spawns neither `git remote -v` nor `git status` for it.
#[tokio::test]
async fn a_disabled_repository_config_change_spawns_no_remote_or_status_read() {
    use openspec_core::presentation::{PresentationKey, WorkspacePresentationStore};

    invocation_log::enable();
    let tmp = TempDir::new().unwrap();
    let root = init_openspec_repo(&tmp.path().join("repo"));
    git(
        &["remote", "add", "origin", "git@github.com:acme/api.git"],
        &root,
    );
    let registry = Arc::new(Mutex::new(WorkspaceRegistry::new(
        tmp.path().join("workspaces.json"),
    )));
    registry.lock().unwrap().register(root.clone()).unwrap();
    let repo_id = registry
        .lock()
        .unwrap()
        .entry(&root)
        .unwrap()
        .repo_id
        .clone()
        .unwrap();
    let store = Arc::new(Mutex::new(WorkspacePresentationStore::new(
        tmp.path().join("presentation.json"),
    )));
    store
        .lock()
        .unwrap()
        .set_disabled(PresentationKey::Repo(repo_id.as_path().to_path_buf()), true)
        .unwrap();
    let watcher = WatcherManager::with_registry(TEST_DEBOUNCE, Some(registry.clone()));
    watcher.set_presentation(store);
    let folders = registry.lock().unwrap().folders();
    for folder in folders {
        watcher.add_workspace(folder).await.unwrap();
    }
    watcher.sync_repos();
    wait_for_git_quiescence().await;

    let mark = invocation_log::mark();
    let mut rx = watcher.subscribe();
    git(&["config", "user.name", "someone-else"], &root);
    assert!(
        wait_for_event(&mut rx, |ev| matches!(ev, CacheEvent::Updated { .. })).await,
        "the config change is still announced"
    );
    wait_for_git_quiescence().await;
    let spawned: Vec<_> = invocation_log::recorded_since(mark)
        .into_iter()
        .filter(|inv| {
            inv.anchor.starts_with(&root) && inv.args.iter().any(|a| a == "status" || a == "remote")
        })
        .collect();
    assert!(
        spawned.is_empty(),
        "a disabled repository reads no remotes and no status: {spawned:?}"
    );
}

/// `git branch -u` changes a branch's upstream through `.git/config` alone —
/// no worktree file changes — yet the recorded upstream follows it without a
/// restart, because the `remotes` concern refreshes status
/// (`pull-request-worktree-links`: *An upstream set through configuration
/// takes effect*; *Worktree Upstreams Are Recorded*).
#[tokio::test]
async fn an_upstream_set_through_configuration_reaches_the_view() {
    let tmp = TempDir::new().unwrap();
    let root = init_openspec_repo(&tmp.path().join("repo"));
    git(
        &["remote", "add", "origin", "git@github.com:acme/api.git"],
        &root,
    );
    let sha = git_stdout(&["rev-parse", "HEAD"], &root);
    git(&["update-ref", "refs/remotes/origin/feature", &sha], &root);
    let registry = Arc::new(Mutex::new(WorkspaceRegistry::new(
        tmp.path().join("workspaces.json"),
    )));
    let watcher = watched_repo(&root, registry).await;
    watcher.aggregate_and_emit();

    let refs_of = |watcher: &WatcherManager| {
        watcher
            .workspace_views()
            .iter()
            .find_map(|view| match view {
                WorkspaceView::Repo(repo) => repo.worktree_refs.first().cloned(),
                WorkspaceView::Flat { .. } => None,
            })
            .expect("the repository's worktree")
    };
    let before = refs_of(&watcher);
    assert_eq!(before.branch.as_deref(), Some("main"));
    assert_eq!(before.upstream, None, "nothing tracked yet");

    wait_for_git_quiescence().await;
    let mut rx = watcher.subscribe();
    git(&["branch", "-u", "origin/feature"], &root);
    assert!(
        wait_for_event(&mut rx, |ev| matches!(ev, CacheEvent::Updated { .. })).await,
        "the config-only change is announced as a refresh"
    );
    wait_until(|| refs_of(&watcher).upstream.as_deref() == Some("origin/feature")).await;
    assert_eq!(refs_of(&watcher).branch.as_deref(), Some("main"));
}
