//! Per-repository monitors that keep the registry's discovered-worktree
//! set in sync with `git worktree list`, refresh the cached default branch
//! when the repository's git config or `origin/HEAD` ref changes, signal
//! commit-graph movement, and refresh the working-tree status rollup.
//!
//! One [`RepoMonitor`] is installed per repository that has at least one
//! tracked workspace. It owns a **single** `notify` debouncer watching every
//! repo-level git path; a debounced batch is classified by event path and
//! dispatched to the affected concerns, each run at most once per batch.
//! Dropping the monitor tears down the watcher and aborts its task.

use crate::git::{self, RepoId};
use crate::registry::WorkspaceRegistry;
use crate::watcher::{CacheEvent, WatcherManager};
use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Watches a single git repository for worktree adds/removes, default-branch
/// changes, commit-graph movement, and working-tree index changes — all through
/// one underlying `notify` watcher. Dropping the monitor disposes it.
pub struct RepoMonitor {
    repo_id: RepoId,
    default_branch: Arc<RwLock<Option<String>>>,
    /// The held debouncer keeps its internal watcher thread alive until the
    /// entry is dropped; it is never read back.
    _debouncer: Option<Debouncer<notify::RecommendedWatcher, RecommendedCache>>,
    task: Option<JoinHandle<()>>,
}

impl RepoMonitor {
    /// Install a monitor for `repo_id`. Spawns one tokio task driven by a single
    /// debouncer that watches `.git/worktrees/` (recursive), `.git/config`,
    /// `.git/refs` (recursive, which also covers `refs/remotes/origin/HEAD`),
    /// `.git/HEAD`, `.git/logs/HEAD`, `.git/packed-refs`, and `.git/index`. Each
    /// path is best-effort — a missing one (the common case for `logs/HEAD` /
    /// `packed-refs` before they exist) is simply not watched and is picked up on
    /// the next sync once it appears.
    pub fn install(
        repo_id: RepoId,
        registry: Arc<Mutex<WorkspaceRegistry>>,
        watcher: WatcherManager,
        debounce: Duration,
    ) -> Self {
        let default_branch = Arc::new(RwLock::new(git::default_branch(&repo_id)));
        let (debouncer, task) = install_watcher(
            &repo_id,
            registry,
            watcher,
            default_branch.clone(),
            debounce,
        );

        Self {
            repo_id,
            default_branch,
            _debouncer: debouncer,
            task,
        }
    }

    pub fn repo_id(&self) -> &RepoId {
        &self.repo_id
    }

    pub fn default_branch(&self) -> Option<String> {
        self.default_branch.read().unwrap().clone()
    }
}

impl Drop for RepoMonitor {
    fn drop(&mut self) {
        if let Some(t) = self.task.take() {
            t.abort();
        }
    }
}

/// The set of concerns a single debounced batch touches. A batch can touch more
/// than one (e.g. a checkout moves both refs and the index); each set concern is
/// dispatched exactly once, which is what coalesces a burst into one refresh.
#[derive(Default, Debug, PartialEq, Eq)]
struct Concerns {
    /// A worktree was added or removed (`.git/worktrees/<name>` appeared/vanished).
    reconcile: bool,
    /// The default branch may have moved (`.git/config` or `origin/HEAD`).
    default_branch: bool,
    /// History moved — emit `GraphChanged` (`.git/refs`, `HEAD`, `logs/HEAD`,
    /// `packed-refs`).
    graph: bool,
    /// Working-tree status may have changed (`.git/index` or a linked worktree's
    /// index/HEAD under `.git/worktrees/<name>/`).
    status: bool,
}

impl Concerns {
    /// OR another batch-path's concerns into this set. Merging is what collapses
    /// a burst (many paths, possibly many of the same concern) into a single
    /// dispatch per concern.
    fn merge(&mut self, other: Concerns) {
        self.reconcile |= other.reconcile;
        self.default_branch |= other.default_branch;
        self.graph |= other.graph;
        self.status |= other.status;
    }
}

/// The repo-level git paths a [`RepoMonitor`] watches, precomputed once so each
/// debounced path can be classified into the [`Concerns`] it touches.
struct RepoPaths {
    worktrees_dir: PathBuf,
    config_path: PathBuf,
    origin_dir: PathBuf,
    refs_dir: PathBuf,
    index_path: PathBuf,
    head: PathBuf,
    logs_head: PathBuf,
    packed_refs: PathBuf,
}

