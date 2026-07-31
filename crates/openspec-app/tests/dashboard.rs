//! The dashboard assembly used to live behind `#[tauri::command]` in the Tauri
//! shell, so it could not be exercised from `cargo test`. After the extraction
//! into `openspec-app::AppService` it is plain, in-process Rust — these tests
//! are the regression net that the extraction unlocked.

use openspec_app::AppService;
use openspec_core::git::invocation_log;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

/// The assembly is callable with no Tauri and no registered workspaces, and
/// returns the analytics-only payload (gamification is off by default).
#[tokio::test]
async fn dashboard_is_callable_headless_with_no_workspaces() {
    let dir = tempdir().unwrap();
    let svc = AppService::bootstrap(dir.path().to_path_buf());

    let data = svc.dashboard().await.expect("dashboard assembles headless");

    assert!(
        !data.gamification_enabled,
        "gamification is off by default, so the gamified layer is skipped"
    );
    assert!(
        data.season.is_none(),
        "no season standing without gamification"
    );
    assert!(data.repos.is_empty(), "no repos registered");
}

/// The season completion target is paced from the developer's *entry* baseline
/// and held fixed for the season — it must not drift between reads. This is the
/// regression the entry-baseline pacing fixed; before the extraction it could
/// not be asserted at the assembly level at all.
#[tokio::test]
async fn season_target_is_stable_across_reads() {
    let dir = tempdir().unwrap();
    let svc = AppService::bootstrap(dir.path().to_path_buf());
    svc.settings
        .set_gamification_enabled(true)
        .expect("enable gamification");

    let first = svc.dashboard().await.expect("first read");
    let second = svc.dashboard().await.expect("second read");

    assert!(first.gamification_enabled && second.gamification_enabled);
    assert!(
        first.season.is_some(),
        "gamified dashboard has a season standing"
    );

    let a = serde_json::to_value(&first.season).unwrap();
    let b = serde_json::to_value(&second.season).unwrap();
    assert_eq!(a, b, "season standing drifted between dashboard reads");
}

// --- Lifecycle-cache mining (cache-change-lifecycle, tasks 5.1 / 5.2) -------
//
// These exercise the *real* `change_lifecycle` git invocation through
// `AppService::dashboard()`, counted via `openspec_core::git::invocation_log`
// (the same test-only instrumentation `openspec-core`'s own watcher/repo
// monitor tests use to bound git spawn counts). Wall-clock is deliberately
// not asserted — see design.md's "Verification strategy".

