use crate::cache::WorkspaceCache;
use crate::git::RepoId;
use crate::parser::parse_all_changes;
use crate::registry::WorkspaceRegistry;
use crate::repo_monitor::RepoMonitor;
use crate::repo_view::{
    self, diff_views, replace_flat_view, replace_repo_view, LogicalIndex, Restatus, WorkspaceView,
    WorktreeGitMemo, WorktreeGitState,
};
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
///
/// `rename_all_fields` here is trap-removal, not a bug fix. This type's
/// serialized form does not currently cross the IPC boundary — the shell
/// translates each variant into an explicit named payload rather than
/// forwarding the enum — so no frontend reads these keys today and nothing
/// is observably broken without it. But `rename_all` on an enum renames
/// only the variants, so the struct variants below would emit `change_id` /
/// `repo_id` / `change_name` / `worktree_path` the moment anyone forwarded
/// one. Deliberately absent from the wire guard's roots: it is not part of
/// the IPC contract, and asserting one it does not have would be a lie. If
/// it ever crosses the wire, it joins the roots then.
///
/// This closes the field-naming half of the trap and NOT the envelope half:
/// there is no `tag = "..."`, so forwarding a variant directly would emit an
/// externally-tagged `{"changeAdded":{…}}` that `src/types.ts` has no union
/// for. Anyone wiring this to a frontend needs to choose a representation
/// first — or, following `document_watch.rs`'s precedent, drop `Serialize`
/// here entirely and let `openspec_app::events` own the wire shape alone.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
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
    /// The opt-in BitBucket pull-request snapshot changed — emitted by the
    /// BitBucket poller (`openspec-app`'s `bitbucket` module; the
    /// `bitbucket-pull-requests` capability's *The Snapshot Is Announced on the
    /// Cache Stream*). Carries no payload — subscribers re-read the latest
    /// snapshot via `AppService::bitbucket_pull_requests()`. Emitted only when
    /// the snapshot actually changed, and never while the feature is disabled
    /// except once, as it collapses to the disabled state.
    BitbucketPullRequestsUpdated,
    /// The opt-in GitHub pull-request snapshot changed — emitted by the GitHub
    /// poller (`openspec-app`'s `github` module; the `github-pull-requests`
    /// capability's *The GitHub Snapshot Is Announced on the Cache Stream*).
    /// Carries no payload — subscribers re-read the latest snapshot via
    /// `AppService::github_pull_requests()`. Same emission rule as the
    /// BitBucket variant: only on a changed snapshot, and once on disabling.
    GithubPullRequestsUpdated,
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
    /// Memoized remotes (name and fetch URL) per repository, read lazily by
    /// [`WatcherManager::remotes`] and dropped by
    /// [`WatcherManager::invalidate_remotes`] when `RepoMonitor` observes a
    /// `.git/config` write. Only warm repositories ever reach the reader —
    /// its one caller walks the aggregated views, which exclude disabled
    /// rows — so a disabled repository never spawns `git remote -v`
    /// (`pull-request-worktree-links`: *Repository Remote Identities*).
    remotes_cache: Mutex<HashMap<RepoId, Vec<crate::git::Remote>>>,
    /// The `identity_generation` twin for `remotes_cache`: a read-through
    /// that raced an invalidation discards its result instead of caching it.
    remotes_generation: AtomicU64,
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
    /// The git state (branch, upstream, status, and the active change ids it
    /// classified) of every warm tracked worktree, as its most recent status
    /// computation produced it. A file batch reuses a sibling worktree's
    /// entry instead of re-statusing it (`working-tree-status`: *Status
    /// Freshness*). Read and written only while `recompute` is held, and
    /// only through [`Self::record_worktree_git`]: a full recompute replaces
    /// it wholesale, a scoped one replaces its repository's entries. A cold
    /// row's defaults are never recorded, and neither is a read of a
    /// repository whose git events were not yet observed when the read
    /// began ([`Self::observes_git_events`]), so every entry is one a later
    /// git event is guaranteed to replace.
    worktree_git: Mutex<WorktreeGitMemo>,
}

/// Which memo entries [`Inner::record_worktree_git`] replaces.
enum MemoScope<'a> {
    /// A full recompute: every entry.
    All,
    /// A scoped recompute: the entries of the repository's worktrees as the
    /// replaced view listed them, which prunes a worktree since removed.
    Worktrees(&'a [PathBuf]),
}

struct WatcherEntry {
    /// Kept alive so its internal threads keep running.
    _debouncer: DebouncerKind,
    /// Aborted on workspace removal.
    task: JoinHandle<()>,
    /// True while the workspace has no `openspec/`: its own folder's direct
    /// entries are watched, waiting for one to appear (`workspace-registry`:
    /// *Filesystem Watching of Registered Workspaces*).
    awaiting_openspec: bool,
}

/// Whether a freshly armed watcher must replay a parse before anything else:
/// it watches an `openspec/` folder that the cache's last parse did not see.
fn needs_replay(awaiting_openspec: bool, parsed_with_openspec: bool) -> bool {
    !awaiting_openspec && !parsed_with_openspec
}

/// The batch a replay feeds `handle_events` once `openspec/` is watched under
/// a workspace whose cache was parsed without it: a folder created at
/// `openspec/changes/`, which passes the batch filter like any change edit.
fn openspec_changes_appeared(workspace: &Path) -> DebouncedEvent {
    DebouncedEvent::new(
        notify::Event::new(notify::EventKind::Create(notify::event::CreateKind::Folder))
            .add_path(workspace.join("openspec").join("changes")),
        std::time::Instant::now(),
    )
}

/// Whether [`WatcherManager::arm`] installs its watcher unconditionally (a
/// registration) or only while the workspace is still tracked (the re-arm
/// after `openspec/` appears, which must never resurrect a workspace that was
/// removed while it ran).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Arm {
    Always,
    IfStillTracked,
}

