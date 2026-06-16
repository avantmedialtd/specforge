use crate::cache::WorkspaceCache;
use crate::git::RepoId;
use crate::parser::parse_all_changes;
use crate::registry::WorkspaceRegistry;
use crate::repo_monitor::RepoMonitor;
use crate::repo_view::{compute_views, diff_views, WorkspaceView};
use crate::self_write::SelfWriteTracker;
use crate::types::WorkspaceFolder;
use notify::{RecursiveMode, Watcher};
#[cfg(target_os = "windows")]
use notify_debouncer_full::new_debouncer_opt;
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
/// Default re-scan cadence for the polling watcher used on WSL 9P shares.
/// Coarse by design — the watched tree is a handful of small markdown files
/// and the dashboard is ambient. User-configurable (Windows-only setting).
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(10);

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
    /// The opt-in Claude usage-quota snapshot was refreshed by the quota
    /// poller. Carries no payload — subscribers re-read the latest snapshot via
    /// `AppService::claude_quota()`. Only emitted while the feature is enabled.
    QuotaUpdated,
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
    /// Re-scan cadence for the polling watcher used on WSL workspaces. Only
    /// read on Windows (WSL paths cannot occur elsewhere); kept cross-platform
    /// so the field and its setter have one definition.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    poll_interval: RwLock<Duration>,
    registry: Option<Arc<Mutex<WorkspaceRegistry>>>,
    /// Optional activity log the watcher records observed achievements into.
    /// `None` in unit-test contexts that don't persist activity.
    activity_log: RwLock<Option<Arc<crate::activity_log::ActivityLog>>>,
}

struct WatcherEntry {
    /// Kept alive so its internal threads keep running.
    _debouncer: DebouncerKind,
    /// Aborted on workspace removal.
    task: JoinHandle<()>,
}

/// The live debouncer backing a workspace's watcher. WSL workspaces (Windows
/// only) use a polling backend because the 9P share delivers no OS change
/// events; every other workspace uses the native event-driven backend. The
/// variants hold distinct watcher types, so this enum is how one `WatcherEntry`
/// can own either. The held debouncers are never read back — they exist only
/// to keep their internal watcher threads alive until the entry is dropped —
/// hence `allow(dead_code)`.
#[allow(dead_code)]
enum DebouncerKind {
    Native(Debouncer<notify::RecommendedWatcher, FileIdMap>),
    #[cfg(target_os = "windows")]
    Poll(Debouncer<notify::PollWatcher, FileIdMap>),
}

/// Build a native (event-driven) debounced watcher rooted at `watch_root`,
/// forwarding debounced batches to `tx`.
fn build_native_debouncer(
    debounce: Duration,
    watch_root: &Path,
    tx: mpsc::UnboundedSender<DebounceEventResult>,
) -> Result<Debouncer<notify::RecommendedWatcher, FileIdMap>, WatcherError> {
    let mut debouncer = new_debouncer(debounce, None, move |result| {
        let _ = tx.send(result);
    })?;
    if watch_root.is_dir() {
        debouncer
            .watcher()
            .watch(watch_root, RecursiveMode::Recursive)?;
    }
    Ok(debouncer)
}