fn git(args: &[&str], cwd: &Path) {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git invocation");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A git repo carrying an `openspec/changes/` tree with one committed active
/// change, so `change_lifecycle` has a real add-event to mine. An empty repo
/// would yield zero `A` rows regardless of whether mining ran at all, which
/// would make an invocation-count-only assertion the test's entire signal.
fn init_repo_with_a_change(root: &Path) -> PathBuf {
    let change_dir = root.join("openspec").join("changes").join("add-x");
    std::fs::create_dir_all(&change_dir).unwrap();
    std::fs::write(change_dir.join("proposal.md"), "# X").unwrap();
    git(&["init", "-b", "main"], root);
    git(&["config", "user.email", "t@t"], root);
    git(&["config", "user.name", "t"], root);
    git(&["add", "."], root);
    git(&["commit", "-m", "add x"], root);
    root.canonicalize().unwrap()
}

fn register(svc: &AppService, path: &Path) {
    svc.registry
        .lock()
        .unwrap()
        .register(path.to_path_buf())
        .unwrap();
}

/// Count real `change_lifecycle`/`change_lifecycle_checked` git invocations
/// issued for `repo_root` since `mark`. `--diff-filter=A` uniquely
/// discriminates this specific git call — no other invocation in this
/// codebase passes that flag — so filtering on it (plus the anchor path, to
/// stay isolated from any other test's git calls landing in the same
/// process-global log — see `invocation_log`'s module doc) counts exactly
/// the lifecycle-mining spawns.
fn lifecycle_mining_calls_for(mark: usize, repo_root: &Path) -> usize {
    let repo_root_canonical = repo_root.canonicalize().unwrap();
    invocation_log::recorded_since(mark)
        .into_iter()
        .filter(|inv| {
            inv.anchor.starts_with(&repo_root_canonical)
                && inv.args.iter().any(|a| a == "--diff-filter=A")
        })
        .count()
}

/// Task 5.1: two consecutive `dashboard()` fetches with no intervening
/// `GraphChanged` mine a registered repository exactly once, and report
/// identical lifecycle metrics.
#[tokio::test]
async fn unchanged_repository_is_mined_at_most_once_across_two_fetches() {
    invocation_log::enable();
    let dir = tempdir().unwrap();
    let svc = AppService::bootstrap(dir.path().to_path_buf());

    let roots = tempdir().unwrap();
    let repo = init_repo_with_a_change(&roots.path().join("repo"));
    register(&svc, &repo);

    // Marked BEFORE `populate()`, not after: `populate` starts a
    // fire-and-forget background warm (see `AppService::populate`), and
    // marking afterwards races it — if the warm wins, both `dashboard()`
    // fetches below would observe zero *new* mining calls instead of one.
    // Marking first brings the warm's own mining invocation inside the
    // counted window, so the count is deterministically 1 either way:
    // single-flight guarantees exactly one real invocation happens for this
    // repo, whether it's the warm, the first fetch, or both racing into one.
    let mark = invocation_log::mark();
    svc.populate().await;
    let first = svc.dashboard().await.expect("first fetch");
    let second = svc
        .dashboard()
        .await
        .expect("second fetch, no intervening event");

    assert_eq!(
        lifecycle_mining_calls_for(mark, &repo),
        1,
        "two fetches with no intervening GraphChanged must mine the repo exactly once"
    );
    assert_eq!(
        serde_json::to_value(&first.lifecycle).unwrap(),
        serde_json::to_value(&second.lifecycle).unwrap(),
        "lifecycle metrics must be identical across the two fetches"
    );
}

/// Task 5.2: a `GraphChanged` for repository A causes the *next* fetch to
/// re-mine A only — repository B, untouched by the event, stays cached.
/// The invalidation is processed by a background subscriber
/// (`AppService::spawn_lifecycle_cache_invalidator`), so this polls
/// `dashboard()` until the re-mine is observed rather than assuming a fixed
/// delay (a fixed sleep here would flake under CI load — see the project's
/// established "wait for activity, not a timer" testing convention).
#[tokio::test]
async fn graph_changed_re_mines_only_the_affected_repository() {
    invocation_log::enable();
    let dir = tempdir().unwrap();
    let svc = AppService::bootstrap(dir.path().to_path_buf());

    let roots = tempdir().unwrap();
    let repo_a = init_repo_with_a_change(&roots.path().join("a"));
    let repo_b = init_repo_with_a_change(&roots.path().join("b"));
    register(&svc, &repo_a);
    register(&svc, &repo_b);
    svc.populate().await;

    // Warm both repositories via one real fetch.
    svc.dashboard().await.expect("warm fetch");

    let repo_a_id = openspec_core::git_common_dir(&repo_a).expect("repo A has a git dir");

    let mark = invocation_log::mark();
    svc.watcher.emit(openspec_core::CacheEvent::GraphChanged {
        repo_id: repo_a_id.into_path_buf(),
    });

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        svc.dashboard().await.expect("poll fetch");
        if lifecycle_mining_calls_for(mark, &repo_a) >= 1 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "GraphChanged for repo A was never reflected in a re-mine within the timeout"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    assert_eq!(
        lifecycle_mining_calls_for(mark, &repo_a),
        1,
        "repo A must be re-mined exactly once after its GraphChanged"
    );
    assert_eq!(
        lifecycle_mining_calls_for(mark, &repo_b),
        0,
        "repo B must not be re-mined by repo A's GraphChanged"
    );
}

/// Regression test for finding M2 (adversarial review of `cache-change-lifecycle`):
/// unregistering a workspace must evict its repository's lifecycle-cache
/// entry, not just tear down its watcher/monitor. Without this, a repo's
/// `RepoMonitor` (and with it, the only source of `GraphChanged` — the
/// cache's sole invalidation signal) is torn down on unregister; a commit
/// landing while the repo is unregistered would then go unnoticed, and on
/// re-registration the Dashboard would keep serving the pre-removal snapshot
/// until some later, unrelated commit happened to invalidate it.
#[tokio::test]
async fn unregistering_a_workspace_evicts_its_lifecycle_cache_entry() {
    let dir = tempdir().unwrap();
    let svc = AppService::bootstrap(dir.path().to_path_buf());

    let roots = tempdir().unwrap();
    let repo = init_repo_with_a_change(&roots.path().join("repo"));
    register(&svc, &repo);
    svc.populate().await;

    // Warm the cache with a real fetch: nothing archived yet.
    let before = svc.dashboard().await.expect("warm fetch");
    assert_eq!(
        before.lifecycle.archived_in_window, 0,
        "precondition: nothing archived before the repo is unregistered"
    );

    // Unregister — this must evict the cached entry, not just the watcher.
    svc.remove_workspace(repo.clone())
        .await
        .expect("unregister");

    // A commit lands while the repo is unregistered — no monitor is
    // watching it, so no `GraphChanged` is possible for this change. Archive
    // the change that `init_repo_with_a_change` created.
    let archive_dir = repo.join("openspec/changes/archive/2026-01-01-add-x");
    std::fs::create_dir_all(&archive_dir).unwrap();
    std::fs::rename(
        repo.join("openspec/changes/add-x/proposal.md"),
        archive_dir.join("proposal.md"),
    )
    .unwrap();
    std::fs::remove_dir_all(repo.join("openspec/changes/add-x")).unwrap();
    git(&["add", "-A"], &repo);
    git(&["commit", "-m", "archive x"], &repo);

    // Re-register and refetch: the Dashboard must reflect the new history
    // immediately — not the pre-removal cached lifecycle, which (absent the
    // M2 fix) would still show zero archives.
    register(&svc, &repo);
    svc.populate().await;
    let after = svc.dashboard().await.expect("post-re-register fetch");

    assert_eq!(
        after.lifecycle.archived_in_window, 1,
        "the archive that landed while unregistered must be reflected once \
         re-registered, not served from a stale pre-removal cache entry"
    );
}
