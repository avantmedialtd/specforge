//! End-to-end coverage of [`openspec_core::repo_monitor::RepoMonitor`] +
//! the `WatcherManager::sync_repos` integration.
//!
//! These tests shell out to the real `git` binary to set up worktrees and
//! verify that the meta-watcher picks up runtime worktree additions and
//! removals without user action.

use openspec_core::{WatcherManager, WorkspaceRegistry};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tempfile::TempDir;

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
