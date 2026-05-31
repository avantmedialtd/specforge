use crate::cache::WorkspaceCache;
use crate::git::RepoId;
use crate::parser::parse_all_changes;
use crate::registry::WorkspaceRegistry;
use crate::repo_monitor::RepoMonitor;
use crate::repo_view::{compute_views, diff_views, WorkspaceView};
use crate::self_write::SelfWriteTracker;
use crate::types::WorkspaceFolder;
use notify::{RecursiveMode, Watcher};
use notify_debouncer_full::{
    new_debouncer, DebounceEventResult, DebouncedEvent, Debouncer, FileIdMap,
};
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(200);
const SELF_WRITE_TTL: Duration = Duration::from_secs(2);
const EVENT_CHANNEL_CAPACITY: usize = 128;

/// Notification the watcher emits when the cache changes. The Tauri shell
/// translates these variants into named Tauri events.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CacheEvent {
    /// One or more files in the workspace changed; the cache for the
    /// workspace was re-parsed.
    Updated { workspace: PathBuf },
    /// A new active change directory appeared in the workspace.
    ChangeAdded {
        workspace: PathBuf,
        change_id: String,
    },
    /// An existing change directory moved into `openspec/changes/archive/`.
    ChangeArchived {
        workspace: PathBuf,
        change_id: String,
    },
    /// A previously-tracked workspace was removed (the worktree containing
    /// it disappeared from `git worktree list`, or was unregistered).
    WorkspaceRemoved { workspace: PathBuf },
    /// A new logical change first appeared in a repository — emitted by the
    /// aggregator when the `(repo_id, change_name)` tuple has its first
    /// non-archived instance anywhere.
    LogicalChangeAdded {
        repo_id: PathBuf,
        change_name: String,
    },
    /// Every instance of a logical change is now archived — emitted by the
    /// aggregator when the last active instance moves into `archive/`.
    LogicalChangeArchived {
        repo_id: PathBuf,
        change_name: String,
    },
    /// A new instance of a logical change appeared (a worktree began
    /// containing it). Fires per-worktree, not for the first appearance.
    InstanceAdded {
        repo_id: PathBuf,
        change_name: String,
        worktree_path: PathBuf,
    },
    /// An instance of a logical change disappeared (the worktree was pruned
    /// or the change directory was removed from that worktree).
    InstanceRemoved {
        repo_id: PathBuf,
        change_name: String,
        worktree_path: PathBuf,
    },
    /// A repository's refs moved — a new commit, branch create/delete/move,
    /// tag change, or HEAD movement. The commit-graph rail re-fetches the
    /// affected repo's graph. Unlike the cache events above this carries no
    /// OpenSpec state; it is a pure "the git history changed" signal.
    GraphChanged { repo_id: PathBuf },
}