/// Build a polling debounced watcher rooted at `watch_root`, re-scanning every
/// `poll_interval`. Used for WSL 9P shares where the native Windows backend
/// receives no events. Windows-only.
#[cfg(target_os = "windows")]
fn build_poll_debouncer(
    debounce: Duration,
    poll_interval: Duration,
    watch_root: &Path,
    tx: mpsc::UnboundedSender<DebounceEventResult>,
) -> Result<Debouncer<notify::PollWatcher, FileIdMap>, WatcherError> {
    let config = notify::Config::default().with_poll_interval(poll_interval);
    let mut debouncer = new_debouncer_opt::<_, notify::PollWatcher, FileIdMap>(
        debounce,
        None,
        move |result| {
            let _ = tx.send(result);
        },
        FileIdMap::new(),
        config,
    )?;
    if watch_root.is_dir() {
        debouncer
            .watcher()
            .watch(watch_root, RecursiveMode::Recursive)?;
    }
    Ok(debouncer)
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
                poll_interval: RwLock::new(DEFAULT_POLL_INTERVAL),
                registry,
                activity_log: RwLock::new(None),
            }),
        }
    }

    /// Set the re-scan cadence used by the polling watcher for WSL workspaces.
    /// Takes effect for watchers established after this call; existing watchers
    /// keep the interval they were built with (re-add the workspace to apply a
    /// new interval to it). Only consulted on Windows; harmless elsewhere.
    pub fn set_poll_interval(&self, interval: Duration) {
        *self.inner.poll_interval.write().unwrap() = interval;
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
        // Watch the workspace's `openspec/` directory recursively. This way
        // `openspec/changes/` appearing later (or being recreated) is still
        // captured. We filter events to paths under `openspec/changes/`
        // before re-parsing.
        //
        // A WSL-hosted workspace (Windows only) uses a polling backend: the
        // 9P share delivers no `ReadDirectoryChangesW` events, so the native
        // watcher would go permanently deaf. Every other workspace keeps the
        // event-driven native backend.
        let (tx, mut rx) = mpsc::unbounded_channel::<DebounceEventResult>();
        let watch_root = workspace.uri.join("openspec");
        let debouncer = {
            #[cfg(target_os = "windows")]
            {
                match crate::wsl::watch_strategy(&workspace.uri) {
                    crate::wsl::WatchStrategy::Poll => {
                        let interval = *self.inner.poll_interval.read().unwrap();
                        DebouncerKind::Poll(build_poll_debouncer(
                            self.inner.debounce,
                            interval,
                            &watch_root,
                            tx,
                        )?)
                    }
                    crate::wsl::WatchStrategy::Native => DebouncerKind::Native(
                        build_native_debouncer(self.inner.debounce, &watch_root, tx)?,
                    ),
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                DebouncerKind::Native(build_native_debouncer(
                    self.inner.debounce,
                    &watch_root,
                    tx,
                )?)
            }
        };

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
        //
        // Live events are attributed to the watched repository's local git
        // identity (read once per batch, repo-local with global fallback). A
        // flat workspace with no resolvable identity yields `None` and records
        // author-less events, which resolve as the local developer's. The
        // attribution is reused by the archival branch below.
        let now = crate::activity_log::now_unix();
        let local_identity = crate::git::git_identity(&workspace.uri);
        if let Some(log) = self.activity_log.read().unwrap().clone() {
            let achievements = crate::activity_log::diff_achievements(
                &old_changes,
                &new_changes,
                &workspace.uri,
                now,
                local_identity.clone(),
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
        // "Removed from active" can mean archived (moved into archive/) or
        // simply deleted. Only the former emits ChangeArchived. The archive
        // tooling writes `archive/<YYYY-MM-DD>-<id>/`, so we can't stat an
        // exact `archive/<id>` path — instead build the set of archived
        // logical ids once (date prefix stripped) and test membership.
        let removed_ids: Vec<&String> = old_ids.difference(&new_ids).collect();
        let archived_ids: HashSet<String> = if removed_ids.is_empty() {
            HashSet::new()
        } else {
            crate::parser::list_archived_changes(&workspace.uri)
                .unwrap_or_default()
                .iter()
                .map(|name| crate::parser::archive_dir_logical_id(name).to_string())
                .collect()
        };
        for removed in removed_ids {
            if archived_ids.contains(removed) {
                let _ = self.event_tx.send(CacheEvent::ChangeArchived {
                    workspace: workspace.uri.clone(),
                    change_id: removed.clone(),
                });
                if let Some(log) = self.activity_log.read().unwrap().clone() {
                    log.record(
                        crate::activity_log::Achievement::new(
                            crate::activity_log::AchievementKind::ChangeArchived,
                            now,
                            workspace.uri.clone(),
                            Some(removed.clone()),
                            1,
                        )
                        .with_author(local_identity.clone()),
                    );
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