impl RepoPaths {
    fn new(git_dir: &Path) -> Self {
        Self {
            worktrees_dir: git_dir.join("worktrees"),
            config_path: git_dir.join("config"),
            origin_dir: git_dir.join("refs/remotes/origin"),
            refs_dir: git_dir.join("refs"),
            index_path: git_dir.join("index"),
            head: git_dir.join("HEAD"),
            logs_head: git_dir.join("logs/HEAD"),
            packed_refs: git_dir.join("packed-refs"),
        }
    }

    /// Classify a single changed path into the concerns it touches. A path may
    /// touch more than one (e.g. `refs/remotes/origin/HEAD` is both a graph ref
    /// and a default-branch signal).
    fn classify(&self, path: &Path) -> Concerns {
        let mut c = Concerns::default();
        if path.starts_with(&self.worktrees_dir) || path == self.index_path {
            c.status = true;
        }
        // Any event under `.git/worktrees/` may signal a worktree add/remove.
        // `git worktree add` emits the new entry's directory event interleaved
        // with the metadata it writes, so that event can arrive before
        // `git worktree list` would report it; keying reconcile on the whole
        // subtree (not just the bare entry) guarantees a later, settled batch
        // reconciles it. `reconcile` is idempotent and cheap (one
        // `git worktree list`), so the extra calls during linked-worktree git
        // activity are immaterial.
        if path.starts_with(&self.worktrees_dir) {
            c.reconcile = true;
        }
        if path == self.config_path || path.starts_with(&self.origin_dir) {
            c.default_branch = true;
        }
        if path.starts_with(&self.refs_dir)
            || path == self.head
            || path == self.logs_head
            || path == self.packed_refs
        {
            c.graph = true;
        }
        c
    }
}

/// Build the single debouncer and the processing task for a repository.
fn install_watcher(
    repo_id: &RepoId,
    registry: Arc<Mutex<WorkspaceRegistry>>,
    watcher: WatcherManager,
    default_branch: Arc<RwLock<Option<String>>>,
    debounce: Duration,
) -> (
    Option<Debouncer<notify::RecommendedWatcher, RecommendedCache>>,
    Option<JoinHandle<()>>,
) {
    let git_dir = repo_id.as_path().to_path_buf();
    let worktrees_dir = git_dir.join("worktrees");
    // `git` lazily creates `.git/worktrees/` on the first `git worktree add`.
    // Force-create it so the watcher attaches immediately and catches the very
    // first worktree the user (or harness) adds. A pre-existing empty
    // `.git/worktrees/` is benign — git treats it the same as missing.
    let _ = std::fs::create_dir_all(&worktrees_dir);

    let (tx, mut rx) = mpsc::unbounded_channel::<DebounceEventResult>();
    let debouncer_result = new_debouncer(debounce, None, move |result| {
        let _ = tx.send(result);
    });
    let Ok(mut debouncer) = debouncer_result else {
        return (None, None);
    };

    // One watcher, every repo-level path. `.git/refs` recursively also covers
    // `refs/remotes/origin/HEAD`, so origin is not watched separately. Each
    // `.watch()` is best-effort.
    {
        if worktrees_dir.is_dir() {
            let _ = debouncer.watch(&worktrees_dir, RecursiveMode::Recursive);
        }
        let config_path = git_dir.join("config");
        if config_path.is_file() {
            let _ = debouncer.watch(&config_path, RecursiveMode::NonRecursive);
        }
        let refs_dir = git_dir.join("refs");
        if refs_dir.is_dir() {
            let _ = debouncer.watch(&refs_dir, RecursiveMode::Recursive);
        }
        for file in ["HEAD", "logs/HEAD", "packed-refs"] {
            let path = git_dir.join(file);
            if path.is_file() {
                let _ = debouncer.watch(&path, RecursiveMode::NonRecursive);
            }
        }
        let index_path = git_dir.join("index");
        if index_path.is_file() {
            let _ = debouncer.watch(&index_path, RecursiveMode::NonRecursive);
        }
    }

    let repo_id = repo_id.clone();
    let task = tokio::spawn(async move {
        let paths = RepoPaths::new(repo_id.as_path());

        while let Some(result) = rx.recv().await {
            let events = match result {
                Ok(events) => events,
                Err(_errors) => continue,
            };

            // Merge every changed path's concerns into one set, so a burst of
            // many paths collapses to a single dispatch per concern.
            let mut concerns = Concerns::default();
            for event in &events {
                for path in &event.paths {
                    concerns.merge(paths.classify(path));
                }
            }

            // Dispatch each concern at most once per batch. Reconcile first so a
            // newly-added worktree is registered before the status recompute that
            // follows reflects it.
            if concerns.reconcile {
                reconcile(&repo_id, &registry, &watcher).await;
            }
            if concerns.default_branch {
                // Off the async runtime — `git::default_branch` shells out
                // (up to 3 spawns: the documented cascade), and a tokio
                // worker must not block on that subprocess I/O.
                let repo_id_for_blocking = repo_id.clone();
                let next =
                    tokio::task::spawn_blocking(move || git::default_branch(&repo_id_for_blocking))
                        .await
                        .unwrap();
                *default_branch.write().unwrap() = next;
                // `.git/config` is also the signal that invalidates the
                // memoized local git identity (`user.name`/`user.email` can
                // change in the same edit as `init.defaultBranch`), so both
                // are refreshed from this one dispatch.
                watcher.invalidate_identity(&repo_id);
            }
            if concerns.graph {
                watcher.emit(CacheEvent::GraphChanged {
                    repo_id: repo_id.as_path().to_path_buf(),
                });
            }
            if concerns.status {
                // Repo-scoped: a git event in this repo never triggers a
                // `git status` sweep of the other registered repos. Off the
                // async runtime, matching the `reconcile`/`default_branch`
                // dispatches above — this fires on every `.git/index`
                // write, the hottest of the four concerns, and
                // `refresh_status_for` shells out to `git status` per
                // worktree via the scoped recompute (plus now serializes
                // against any other in-flight recompute on the dedicated
                // `recompute` lock), so it must not run inline on a tokio
                // worker.
                let watcher_for_blocking = watcher.clone();
                let repo_id_for_blocking = repo_id.clone();
                tokio::task::spawn_blocking(move || {
                    watcher_for_blocking.refresh_status_for(&repo_id_for_blocking);
                })
                .await
                .unwrap();
            }
        }
    });

    (Some(debouncer), Some(task))
}

