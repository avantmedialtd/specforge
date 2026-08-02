//! `Non-Blocking Aggregated Recompute`: a concurrent cache *writer* must not
//! be blocked for the duration of a recompute's git I/O.
//!
//! This lives in its own integration target, with exactly one test, because
//! [`openspec_core::watcher::recompute_gate`] is process-global and
//! single-shot — a concurrently-running test that also triggered a recompute
//! could consume the gate. Nothing else in this binary recomputes.

use openspec_core::watcher::recompute_gate;
use openspec_core::{WatcherManager, WorkspaceRegistry};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;

const TEST_DEBOUNCE: Duration = Duration::from_millis(50);

/// Generous, and never reached on a pass — the recompute parks at the gate
/// immediately, and the probe either returns in microseconds or never. It
/// exists only so that a violated invariant fails with a diagnostic instead of
/// hanging the suite.
const RENDEZVOUS_TIMEOUT: Duration = Duration::from_secs(10);

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

#[tokio::test]
async fn cache_is_writable_while_a_recompute_is_in_its_git_io_phase() {
    let tmp = TempDir::new().unwrap();
    let root = init_openspec_repo(&tmp.path().join("repo"));

    // One worktree is enough. The earlier form of this coverage needed sixty,
    // purely to stretch the recompute's git I/O long enough to win a race
    // against the probe; the gate below removes the race, so the registry can
    // be minimal and the test costs one `git init` instead of sixty
    // `git worktree add`s.
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

    let gate = recompute_gate::arm();

    let watcher_for_recompute = watcher.clone();
    let recompute = std::thread::spawn(move || {
        // Sync call on a plain OS thread — `aggregate_and_emit` /
        // `refresh_aggregated_view` are fully synchronous (the concurrency
        // inside is `std::thread::scope`, not tokio), so this needs no
        // runtime context.
        watcher_for_recompute.aggregate_and_emit();
    });

    // Deterministic: the recompute is now parked between phase 1 and phase 2.
    // Its gather-phase registry and cache guards have been dropped; its git
    // subprocesses have not started. It stays there until we release it.
    gate.reached
        .recv_timeout(RENDEZVOUS_TIMEOUT)
        .expect("the recompute never reached the gather/compute phase boundary");

    // The probe: a bare cache write. `remove_workspace` on a path that was
    // never registered takes `watchers` and then the `cache` write lock, and
    // does nothing else — so this is a lock acquisition and nothing more, with
    // none of the directory parsing or OS-watcher setup that made
    // `add_workspace` the wrong instrument here.
    //
    // Run on its own thread with a bounded receive so a violated invariant
    // fails with a diagnostic instead of hanging: if the recompute still held
    // the cache read lock across phase 2, this write would block until the
    // recompute finished — and the recompute cannot finish, because we have
    // not released the gate. That is a deadlock by construction, which is
    // precisely the property under test, and the timeout turns it into a clean
    // failure.
    let never_registered = tmp.path().join("never-registered");
    let (probe_tx, probe_rx) = mpsc::channel();
    let watcher_for_probe = watcher.clone();
    std::thread::spawn(move || {
        let _ = probe_tx.send(watcher_for_probe.remove_workspace(&never_registered));
    });

    let wrote = probe_rx.recv_timeout(RENDEZVOUS_TIMEOUT);

    // Release the recompute and let it finish before asserting, so a failure
    // does not also leak a parked thread.
    drop(gate.release);
    recompute.join().unwrap();

    assert!(
        wrote.is_ok(),
        "a cache write blocked while a recompute was parked in its lock-free \
         git-I/O phase — the cache lock is being held across the recompute, \
         violating `Non-Blocking Aggregated Recompute`"
    );
}