#[derive(Debug, Error)]
pub enum WatcherError {
    #[error("failed to parse workspace at {path}")]
    Parse {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Notify(#[from] notify::Error),
    #[error("workspace path is not a directory: {0}")]
    NotADirectory(PathBuf),
}

/// Manages per-workspace filesystem watchers and an in-memory cache of
/// parsed OpenSpec state. Clone the manager to share it across tasks; all
/// clones share the same inner state via `Arc`.
#[derive(Clone)]
pub struct WatcherManager {
    inner: Arc<Inner>,
}

struct Inner {
    cache: RwLock<WorkspaceCache>,
    watchers: Mutex<HashMap<PathBuf, WatcherEntry>>,
    repo_monitors: Mutex<HashMap<RepoId, RepoMonitor>>,
    last_views: RwLock<Vec<WorkspaceView>>,
    event_tx: broadcast::Sender<CacheEvent>,
    self_writes: SelfWriteTracker,
    debounce: Duration,
    registry: Option<Arc<Mutex<WorkspaceRegistry>>>,
    /// Optional activity log the watcher records observed achievements into.
    /// `None` in unit-test contexts that don't persist activity.
    activity_log: RwLock<Option<Arc<crate::activity_log::ActivityLog>>>,
}

struct WatcherEntry {
    /// Kept alive so its internal threads keep running.
    _debouncer: Debouncer<notify::RecommendedWatcher, FileIdMap>,
    /// Aborted on workspace removal.
    task: JoinHandle<()>,
}

impl Default for WatcherManager {
    fn default() -> Self {
        Self::new(DEFAULT_DEBOUNCE)
    }
}

impl WatcherManager {
    pub fn new(debounce: Duration) -> Self {
        Self::with_registry(debounce, None)
    }

    /// Build a manager that knows about a [`WorkspaceRegistry`]. The registry
    /// is required for repository-monitor functionality (worktree
    /// auto-discovery). Pass `None` for unit-test contexts that don't need
    /// repo monitoring.
    pub fn with_registry(
        debounce: Duration,
        registry: Option<Arc<Mutex<WorkspaceRegistry>>>,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            inner: Arc::new(Inner {
                cache: RwLock::new(WorkspaceCache::new()),
                watchers: Mutex::new(HashMap::new()),
                repo_monitors: Mutex::new(HashMap::new()),
                last_views: RwLock::new(Vec::new()),
                event_tx,
                self_writes: SelfWriteTracker::new(SELF_WRITE_TTL),
                debounce,
                registry,
                activity_log: RwLock::new(None),
            }),
        }
    }

    /// Attach the activity log the watcher records observed achievements into.
    /// Optional — without it, detection still runs but nothing is persisted.
    pub fn set_activity_log(&self, log: Arc<crate::activity_log::ActivityLog>) {
        *self.inner.activity_log.write().unwrap() = Some(log);
    }

    /// Public emit helper used by the repo monitor to surface events on the
    /// shared broadcast channel.
    ///
    /// Callers emitting a raw cache event (`Updated`, `ChangeAdded`,
    /// `ChangeArchived`, `WorkspaceRemoved`) MUST call
    /// [`Self::refresh_aggregated_view`] (or [`Self::aggregate_and_emit`])
    /// *before* this `emit` so the cached `last_views` snapshot already
    /// reflects the post-event state when subscribers wake. The broadcast
    /// channel has no aggregator subscriber to catch up after the fact;
    /// subscribers that read `workspace_views()` in response to an event
    /// would otherwise observe the pre-event snapshot.
    pub fn emit(&self, event: CacheEvent) {
        let _ = self.inner.event_tx.send(event);
    }

    /// Cached default branch for `repo_id`, if a monitor is installed and
    /// has resolved one. `None` means either no monitor for this repo or
    /// no branch could be determined.
    pub fn default_branch(&self, repo_id: &RepoId) -> Option<String> {
        self.inner
            .repo_monitors
            .lock()
            .ok()?
            .get(repo_id)
            .and_then(RepoMonitor::default_branch)
    }

    /// Cached aggregated views (one per top-level entry — git repo or flat
    /// workspace). Recomputed by [`Self::aggregate_and_emit`] on every raw
    /// cache change. Drives the new `get_workspace_views` frontend command.
    pub fn workspace_views(&self) -> Vec<WorkspaceView> {
        self.inner.last_views.read().unwrap().clone()
    }

    /// Total number of non-archived *logical changes* across all
    /// `WorkspaceView::Repo` entries plus all `WorkspaceView::Flat` changes.
    /// Drives the tray badge — a logical change touched by N worktrees
    /// contributes 1, not N.
    pub fn total_active_logical_count(&self) -> usize {
        let views = self.inner.last_views.read().unwrap();
        views
            .iter()
            .map(|v| match v {
                WorkspaceView::Repo(r) => r.active.len(),
                WorkspaceView::Flat { changes, .. } => changes.len(),
            })
            .sum()
    }

    /// Recompute the aggregated views from the registry + cache, write the
    /// new `last_views` snapshot, and return the logical/instance diff
    /// events that should be broadcast. The caller is responsible for
    /// sending the returned events through [`Self::emit`] at the
    /// appropriate point in the pipeline — typically *after* the raw
    /// cache events that triggered the refresh, so subscribers see
    /// `Updated → LogicalChangeAdded/InstanceAdded/…` in the same order
    /// the previous broadcast-subscriber aggregator produced. Idempotent
    /// — running twice without intervening state changes returns an empty
    /// vector.
    pub fn refresh_aggregated_view(&self) -> Vec<CacheEvent> {
        self.inner.refresh_aggregated_view()
    }

    /// Convenience wrapper that calls [`Self::refresh_aggregated_view`] and
    /// then broadcasts every returned event. Used by callers that want a
    /// one-shot "recompute and announce" without managing event ordering
    /// themselves — e.g. the startup populate path and external test code.
    pub fn aggregate_and_emit(&self) {
        for event in self.refresh_aggregated_view() {
            self.emit(event);
        }
    }

    /// Reconcile the set of installed repo monitors against the set of
    /// distinct repositories in the registry. Called after every register /
    /// unregister, plus once at startup. Idempotent.
    ///
    /// Requires the manager to have been built via [`Self::with_registry`].
    /// In contexts without a registry (e.g. unit tests of the watcher
    /// itself) this is a no-op.
    pub fn sync_repos(&self) {
        let registry = match self.inner.registry.as_ref() {
            Some(r) => r.clone(),
            None => return,
        };
        let desired: HashSet<RepoId> = match registry.lock() {
            Ok(g) => g.repos().into_iter().collect(),
            Err(_) => return,
        };

        let mut monitors = self.inner.repo_monitors.lock().unwrap();
        // Remove monitors for repos no longer in the registry.
        let to_remove: Vec<RepoId> = monitors
            .keys()
            .filter(|id| !desired.contains(*id))
            .cloned()
            .collect();
        for id in to_remove {
            monitors.remove(&id);
        }
        // Add monitors for newly-tracked repos.
        for id in desired {
            if monitors.contains_key(&id) {
                continue;
            }
            let monitor = RepoMonitor::install(
                id.clone(),
                registry.clone(),
                self.clone(),
                self.inner.debounce,
            );
            monitors.insert(id, monitor);
        }
    }

    /// Subscribe to cache events. Subscribers must keep up; if a subscriber
    /// lags by more than `EVENT_CHANNEL_CAPACITY` events, it will see
    /// `RecvError::Lagged` on its next recv.
    pub fn subscribe(&self) -> broadcast::Receiver<CacheEvent> {
        self.inner.event_tx.subscribe()
    }

    /// Snapshot of the cache's current contents.
    pub fn snapshot(&self) -> HashMap<PathBuf, Vec<crate::types::ChangeData>> {
        self.inner.cache.read().unwrap().snapshot()
    }

    /// Cached changes for a single workspace. Empty if the workspace is not
    /// registered.
    pub fn changes_for(&self, workspace: &Path) -> Vec<crate::types::ChangeData> {
        self.inner
            .cache
            .read()
            .unwrap()
            .changes_for(workspace)
            .to_vec()
    }

    /// Sum of non-archived changes across all cached workspaces. Drives the
    /// tray badge.
    pub fn total_active_count(&self) -> usize {
        self.inner.cache.read().unwrap().total_active_count()
    }

    /// Whether any cached change in any workspace has at least one capability
    /// spec delta. Drives the tray glyph variant selection.
    pub fn any_change_touches_specs(&self) -> bool {
        self.inner.cache.read().unwrap().any_change_touches_specs()
    }

    /// Whether the manager is currently watching `workspace`.
    pub fn is_watching(&self, workspace: &Path) -> bool {
        self.inner.watchers.lock().unwrap().contains_key(workspace)
    }

    /// Number of currently-watched workspaces.
    pub fn watched_count(&self) -> usize {
        self.inner.watchers.lock().unwrap().len()
    }

    /// Record that the app itself just wrote `path`; filters out the
    /// resulting watcher event.
    pub fn record_self_write(&self, path: impl Into<PathBuf>) {
        self.inner.self_writes.record(path);
    }

    /// Begin watching `workspace`. Populates the cache initially. Idempotent
    /// — a second call for the same workspace tears down the existing
    /// watcher and re-creates it (useful if the underlying folder was
    /// recreated).
    pub async fn add_workspace(&self, workspace: WorkspaceFolder) -> Result<(), WatcherError> {
        if !workspace.uri.is_dir() {
            return Err(WatcherError::NotADirectory(workspace.uri.clone()));
        }

        // Tear down any existing watcher for the same path so we never have
        // two watchers on one workspace.
        self.remove_workspace(&workspace.uri);

        // Initial populate (may legitimately return Ok(empty) if there are
        // no change directories yet).
        let initial = {
            let workspace_for_blocking = workspace.clone();
            tokio::task::spawn_blocking(move || parse_all_changes(&workspace_for_blocking))
                .await
                .unwrap()
                .map_err(|e| WatcherError::Parse {
                    path: workspace.uri.clone(),
                    source: e,
                })?
        };
        self.inner
            .cache
            .write()
            .unwrap()
            .insert(workspace.uri.clone(), initial);

        // Build the debouncer and bridge its callback to an async channel.
        let (tx, mut rx) = mpsc::unbounded_channel::<DebounceEventResult>();
        let mut debouncer = new_debouncer(self.inner.debounce, None, move |result| {
            let _ = tx.send(result);
        })?;

        // Watch the workspace's `openspec/` directory recursively. This way
        // `openspec/changes/` appearing later (or being recreated) is still
        // captured. We filter events to paths under `openspec/changes/`
        // before re-parsing.
        let watch_root = workspace.uri.join("openspec");
        if watch_root.is_dir() {
            debouncer
                .watcher()
                .watch(&watch_root, RecursiveMode::Recursive)?;
        }

        // Spawn a task to process debounced events. The task holds a Weak
        // reference to Inner so dropping the WatcherManager doesn't keep the
        // task alive via Arc — letting the sender drop closes the channel
        // and the task exits.
        let weak = Arc::downgrade(&self.inner);
        let workspace_for_task = workspace.clone();
        let task = tokio::spawn(async move {
            while let Some(result) = rx.recv().await {
                let events = match result {
                    Ok(events) => events,
                    Err(_errors) => continue,
                };
                let Some(inner) = weak.upgrade() else {
                    return;
                };
                inner.handle_events(&workspace_for_task, events).await;
            }
        });

        self.inner.watchers.lock().unwrap().insert(
            workspace.uri.clone(),
            WatcherEntry {
                _debouncer: debouncer,
                task,
            },
        );

        Ok(())
    }

    /// Stop watching `workspace`. Removes the cache entry and aborts the
    /// processing task. No-op if the workspace was not registered.
    pub fn remove_workspace(&self, workspace: &Path) -> bool {
        let removed = self.inner.watchers.lock().unwrap().remove(workspace);
        if let Some(entry) = removed {
            entry.task.abort();
        }
        self.inner
            .cache
            .write()
            .unwrap()
            .remove(workspace)
            .is_some()
    }
}

impl Inner {
    async fn handle_events(&self, workspace: &WorkspaceFolder, events: Vec<DebouncedEvent>) {
        let changes_root = workspace.uri.join("openspec").join("changes");

        // Drop events outside `openspec/changes/` and events we caused
        // ourselves.
        let relevant = events
            .iter()
            .filter(|ev| {
                ev.paths.iter().any(|p| p.starts_with(&changes_root))
                    && !ev
                        .paths
                        .iter()
                        .all(|p| self.self_writes.was_self_written(p))
            })
            .count();
        if relevant == 0 {
            return;
        }

        // Capture old changes for transition + achievement detection.
        let old_changes: Vec<crate::types::ChangeData> = self
            .cache
            .read()
            .unwrap()
            .changes_for(&workspace.uri)
            .to_vec();
        let old_ids: HashSet<String> = old_changes.iter().map(|c| c.change_id.clone()).collect();

        // Re-parse on the blocking pool.
        let workspace_for_blocking = workspace.clone();
        let parsed =
            tokio::task::spawn_blocking(move || parse_all_changes(&workspace_for_blocking))
                .await
                .unwrap();

        let new_changes = match parsed {
            Ok(c) => c,
            Err(_e) => return,
        };

        let new_ids: HashSet<String> = new_changes.iter().map(|c| c.change_id.clone()).collect();

        // Record forward-progress achievements (task completions, artifact
        // advances, new changes) into the activity log. `now` is also reused by
        // the archival transition loop below. Archival itself is recorded
        // there, where the archive directory is checked.
        let now = crate::activity_log::now_unix();
        if let Some(log) = self.activity_log.read().unwrap().clone() {
            let achievements = crate::activity_log::diff_achievements(
                &old_changes,
                &new_changes,
                &workspace.uri,
                now,
            );
            log.record_all(achievements);
        }

        // Update cache.
        self.cache
            .write()
            .unwrap()
            .insert(workspace.uri.clone(), new_changes);

        // Refresh the aggregated `last_views` snapshot synchronously before
        // any subscriber learns the cache moved. This is the ordering
        // guarantee the public emit contract relies on — if we broadcast
        // first, the event forwarder (and any other broadcast subscriber)
        // can wake up and call `workspace_views()` before `last_views`
        // catches up, leaving the UI one event behind on every content-only
        // change inside an existing change (artifact creation, task
        // checkbox toggles, etc.).
        let derived_events = self.refresh_aggregated_view();

        // Emit structural transitions.
        for added in new_ids.difference(&old_ids) {
            let _ = self.event_tx.send(CacheEvent::ChangeAdded {
                workspace: workspace.uri.clone(),
                change_id: added.clone(),
            });
        }
        for removed in old_ids.difference(&new_ids) {
            // "Removed from active" can mean archived (moved into archive/)
            // or simply deleted. Only emit ChangeArchived for the former.
            let archive_path = workspace.uri.join("openspec/changes/archive").join(removed);
            if archive_path.is_dir() {
                let _ = self.event_tx.send(CacheEvent::ChangeArchived {
                    workspace: workspace.uri.clone(),
                    change_id: removed.clone(),
                });
                if let Some(log) = self.activity_log.read().unwrap().clone() {
                    log.record(crate::activity_log::Achievement::new(
                        crate::activity_log::AchievementKind::ChangeArchived,
                        now,
                        workspace.uri.clone(),
                        Some(removed.clone()),
                        1,
                    ));
                }
            }
        }

        // Always emit a generic Updated so listeners that don't care about
        // the specifics (badge, generic refreshers) get a single signal.
        let _ = self.event_tx.send(CacheEvent::Updated {
            workspace: workspace.uri.clone(),
        });

        // Finally, emit the logical/instance diff events the aggregator
        // produced. They follow `Updated` so the broadcast ordering for a
        // batch matches the previous broadcast-subscriber aggregator's
        // behaviour: structural events first, then Updated, then the diff
        // events.
        for event in derived_events {
            let _ = self.event_tx.send(event);
        }
    }