/// Reconcile the registry's discovered set for `repo_id` against
/// `git worktree list`, adding watchers for newly-appeared worktrees and
/// removing watchers for vanished ones. Idempotent — calling twice with no
/// on-disk change is a no-op. Emits `Updated` events so frontend subscribers
/// learn about the new/removed instance contents.
async fn reconcile(
    repo_id: &RepoId,
    registry: &Arc<Mutex<WorkspaceRegistry>>,
    watcher: &WatcherManager,
) {
    let (added, removed) = {
        let mut reg = match registry.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        reg.reconcile_repo(repo_id)
    };

    if added.is_empty() && removed.is_empty() {
        return;
    }

    // Stop watching paths that are gone first so we never leave a stale
    // watcher pinning a path that's been deleted. The per-worktree
    // `WorkspaceRemoved` / `Updated` raw events are *buffered* here rather
    // than emitted inline — `WatcherManager::emit`'s documented contract
    // requires the aggregated snapshot to be refreshed *before* any raw
    // cache event is emitted, so a subscriber reacting to the very first
    // event already sees the post-batch `workspace_views()` (otherwise a
    // frontend that fetches in response to `WorkspaceRemoved` could still
    // observe the removed worktree in the snapshot — it flashes in the tree
    // before the recompute catches up). Only the aggregated recompute below
    // is coalesced to a single call for the whole batch (previously this
    // loop called `refresh_aggregated_view()` on every iteration — a batch
    // touching N worktrees performed N full sweeps).
    let mut raw_events = Vec::new();
    for path in &removed {
        watcher.remove_workspace(path);
        raw_events.push(CacheEvent::WorkspaceRemoved {
            workspace: path.clone(),
        });
    }
    for folder in added {
        if !folder.uri.is_dir() {
            continue;
        }
        let workspace_path = folder.uri.clone();
        if let Err(e) = watcher.add_workspace(folder).await {
            eprintln!("failed to install watcher for discovered worktree: {e}");
            continue;
        }
        raw_events.push(CacheEvent::Updated {
            workspace: workspace_path,
        });
    }

    // One recompute for the whole batch rather than one per added/removed
    // worktree. Off the async runtime, matching the window-focus refresh
    // (`crates/specforge/src/lib.rs`): the recompute shells out to
    // `git status`/`git branch` per worktree, and a tokio worker must not
    // block on that subprocess I/O.
    let watcher_for_blocking = watcher.clone();
    let derived =
        tokio::task::spawn_blocking(move || watcher_for_blocking.refresh_aggregated_view())
            .await
            .unwrap();

    // Now emit: the aggregated snapshot already reflects the post-batch
    // state (the recompute above has completed), so every event below is
    // safe for a subscriber to react to by reading `workspace_views()`
    // immediately. Raw per-worktree events first (per the coalescing
    // contract), then the derived logical/instance diff events, computed
    // once against the post-batch set of worktrees.
    for event in raw_events {
        watcher.emit(event);
    }
    for event in derived {
        watcher.emit(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> RepoPaths {
        RepoPaths::new(Path::new("/repo/.git"))
    }

    fn classify(path: &str) -> Concerns {
        paths().classify(Path::new(path))
    }

    #[test]
    fn index_change_is_status_only() {
        assert_eq!(
            classify("/repo/.git/index"),
            Concerns {
                status: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn config_change_is_default_branch_only() {
        assert_eq!(
            classify("/repo/.git/config"),
            Concerns {
                default_branch: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn local_ref_change_is_graph_only() {
        assert_eq!(
            classify("/repo/.git/refs/heads/main"),
            Concerns {
                graph: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn head_and_logs_and_packed_refs_are_graph() {
        let graph = Concerns {
            graph: true,
            ..Default::default()
        };
        assert_eq!(classify("/repo/.git/HEAD"), graph);
        assert_eq!(
            classify("/repo/.git/logs/HEAD"),
            Concerns {
                graph: true,
                ..Default::default()
            }
        );
        assert_eq!(
            classify("/repo/.git/packed-refs"),
            Concerns {
                graph: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn origin_head_is_both_default_branch_and_graph() {
        // `refs/remotes/origin/HEAD` lives under `refs/` (graph) and under the
        // origin dir (default-branch signal) — it must trigger both.
        assert_eq!(
            classify("/repo/.git/refs/remotes/origin/HEAD"),
            Concerns {
                default_branch: true,
                graph: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn worktree_entry_is_reconcile_and_status() {
        assert_eq!(
            classify("/repo/.git/worktrees/wt2"),
            Concerns {
                reconcile: true,
                status: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn linked_worktree_index_is_reconcile_and_status() {
        assert_eq!(
            classify("/repo/.git/worktrees/wt2/index"),
            Concerns {
                reconcile: true,
                status: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn unrelated_git_path_touches_nothing() {
        assert_eq!(classify("/repo/.git/COMMIT_EDITMSG"), Concerns::default());
    }

    #[test]
    fn commit_burst_coalesces_to_one_status_and_one_graph() {
        // A commit writes the index plus refs/HEAD/logs in one batch. Merging the
        // per-path concerns must collapse to a single status + single graph
        // dispatch (not one per path) — the coalescing guarantee.
        let p = paths();
        let mut concerns = Concerns::default();
        for path in [
            "/repo/.git/index",
            "/repo/.git/refs/heads/main",
            "/repo/.git/HEAD",
            "/repo/.git/logs/HEAD",
        ] {
            concerns.merge(p.classify(Path::new(path)));
        }
        assert_eq!(
            concerns,
            Concerns {
                status: true,
                graph: true,
                ..Default::default()
            }
        );
    }

    // -------------------------------------------------------------------
    // `reconcile()` invocation counting, called directly (bypassing the
    // debouncer entirely) so the assertion has zero dependency on
    // filesystem-event timing or debounce-batch splitting. This is
    // deliberately *not* driven through the real notify watcher: an
    // earlier version of this test lived in `tests/repo_monitor.rs` and
    // drove it by issuing real `git worktree add` calls and waiting for the
    // debouncer to settle, which proved flaky under `cargo test --workspace`
    // (heavy scheduling contention could stretch the gap between `reconcile`'s
    // own recompute finishing and the *independent* `concerns.status`
    // dispatch's recompute starting — both now serialize on the `recompute`
    // lock — well past any reasonable fixed "quiet" window). Calling
    // `reconcile` directly and awaiting it to completion sidesteps that
    // entirely: every git call it issues is guaranteed to be in the log the
    // moment `.await` returns, no polling required.
    // -------------------------------------------------------------------

    fn test_git(args: &[&str], cwd: &Path) {
        let out = std::process::Command::new("git")
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

    fn test_init_openspec_repo(root: &Path) -> PathBuf {
        std::fs::create_dir_all(root.join("openspec/changes")).unwrap();
        test_git(&["init", "-b", "main"], root);
        test_git(&["config", "user.email", "t@t"], root);
        test_git(&["config", "user.name", "t"], root);
        test_git(&["commit", "--allow-empty", "-m", "init"], root);
        root.canonicalize().unwrap()
    }

    #[tokio::test]
    async fn reconcile_adding_three_worktrees_performs_exactly_one_recompute() {
        crate::git::invocation_log::enable();
        let tmp = tempfile::TempDir::new().unwrap();
        let root = test_init_openspec_repo(&tmp.path().join("repo"));

        let cfg = tmp.path().join("workspaces.json");
        let registry = Arc::new(Mutex::new(WorkspaceRegistry::new(cfg)));
        registry.lock().unwrap().register(root.clone()).unwrap();
        let watcher =
            WatcherManager::with_registry(Duration::from_millis(50), Some(registry.clone()));
        {
            let folders = registry.lock().unwrap().folders();
            for folder in folders {
                watcher.add_workspace(folder).await.unwrap();
            }
        }
        watcher.aggregate_and_emit();
        let repo_id = registry
            .lock()
            .unwrap()
            .entry(&root)
            .unwrap()
            .repo_id
            .clone()
            .unwrap();

        // Add all 3 worktrees on disk BEFORE calling reconcile — a single
        // `reconcile` call's own `git worktree list` diff then discovers all
        // 3 as newly-added in one pass, exactly like a settled debounced
        // batch would.
        for w in 1..=3 {
            let wt = tmp.path().join(format!("wt{w}"));
            test_git(
                &[
                    "worktree",
                    "add",
                    "-b",
                    &format!("b{w}"),
                    wt.to_str().unwrap(),
                ],
                &root,
            );
            std::fs::create_dir_all(wt.join("openspec/changes")).unwrap();
        }

        let mark = crate::git::invocation_log::mark();
        reconcile(&repo_id, &registry, &watcher).await;

        // `invocation_log` is process-global and shared with every other
        // test running concurrently in this same lib-test binary — filter
        // to this test's own tempdir so an unrelated test's git calls
        // landing inside this `mark`..`recorded_since` window (very
        // plausible under `cargo test --workspace`'s parallelism) can't
        // inflate the counts below. See the `invocation_log` module doc.
        // Canonicalized because recorded anchors are (via `root`'s own
        // `.canonicalize()` above) — `tmp.path()` itself is frequently a
        // symlinked form (e.g. macOS's `/var/...` vs. `/private/var/...`)
        // that would never prefix-match otherwise.
        let tmp_canonical = tmp.path().canonicalize().unwrap();
        let invocations: Vec<_> = crate::git::invocation_log::recorded_since(mark)
            .into_iter()
            .filter(|inv| inv.anchor.starts_with(&tmp_canonical))
            .collect();
        let status_calls = invocations
            .iter()
            .filter(|inv| inv.args.iter().any(|a| a == "status"))
            .count();
        assert_eq!(
            status_calls, 4,
            "one reconcile() call over a repo with 4 worktrees (main + 3 added) must \
             perform exactly one coalesced recompute — 4 status calls, not one recompute \
             per added worktree: {invocations:?}"
        );
        let worktree_list_calls = invocations
            .iter()
            .filter(|inv| inv.args.first().map(String::as_str) == Some("worktree"))
            .count();
        assert_eq!(
            worktree_list_calls, 2,
            "exactly two `git worktree list` calls: one from `reg.reconcile_repo`'s truth \
             lookup, one from the single coalesced recompute's main-worktree resolution — \
             not one additional call per added worktree: {invocations:?}"
        );
    }
}
