use crate::cache::WorkspaceCache;
use crate::git::RepoId;
use crate::parser::parse_all_changes;
use crate::registry::WorkspaceRegistry;
use crate::repo_monitor::RepoMonitor;
use crate::repo_view::{self, diff_views, replace_repo_view, WorkspaceView};
use crate::self_write::SelfWriteTracker;
use crate::types::WorkspaceFolder;
use notify::RecursiveMode;
use notify_debouncer_full::{
    new_debouncer_opt, DebounceEventResult, DebouncedEvent, Debouncer, FileIdMap,
};
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
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

/// Test-only rendezvous at the phase boundary inside
/// [`Inner::refresh_aggregated_view_locked`] — between phase 1 (gather, under
/// the registry and cache locks) and phase 2 (the git I/O, with no lock held).
///
/// The `Non-Blocking Aggregated Recompute` invariant — that a concurrent cache
/// writer is never blocked for the duration of a recompute's git subprocesses
/// — is an ordering property between two threads. Proving it by racing them is
/// inherently machine-speed dependent: any probe cheap enough to win the race
/// reliably is one whose cost you can no longer reason about, and any probe
/// expensive enough to be realistic can lose it on fast hardware. That is
/// exactly how the earlier 60-worktree form of this coverage came to fail
/// deterministically on aarch64 macOS — it raced the recompute against an
/// `add_workspace` that stands up a real OS filesystem watcher. Arming this
/// gate instead pins the recompute *inside* the lock-free window for as long
/// as the test wants, so the assertion has no timing component at all.
///
/// Not `#[cfg(test)]`, deliberately, for the same reason as
/// [`crate::git::invocation_log`]: integration tests under
/// `openspec-core/tests/` compile this crate as an ordinary dependency, where
/// `#[cfg(test)]` items are invisible. When disarmed — which is every real
/// build, and every test that does not call [`recompute_gate::arm`] — the cost
/// is one relaxed atomic load per recompute, next to work measured in whole
/// git subprocess spawns.
///
/// Single-shot and process-global: [`arm`] installs a gate that the *next*
/// recompute to reach the boundary consumes and clears. That makes it unsafe
/// to use from a test binary where other tests recompute concurrently, so the
/// only consumer lives in its own integration target
/// (`tests/recompute_concurrency.rs`) with no other test in the process.
///
/// `#[doc(hidden)]` keeps it out of generated docs without making it private,
/// which the cross-crate visibility above requires.
#[doc(hidden)]
pub mod recompute_gate {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Receiver, SyncSender};
    use std::sync::Mutex;

    /// Handle returned by [`arm`]. Dropping it releases a recompute currently
    /// parked at the boundary (the `release` sender hangs up and the
    /// recompute's `recv` returns `Err`), so a panicking test can never wedge
    /// a thread.
    pub struct Gate {
        /// Fires once, when a recompute reaches the phase boundary.
        pub reached: Receiver<()>,
        /// Send on (or drop) this to let that recompute proceed into its git
        /// I/O.
        pub release: SyncSender<()>,
    }

    /// Fast path. Relaxed is sufficient: the channel operations below carry
    /// the actual happens-before edges, and a stale `false` read can only
    /// occur before `arm` returns — i.e. before any test could observe it.
    static ARMED: AtomicBool = AtomicBool::new(false);
    #[allow(clippy::type_complexity)]
    static GATE: Mutex<Option<(SyncSender<()>, Receiver<()>)>> = Mutex::new(None);

    /// Arm the gate. The next recompute to finish its gather phase signals
    /// [`Gate::reached`] and parks until [`Gate::release`] is used or dropped.
    pub fn arm() -> Gate {
        let (reached_tx, reached_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(0);
        *GATE.lock().unwrap_or_else(|e| e.into_inner()) = Some((reached_tx, release_rx));
        ARMED.store(true, Ordering::SeqCst);
        Gate {
            reached: reached_rx,
            release: release_tx,
        }
    }

    /// Called by the recompute between phase 1 and phase 2.
    pub(crate) fn rendezvous_if_armed() {
        if !ARMED.load(Ordering::Relaxed) {
            return;
        }
        // Take the gate out and disarm *before* parking, so a second
        // recompute arriving behind this one runs straight through instead of
        // blocking on a gate that has already been spent.
        let gate = GATE.lock().unwrap_or_else(|e| e.into_inner()).take();
        ARMED.store(false, Ordering::SeqCst);
        if let Some((reached, release)) = gate {
            // Either `Err` means the test is gone (panicked, or dropped the
            // handle). Proceed rather than hang.
            let _ = reached.send(());
            let _ = release.recv();
        }
    }
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
    /// Optional presentation store, consulted only for the per-row disabled
    /// flag that decides whether a top-level row is aggregated cold. `None` in
    /// unit-test contexts, where every row is enabled.
    presentation: RwLock<Option<Arc<Mutex<crate::presentation::WorkspacePresentationStore>>>>,
    /// Memoized local git identity per repository, populated lazily on first
    /// lookup (see [`Self::git_identity_for`]) and invalidated by
    /// [`WatcherManager::invalidate_identity`] when `RepoMonitor` observes a
    /// `.git/config` write. Values that read as `None` (no identity
    /// configured) are cached too, so a repo with no configured identity
    /// doesn't re-spawn `git config` on every batch either.
    identity_cache: Mutex<HashMap<RepoId, Option<crate::identity::Author>>>,
    /// Bumped by [`WatcherManager::invalidate_identity`] on every call. A
    /// `git_identity_for` read-through captures this before its git spawns
    /// and re-checks it before inserting into `identity_cache`; a mismatch
    /// means an invalidation landed while the spawns were outstanding, so
    /// the (possibly stale) result is discarded instead of being cached —
    /// see [`Self::git_identity_for`] for the full race this closes.
    identity_generation: AtomicU64,
    /// Serializes the *entire* aggregated recompute (gather + compute +
    /// merge) — both [`Self::refresh_aggregated_view`] and
    /// [`Self::refresh_aggregated_view_for`] hold this for their whole
    /// duration, via [`Self::refresh_aggregated_view_locked`]. This is
    /// deliberately a dedicated lock, not the registry/cache locks: gather
    /// releases those before any git I/O (per `Non-Blocking Aggregated
    /// Recompute`), which means two recomputes triggered concurrently (a
    /// file-edit batch's `handle_events`, a repo-monitor `status`/`reconcile`
    /// dispatch, the window-focus refresh, and three `aggregate_and_emit`
    /// call sites in `openspec-app` can all race) would otherwise perform an
    /// unsynchronized read-modify-write of `last_views` — the later writer
    /// silently discards the earlier one's result, which can resurrect an
    /// already-archived change or drop a dirty-state update with no
    /// corrective event. `recompute` guards only recompute-vs-recompute
    /// exclusion; a concurrent cache reader/writer (`add_workspace`,
    /// `changes_for`, …) never touches it and is therefore still never
    /// blocked by an in-flight recompute's git I/O — the non-blocking
    /// guarantee this change makes is about the registry/cache locks
    /// specifically, not about all synchronization whatsoever. As a side
    /// effect this also bounds the *total* concurrent git subprocess count
    /// process-wide to one recompute's own worker cap, rather than letting K
    /// concurrent recomputes each fan out to that cap independently.
    recompute: Mutex<()>,
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
/// The file-id cache both debouncers use, on every platform.
///
/// `new_debouncer` picks `RecommendedCache` for you, which is `FileIdMap` on
/// macOS/Windows but `NoCache` on Linux. That is a *behaviour* change, not a
/// naming one: before notify-debouncer-full 0.4 this code got a `FileIdMap`
/// everywhere, and dropping it on Linux changes how a batch's renames are
/// correlated — which is how an edit confined to one repository can end up
/// looking like a change that needs the whole registry re-derived. Naming the
/// cache explicitly keeps every platform on the pre-upgrade behaviour; adopting
/// the per-platform default would be its own change, with its own evidence.
fn build_native_debouncer(
    debounce: Duration,
    watch_root: &Path,
    tx: mpsc::UnboundedSender<DebounceEventResult>,
) -> Result<Debouncer<notify::RecommendedWatcher, FileIdMap>, WatcherError> {
    let mut debouncer = new_debouncer_opt::<_, notify::RecommendedWatcher, FileIdMap>(
        debounce,
        None,
        move |result| {
            let _ = tx.send(result);
        },
        FileIdMap::new(),
        notify::Config::default(),
    )?;
    if watch_root.is_dir() {
        debouncer.watch(watch_root, RecursiveMode::Recursive)?;
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
        debouncer.watch(watch_root, RecursiveMode::Recursive)?;
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
                presentation: RwLock::new(None),
                identity_cache: Mutex::new(HashMap::new()),
                identity_generation: AtomicU64::new(0),
                recompute: Mutex::new(()),
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

    /// Install the presentation store the aggregator reads the per-row disabled
    /// flag from. Without it every row aggregates warm, which is the correct
    /// default for a manager built without a shell.
    pub fn set_presentation(
        &self,
        store: Arc<Mutex<crate::presentation::WorkspacePresentationStore>>,
    ) {
        *self.inner.presentation.write().unwrap() = Some(store);
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

    /// Drop the memoized local git identity for `repo_id`, so the next
    /// file-edit batch re-reads it from git. Called by [`RepoMonitor`] when
    /// `.git/config` changes — the same event that already invalidates the
    /// cached default branch. A change to the *global* `~/.gitconfig` (not
    /// watched) leaves the memo stale until the next app start, matching the
    /// existing staleness tolerance for `default_branch`.
    ///
    /// Bumps `identity_generation` under the same `identity_cache` lock as
    /// the removal, so it composes correctly with `git_identity_for`'s
    /// generation check regardless of which of the two runs first — see
    /// that method's doc comment for the race this closes.
    pub fn invalidate_identity(&self, repo_id: &RepoId) {
        let mut cache = self
            .inner
            .identity_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.inner
            .identity_generation
            .fetch_add(1, Ordering::SeqCst);
        cache.remove(repo_id);
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
    /// Disabled rows contribute nothing: the badge is an attention surface, and
    /// a parked workspace is by definition not asking for attention. The
    /// Dashboard reads the unfiltered snapshot and still counts them.
    pub fn total_active_logical_count(&self) -> usize {
        let views = self.inner.last_views.read().unwrap();
        attention_rows(&views)
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

    /// Repo-scoped recompute: re-derive only `repo_id`'s [`WorkspaceView::Repo`]
    /// (running `git status` for that repo's worktrees only) and splice it into
    /// the cached snapshot, returning the resulting diff events. Falls back to a
    /// full [`Self::refresh_aggregated_view`] when the repo isn't in the snapshot
    /// yet (e.g. its first appearance). Same ordering contract as the full path.
    pub fn refresh_aggregated_view_for(&self, repo_id: &RepoId) -> Vec<CacheEvent> {
        self.inner.refresh_aggregated_view_for(repo_id)
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

    /// Recompute the aggregated views — refreshing each worktree's git
    /// working-tree status — and notify view consumers *even when the
    /// structural diff is empty*. A pure working-tree status change (a file
    /// staged, an edit to a non-spec file) produces no logical/instance diff,
    /// so [`diff_views`] returns nothing; this method additionally emits a
    /// single [`CacheEvent::Updated`] so `cache-updated` listeners refetch the
    /// views and pick up the new dirty/commit-state rollups. Used by the
    /// repo-monitor index watcher and the window-focus refresh path.
    pub fn refresh_status_and_notify(&self) {
        for event in self.refresh_aggregated_view() {
            self.emit(event);
        }
        // One nudge is enough: `cache-updated` consumers refetch *all* views,
        // so any tracked workspace path serves as the carrier.
        let carrier = {
            let views = self.inner.last_views.read().unwrap();
            views.first().map(|v| match v {
                WorkspaceView::Repo(r) => r.main_worktree.clone(),
                WorkspaceView::Flat { workspace, .. } => workspace.uri.clone(),
            })
        };
        if let Some(workspace) = carrier {
            self.emit(CacheEvent::Updated { workspace });
        }
    }

    /// Repo-scoped sibling of [`Self::refresh_status_and_notify`]: recompute and
    /// announce the working-tree status for a single repository. Used by the
    /// repo-monitor when a git event is attributable to one repo, so a stage in
    /// repo A never triggers a `git status` sweep of repos B, C, … The single
    /// `Updated` carrier is preferentially `repo_id`'s main worktree; any tracked
    /// path works since `cache-updated` consumers refetch all views.
    pub fn refresh_status_for(&self, repo_id: &RepoId) {
        for event in self.refresh_aggregated_view_for(repo_id) {
            self.emit(event);
        }
        let carrier = {
            let views = self.inner.last_views.read().unwrap();
            views
                .iter()
                .find_map(|v| match v {
                    WorkspaceView::Repo(r) if r.repo_id.as_path() == repo_id.as_path() => {
                        Some(r.main_worktree.clone())
                    }
                    _ => None,
                })
                .or_else(|| {
                    views.first().map(|v| match v {
                        WorkspaceView::Repo(r) => r.main_worktree.clone(),
                        WorkspaceView::Flat { workspace, .. } => workspace.uri.clone(),
                    })
                })
        };
        if let Some(workspace) = carrier {
            self.emit(CacheEvent::Updated { workspace });
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

    /// Whether any *enabled* top-level row holds a non-archived change with at
    /// least one capability spec delta. Drives the tray glyph variant selection.
    ///
    /// Reads the aggregated `last_views` snapshot rather than the raw cache, for
    /// the same reason [`Self::total_active_logical_count`] does — see
    /// [`attention_rows`]. The raw cache stays live for a parked workspace by
    /// design, so reading it here is what kept a parked repository flipping the
    /// menu-bar glyph while its badge contribution was already excluded.
    /// `last_views` is refreshed before any `CacheEvent` is broadcast (see
    /// [`Self::emit`]), so the answer is no staler than the cache's was.
    ///
    /// Only `active` is scanned: every cached non-archived change contributes an
    /// instance with `is_archived_here: false`, so its logical change always
    /// lands there, and the archived-here instances that ride along inside
    /// `active` carry `list_archived_stubs` stubs whose `specs` is empty.
    pub fn any_change_touches_specs(&self) -> bool {
        // The read guard stays a temporary of this expression rather than a
        // `let`-bound local, unlike its twin in `total_active_logical_count`.
        // `Iterator::any` takes `&mut self`, so the iterator is materialised as
        // a temporary of the block's tail expression — which is dropped *after*
        // the block's locals, so a `let`-bound guard would already be gone.
        // (`sum` there consumes the iterator by value, so the question never
        // arises.)
        attention_rows(&self.inner.last_views.read().unwrap()).any(|v| match v {
            WorkspaceView::Repo(r) => r
                .active
                .iter()
                .flat_map(|lc| &lc.instances)
                .any(|i| !i.change.artifacts.specs.is_empty()),
            WorkspaceView::Flat { changes, .. } => {
                changes.iter().any(|c| !c.artifacts.specs.is_empty())
            }
        })
    }

    /// Whether the manager is currently watching `workspace`.
    pub fn is_watching(&self, workspace: &Path) -> bool {
        self.inner.watchers.lock().unwrap().contains_key(workspace)
    }

    /// Number of currently-watched workspaces.
    pub fn watched_count(&self) -> usize {
        self.inner.watchers.lock().unwrap().len()
    }

    /// Number of installed repository monitors — exactly one per distinct
    /// repository with a tracked workspace. Each monitor owns a single
    /// filesystem watcher, so this is also the count of repo-level watchers.
    pub fn repo_monitor_count(&self) -> usize {
        self.inner.repo_monitors.lock().unwrap().len()
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

/// The tray's attention surface: every top-level row the user has *not* parked.
///
/// The single exclusion point shared by the badge count and the glyph predicate.
/// Reading the raw cache for one of them instead is how a parked workspace kept
/// flipping the menu-bar glyph in v0.16.1 while its badge contribution was
/// already excluded — the two are one surface and must agree about which rows
/// are asking for attention.
fn attention_rows(views: &[WorkspaceView]) -> impl Iterator<Item = &WorkspaceView> + '_ {
    views.iter().filter(|v| !v.is_disabled())
}

impl Inner {
    // `self: Arc<Self>` (rather than `&self`) so the recompute below can be
    // moved into `spawn_blocking` — see the comment at that call site. The
    // sole caller already holds an `Arc<Inner>` (from `weak.upgrade()`), so
    // this changes nothing at the call site.
    async fn handle_events(
        self: Arc<Self>,
        workspace: &WorkspaceFolder,
        events: Vec<DebouncedEvent>,
    ) {
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

        // Resolved once and reused below: scopes the aggregated recompute to
        // this workspace's own repository, and keys the memoized identity
        // lookup. A registry lookup, not a git call.
        let repo_id = self.repo_id_for(&workspace.uri);

        // Record forward-progress achievements (task completions, artifact
        // advances, new changes) into the activity log. `now` is also reused by
        // the archival transition loop below. Archival itself is recorded
        // there, where the archive directory is checked.
        //
        // Live events are attributed to the watched repository's local git
        // identity (memoized per repository — see `git_identity_for` — so
        // this reads through to git only on the first batch after a repo is
        // discovered or its `.git/config` last changed). A flat workspace
        // with no resolvable identity yields `None` and records author-less
        // events, which resolve as the local developer's. The attribution is
        // reused by the archival branch below.
        let now = crate::activity_log::now_unix();
        let local_identity = self.git_identity_for(repo_id.as_ref(), &workspace.uri);
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

        // Refresh the aggregated `last_views` snapshot before any subscriber
        // learns the cache moved. This is the ordering guarantee the public
        // emit contract relies on — if we broadcast first, the event
        // forwarder (and any other broadcast subscriber) can wake up and
        // call `workspace_views()` before `last_views` catches up, leaving
        // the UI one event behind on every content-only change inside an
        // existing change (artifact creation, task checkbox toggles, etc.).
        //
        // Run off the async runtime, matching the window-focus refresh
        // (`crates/specforge/src/lib.rs`) — the recompute shells out to
        // `git status`/`git branch` per worktree, and a tokio worker must
        // not block on that subprocess I/O.
        //
        // Scoped to the edited workspace's own repository (resolved above)
        // when one is available — the same bound `repo_monitor` already
        // applies to git events (index/refs changes), now extended to
        // file-change events per the amended *Status Freshness* requirement:
        // an edit in one repository must not sweep every other registered
        // repository's worktrees. Flat (non-git) workspaces have no
        // repository to scope to and fall back to the full recompute;
        // `refresh_aggregated_view_for` also falls back on its own for a
        // repo's first appearance.
        let inner = Arc::clone(&self);
        let derived_events = tokio::task::spawn_blocking(move || match &repo_id {
            Some(id) => inner.refresh_aggregated_view_for(id),
            None => inner.refresh_aggregated_view(),
        })
        .await
        .unwrap();

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
    /// Acquires `recompute` for its entire duration (see that field's doc
    /// comment on [`Inner`] for why) and delegates to
    /// [`Self::refresh_aggregated_view_locked`] for the actual work.
    fn refresh_aggregated_view(&self) -> Vec<CacheEvent> {
        let _guard = self.recompute.lock().unwrap_or_else(|e| e.into_inner());
        self.refresh_aggregated_view_locked()
    }

    /// The body of [`Self::refresh_aggregated_view`], *without* acquiring
    /// `recompute` — [`Self::refresh_aggregated_view_for`] calls this
    /// directly (while already holding the lock itself) for its
    /// first-appearance fallback, so the same thread never tries to lock a
    /// plain (non-reentrant) `Mutex` twice.
    ///
    /// Three phases (see `Non-Blocking Aggregated Recompute`): gather the
    /// registry/cache inputs under their locks (microseconds, no I/O), drop
    /// those locks, then perform the git I/O with nothing held, and finally
    /// merge under a short `last_views` write lock. A concurrent reader or
    /// writer of the registry/cache is therefore never blocked for the
    /// duration of this recompute's git subprocesses — the `recompute` lock
    /// only excludes other recomputes, not cache/registry access.
    ///
    /// Returns an empty vector when there is no registry (the unit-test
    /// shape that constructs the manager via [`WatcherManager::new`]
    /// without a registry) or when the registry mutex is poisoned —
    /// callers must not depend on the absence of a return value implying
    /// the snapshot was refreshed.
    /// One consistent answer to "which top-level rows are parked?" for the
    /// duration of a recompute. Empty when no presentation store is installed
    /// (the unit-test shape), so every row aggregates warm.
    fn disabled_snapshot(&self) -> HashSet<crate::presentation::PresentationKey> {
        self.presentation
            .read()
            .unwrap()
            .as_ref()
            .and_then(|store| store.lock().ok().map(|g| g.disabled_keys()))
            .unwrap_or_default()
    }

    fn refresh_aggregated_view_locked(&self) -> Vec<CacheEvent> {
        let registry = match self.registry.as_ref() {
            Some(r) => r.clone(),
            None => return Vec::new(),
        };

        // Taken before the gather guards below, never underneath them — see
        // `WorkspacePresentationStore::disabled_keys`.
        let disabled = self.disabled_snapshot();

        // PHASE 1 (gather): registry + cache guards live only for this block.
        let gathered = {
            let reg = match registry.lock() {
                Ok(g) => g,
                Err(_) => return Vec::new(),
            };
            let cache = self.cache.read().unwrap();
            repo_view::gather_views(
                &reg,
                &cache,
                |repo_id| self.default_branch(repo_id),
                |key| disabled.contains(key),
            )
        };

        // Test-only rendezvous (see [`recompute_gate`]): exactly here, after
        // the phase-1 guards above are dropped and before the git I/O below,
        // is the window in which the cache must remain writable by another
        // thread. One relaxed atomic load in a real build.
        recompute_gate::rendezvous_if_armed();

        // PHASE 2 (compute): the git I/O, with no lock held.
        let new_views = repo_view::compute_views_from_gathered(gathered);

        // PHASE 3 (merge): a short `last_views` write lock. Safe against a
        // concurrent recompute's read-modify-write only because the caller
        // (`refresh_aggregated_view` / `refresh_aggregated_view_for`) holds
        // `recompute` across this whole method.
        let events = {
            let last = self.last_views.read().unwrap();
            diff_views(&last, &new_views)
        };
        *self.last_views.write().unwrap() = new_views;
        events
    }

    /// Scoped recompute: re-derive only `repo_id`'s view and splice it into the
    /// snapshot. Falls back to the full recompute when the repo has no tracked
    /// worktrees yet or is not present in the current snapshot — in both cases
    /// the repo is appearing for the first time and the global path is correct.
    /// Same gather/compute/merge lock-release structure as
    /// [`Self::refresh_aggregated_view_locked`], and — critically — the same
    /// `recompute` exclusion: held for this method's entire duration,
    /// including its fallback calls into [`Self::refresh_aggregated_view_locked`]
    /// (never the public, re-locking [`Self::refresh_aggregated_view`], which
    /// would deadlock against the guard already held here).
    fn refresh_aggregated_view_for(&self, repo_id: &RepoId) -> Vec<CacheEvent> {
        let _guard = self.recompute.lock().unwrap_or_else(|e| e.into_inner());

        let registry = match self.registry.as_ref() {
            Some(r) => r.clone(),
            None => return Vec::new(),
        };

        let disabled = self.disabled_snapshot();

        let gathered = {
            let reg = match registry.lock() {
                Ok(g) => g,
                Err(_) => return Vec::new(),
            };
            let cache = self.cache.read().unwrap();
            repo_view::gather_repo_view(
                &reg,
                &cache,
                repo_id,
                |id| self.default_branch(id),
                |key| disabled.contains(key),
            )
        };
        let Some(gathered) = gathered else {
            return self.refresh_aggregated_view_locked();
        };

        let new_repo_view = repo_view::build_repo_view(repo_view::compute_repo_snapshot(gathered));

        let last_snapshot = self.last_views.read().unwrap().clone();
        let mut next = last_snapshot.clone();
        if !replace_repo_view(&mut next, new_repo_view) {
            // Repo is registered but not yet in the snapshot — first appearance.
            return self.refresh_aggregated_view_locked();
        }
        let events = diff_views(&last_snapshot, &next);
        *self.last_views.write().unwrap() = next;
        events
    }

    fn default_branch(&self, repo_id: &RepoId) -> Option<String> {
        self.repo_monitors
            .lock()
            .ok()?
            .get(repo_id)
            .and_then(RepoMonitor::default_branch)
    }

    /// Resolve the `RepoId` that owns `workspace_uri`, via a registry
    /// lookup — never a git call, since the registry already stores
    /// `repo_id` per entry. `None` when there is no registry (unit-test
    /// contexts built via [`WatcherManager::new`]), the workspace isn't
    /// registered, or it's a flat non-git workspace — in all of those cases
    /// the caller falls back to the full recompute.
    fn repo_id_for(&self, workspace_uri: &Path) -> Option<RepoId> {
        let registry = self.registry.as_ref()?;
        let reg = registry.lock().ok()?;
        reg.entry(workspace_uri)?.repo_id.clone()
    }

    /// The local git identity for `workspace_uri`, memoized per `repo_id`
    /// when one is available (see [`Self::identity_cache`] on the struct).
    /// A flat, non-git workspace (`repo_id: None`) has nothing to key a
    /// memo on and always reads through to [`crate::git::git_identity`] —
    /// acceptable since it costs 2 spawns with no repo-wide fan-out to
    /// amortize against, unlike the git-backed case this memo targets.
    ///
    /// Race with [`WatcherManager::invalidate_identity`]: the cache lock is
    /// released for the duration of the git spawns below (so a config
    /// change can't be blocked on this read), which means an invalidation
    /// can land *after* this call already missed the cache but *before* it
    /// inserts its (now-stale) result — a plain "check, spawn, insert"
    /// would silently resurrect the stale value with the invalidation lost.
    /// `identity_generation` closes that: captured before the spawns,
    /// re-checked in the same locked section as the insert, so if it moved
    /// in between, the result is discarded instead of cached and the next
    /// lookup re-reads from git.
    fn git_identity_for(
        &self,
        repo_id: Option<&RepoId>,
        workspace_uri: &Path,
    ) -> Option<crate::identity::Author> {
        let Some(repo_id) = repo_id else {
            return crate::git::git_identity(workspace_uri);
        };
        let generation_before = self.identity_generation.load(Ordering::SeqCst);
        {
            let cache = self
                .identity_cache
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(cached) = cache.get(repo_id) {
                return cached.clone();
            }
        }
        let identity = crate::git::git_identity(workspace_uri);
        {
            let mut cache = self
                .identity_cache
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if self.identity_generation.load(Ordering::SeqCst) == generation_before {
                cache.insert(repo_id.clone(), identity.clone());
            }
            // else: an invalidation landed while the spawns above were
            // outstanding — don't resurrect a stale value into the cache.
        }
        identity
    }
}
