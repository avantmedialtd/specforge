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
use notify::{RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
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
    _debouncer: Option<Debouncer<notify::RecommendedWatcher, FileIdMap>>,
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
    Option<Debouncer<notify::RecommendedWatcher, FileIdMap>>,
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
        let w = debouncer.watcher();
        if worktrees_dir.is_dir() {
            let _ = w.watch(&worktrees_dir, RecursiveMode::Recursive);
        }
        let config_path = git_dir.join("config");
        if config_path.is_file() {
            let _ = w.watch(&config_path, RecursiveMode::NonRecursive);
        }
        let refs_dir = git_dir.join("refs");
        if refs_dir.is_dir() {
            let _ = w.watch(&refs_dir, RecursiveMode::Recursive);
        }
        for file in ["HEAD", "logs/HEAD", "packed-refs"] {
            let path = git_dir.join(file);
            if path.is_file() {
                let _ = w.watch(&path, RecursiveMode::NonRecursive);
            }
        }
        let index_path = git_dir.join("index");
        if index_path.is_file() {
            let _ = w.watch(&index_path, RecursiveMode::NonRecursive);
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
                let next = git::default_branch(&repo_id);
                *default_branch.write().unwrap() = next;
            }
            if concerns.graph {
                watcher.emit(CacheEvent::GraphChanged {
                    repo_id: repo_id.as_path().to_path_buf(),
                });
            }
            if concerns.status {
                // Repo-scoped: a git event in this repo never triggers a
                // `git status` sweep of the other registered repos.
                watcher.refresh_status_for(&repo_id);
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
    // watcher pinning a path that's been deleted. Refresh the aggregated
    // view *before* each raw emit so any subscriber that reads
    // `workspace_views()` in response observes the post-event snapshot —
    // matching the ordering guarantee `Inner::handle_events` provides on
    // file-edit batches.
    for path in &removed {
        watcher.remove_workspace(path);
        let derived = watcher.refresh_aggregated_view();
        watcher.emit(CacheEvent::WorkspaceRemoved {
            workspace: path.clone(),
        });
        for event in derived {
            watcher.emit(event);
        }
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
        let derived = watcher.refresh_aggregated_view();
        watcher.emit(CacheEvent::Updated {
            workspace: workspace_path,
        });
        for event in derived {
            watcher.emit(event);
        }
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
}
