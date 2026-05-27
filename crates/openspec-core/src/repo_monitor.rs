//! Per-repository monitors that keep the registry's discovered-worktree
//! set in sync with `git worktree list`, and that refresh the cached
//! default branch when the repository's git config or `origin/HEAD` ref
//! changes.
//!
//! One [`RepoMonitor`] is installed per repository that has at least one
//! tracked workspace. Dropping the monitor tears down its watchers and
//! aborts its tasks.

use crate::git::{self, RepoId};
use crate::registry::WorkspaceRegistry;
use crate::watcher::{CacheEvent, WatcherManager};
use notify::{RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Watches a single git repository for worktree adds/removes and for
/// default-branch changes. Owns the underlying `notify` watchers; dropping
/// the monitor disposes them.
pub struct RepoMonitor {
    repo_id: RepoId,
    default_branch: Arc<RwLock<Option<String>>>,
    _meta_debouncer: Option<Debouncer<notify::RecommendedWatcher, FileIdMap>>,
    _config_debouncer: Option<Debouncer<notify::RecommendedWatcher, FileIdMap>>,
    meta_task: Option<JoinHandle<()>>,
    config_task: Option<JoinHandle<()>>,
}

impl RepoMonitor {
    /// Install a monitor for `repo_id`. Spawns two tokio tasks: one that
    /// reconciles the registry's discovered set on `.git/worktrees/` events,
    /// and one that refreshes the cached default branch on `.git/config` or
    /// `.git/refs/remotes/origin/HEAD` events. Both are installed on a
    /// best-effort basis — a missing `.git/worktrees/` directory (the
    /// common case before any worktrees have ever been added) means no
    /// meta-watcher; it'll be installed on the next sync after a worktree
    /// appears.
    pub fn install(
        repo_id: RepoId,
        registry: Arc<Mutex<WorkspaceRegistry>>,
        watcher: WatcherManager,
        debounce: Duration,
    ) -> Self {
        let default_branch = Arc::new(RwLock::new(git::default_branch(&repo_id)));

        let (meta_debouncer, meta_task) =
            install_meta_watcher(&repo_id, registry.clone(), watcher.clone(), debounce);
        let (config_debouncer, config_task) =
            install_config_watcher(&repo_id, default_branch.clone(), debounce);

        Self {
            repo_id,
            default_branch,
            _meta_debouncer: meta_debouncer,
            _config_debouncer: config_debouncer,
            meta_task,
            config_task,
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
        if let Some(t) = self.meta_task.take() {
            t.abort();
        }
        if let Some(t) = self.config_task.take() {
            t.abort();
        }
    }
}

fn install_meta_watcher(
    repo_id: &RepoId,
    registry: Arc<Mutex<WorkspaceRegistry>>,
    watcher: WatcherManager,
    debounce: Duration,
) -> (
    Option<Debouncer<notify::RecommendedWatcher, FileIdMap>>,
    Option<JoinHandle<()>>,
) {
    let worktrees_dir = repo_id.as_path().join("worktrees");
    // `git` lazily creates `.git/worktrees/` on the first `git worktree add`.
    // Force-create it so the meta-watcher can attach immediately and pick
    // up the very first worktree the user (or harness) adds without us
    // missing the directory-creation event. Pre-existing empty
    // `.git/worktrees/` is benign — git treats it the same as missing.
    let _ = std::fs::create_dir_all(&worktrees_dir);
    let (tx, mut rx) = mpsc::unbounded_channel::<DebounceEventResult>();
    let debouncer_result = new_debouncer(debounce, None, move |result| {
        let _ = tx.send(result);
    });
    let Ok(mut debouncer) = debouncer_result else {
        return (None, None);
    };

    if worktrees_dir.is_dir() {
        let _ = debouncer
            .watcher()
            .watch(&worktrees_dir, RecursiveMode::NonRecursive);
    }

    let repo_id = repo_id.clone();
    let task = tokio::spawn(async move {
        while let Some(_result) = rx.recv().await {
            reconcile(&repo_id, &registry, &watcher).await;
        }
    });

    (Some(debouncer), Some(task))
}

fn install_config_watcher(
    repo_id: &RepoId,
    default_branch: Arc<RwLock<Option<String>>>,
    debounce: Duration,
) -> (
    Option<Debouncer<notify::RecommendedWatcher, FileIdMap>>,
    Option<JoinHandle<()>>,
) {
    let (tx, mut rx) = mpsc::unbounded_channel::<DebounceEventResult>();
    let debouncer_result = new_debouncer(debounce, None, move |result| {
        let _ = tx.send(result);
    });
    let Ok(mut debouncer) = debouncer_result else {
        return (None, None);
    };

    let config_path = repo_id.as_path().join("config");
    if config_path.is_file() {
        let _ = debouncer
            .watcher()
            .watch(&config_path, RecursiveMode::NonRecursive);
    }
    // Watch the directory that contains origin/HEAD — the file itself may
    // not exist for repos without an `origin` remote. The directory
    // typically does (.git/refs/remotes/origin/) if the repo has ever
    // had origin set up.
    let origin_dir = repo_id.as_path().join("refs/remotes/origin");
    if origin_dir.is_dir() {
        let _ = debouncer
            .watcher()
            .watch(&origin_dir, RecursiveMode::NonRecursive);
    }

    let repo_id = repo_id.clone();
    let task = tokio::spawn(async move {
        while let Some(_result) = rx.recv().await {
            let next = git::default_branch(&repo_id);
            *default_branch.write().unwrap() = next;
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