    /// Recompute the aggregated views from the registry + cache, write the
    /// new `last_views` snapshot, and return the logical/instance diff
    /// events that fall out of comparing the previous and new snapshots.
    /// The caller broadcasts the returned events; this helper never sends
    /// on the broadcast channel itself.
    ///
    /// Returns an empty vector when there is no registry (the unit-test
    /// shape that constructs the manager via [`WatcherManager::new`]
    /// without a registry) or when the registry mutex is poisoned —
    /// callers must not depend on the absence of a return value implying
    /// the snapshot was refreshed.
    fn refresh_aggregated_view(&self) -> Vec<CacheEvent> {
        let registry = match self.registry.as_ref() {
            Some(r) => r.clone(),
            None => return Vec::new(),
        };
        let new_views = {
            let reg = match registry.lock() {
                Ok(g) => g,
                Err(_) => return Vec::new(),
            };
            let cache = self.cache.read().unwrap();
            compute_views(&reg, &cache, |repo_id| self.default_branch(repo_id))
        };

        let events = {
            let last = self.last_views.read().unwrap();
            diff_views(&last, &new_views)
        };

        *self.last_views.write().unwrap() = new_views;
        events
    }

    fn default_branch(&self, repo_id: &RepoId) -> Option<String> {
        self.repo_monitors
            .lock()
            .ok()?
            .get(repo_id)
            .and_then(RepoMonitor::default_branch)
    }
}
