use crate::cache::WorkspaceCache;
use crate::parser::parse_all_changes;
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
use std::sync::{Arc, Mutex, RwLock, Weak};
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
    event_tx: broadcast::Sender<CacheEvent>,
    self_writes: SelfWriteTracker,
    debounce: Duration,
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
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            inner: Arc::new(Inner {
                cache: RwLock::new(WorkspaceCache::new()),
                watchers: Mutex::new(HashMap::new()),
                event_tx,
                self_writes: SelfWriteTracker::new(SELF_WRITE_TTL),
                debounce,
            }),
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

        // Capture old change-id set for transition detection.
        let old_ids: HashSet<String> = self
            .cache
            .read()
            .unwrap()
            .changes_for(&workspace.uri)
            .iter()
            .map(|c| c.change_id.clone())
            .collect();

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

        // Update cache.
        self.cache
            .write()
            .unwrap()
            .insert(workspace.uri.clone(), new_changes);

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
            }
        }

        // Always emit a generic Updated so listeners that don't care about
        // the specifics (badge, generic refreshers) get a single signal.
        let _ = self.event_tx.send(CacheEvent::Updated {
            workspace: workspace.uri.clone(),
        });
    }
}

// Silence "Weak unused if no events ever arrive" warnings in static analysis.
#[allow(dead_code)]
fn _phantom(_: Weak<Inner>) {}