/// The live debouncer backing a workspace's watcher. WSL workspaces (Windows
/// only) use a polling backend because the 9P share delivers no OS change
/// events; every other workspace uses the native event-driven backend. The
/// variants hold distinct watcher types, so this enum is how one `WatcherEntry`
/// can own either. The held debouncers are never read back — they exist only
/// to keep their internal watcher threads alive until the entry is dropped —
/// hence the `dead_code` expectation.
#[expect(dead_code, reason = "held only to keep the watcher threads alive")]
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
    mode: RecursiveMode,
    tx: mpsc::UnboundedSender<DebounceEventResult>,
) -> Result<Debouncer<notify::RecommendedWatcher, FileIdMap>, WatcherError> {
    let mut debouncer = new_debouncer(debounce, None, move |result| {
        let _ = tx.send(result);
    })?;
    if watch_root.is_dir() {
        debouncer.watcher().watch(watch_root, mode)?;
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
    mode: RecursiveMode,
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
        debouncer.watcher().watch(watch_root, mode)?;
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
                remotes_cache: Mutex::new(HashMap::new()),
                remotes_generation: AtomicU64::new(0),
                recompute: Mutex::new(()),
                worktree_git: Mutex::new(WorktreeGitMemo::new()),
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

    /// The remotes of `repo_id`, memoized: the first call runs one
    /// `git remote -v` ([`crate::git::remote_urls`]) and later calls read the
    /// memo until [`Self::invalidate_remotes`] drops it. Same race handling as
    /// `git_identity_for`: the lock is released across the spawn, and a
    /// result that raced an invalidation is returned but not cached.
    pub fn remotes(&self, repo_id: &RepoId) -> Vec<crate::git::Remote> {
        let generation_before = self.inner.remotes_generation.load(Ordering::SeqCst);
        {
            let cache = self
                .inner
                .remotes_cache
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(cached) = cache.get(repo_id) {
                return cached.clone();
            }
        }
        let remotes = crate::git::remote_urls(repo_id);
        {
            let mut cache = self
                .inner
                .remotes_cache
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if self.inner.remotes_generation.load(Ordering::SeqCst) == generation_before {
                cache.insert(repo_id.clone(), remotes.clone());
            }
        }
        remotes
    }

    /// Drop the memoized remotes of `repo_id`, so the next [`Self::remotes`]
    /// re-reads them. Called by `RepoMonitor` on a `.git/config` write. Bumps
    /// the generation under the cache lock, as `invalidate_identity` does.
    pub fn invalidate_remotes(&self, repo_id: &RepoId) {
        let mut cache = self
            .inner
            .remotes_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.inner.remotes_generation.fetch_add(1, Ordering::SeqCst);
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
    ///
    /// Re-statuses every tracked worktree of the repository, as a git event
    /// requires; only the watcher's own file batches narrow that.
    pub fn refresh_aggregated_view_for(&self, repo_id: &RepoId) -> Vec<CacheEvent> {
        self.inner
            .refresh_aggregated_view_for(repo_id, Restatus::All)
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

    /// True while `workspace` is watched only for an `openspec/` to appear —
    /// it had none when its watcher was armed — rather than for change edits.
    pub fn is_awaiting_openspec(&self, workspace: &Path) -> bool {
        self.inner
            .watchers
            .lock()
            .unwrap()
            .get(workspace)
            .is_some_and(|entry| entry.awaiting_openspec)
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

        // Whether the parse below sees an `openspec/` folder. If one appears
        // before the watcher is armed, `arm` replays the parse rather than
        // leave the cache empty under a folder it now watches.
        let parsed_with_openspec = workspace.uri.join("openspec").is_dir();

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

        self.arm(workspace, Arm::Always, parsed_with_openspec)?;
        Ok(())
    }

    /// Build `workspace`'s debouncer and event task and install them in place
    /// of any existing ones, leaving the cache alone.
    ///
    /// With an `openspec/` subdirectory, that directory is watched recursively
    /// and batches are filtered to paths under `openspec/changes/` before
    /// re-parsing — so `openspec/changes/` appearing later (or being
    /// recreated) is still captured. Without one, only the workspace folder's
    /// direct entries are watched, so a build or install inside a repository
    /// without OpenSpec reaches nothing; any batch there just checks whether
    /// `openspec/` has appeared, and re-arms if it has.
    ///
    /// `parsed_with_openspec` says whether the cache's last parse saw an
    /// `openspec/` folder. When it did not but one is watched now, the new
    /// task's first act is to replay that parse as a batch of its own — after
    /// the entry is installed and before any other batch, so a replay never
    /// runs beside one, and never for a workspace removed meanwhile.
    ///
    /// A WSL-hosted workspace (Windows only) uses a polling backend: the 9P
    /// share delivers no `ReadDirectoryChangesW` events, so the native watcher
    /// would go permanently deaf. Every other workspace keeps the event-driven
    /// native backend.
    fn arm(
        &self,
        workspace: WorkspaceFolder,
        when: Arm,
        parsed_with_openspec: bool,
    ) -> Result<(), WatcherError> {
        let openspec_dir = workspace.uri.join("openspec");
        let awaiting_openspec = !openspec_dir.is_dir();
        let replay = needs_replay(awaiting_openspec, parsed_with_openspec);
        let (watch_root, mode) = if awaiting_openspec {
            (workspace.uri.clone(), RecursiveMode::NonRecursive)
        } else {
            (openspec_dir, RecursiveMode::Recursive)
        };

        let (tx, mut rx) = mpsc::unbounded_channel::<DebounceEventResult>();
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
                            mode,
                            tx,
                        )?)
                    }
                    crate::wsl::WatchStrategy::Native => DebouncerKind::Native(
                        build_native_debouncer(self.inner.debounce, &watch_root, mode, tx)?,
                    ),
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                DebouncerKind::Native(build_native_debouncer(
                    self.inner.debounce,
                    &watch_root,
                    mode,
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
        let (installed_tx, installed_rx) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            // Nothing runs until this entry is installed. A re-arm that finds
            // its workspace removed drops the sender instead, and the task
            // ends here.
            if installed_rx.await.is_err() {
                return;
            }
            if replay {
                let Some(inner) = weak.upgrade() else {
                    return;
                };
                let appeared = openspec_changes_appeared(&workspace_for_task.uri);
                inner
                    .handle_events(&workspace_for_task, vec![appeared])
                    .await;
            }
            while let Some(result) = rx.recv().await {
                let events = match result {
                    Ok(events) => events,
                    Err(_errors) => continue,
                };
                let Some(inner) = weak.upgrade() else {
                    return;
                };
                if !awaiting_openspec {
                    inner.handle_events(&workspace_for_task, events).await;
                } else if workspace_for_task.uri.join("openspec").is_dir() {
                    // Re-arm in place. Installing the new entry aborts this
                    // task at its next await; a re-arm that fails leaves it
                    // running, so the next batch tries again.
                    let manager = WatcherManager { inner };
                    match manager.arm(workspace_for_task.clone(), Arm::IfStillTracked, false) {
                        Ok(()) => return,
                        Err(e) => eprintln!(
                            "failed to re-arm watcher for {}: {e}; retrying on the next change",
                            workspace_for_task.uri.display()
                        ),
                    }
                }
            }
        });

        let entry = WatcherEntry {
            _debouncer: debouncer,
            task,
            awaiting_openspec,
        };
        let mut watchers = self.inner.watchers.lock().unwrap();
        if when == Arm::IfStillTracked && !watchers.contains_key(&workspace.uri) {
            // Unregistered while the re-arm was being built: install nothing.
            // Dropping `installed_tx` on return ends the new task unstarted.
            drop(watchers);
            entry.task.abort();
            return Ok(());
        }
        let previous = watchers.insert(workspace.uri.clone(), entry);
        // Released before the previous entry goes: dropping a debouncer joins
        // its OS watcher's thread, which no other caller should wait behind.
        drop(watchers);
        if let Some(previous) = previous {
            previous.task.abort();
        }
        let _ = installed_tx.send(());
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
        // advances, new changes) into the activity log, together with the
        // batch's archivals (below), in one `record_all`: one append however
        // many changes the batch archived.
        //
        // Live events are attributed to the watched repository's local git
        // identity (memoized per repository — see `git_identity_for` — so
        // this reads through to git only on the first batch after a repo is
        // discovered or its `.git/config` last changed). A flat workspace
        // resolves no identity and records author-less events, which resolve
        // as the local developer's. The archivals carry the same attribution.
        //
        // The identity is only needed to record, so it is resolved only when
        // an activity log is attached: read once, and the one result serves
        // both the achievements here and the archivals below.
        let now = crate::activity_log::now_unix();
        let activity_log = self.activity_log.read().unwrap().clone();
        let local_identity = activity_log
            .as_ref()
            .and_then(|_| self.git_identity_for(repo_id.as_ref(), &workspace.uri));
        let achievements = activity_log.map(|log| {
            let achievements = crate::activity_log::diff_achievements(
                &old_changes,
                &new_changes,
                &workspace.uri,
                now,
                local_identity.clone(),
            );
            (log, achievements)
        });

        // Update cache — unless the workspace was removed while this batch
        // was parsing. Writing it back would resurrect an unregistered
        // workspace, and everything below would announce it.
        {
            let mut cache = self.cache.write().unwrap();
            if !cache.contains(&workspace.uri) {
                return;
            }
            cache.insert(workspace.uri.clone(), new_changes);
        }

        // "Removed from active" can mean archived (moved into archive/) or
        // simply deleted. Only the former is an archival. The archive tooling
        // writes `archive/<YYYY-MM-DD>-<id>/`, so we can't stat an exact
        // `archive/<id>` path — instead build the set of archived logical ids
        // once (date prefix stripped) and test membership. Computed here,
        // before anything is recorded or announced, so the archivals join the
        // batch's one `record_all` and reach the log before their
        // `ChangeArchived` reaches a Dashboard that refetches on it. Nothing
        // below changes `old_ids`, `new_ids` or the `archive/` listing.
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
        let archived: Vec<String> = removed_ids
            .into_iter()
            .filter(|removed| archived_ids.contains(*removed))
            .cloned()
            .collect();
        if let Some((log, mut achievements)) = achievements {
            achievements.extend(archived.iter().map(|change_id| {
                crate::activity_log::Achievement::new(
                    crate::activity_log::AchievementKind::ChangeArchived,
                    now,
                    workspace.uri.clone(),
                    Some(change_id.clone()),
                    1,
                )
                .with_author(local_identity.clone())
            }));
            log.record_all(achievements);
        }

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
        // Scoped to the edited workspace (*Status Freshness*). In a
        // repository, only this worktree is re-statused: an edit inside its
        // `openspec/` cannot change a sibling's branch, upstream or spec
        // commit states, so each sibling reuses its recorded git state unless
        // its active change set moved since. Git events still re-status the
        // whole repository through `refresh_status_for`. A flat (non-git)
        // workspace rebuilds only its own row and spawns no git at all. Both
        // fall back to the full recompute on the row's first appearance.
        let inner = Arc::clone(&self);
        let origin = workspace.uri.clone();
        let derived_events = tokio::task::spawn_blocking(move || match &repo_id {
            Some(id) => inner.refresh_aggregated_view_for(id, Restatus::Worktree(origin)),
            None => inner.refresh_flat_view_for(&origin),
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
        // Only an archival emits ChangeArchived (a deletion does not), in
        // the order the archivals were recorded above.
        for change_id in archived {
            let _ = self.event_tx.send(CacheEvent::ChangeArchived {
                workspace: workspace.uri.clone(),
                change_id,
            });
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

        // Only a repository whose git events are observed before its status
        // reads begin may have them recorded: a git event that lands after
        // that is then guaranteed to re-status it. A read that started before
        // its monitor existed is never kept.
        let recordable: HashSet<PathBuf> = gathered
            .iter()
            .filter_map(|input| match input {
                repo_view::GatheredInput::Repo(repo)
                    if self.observes_git_events(repo.repo_id()) =>
                {
                    Some(repo.worktree_paths().map(Path::to_path_buf))
                }
                _ => None,
            })
            .flatten()
            .collect();

        // PHASE 2 (compute): the git I/O, with no lock held.
        let (new_views, statused) = repo_view::compute_views_with_states(gathered);

        // PHASE 3 (merge): a short `last_views` write lock. Safe against a
        // concurrent recompute's read-modify-write only because the caller
        // (`refresh_aggregated_view` / `refresh_aggregated_view_for`) holds
        // `recompute` across this whole method.
        let events = {
            let last = self.last_views.read().unwrap();
            diff_views(&last, &new_views)
        };
        *self.last_views.write().unwrap() = new_views;
        self.record_worktree_git(
            MemoScope::All,
            statused
                .into_iter()
                .filter(|(path, _)| recordable.contains(path)),
        );
        events
    }

    /// Record freshly computed worktree git states: the one write path into
    /// `worktree_git`, shared by the full and scoped recomputes. Replaces the
    /// entries `scope` names with `fresh`, which holds warm worktrees only.
    /// Caller holds `recompute`.
    fn record_worktree_git(
        &self,
        scope: MemoScope<'_>,
        fresh: impl IntoIterator<Item = (PathBuf, WorktreeGitState)>,
    ) {
        let mut memo = self.worktree_git.lock().unwrap_or_else(|e| e.into_inner());
        match scope {
            MemoScope::All => memo.clear(),
            MemoScope::Worktrees(previous) => {
                for path in previous {
                    memo.remove(path);
                }
            }
        }
        memo.extend(fresh);
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
    ///
    /// `restatus` bounds the git I/O within the repository: [`Restatus::All`]
    /// re-statuses every warm worktree (a git event, re-enable), and
    /// [`Restatus::Worktree`] re-statuses that worktree alone, every sibling
    /// reusing its recorded state when that state classified the active
    /// change ids it holds now (a file batch; see [`Self::worktree_git`]).
    ///
    /// The merge swaps the new view into its slot in place and diffs only the
    /// replaced repository's old view against the new one, with the
    /// `last_views` lock released: every other row is untouched, so the
    /// events equal a whole-snapshot diff's.
    fn refresh_aggregated_view_for(&self, repo_id: &RepoId, restatus: Restatus) -> Vec<CacheEvent> {
        let _guard = self.recompute.lock().unwrap_or_else(|e| e.into_inner());

        let registry = match self.registry.as_ref() {
            Some(r) => r.clone(),
            None => return Vec::new(),
        };

        let disabled = self.disabled_snapshot();
        // Reuse stands in for a status only while the repository's git
        // events re-status it; with no live monitor nothing ever would. For
        // the same reason, nothing read here is recorded unless the monitor
        // already observed the repository before the reads began.
        let observed = self.observes_git_events(repo_id);
        let restatus = if observed { restatus } else { Restatus::All };

        let gathered = {
            let reg = match registry.lock() {
                Ok(g) => g,
                Err(_) => return Vec::new(),
            };
            let cache = self.cache.read().unwrap();
            let memo = self.worktree_git.lock().unwrap_or_else(|e| e.into_inner());
            repo_view::gather_repo_view(
                &reg,
                &cache,
                repo_id,
                |id| self.default_branch(id),
                |key| disabled.contains(key),
                &restatus,
                &memo,
            )
        };
        let Some(gathered) = gathered else {
            return self.refresh_aggregated_view_locked();
        };

        let (new_repo_view, statused) = repo_view::compute_gathered_repo_view_with_states(gathered);
        let new_index = LogicalIndex::of_repo(&new_repo_view);

        let replaced = replace_repo_view(&mut self.last_views.write().unwrap(), new_repo_view);
        let Some(old_repo_view) = replaced else {
            // Repo is registered but not yet in the snapshot — first appearance.
            return self.refresh_aggregated_view_locked();
        };
        self.record_worktree_git(
            MemoScope::Worktrees(&old_repo_view.worktrees),
            statused.into_iter().filter(|_| observed),
        );
        LogicalIndex::of_repo(&old_repo_view).diff(&new_index)
    }

    /// A flat (non-git) workspace's file batch: rebuild that one top-level row
    /// from the cache, re-stamp its OpenSpec presence, and splice it in at its
    /// config position, spawning no git process (`working-tree-status`:
    /// *Status Freshness*). Falls back to the full recompute when the row is
    /// not in the snapshot yet, or `uri` is no longer a user-registered flat
    /// entry. Returns the full recompute's events in that case; a splice
    /// yields none, because [`diff_views`] never indexes flat rows.
    fn refresh_flat_view_for(&self, uri: &Path) -> Vec<CacheEvent> {
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
            repo_view::gather_flat_view(&reg, &cache, uri, |key| disabled.contains(key))
        };
        let Some(mut row) = gathered else {
            return self.refresh_aggregated_view_locked();
        };
        repo_view::stamp_openspec_presence(std::slice::from_mut(&mut row));

        if !replace_flat_view(&mut self.last_views.write().unwrap(), row) {
            return self.refresh_aggregated_view_locked();
        }
        Vec::new()
    }

    /// Whether `repo_id`'s git events are observed: a monitor is installed
    /// and its watcher was built. Only then may a file batch reuse a sibling
    /// worktree's recorded git state, because only then does a branch move or
    /// a stage there re-status it (`working-tree-status`: *Status
    /// Freshness*). A WSL repository is refused separately, in
    /// [`repo_view::gather_repo_view`].
    fn observes_git_events(&self, repo_id: &RepoId) -> bool {
        self.repo_monitors
            .lock()
            .ok()
            .and_then(|monitors| monitors.get(repo_id).map(RepoMonitor::observes_git_events))
            .unwrap_or(false)
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
    /// registered, or it's a flat non-git workspace. The batch then goes to
    /// [`Self::refresh_flat_view_for`], which splices a registered flat row
    /// in place and falls back to the full recompute for anything else.
    fn repo_id_for(&self, workspace_uri: &Path) -> Option<RepoId> {
        let registry = self.registry.as_ref()?;
        let reg = registry.lock().ok()?;
        reg.entry(workspace_uri)?.repo_id.clone()
    }

    /// The local git identity for `workspace_uri`, memoized per `repo_id`
    /// (see [`Self::identity_cache`] on the struct).
    ///
    /// A flat, non-git workspace (`repo_id: None`) resolves no identity and
    /// spawns nothing (`developer-identity`: *Local Identity Resolution from
    /// Git Config*). Asking git there would fall through to the global config
    /// and stamp a global author on an event the spec records author-less.
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
        let repo_id = repo_id?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast::error::TryRecvError;

    const DEBOUNCE: Duration = Duration::from_millis(50);

    fn workspace_in(tmp: &tempfile::TempDir) -> WorkspaceFolder {
        WorkspaceFolder::from_path(tmp.path().canonicalize().unwrap())
    }

    fn write_change(workspace: &WorkspaceFolder, id: &str) {
        let dir = workspace.uri.join("openspec/changes").join(id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("proposal.md"), format!("# {id}\n")).unwrap();
    }

    #[test]
    fn a_replay_runs_only_when_a_watched_openspec_folder_was_not_parsed() {
        assert!(
            needs_replay(false, false),
            "watched now, unseen by the parse"
        );
        assert!(!needs_replay(false, true), "the parse already saw it");
        assert!(!needs_replay(true, false), "nothing watched yet");
        assert!(!needs_replay(true, true));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_re_arm_for_a_removed_workspace_installs_nothing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let workspace = workspace_in(&tmp);
        let manager = WatcherManager::new(DEBOUNCE);
        manager.add_workspace(workspace.clone()).await.unwrap();
        assert!(manager.is_awaiting_openspec(&workspace.uri));

        // Removed while its re-arm was being built.
        manager.remove_workspace(&workspace.uri);
        write_change(&workspace, "demo");
        manager
            .arm(workspace.clone(), Arm::IfStillTracked, false)
            .unwrap();

        assert!(!manager.is_watching(&workspace.uri));
        assert!(!manager.inner.cache.read().unwrap().contains(&workspace.uri));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_batch_for_a_removed_workspace_writes_and_announces_nothing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config = tempfile::TempDir::new().unwrap();
        let workspace = workspace_in(&tmp);
        write_change(&workspace, "demo");
        let manager = WatcherManager::new(DEBOUNCE);
        let log = Arc::new(crate::activity_log::ActivityLog::open(config.path()));
        manager.set_activity_log(log.clone());
        let mut rx = manager.subscribe();

        // No cache entry: the state a removal leaves behind while a batch for
        // the workspace is still parsing.
        Arc::clone(&manager.inner)
            .handle_events(&workspace, vec![openspec_changes_appeared(&workspace.uri)])
            .await;

        assert!(!manager.inner.cache.read().unwrap().contains(&workspace.uri));
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
        assert!(log.is_empty(), "nothing recorded: {:?}", log.snapshot());
        assert_eq!(log.persisted_appends(), 0);
    }

    /// A batch that archives several changes, and also creates one, records
    /// everything in one append, and still announces each archival.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_batch_that_archives_several_changes_appends_once() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config = tempfile::TempDir::new().unwrap();
        let workspace = workspace_in(&tmp);
        for id in ["one", "two", "three", "gone"] {
            write_change(&workspace, id);
        }
        let manager = WatcherManager::new(DEBOUNCE);
        let log = Arc::new(crate::activity_log::ActivityLog::open(config.path()));
        manager.set_activity_log(log.clone());
        let parsed = parse_all_changes(&workspace).unwrap();
        manager
            .inner
            .cache
            .write()
            .unwrap()
            .insert(workspace.uri.clone(), parsed);
        let changes = workspace.uri.join("openspec/changes");
        std::fs::create_dir_all(changes.join("archive")).unwrap();
        for id in ["one", "two", "three"] {
            std::fs::rename(
                changes.join(id),
                changes
                    .join("archive")
                    .join(format!("2026-01-0{}-{id}", id.len())),
            )
            .unwrap();
        }
        std::fs::remove_dir_all(changes.join("gone")).unwrap();
        write_change(&workspace, "fresh");
        let mut rx = manager.subscribe();

        Arc::clone(&manager.inner)
            .handle_events(&workspace, vec![openspec_changes_appeared(&workspace.uri)])
            .await;

        assert_eq!(log.persisted_appends(), 1);
        let recorded = log.snapshot();
        let mut archived: Vec<&str> = recorded
            .iter()
            .filter(|a| a.kind == crate::activity_log::AchievementKind::ChangeArchived)
            .filter_map(|a| a.change_id.as_deref())
            .collect();
        archived.sort_unstable();
        assert_eq!(archived, ["one", "three", "two"], "{recorded:?}");
        assert!(recorded.iter().any(|a| {
            a.kind == crate::activity_log::AchievementKind::ChangeCreated
                && a.change_id.as_deref() == Some("fresh")
        }));
        let announced: Vec<String> = std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|event| match event {
                CacheEvent::ChangeArchived { change_id, .. } => Some(change_id),
                _ => None,
            })
            .collect();
        let recorded_order: Vec<String> = recorded
            .iter()
            .filter(|a| a.kind == crate::activity_log::AchievementKind::ChangeArchived)
            .filter_map(|a| a.change_id.clone())
            .collect();
        assert_eq!(announced, recorded_order, "one announcement per archival");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn arming_over_a_parse_that_missed_openspec_replays_it() {
        let tmp = tempfile::TempDir::new().unwrap();
        let workspace = workspace_in(&tmp);
        let manager = WatcherManager::new(DEBOUNCE);
        let mut rx = manager.subscribe();
        // The cache was parsed before `openspec/` existed...
        manager
            .inner
            .cache
            .write()
            .unwrap()
            .insert(workspace.uri.clone(), Vec::new());
        // ...and it appeared before the watcher was armed.
        write_change(&workspace, "demo");
        manager.arm(workspace.clone(), Arm::Always, false).unwrap();
        assert!(!manager.is_awaiting_openspec(&workspace.uri));

        let added = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match rx.recv().await {
                    Ok(CacheEvent::ChangeAdded { change_id, .. }) => return change_id,
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => panic!("channel closed"),
                }
            }
        })
        .await
        .expect("the replay announces the change");
        assert_eq!(added, "demo");
        assert_eq!(manager.changes_for(&workspace.uri).len(), 1);
    }

    // ---------------------------------------------------------------------
    // Scoped recompute (`working-tree-status`: *Status Freshness*). Each
    // drives `handle_events` directly with a synthetic batch, so nothing
    // depends on a real filesystem watcher's timing, and filters the
    // process-global invocation log to its own temp directory.
    // ---------------------------------------------------------------------

    use crate::git::invocation_log;
    use crate::presentation::{PresentationKey, WorkspacePresentationStore};
    use crate::registry::WorkspaceRegistry;

    fn git_in(dir: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git invocation");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A repository with a committed `openspec/changes/` and a local identity.
    fn init_repo(root: &Path) -> PathBuf {
        std::fs::create_dir_all(root.join("openspec/changes")).unwrap();
        std::fs::write(root.join("openspec/changes/.keep"), "").unwrap();
        git_in(root, &["init", "-b", "main"]);
        git_in(root, &["config", "user.email", "t@t"]);
        git_in(root, &["config", "user.name", "t"]);
        git_in(root, &["add", "."]);
        git_in(root, &["commit", "-m", "init"]);
        root.canonicalize().unwrap()
    }

    fn add_worktree(main: &Path, branch: &str, path: &Path) -> PathBuf {
        git_in(
            main,
            &["worktree", "add", "-b", branch, path.to_str().unwrap()],
        );
        path.canonicalize().unwrap()
    }

    fn folder(path: &Path) -> WorkspaceFolder {
        WorkspaceFolder::from_path(path.to_path_buf())
    }

    /// A manager over `registry`, with every entry's changes parsed into the
    /// cache — what `add_workspace` leaves behind, without a real watcher.
    fn manager_over(registry: WorkspaceRegistry) -> WatcherManager {
        let folders = registry.folders();
        let manager = WatcherManager::with_registry(DEBOUNCE, Some(Arc::new(Mutex::new(registry))));
        for workspace in folders {
            let parsed = parse_all_changes(&workspace).unwrap_or_default();
            manager
                .inner
                .cache
                .write()
                .unwrap()
                .insert(workspace.uri, parsed);
        }
        manager
    }

    fn reparse_into_cache(manager: &WatcherManager, workspace: &Path) {
        let parsed = parse_all_changes(&folder(workspace)).unwrap();
        manager
            .inner
            .cache
            .write()
            .unwrap()
            .insert(workspace.to_path_buf(), parsed);
    }

    async fn batch(manager: &WatcherManager, workspace: &Path) {
        Arc::clone(&manager.inner)
            .handle_events(
                &folder(workspace),
                vec![openspec_changes_appeared(workspace)],
            )
            .await;
    }

    /// Git invocations since `mark` anchored under `root`.
    fn invocations_under(mark: usize, root: &Path) -> Vec<invocation_log::Invocation> {
        invocation_log::recorded_since(mark)
            .into_iter()
            .filter(|inv| inv.anchor.starts_with(root))
            .collect()
    }

    fn status_anchors(invocations: &[invocation_log::Invocation]) -> Vec<PathBuf> {
        invocations
            .iter()
            .filter(|inv| inv.args.iter().any(|a| a == "status"))
            .map(|inv| inv.anchor.clone())
            .collect()
    }

    fn memo_keys(manager: &WatcherManager) -> Vec<PathBuf> {
        let mut keys: Vec<PathBuf> = manager
            .inner
            .worktree_git
            .lock()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        keys.sort();
        keys
    }

    fn repo_row(manager: &WatcherManager, main: &Path) -> crate::repo_view::RepoView {
        manager
            .workspace_views()
            .into_iter()
            .find_map(|view| match view {
                WorkspaceView::Repo(repo) if repo.main_worktree == main => Some(repo),
                _ => None,
            })
            .expect("the repository's row")
    }

    /// Wait until no git invocation under `root` has been recorded for a few
    /// consecutive polls, so a repo monitor's install-time work lands before
    /// a test takes its mark. Best-effort: waiting longer can only move
    /// invocations before the mark.
    async fn quiesce(root: &Path) {
        let count = || invocations_under(0, root).len();
        let mut last = count();
        let mut stable = 0;
        let start = std::time::Instant::now();
        while stable < 3 && start.elapsed() < Duration::from_secs(5) {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let now = count();
            stable = if now == last { stable + 1 } else { 0 };
            last = now;
        }
    }

    /// Repository `api` with a linked worktree `api-feature` that holds a
    /// committed change `bar`, both tracked and statused once by a full
    /// recompute. With `monitored`, the repository's monitor is installed, so
    /// its git events are observed and a file batch may reuse a sibling's
    /// recorded state. Returns the temp root, the manager and the worktrees.
    async fn two_worktrees(
        tmp: &tempfile::TempDir,
        monitored: bool,
    ) -> (PathBuf, WatcherManager, PathBuf, PathBuf) {
        let root = tmp.path().canonicalize().unwrap();
        let w1 = init_repo(&root.join("api"));
        let w2 = add_worktree(&w1, "feature", &root.join("api-feature"));
        write_change(&folder(&w2), "bar");
        git_in(&w2, &["add", "."]);
        git_in(&w2, &["commit", "-m", "bar"]);
        let mut registry = WorkspaceRegistry::new(root.join("ws.json"));
        registry.register(w1.clone()).unwrap();
        assert!(
            registry.entry(&w2).is_some(),
            "the linked worktree is tracked"
        );
        let manager = manager_over(registry);
        if monitored {
            manager.sync_repos();
        }
        manager.refresh_aggregated_view();
        quiesce(&root).await;
        (root, manager, w1, w2)
    }

    /// *A spec file edit re-statuses only its own worktree*: W2 is not
    /// statused, so an unsignalled non-spec edit there stays invisible, and
    /// its branch and spec commit states are the recorded ones.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_spec_edit_re_statuses_only_its_own_worktree() {
        invocation_log::enable();
        let tmp = tempfile::TempDir::new().unwrap();
        let (root, manager, w1, w2) = two_worktrees(&tmp, true).await;
        assert_eq!(memo_keys(&manager), vec![w1.clone(), w2.clone()]);
        let before = repo_row(&manager, &w1);
        assert!(before.dirty_worktrees.is_empty(), "both start clean");

        std::fs::write(w2.join("scratch.txt"), "unsignalled").unwrap();
        write_change(&folder(&w1), "foo");
        let mark = invocation_log::mark();
        batch(&manager, &w1).await;

        let invocations = invocations_under(mark, &root);
        assert_eq!(
            status_anchors(&invocations),
            vec![w1.clone()],
            "{invocations:?}"
        );
        assert!(
            invocations.iter().all(|inv| !inv.anchor.starts_with(&w2)),
            "nothing at all is spawned for W2: {invocations:?}"
        );
        let after = repo_row(&manager, &w1);
        assert_eq!(
            after.dirty_worktrees,
            vec![w1.clone()],
            "W1's new change is untracked"
        );
        assert_eq!(
            after.worktree_refs, before.worktree_refs,
            "branches as recorded"
        );
        let bar = after
            .active
            .iter()
            .find(|lc| lc.name == "bar")
            .expect("bar");
        assert_eq!(
            bar.instances[0].spec_commit_state,
            crate::git::SpecCommitState::Committed
        );
        assert!(after.active.iter().any(|lc| lc.name == "foo"));
    }

    /// *A sibling worktree with a changed change set is re-statused*: W2's
    /// active changes moved (as a populate or re-registration moves them)
    /// without W2 being statused, so W1's batch statuses it too.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_sibling_with_a_changed_change_set_is_re_statused() {
        invocation_log::enable();
        let tmp = tempfile::TempDir::new().unwrap();
        let (root, manager, w1, w2) = two_worktrees(&tmp, true).await;

        write_change(&folder(&w2), "baz");
        reparse_into_cache(&manager, &w2);
        write_change(&folder(&w1), "foo");
        let mark = invocation_log::mark();
        batch(&manager, &w1).await;

        let mut statused = status_anchors(&invocations_under(mark, &root));
        statused.sort();
        assert_eq!(statused, vec![w1.clone(), w2.clone()]);
        let after = repo_row(&manager, &w1);
        assert_eq!(after.dirty_worktrees, vec![w1.clone(), w2.clone()]);
        let recorded = manager.inner.worktree_git.lock().unwrap()[&w2]
            .classified_ids
            .clone();
        assert_eq!(recorded, vec!["bar".to_string(), "baz".to_string()]);
    }

    /// With no live repo monitor, the repository's git events are never
    /// observed, so nothing would ever re-status a sibling: a file batch
    /// statuses every worktree.
    #[tokio::test(flavor = "multi_thread")]
    async fn an_unmonitored_repository_re_statuses_every_worktree() {
        invocation_log::enable();
        let tmp = tempfile::TempDir::new().unwrap();
        let (root, manager, w1, w2) = two_worktrees(&tmp, false).await;

        std::fs::write(w2.join("scratch.txt"), "unsignalled").unwrap();
        write_change(&folder(&w1), "foo");
        let mark = invocation_log::mark();
        batch(&manager, &w1).await;

        let mut statused = status_anchors(&invocations_under(mark, &root));
        statused.sort();
        assert_eq!(statused, vec![w1.clone(), w2.clone()]);
        assert_eq!(
            repo_row(&manager, &w1).dirty_worktrees,
            vec![w1.clone(), w2.clone()]
        );
        assert!(memo_keys(&manager).is_empty(), "and nothing is recorded");
    }

    /// The memo holds exactly the warm tracked worktrees after a full
    /// recompute, a scoped one prunes a worktree no longer tracked, and a
    /// disabled repository records nothing — so re-enabling it reads git
    /// again rather than inheriting a clean result.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_memo_holds_exactly_the_warm_tracked_worktrees() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let w1 = init_repo(&root.join("api"));
        let w2 = add_worktree(&w1, "feature", &root.join("api-feature"));
        let parked = init_repo(&root.join("parked"));
        let mut registry = WorkspaceRegistry::new(root.join("ws.json"));
        registry.register(w1.clone()).unwrap();
        registry.register(parked.clone()).unwrap();
        let parked_id = registry.entry(&parked).unwrap().repo_id.clone().unwrap();
        let api_id = registry.entry(&w1).unwrap().repo_id.clone().unwrap();
        let store = Arc::new(Mutex::new(WorkspacePresentationStore::new(
            root.join("presentation.json"),
        )));
        let parked_key = PresentationKey::Repo(parked_id.as_path().to_path_buf());
        store
            .lock()
            .unwrap()
            .set_disabled(parked_key.clone(), true)
            .unwrap();
        let manager = manager_over(registry);
        manager.set_presentation(store.clone());

        // Nothing is recorded for a repository whose git events nobody
        // observes yet: no later git event would replace it.
        manager.refresh_aggregated_view();
        assert!(memo_keys(&manager).is_empty());

        manager.sync_repos();
        manager.refresh_aggregated_view();
        assert_eq!(memo_keys(&manager), vec![w1.clone(), w2.clone()]);

        // A worktree no longer tracked leaves the memo on the scoped path.
        manager
            .inner
            .registry
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .unregister(&w2)
            .unwrap();
        manager.refresh_status_for(&api_id);
        assert_eq!(memo_keys(&manager), vec![w1.clone()]);

        // Re-enable: the row is statused for real.
        std::fs::write(parked.join("scratch.txt"), "dirty").unwrap();
        store
            .lock()
            .unwrap()
            .set_disabled(parked_key, false)
            .unwrap();
        manager.refresh_status_for(&parked_id);
        assert!(
            repo_row(&manager, &parked).dirty,
            "real git state, not a cached clean"
        );
        assert_eq!(memo_keys(&manager), vec![w1.clone(), parked.clone()]);
    }

    /// *A file edit in a flat workspace spawns no git process*, and still
    /// refreshes its own row in place; its live event carries no author.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_flat_batch_spawns_no_git_and_rebuilds_only_its_row() {
        invocation_log::enable();
        let tmp = tempfile::TempDir::new().unwrap();
        let config = tempfile::TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let a = init_repo(&root.join("a"));
        let notes = root.join("notes");
        std::fs::create_dir_all(notes.join("openspec/changes")).unwrap();
        let b = init_repo(&root.join("b"));
        let mut registry = WorkspaceRegistry::new(root.join("ws.json"));
        for path in [&a, &notes, &b] {
            registry.register(path.clone()).unwrap();
        }
        let manager = manager_over(registry);
        let log = Arc::new(crate::activity_log::ActivityLog::open(config.path()));
        manager.set_activity_log(log.clone());
        manager.refresh_aggregated_view();
        let before = manager.workspace_views();

        write_change(&folder(&notes), "idea");
        let mark = invocation_log::mark();
        batch(&manager, &notes).await;

        let spawned = invocations_under(mark, &root);
        assert!(
            spawned.is_empty(),
            "no git process of any kind: {spawned:?}"
        );
        let after = manager.workspace_views();
        assert_eq!(after.len(), 3);
        assert_eq!(after[0], before[0], "repository A untouched, in place");
        assert_eq!(after[2], before[2], "repository B untouched, in place");
        match &after[1] {
            WorkspaceView::Flat {
                workspace,
                changes,
                has_open_spec,
                ..
            } => {
                assert_eq!(workspace.uri, notes);
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0].change_id, "idea");
                assert!(*has_open_spec);
            }
            other @ WorkspaceView::Repo(_) => panic!("the flat row keeps its position: {other:?}"),
        }
        let created: Vec<_> = log
            .snapshot()
            .into_iter()
            .filter(|a| a.kind == crate::activity_log::AchievementKind::ChangeCreated)
            .collect();
        assert_eq!(created.len(), 1, "{created:?}");
        assert_eq!(
            created[0].author, None,
            "a flat workspace resolves no identity"
        );

        // The control: a repository's batch is attributed to its identity.
        write_change(&folder(&a), "feature");
        batch(&manager, &a).await;
        let author = log
            .snapshot()
            .into_iter()
            .find(|a| a.change_id.as_deref() == Some("feature"))
            .expect("the repository's change is recorded")
            .author;
        assert_eq!(
            author,
            Some(crate::identity::Author {
                name: Some("t".into()),
                email: Some("t@t".into()),
            })
        );
    }

    /// With no activity log attached there is nothing to attribute, so a
    /// batch in a repository never reads the identity either.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_batch_with_no_activity_log_reads_no_identity() {
        invocation_log::enable();
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let a = init_repo(&root.join("a"));
        let mut registry = WorkspaceRegistry::new(root.join("ws.json"));
        registry.register(a.clone()).unwrap();
        let manager = manager_over(registry);
        manager.refresh_aggregated_view();

        write_change(&folder(&a), "foo");
        let mark = invocation_log::mark();
        batch(&manager, &a).await;

        let spawned = invocations_under(mark, &root);
        assert!(
            !spawned
                .iter()
                .any(|inv| inv.args.iter().any(|arg| arg == "config")),
            "{spawned:?}"
        );
        assert_eq!(
            status_anchors(&spawned),
            vec![a.clone()],
            "the batch still statuses A"
        );
        assert!(repo_row(&manager, &a)
            .active
            .iter()
            .any(|lc| lc.name == "foo"));
    }

    /// A flat row whose `openspec/` went away re-derives its presence on the
    /// splice; a flat row not in the snapshot yet falls back to the full
    /// recompute, whose events it returns.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_flat_splice_restamps_presence_and_falls_back_on_first_appearance() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let a = init_repo(&root.join("a"));
        write_change(&folder(&a), "shipped");
        let notes = root.join("notes");
        std::fs::create_dir_all(notes.join("openspec/changes")).unwrap();
        let mut registry = WorkspaceRegistry::new(root.join("ws.json"));
        registry.register(a.clone()).unwrap();
        registry.register(notes.clone()).unwrap();
        let manager = manager_over(registry);

        // First appearance: nothing aggregated yet.
        let events = manager.inner.refresh_flat_view_for(&notes);
        assert!(
            events.contains(&CacheEvent::LogicalChangeAdded {
                repo_id: repo_row(&manager, &a).repo_id,
                change_name: "shipped".into(),
            }),
            "the full recompute's events: {events:?}"
        );
        assert_eq!(manager.workspace_views().len(), 2);

        std::fs::remove_dir_all(notes.join("openspec")).unwrap();
        let events = manager.inner.refresh_flat_view_for(&notes);
        assert!(events.is_empty(), "a splice yields no event: {events:?}");
        let views = manager.workspace_views();
        assert!(matches!(
            &views[1],
            WorkspaceView::Flat {
                has_open_spec: false,
                ..
            }
        ));

        // No longer a registered flat entry: the full recompute drops it.
        manager
            .inner
            .registry
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .unregister(&notes)
            .unwrap();
        manager.inner.refresh_flat_view_for(&notes);
        assert_eq!(manager.workspace_views().len(), 1);
    }
}
