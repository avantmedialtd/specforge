//! The headless application service shared by both frontends.
//!
//! `AppService` owns the stateful handles (registry, settings, presentation,
//! activity log, watcher) and exposes the read surface both the Tauri shell and
//! the terminal frontend render. The orchestration that previously lived behind
//! `#[tauri::command]` in the shell — most importantly the ~270-line dashboard
//! assembly — lives here as plain methods, so it is callable in-process by
//! either frontend and reachable from `cargo test`.

use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use openspec_core::{
    build_backfill, change_lifecycle_checked, commit_activity_with_authors, commit_base,
    commit_file_blobs, commit_file_diff, commit_file_list, commit_log, commit_log_authored,
    commit_patch, compute_dashboard, compute_garden, compute_progress, day_axis,
    detect_candidate_identities, eager_by_lines, event_is_me, git_common_dir, group_archived_rows,
    group_workspace_file_rows, is_me, is_object_id, layout_commit_graph, list_archived_summaries,
    local_today, mark_divergent_rows, markdown_files, parse_artifact_status, parse_proposal_title,
    sort_plots, task_completion_history, today_str, walk_markdown_files, worktree_list,
    ActivityLog, ArchiveScope, ArchivedChangeRow, ArchivedChangeSummary, ArtifactStatus, Author,
    CacheEvent, ChangeData, ChangeLifecycle, CommitActivityCache, CommitGraph, CommitReadError,
    DashboardData, DiffContent, DiffFile, DocumentKey, DocumentWatcher, FileScope, FileStatus,
    IdentityConfig, LifecycleCache, PaletteColor, PresentationKey, RegisteredWorkspace, RepoId,
    WatcherManager, WorkspaceFileRow, WorkspaceGarden, WorkspaceOrigin, WorkspacePresentationStore,
    WorkspaceRegistry, WorkspaceView,
};
use serde::Serialize;
use tokio::sync::broadcast;

use crate::bitbucket::{BitbucketLimits, BitbucketPullRequestsHandle, BitbucketPullRequestsState};
use crate::chatgpt_quota::{ChatGptQuotaHandle, ChatGptQuotaState};
use crate::events::{PullRequestProvider, PullRequestProviderChangedPayload, ServiceNotice};
use crate::github::{GithubLimits, GithubPullRequestsHandle, GithubPullRequestsState};
use crate::pull_request_cache::{FileAnswer, FileChanged, PullRequestDetails};
use crate::pull_request_detail::{
    listed_row, openable_link, CachedFile, ImageSide, ImageVersions, PullRequestDetail,
    PullRequestDetailOutcome, PullRequestFileOutcome, PullRequestImageOutcome,
    PullRequestReference, ReadEnd,
};
use crate::pull_request_read::{
    file_failure, image_failure, now_unix, provider_enabled, read_pull_request,
    read_pull_request_file, read_pull_request_image, DetailIo, LiveIo, ReadContext,
};
use crate::quota::{ClaudeQuotaState, QuotaHandle};
use crate::review_progress::{self, Lists, MarkWrite, ReviewProgress, ReviewProgressStore};
use crate::settings::SettingsStore;

/// The progress layer's heatmap / streak window — 53 weeks of local calendar
/// days, so
/// the contribution grid reads as a full-year GitHub-style band. Bounded.
pub const DASHBOARD_HEATMAP_WINDOW_DAYS: u64 = 371;
/// How many commits per repo the garden reads before filtering to today.
const GARDEN_COMMIT_LIMIT: usize = 500;
/// Bounded window for the one-time git backfill of historical achievements.
/// Matches the heatmap window so a year of contribution cells has data to show.
const BACKFILL_SINCE: &str = "54 weeks ago";
/// Debounce for the filesystem watcher.
const WATCH_DEBOUNCE_MS: u64 = 200;
/// Size cap for a workspace file browser read — defensive; markdown this
/// large would drown the renderer anyway.
const MAX_WORKSPACE_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// How many service notices a lagging subscriber may fall behind by before it
/// skips some. A notice only asks a view to re-read, so a skipped one costs a
/// stale view until the next.
const NOTICE_CHANNEL_CAPACITY: usize = 64;

/// The path guard shared by the workspace-file read and the document watch —
/// the *where within a root* half of the browsing contract, applied on top of
/// (never instead of) the registry authorisation that decides *which* roots
/// may be reached at all.
///
/// Rejects absolute paths and `..` components lexically, resolves the path
/// under `root`, requires the result to stay under the canonical root — which
/// is what catches a symlink escape — and requires a case-insensitive `.md`
/// extension.
///
/// The resolved path is **not** required to exist. A read adds that check
/// itself; a document watch must not, because a reader keeps watching a
/// document that has been deleted and may reappear, which is the whole reason
/// the guard is shared rather than duplicated: the two callers differ in
/// exactly one rule, and every other rule now has one definition.
fn guard_workspace_document(root: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let rel = Path::new(rel_path);
    if rel.is_absolute() {
        return Err("path must be relative".to_string());
    }
    if rel.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("path must not contain `..`".to_string());
    }

    let root_canonical =
        openspec_core::canonicalize(root).map_err(|e| format!("workspace root not found: {e}"))?;
    let resolved = openspec_core::canonicalize_existing_prefix(&root.join(rel));
    if !resolved.starts_with(&root_canonical) {
        return Err("path escapes workspace".to_string());
    }
    let has_md_extension = resolved
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md"));
    if !has_md_extension {
        return Err("only .md files can be read".to_string());
    }
    Ok(resolved)
}

/// The developer-identity payload for the Settings → Identity section: the saved
/// configuration and the distinct git identities detected across registered
/// workspaces, offered as alias suggestions. Lives here (rather than behind a
/// `#[tauri::command]`) so every frontend — the shell, the terminal UI, and the
/// web server — returns the identical shape.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInfo {
    pub config: IdentityConfig,
    pub candidates: Vec<Author>,
}

/// One artifact read: its markdown together with when the file it came from was
/// last written.
///
/// The two travel together because they must describe the *same* read. Resolved
/// by two separate calls, a body and a modification time could be taken at
/// different instants and nothing in either signature would say they had to
/// match — so the frontend could pair fresh bytes with a stale time, or the
/// reverse, and never know.
///
/// `modified_at` is unix seconds, the encoding `ChangeInstance::modified_at`
/// already uses, so the frontend holds one time representation rather than two.
///
/// It is `None` — never a fabricated epoch — when the filesystem reports no
/// usable modification time. The read as a whole still succeeds, because the
/// artifact is perfectly displayable without one; the caller renders no label
/// rather than rendering 1970, which would state a falsehood in exactly the
/// confident tone it states facts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRead {
    pub body: String,
    pub modified_at: Option<u64>,
}

/// The stateful "brain" shared by the frontends. Cheaply cloneable — every
/// field is an `Arc`/handle that shares its state, so clones observe the same
/// registry, settings, cache, and watcher.
#[derive(Clone)]
pub struct AppService {
    pub registry: Arc<Mutex<WorkspaceRegistry>>,
    pub settings: Arc<SettingsStore>,
    pub presentation: Arc<Mutex<WorkspacePresentationStore>>,
    pub activity: Arc<ActivityLog>,
    pub watcher: WatcherManager,
    /// Per-document filesystem watches, one per document some surface is
    /// currently displaying. Deliberately separate from `watcher`, which is
    /// scoped to `openspec/changes/` and keeps the change cache fresh; this
    /// one keeps an *open document* fresh wherever in the workspace it lives.
    /// See [`openspec_core::document_watch`].
    pub documents: DocumentWatcher,
    /// Latest opt-in Claude usage-quota snapshot, written by the quota poller
    /// and read by both frontends. `Disabled` until the poller runs with the
    /// feature enabled.
    pub quota: QuotaHandle,
    /// Latest opt-in ChatGPT usage-quota snapshot, written by the ChatGPT
    /// quota poller and read by both frontends. `Disabled` until the poller
    /// runs with the feature enabled. A twin of `quota` — see
    /// `chatgpt_quota.rs`.
    pub chatgpt_quota: ChatGptQuotaHandle,
    /// Latest opt-in BitBucket pull-request snapshot, written by the
    /// pull-request poller and read by the desktop app and the browser skin.
    /// `Disabled` until the poller runs with the feature enabled. See
    /// `bitbucket.rs`.
    pub bitbucket: BitbucketPullRequestsHandle,
    /// Latest opt-in GitHub pull-request snapshot, written by the GitHub poller
    /// and read by the desktop app and the browser skin. `Disabled` until the
    /// poller runs with the feature enabled. A twin of `bitbucket` — see
    /// `github.rs`.
    pub github: GithubPullRequestsHandle,
    /// BitBucket's limits — its rate-limit deadline, its hourly detail budget
    /// and its detail reads in flight — shared by its poller and the
    /// pull-request viewer's detail reads. They are the provider's: nothing
    /// resets them, not even disabling it or saving its credential. See
    /// `pull_request_limits.rs`.
    pub bitbucket_limits: BitbucketLimits,
    /// GitHub's limits, the twin of `bitbucket_limits`, with a GraphQL and a
    /// REST deadline.
    pub github_limits: GithubLimits,
    /// The pull-request viewer's detail cache, its reads in flight, and each
    /// provider's credential generation. See `pull_request_cache.rs`.
    pub(crate) pull_request_details: PullRequestDetails,
    /// `review-progress.json`, the files the reader marked viewed. See
    /// `review_progress.rs`.
    pub(crate) review_store: Arc<ReviewProgressStore>,
    /// The service's own broadcast of [`ServiceNotice`]s: state no
    /// `CacheEvent` describes, which every window and every served tab of this
    /// service must hear whichever transport changed it. See
    /// [`Self::subscribe_notices`].
    notices: broadcast::Sender<ServiceNotice>,
    /// Per-repository cache of mined [`openspec_core::ChangeLifecycle`] data
    /// (see `openspec_core::LifecycleCache`), so `dashboard()` and the
    /// first-launch backfill mine a repository's history at most once per
    /// change to it rather than once per fetch. Kept correct by
    /// [`Self::spawn_lifecycle_cache_invalidator`], installed by
    /// [`Self::bootstrap`], which invalidates a repository's entry on
    /// `CacheEvent::GraphChanged`.
    pub lifecycle_cache: LifecycleCache,
    /// Per-repository cache of the year-long commit walk backing the heatmap and
    /// streak. Invalidated by the same `GraphChanged` signal as
    /// `lifecycle_cache`, in the same subscriber.
    pub commit_activity_cache: CommitActivityCache,
}

/// Move a corrupt `workspaces.json` aside to the first free
/// `workspaces.json.corrupt-<n>` sibling, so its data stays recoverable and a
/// later save does not overwrite it. Best-effort: if the file is gone or the
/// rename fails, leave things as-is. `review-progress.json` is moved aside the
/// same way.
pub(crate) fn preserve_corrupt_config(path: &std::path::Path) {
    if !path.exists() {
        return;
    }
    for n in 0u32..10_000 {
        let mut name = path.as_os_str().to_owned();
        name.push(format!(".corrupt-{n}"));
        let backup = std::path::PathBuf::from(name);
        if !backup.exists() {
            let _ = std::fs::rename(path, &backup);
            return;
        }
    }
    // Numeric range exhausted (astronomically unlikely): fall back to a
    // high-entropy suffix so the corrupt file is still moved aside and never
    // left in place to be overwritten by a later save.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".corrupt-{nanos}"));
    let _ = std::fs::rename(path, std::path::PathBuf::from(name));
}

impl AppService {
    /// Build the service against an application config directory: load the
    /// persisted stores, construct the watcher, and seed first-run defaults
    /// (the developer identity). Does **not** start
    /// watching any workspace yet — call [`AppService::populate`] for that, so a
    /// caller can subscribe to the event stream before the populate burst.
    pub fn bootstrap(config_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&config_dir).ok();

        let workspaces_path = config_dir.join("workspaces.json");
        let settings_path = config_dir.join("settings.json");
        let presentation_path = config_dir.join("presentation.json");
        // The activity log lives alongside the other app-data stores — never
        // inside any workspace's `openspec/` tree — preserving the Dashboard's
        // read-only relationship to workspaces.
        let activity_path = config_dir.join("activity.json");
        // So does the pull-request viewer's review progress, for the same
        // reason: it is the reader's own state, never a workspace's or a
        // provider's.
        let review_progress_path = config_dir.join("review-progress.json");

        let registry = match WorkspaceRegistry::load(workspaces_path.clone()) {
            Ok(reg) => reg,
            Err(err) => {
                // A corrupt registry must never be silently erased. Move the
                // unreadable file aside to a backup so the user's workspaces stay
                // recoverable, then start empty. Without this, the next
                // register/unregister would overwrite the corrupt file with `{}`
                // and lose every registered workspace.
                eprintln!(
                    "specforge: could not read {} ({err}); preserving it as a backup and starting with an empty registry",
                    workspaces_path.display()
                );
                preserve_corrupt_config(&workspaces_path);
                WorkspaceRegistry::new(workspaces_path)
            }
        };
        let settings = Arc::new(SettingsStore::load(settings_path));
        let presentation = WorkspacePresentationStore::load(presentation_path.clone())
            .unwrap_or_else(|_| WorkspacePresentationStore::new(presentation_path));
        let shared_presentation = Arc::new(Mutex::new(presentation));
        let shared_registry = Arc::new(Mutex::new(registry));

        // Seed the developer identity on first run from the git identities
        // detected across registered workspaces, so the profile and the
        // Dashboard's Me scope have a sensible default with no interaction.
        if settings.snapshot().identity.aliases.is_empty() {
            let folders: Vec<PathBuf> = shared_registry
                .lock()
                .map(|r| r.entries().iter().map(|e| e.folder.uri.clone()).collect())
                .unwrap_or_default();
            if let Some(primary) = detect_candidate_identities(&folders).into_iter().next() {
                let _ = settings.set_identity(IdentityConfig {
                    display_name: primary.name.clone(),
                    aliases: vec![primary],
                });
            }
        }

        let watcher = WatcherManager::with_registry(
            std::time::Duration::from_millis(WATCH_DEBOUNCE_MS),
            Some(shared_registry.clone()),
        );
        #[cfg(target_os = "windows")]
        watcher.set_poll_interval(std::time::Duration::from_secs(
            settings.wsl_poll_interval_secs(),
        ));
        let documents = DocumentWatcher::default();
        // Same cadence for open documents as for the tree — a reader on a WSL
        // workspace refreshing at a different rate from the row beside it would
        // be one setting with two meanings.
        #[cfg(target_os = "windows")]
        documents.set_poll_interval(std::time::Duration::from_secs(
            settings.wsl_poll_interval_secs(),
        ));

        let activity = Arc::new(ActivityLog::load(activity_path));
        watcher.set_activity_log(activity.clone());
        // The aggregator reads the per-row disabled flag from here, so a parked
        // row is gathered cold from the very first recompute rather than being
        // warmed once and only filtered afterwards.
        watcher.set_presentation(shared_presentation.clone());

        let svc = Self {
            registry: shared_registry,
            settings,
            presentation: shared_presentation,
            activity,
            watcher,
            documents,
            quota: QuotaHandle::new(),
            chatgpt_quota: ChatGptQuotaHandle::new(),
            bitbucket: BitbucketPullRequestsHandle::new(),
            github: GithubPullRequestsHandle::new(),
            bitbucket_limits: BitbucketLimits::new(),
            github_limits: GithubLimits::new(),
            pull_request_details: PullRequestDetails::default(),
            review_store: Arc::new(ReviewProgressStore::open(review_progress_path)),
            notices: broadcast::channel(NOTICE_CHANNEL_CAPACITY).0,
            lifecycle_cache: LifecycleCache::new(),
            commit_activity_cache: CommitActivityCache::new(),
        };

        // Keep `lifecycle_cache` correct as git history moves. Installed here
        // — before any frontend calls `populate`/`spawn_backfill` — so no
        // early `GraphChanged` (e.g. from `spawn_backfill` on first launch)
        // can land before the subscriber is listening.
        svc.spawn_lifecycle_cache_invalidator();
        // And prune review progress as the pull-request lists arrive, for the
        // same reason: listening before either poller can announce one.
        svc.spawn_review_progress_pruner();

        svc
    }

    /// Keep `lifecycle_cache` correct as git history moves: invalidate a
    /// repository's entry on `CacheEvent::GraphChanged { repo_id }`. Because
    /// the broadcast channel drops events for a lagging subscriber, a
    /// `RecvError::Lagged` here is treated as `invalidate_all()` rather than
    /// a no-op — unlike every other `CacheEvent` subscriber in this codebase
    /// (which simply resumes listening on `Lagged`) — because a dropped
    /// event is the only realistic way this specific subscriber can go
    /// stale (see design.md, "Missed-event risk"); a conservative full flush
    /// closes it. Runs on a plain thread (like `spawn_backfill` /
    /// `quota::spawn_poller`) rather than `tokio::spawn`, so the app layer
    /// stays agnostic of whether/how the caller of `bootstrap` manages an
    /// async runtime — `broadcast::Receiver::blocking_recv` needs no entered
    /// runtime. Lives for the process; there is deliberately no unsubscribe.
    fn spawn_lifecycle_cache_invalidator(&self) {
        let mut rx = self.watcher.subscribe();
        let lifecycle = self.lifecycle_cache.clone();
        let commits = self.commit_activity_cache.clone();
        std::thread::spawn(move || loop {
            match rx.blocking_recv() {
                // Both caches derive from the same append-only git history and
                // are therefore invalidated by the same signal, in one place —
                // a second subscriber could observe a different prefix of the
                // stream and leave the two disagreeing about a repository.
                Ok(CacheEvent::GraphChanged { repo_id }) => {
                    let repo = RepoId(repo_id);
                    lifecycle.invalidate(&repo);
                    commits.invalidate(&repo);
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    lifecycle.invalidate_all();
                    commits.invalidate_all();
                }
                Err(broadcast::error::RecvError::Closed) => return,
            }
        });
    }

    /// Prune review progress as the pull-request lists change: an entry
    /// untouched for 90 days whose enabled provider's complete list (nothing
    /// withheld, no workspace skipped) no longer holds its pull request, once
    /// every enabled provider's list has arrived in this run
    /// (`pull-request-viewer`: *Review Progress*). The rule is
    /// `review_progress::Lists`', applied on each `*PullRequestsUpdated`, so
    /// nothing is pruned at load, before any list exists. A lagging subscriber
    /// applies it too, since it reads the lists as they are now. Each pruned
    /// pull request raises `review-progress-changed`, so a view still showing
    /// its cached detail re-reads. A plain thread, as the lifecycle-cache
    /// invalidator's, holding none of the watcher, so it ends with the cache
    /// stream.
    fn spawn_review_progress_pruner(&self) {
        let mut rx = self.watcher.subscribe();
        let store = self.review_store.clone();
        let settings = self.settings.clone();
        let (github, bitbucket) = (self.github.clone(), self.bitbucket.clone());
        let notices = self.notices.clone();
        std::thread::spawn(move || loop {
            match rx.blocking_recv() {
                Ok(
                    CacheEvent::GithubPullRequestsUpdated
                    | CacheEvent::BitbucketPullRequestsUpdated,
                )
                | Err(broadcast::error::RecvError::Lagged(_)) => {
                    let (github, bitbucket) = (github.get(), bitbucket.get());
                    let lists = Lists {
                        github_enabled: settings.github_enabled(),
                        bitbucket_enabled: settings.bitbucket_enabled(),
                        github: &github,
                        bitbucket: &bitbucket,
                    };
                    // A store that cannot be read or written now is pruned
                    // at a later list.
                    for reference in lists.prune(&store, now_unix()).unwrap_or_default() {
                        let _ = notices.send(ServiceNotice::ReviewProgressChanged(reference));
                    }
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Closed) => return,
            }
        });
    }

    /// Subscribe to the watcher's `CacheEvent` stream. Callers re-read the
    /// aggregated view on each event rather than caching it themselves.
    pub fn subscribe(&self) -> broadcast::Receiver<CacheEvent> {
        self.watcher.subscribe()
    }

    /// Subscribe to the service's notices (`review-progress-changed`,
    /// `pull-request-provider-changed`). Each transport drains this through
    /// `notice_envelope`: the desktop shell to every window, the web server to
    /// every tab's event stream.
    pub fn subscribe_notices(&self) -> broadcast::Receiver<ServiceNotice> {
        self.notices.subscribe()
    }

    /// Raise a notice on the service's broadcast, so every window and every
    /// served tab of this service hears it, whichever transport caused it. The
    /// service's own setters raise them; a command never emits one directly
    /// on its own transport (design D9). With nobody subscribed it goes
    /// nowhere, which is not an error.
    pub fn notify(&self, notice: ServiceNotice) {
        let _ = self.notices.send(notice);
    }

    /// Start watching every registered workspace and seed the aggregated view.
    /// Must run inside a tokio runtime (`add_workspace`/`sync_repos` spawn
    /// tasks). Idempotent enough to call once at startup.
    pub async fn populate(&self) {
        let folders = self
            .registry
            .lock()
            .map(|r| r.folders())
            .unwrap_or_default();
        for folder in folders {
            if folder.uri.is_dir() {
                if let Err(e) = self.watcher.add_workspace(folder).await {
                    eprintln!("failed to start watcher: {e}");
                }
            }
        }
        self.watcher.sync_repos();
        // Off the async runtime — `aggregate_and_emit` shells out to `git
        // status`/`git branch` per worktree via the full recompute, and a
        // tokio worker must not block on that subprocess I/O. `populate` is
        // itself invoked via `block_on` at startup (see `crates/specforge/
        // src/lib.rs`), which still provides a blocking pool to spawn onto.
        let watcher_for_blocking = self.watcher.clone();
        tokio::task::spawn_blocking(move || watcher_for_blocking.aggregate_and_emit())
            .await
            .unwrap();

        // Warm the lifecycle cache in the background, off the critical path:
        // otherwise the first Dashboard open pays the full uncached mining
        // pass. Strictly best-effort and NOT awaited — the aggregated view is
        // already fresh (just recomputed above), so this reads it once and
        // mines whatever isn't already cached. On the blocking pool, matching
        // every other git-touching call in this file, since mining shells out
        // to `git log` per repository. If the warm hasn't finished by the
        // time a real fetch needs a given repo, `LifecycleCache::get_or_compute`'s
        // single-flight makes the concurrent cold fetch safe rather than
        // duplicative (design.md, Decision 5).
        let watcher_for_warm = self.watcher.clone();
        let cache_for_warm = self.lifecycle_cache.clone();
        tokio::task::spawn_blocking(move || {
            for view in watcher_for_warm.workspace_views() {
                if let WorkspaceView::Repo(r) = view {
                    let repo_id = RepoId(r.repo_id.clone());
                    cache_for_warm.get_or_compute(&repo_id, change_lifecycle_checked);
                }
            }
        });
    }

    /// Seed the activity log from git history on first launch (when the log is
    /// empty), once per distinct repository. Bounded git scans, so it runs on a
    /// background thread; when done it nudges each repo's graph so an open
    /// Dashboard refetches the now-seeded log.
    ///
    /// The `GraphChanged` nudge only fires when the backfill actually
    /// *recorded* at least one achievement — not merely when it attempted to.
    /// The log being empty is necessary but not sufficient: a registry with
    /// no repos, or repos with no recoverable lifecycle/task-history data,
    /// still "runs" a backfill pass that records nothing. On any launch with
    /// nothing new to announce — including every launch after the first —
    /// unconditionally emitting `GraphChanged` for every repo would
    /// invalidate the lifecycle-cache warm that `populate` just started
    /// (design.md, Decision 5) for no reason, defeating it every time.
    pub fn spawn_backfill(&self) {
        let registry = self.registry.clone();
        let activity = self.activity.clone();
        let watcher = self.watcher.clone();
        let cache = self.lifecycle_cache.clone();
        std::thread::spawn(move || {
            if !backfill_activity(&registry, &activity, &cache) {
                return;
            }
            let repos = registry.lock().map(|r| r.repos()).unwrap_or_default();
            for repo_id in repos {
                watcher.emit(CacheEvent::GraphChanged {
                    repo_id: repo_id.into_path_buf(),
                });
            }
        });
    }

    /// The latest Claude usage-quota snapshot. `Disabled` until the poller has
    /// run with the opt-in feature enabled. A cheap mutex read — safe to call
    /// from a render path.
    pub fn claude_quota(&self) -> ClaudeQuotaState {
        self.quota.get()
    }

    /// Start the opt-in Claude usage-quota poll loop on a background thread
    /// (like [`AppService::spawn_backfill`]). While the feature is disabled the
    /// loop only re-checks the flag and never touches the network; when enabled
    /// it polls on the configured interval and emits `CacheEvent::QuotaUpdated`
    /// on each change. Call once at startup.
    pub fn spawn_quota_poller(&self) {
        crate::quota::spawn_poller(
            self.settings.clone(),
            self.watcher.clone(),
            self.quota.clone(),
        );
    }

    /// The latest ChatGPT usage-quota snapshot. `Disabled` until the poller
    /// has run with the opt-in feature enabled. A cheap mutex read — safe to
    /// call from a render path.
    pub fn chatgpt_quota(&self) -> ChatGptQuotaState {
        self.chatgpt_quota.get()
    }

    /// Start the opt-in ChatGPT usage-quota poll loop on a background thread
    /// (like [`AppService::spawn_quota_poller`]). While the feature is
    /// disabled the loop only re-checks the flag and never touches the
    /// network; when enabled it polls on the configured interval and emits
    /// `CacheEvent::QuotaUpdated` on each change. Call once at startup.
    pub fn spawn_chatgpt_quota_poller(&self) {
        crate::chatgpt_quota::spawn_poller(
            self.settings.clone(),
            self.watcher.clone(),
            self.chatgpt_quota.clone(),
        );
    }

    /// The latest BitBucket pull-request snapshot. `Disabled` until the poller
    /// has run with the opt-in feature enabled. A cheap mutex read — safe to
    /// call from a render path.
    pub fn bitbucket_pull_requests(&self) -> BitbucketPullRequestsState {
        self.bitbucket.get()
    }

    /// The latest GitHub pull-request snapshot. `Disabled` until the poller has
    /// run with the opt-in feature enabled. A cheap mutex read.
    pub fn github_pull_requests(&self) -> GithubPullRequestsState {
        self.github.get()
    }

    /// Which tracked worktrees each open pull request comes from, and the
    /// reverse — the `get_pull_request_links` snapshot
    /// (`pull-request-worktree-links`: *The Pull-Request Links Snapshot*).
    ///
    /// A local join, computed on read: both providers' current rows against
    /// the warm repositories of [`Self::workspace_views`] — disabled rows are
    /// already filtered out, so a disabled repository never reaches the
    /// remotes reader — and their remembered remotes. No network request and
    /// no credential. With no rows at all (both features off, or nothing open)
    /// it returns at once, so the remotes are never read on that path.
    ///
    /// Async because a remotes memo miss spawns `git remote -v` (one per warm
    /// repository, a `wsl.exe` launch for a WSL-hosted one): that runs on the
    /// blocking pool, as every other git-spawning read does, never on the
    /// desktop's main thread or inline on a tokio worker.
    pub async fn pull_request_links(&self) -> crate::pull_request_links::PullRequestLinks {
        let bitbucket = self.bitbucket.get();
        let github = self.github.get();
        if bitbucket.pull_requests.is_empty()
            && github.authored.is_empty()
            && github.review_requested.is_empty()
        {
            return crate::pull_request_links::PullRequestLinks::default();
        }
        let views = self.workspace_views();
        let watcher = self.watcher.clone();
        tokio::task::spawn_blocking(move || {
            join_pull_request_links(&watcher, &bitbucket, &github, &views)
        })
        .await
        .unwrap_or_default()
    }

    /// Start the opt-in BitBucket pull-request poll loop on a background thread
    /// (like [`AppService::spawn_quota_poller`]). While the feature is disabled
    /// the loop only re-checks the flag and never reads a credential or touches
    /// the network; when enabled it refreshes on the configured interval and
    /// emits `CacheEvent::BitbucketPullRequestsUpdated` when the snapshot changes. Call
    /// once at startup — from the desktop shell and the standalone web server,
    /// never from the terminal frontend, which renders no pull-request list.
    pub fn spawn_bitbucket_poller(&self) {
        crate::bitbucket::spawn_poller(
            self.settings.clone(),
            self.watcher.clone(),
            self.bitbucket.clone(),
            self.bitbucket_limits.clone(),
        );
    }

    /// Start the opt-in GitHub pull-request poll loop on a background thread,
    /// the twin of [`AppService::spawn_bitbucket_poller`], emitting
    /// `CacheEvent::GithubPullRequestsUpdated` when the snapshot changes. Call
    /// once at startup — from the desktop shell and the standalone web server,
    /// never from the terminal frontend.
    pub fn spawn_github_poller(&self) {
        crate::github::spawn_poller(
            self.settings.clone(),
            self.watcher.clone(),
            self.github.clone(),
            self.github_limits.clone(),
        );
    }

    /// Authorize a pull-request URL for opening — the service half of the
    /// desktop's `open_pull_request` command. Returns the URL only when it is
    /// exactly the web URL of a row in the *current* BitBucket snapshot or in
    /// either list of the *current* GitHub snapshot
    /// (`github-pull-requests-panel` design D10); any other value is refused, so the frontend gains no general open-URL
    /// capability through it. The caller does the opening. The web transport
    /// exposes no such command at all (`web-ui`: *Link Handling in the Browser
    /// Skin*): opening a URL there would act on the serving host.
    pub fn open_pull_request(&self, url: &str) -> Result<String, String> {
        let listed = !url.is_empty()
            && (self
                .bitbucket
                .get()
                .pull_requests
                .iter()
                .any(|pr| pr.url == url)
                || self.github.get().rows().any(|pr| pr.url == url));
        if listed {
            Ok(url.to_string())
        } else {
            Err("not a pull request in the current list".to_string())
        }
    }

    /// The detail of one pull request — the service half of
    /// `get_pull_request_detail` on both transports (`pull-request-viewer`:
    /// *Detail Reads Are Scoped to the Snapshot*). `manual` says only whether
    /// the ask is a manual refresh, and `cached_only` asks only for what the
    /// cache holds; neither reaches a request.
    ///
    /// A disabled provider refuses without content, a cache-only call
    /// included. The reference is looked up in its provider's current
    /// snapshot, and a read goes only through the matched row's own values,
    /// so nothing the caller supplies beyond the reference reaches a request.
    /// A cache-only call answers the cached detail and its read time, marked
    /// no longer listed when the reference is not listed, or not cached. A
    /// reference not listed answers its cached detail, marked no longer
    /// listed, or not listed, and sends nothing. Otherwise the cache's
    /// freshness rule and the provider's gate apply, and the read runs on the
    /// blocking pool, never on the desktop's main thread: at most one per
    /// pull request, every ask meanwhile receiving its outcome.
    pub async fn pull_request_detail(
        &self,
        reference: PullRequestReference,
        manual: bool,
        cached_only: bool,
    ) -> PullRequestDetailOutcome {
        self.pull_request_detail_with(reference, manual, cached_only, Arc::new(LiveIo))
            .await
    }

    /// [`Self::pull_request_detail`] over `io`: the clock, the credential's
    /// sources and the transport, which tests script.
    pub(crate) async fn pull_request_detail_with(
        &self,
        reference: PullRequestReference,
        manual: bool,
        cached_only: bool,
        io: Arc<dyn DetailIo>,
    ) -> PullRequestDetailOutcome {
        let provider = reference.provider;
        if !provider_enabled(&self.settings, provider) {
            return PullRequestDetailOutcome::Refused;
        }
        let key = reference.key();
        let details = &self.pull_request_details;
        let row = match listed_row(&reference, &self.bitbucket.get(), &self.github.get()) {
            Some(row) if !cached_only => row,
            listed => {
                return match details.cached(&key, listed.is_none()) {
                    Some(detail) => PullRequestDetailOutcome::Detail {
                        detail: Box::new(detail),
                    },
                    None if cached_only => PullRequestDetailOutcome::NotCached,
                    None => PullRequestDetailOutcome::NotListed,
                };
            }
        };
        if let Some(detail) = details.fresh(&key, &row, manual, io.now()) {
            return PullRequestDetailOutcome::Detail {
                detail: Box::new(detail),
            };
        }
        let context = self.read_context();
        details
            .read_once(key, move || {
                read_pull_request(&context, provider, row, manual, &*io)
            })
            .await
    }

    /// One file of a pull request's cached detail, as a withheld file's "Load
    /// diff" asks for it — the service half of `get_pull_request_file`
    /// (`pull-request-viewer`: *Detail Reads Are Scoped to the Snapshot*).
    /// `head` and `base` are the commits the view rendered.
    ///
    /// Refused while the provider is disabled, and `Changed` with nothing
    /// cached, against another commit, or for a path not among the detail's
    /// files. A file the budgets withheld, and a file a file read has already
    /// read, come from the cache with no request. A file GitHub sent without
    /// its patch is read by a file read on the blocking pool, only while the
    /// pull request is in GitHub's current snapshot and spelt as its row
    /// spells it, as a detail read is.
    pub async fn pull_request_file(
        &self,
        reference: &PullRequestReference,
        path: &str,
        head: &str,
        base: &str,
    ) -> PullRequestFileOutcome {
        self.pull_request_file_with(reference, path, head, base, Arc::new(LiveIo))
            .await
    }

    /// [`Self::pull_request_file`] over `io`: the clock, the credential's
    /// sources and the transport, which tests script.
    pub(crate) async fn pull_request_file_with(
        &self,
        reference: &PullRequestReference,
        path: &str,
        head: &str,
        base: &str,
        io: Arc<dyn DetailIo>,
    ) -> PullRequestFileOutcome {
        let provider = reference.provider;
        if !provider_enabled(&self.settings, provider) {
            return file_failure(ReadEnd::Abandoned, false);
        }
        let fetch = match self
            .pull_request_details
            .file(&reference.key(), path, head, base)
        {
            Err(FileChanged) => return PullRequestFileOutcome::Changed,
            Ok(FileAnswer::Ready(file)) => return PullRequestFileOutcome::File { file },
            Ok(FileAnswer::Fetch(fetch)) => fetch,
        };
        // Only GitHub sends a file without its patch, and a file read goes
        // only to a pull request its snapshot lists now, as its row spells it.
        let listed = listed_row(reference, &self.bitbucket.get(), &self.github.get())
            .filter(|_| provider == PullRequestProvider::Github)
            .and_then(|row| PullRequestReference::of_row(provider, &row));
        let Some(listed) = listed else {
            return file_failure(ReadEnd::Unavailable, true);
        };
        let context = self.read_context();
        tokio::task::spawn_blocking(move || read_pull_request_file(&context, &listed, fetch, &*io))
            .await
            .unwrap_or_else(|_| file_failure(ReadEnd::Transient, true))
    }

    /// An image file's two versions — the service half of
    /// `get_pull_request_file_image` (`pull-request-viewer`: *Pull-Request
    /// Image Reads*). `head` and `base` are the commits the view rendered.
    ///
    /// Refused while the provider is disabled, and `Changed` with nothing
    /// cached, against another commit, or for a path not among the detail's
    /// files. Otherwise an image read on the blocking pool, only while the
    /// pull request is in its provider's current snapshot and spelt as its
    /// row spells it, as a detail read is. No version's bytes are kept: each
    /// ask reads its versions again.
    pub async fn pull_request_file_image(
        &self,
        reference: &PullRequestReference,
        path: &str,
        head: &str,
        base: &str,
    ) -> PullRequestImageOutcome {
        self.pull_request_file_image_with(reference, path, head, base, Arc::new(LiveIo))
            .await
    }

    /// [`Self::pull_request_file_image`] over `io`: the clock, the
    /// credential's sources and the transport, which tests script.
    pub(crate) async fn pull_request_file_image_with(
        &self,
        reference: &PullRequestReference,
        path: &str,
        head: &str,
        base: &str,
        io: Arc<dyn DetailIo>,
    ) -> PullRequestImageOutcome {
        let provider = reference.provider;
        if !provider_enabled(&self.settings, provider) {
            return image_failure(ReadEnd::Abandoned, false);
        }
        let Ok(fetch) = self
            .pull_request_details
            .image_fetch(&reference.key(), path, head, base)
        else {
            return PullRequestImageOutcome::Changed;
        };
        // An image read goes only to a pull request its snapshot lists now,
        // as its row spells it.
        let listed = listed_row(reference, &self.bitbucket.get(), &self.github.get())
            .and_then(|row| PullRequestReference::of_row(provider, &row));
        let Some(listed) = listed else {
            return image_failure(ReadEnd::Unavailable, true);
        };
        let context = self.read_context();
        tokio::task::spawn_blocking(move || read_pull_request_image(&context, &listed, fetch, &*io))
            .await
            .unwrap_or_else(|_| image_failure(ReadEnd::Transient, true))
    }

    /// Marks one file of a pull request viewed, or unmarks it — the service
    /// half of `set_file_viewed` on both transports (`pull-request-viewer`:
    /// *Review Progress*; design D9). `head` and `base` are the commits the
    /// view rendered.
    ///
    /// Refused, storing nothing, while the provider is disabled, with nothing
    /// cached, against another commit, as a push or a retarget read since
    /// makes it, and for a path not among the cached detail's files. The key
    /// is computed here from the cached detail, never taken from the caller.
    /// A mark creates the pull request's entry and advances its
    /// `lastMarkedHead`; an unmark never creates one. Each stored mark or
    /// unmark raises `review-progress-changed`, carrying the reference as the
    /// detail spells it, on the notice broadcast, so every window and every
    /// served tab re-reads, whichever transport set it.
    pub fn set_file_viewed(
        &self,
        reference: &PullRequestReference,
        path: &str,
        viewed: bool,
        head: &str,
        base: &str,
    ) -> Result<(), String> {
        self.write_review_marks(reference, path, head, |detail, files| {
            review_progress::file_write(detail, files, path, viewed, head, base)
        })
    }

    /// Marks one hunk of a file of a pull request viewed, or unmarks it — the
    /// service half of `set_hunk_viewed` on both transports
    /// (`pull-request-viewer`: *Review Progress*; `review-hunks-viewed` design
    /// D5). `hunk` counts the file's hunks from zero, in the order the view
    /// renders them, and `head` and `base` are the commits the view rendered.
    ///
    /// Refused, storing nothing, in every case [`Self::set_file_viewed`] is,
    /// and while the file's hunks are not known (a file GitHub sent without
    /// its patch, before a file read of it has been kept) or when `hunk` is
    /// past its last one. The hunk's key and its file's are computed here
    /// from the cached detail. Every stored write raises
    /// `review-progress-changed`, as a file's does.
    pub fn set_hunk_viewed(
        &self,
        reference: &PullRequestReference,
        path: &str,
        hunk: usize,
        viewed: bool,
        head: &str,
        base: &str,
    ) -> Result<(), String> {
        self.write_review_marks(reference, path, head, |detail, files| {
            review_progress::hunk_write(detail, files, path, hunk, viewed, head, base)
        })
    }

    /// What both marking commands share: refused while the provider is
    /// disabled or with nothing cached; `write_of` checks the request against
    /// the cached detail and computes the write's keys; a stored write raises
    /// `review-progress-changed`, carrying the reference as the detail spells
    /// it, on the notice broadcast.
    fn write_review_marks(
        &self,
        reference: &PullRequestReference,
        path: &str,
        head: &str,
        write_of: impl FnOnce(&PullRequestDetail, &[CachedFile]) -> Result<MarkWrite, String>,
    ) -> Result<(), String> {
        if !provider_enabled(&self.settings, reference.provider) {
            return Err("the pull request's provider is disabled".to_string());
        }
        let key = reference.key();
        let (spelt, write) = self
            .pull_request_details
            .with_entry(&key, |detail, files| {
                write_of(detail, files).map(|write| (detail.reference.clone(), write))
            })
            .ok_or_else(|| "no detail of this pull request is cached".to_string())??;
        let stored = self
            .review_store
            .write_marks(&key, path, &write, head, now_unix())
            .map_err(|error| format!("the review progress could not be saved: {error}"))?;
        if stored {
            self.notify(ServiceNotice::ReviewProgressChanged(spelt));
        }
        Ok(())
    }

    /// The review progress of one pull request — the service half of
    /// `get_review_progress` on both transports (`pull-request-viewer`:
    /// *Review Progress*): each file of its cached detail with its state, and
    /// the counts, against the store as it reads now. Never the whole store.
    /// Refused while the provider is disabled, as `get_pull_request_detail`
    /// is, and with nothing cached, since the states are the cached files'.
    pub fn review_progress(
        &self,
        reference: &PullRequestReference,
    ) -> Result<ReviewProgress, String> {
        if !provider_enabled(&self.settings, reference.provider) {
            return Err("the pull request's provider is disabled".to_string());
        }
        let key = reference.key();
        let entry = self
            .review_store
            .entry(&key)
            .map_err(|error| format!("the review progress could not be read: {error}"))?;
        self.pull_request_details
            .with_entry(&key, |detail, files| {
                review_progress::progress(detail, files, entry.as_ref())
            })
            .ok_or_else(|| "no detail of this pull request is cached".to_string())
    }

    /// Authorizes a link from a pull request for the platform opener — the
    /// service half of the desktop-only `open_pull_request_link`
    /// (`pull-request-viewer`: *Desktop Link Opener*; design D10). Returns
    /// the href only while the provider is enabled and the reference has a
    /// cached detail, and only when it is an absolute `http` or `https` URL
    /// with a host. It checks the href's form and the cache, not the pull
    /// request's content, and never fetches the href; the caller does the
    /// opening. The web transport exposes no such command, so it opens
    /// nothing on a serving host.
    pub fn open_pull_request_link(
        &self,
        reference: &PullRequestReference,
        href: &str,
    ) -> Result<String, String> {
        if !provider_enabled(&self.settings, reference.provider) {
            return Err("the pull request's provider is disabled".to_string());
        }
        if self
            .pull_request_details
            .with_entry(&reference.key(), |_, _| ())
            .is_none()
        {
            return Err("no detail of this pull request is cached".to_string());
        }
        if !openable_link(href) {
            return Err("only an absolute http or https link with a host opens".to_string());
        }
        Ok(href.to_string())
    }

    /// What a detail read needs of the service.
    fn read_context(&self) -> ReadContext {
        ReadContext {
            settings: self.settings.clone(),
            github_limits: self.github_limits.clone(),
            bitbucket_limits: self.bitbucket_limits.clone(),
            details: self.pull_request_details.clone(),
        }
    }

    /// Sets GitHub's enabled flag — the one path every transport's toggle
    /// goes through (`pull-request-viewer`: *Provider Enabled Flags Stay
    /// Current*, *Shared Backoff and Detail Budget*; `github-pull-requests`:
    /// *GitHub Polling With Caching and Backoff*).
    ///
    /// Disabling drops GitHub's cached details at once and advances its
    /// credential generation, so a read in flight sends nothing more and is
    /// neither cached nor returned. Enabling while the GraphQL deadline holds
    /// publishes and announces an `unavailable` snapshot before returning, so
    /// the panel and any pull-request address say GitHub is unavailable rather
    /// than loading; the first refresh then waits the deadline out. Neither
    /// touches the deadlines or the spent budget. Every write raises
    /// `pull-request-provider-changed` on the notice broadcast.
    ///
    /// The flag changes in memory even when it cannot be saved, so all of
    /// this follows it whatever the save's result, which is returned.
    pub fn set_github_enabled(&self, enabled: bool) -> std::io::Result<()> {
        let saved = self.settings.set_github_enabled(enabled);
        if enabled {
            crate::github::publish_held_unavailable(
                &self.github,
                &self.watcher,
                &self.github_limits,
                now_unix(),
            );
        } else {
            self.pull_request_details
                .forget(PullRequestProvider::Github);
        }
        self.provider_flag_set(PullRequestProvider::Github, enabled);
        saved
    }

    /// Sets BitBucket's enabled flag, the twin of
    /// [`Self::set_github_enabled`] for BitBucket's one deadline
    /// (`bitbucket-pull-requests`: *Polling With Caching and Backoff*).
    pub fn set_bitbucket_enabled(&self, enabled: bool) -> std::io::Result<()> {
        let saved = self.settings.set_bitbucket_enabled(enabled);
        if enabled {
            crate::bitbucket::publish_held_unavailable(
                &self.bitbucket,
                &self.watcher,
                &self.bitbucket_limits,
                now_unix(),
            );
        } else {
            self.pull_request_details
                .forget(PullRequestProvider::Bitbucket);
        }
        self.provider_flag_set(PullRequestProvider::Bitbucket, enabled);
        saved
    }

    /// Saves GitHub's token, dropping GitHub's cached details at once and
    /// advancing its credential generation, so a read in flight under the
    /// old token sends nothing more and lands nowhere. The deadlines and the
    /// spent budget stay: they are the provider's (design D8).
    pub fn set_github_token(&self, token: String) -> std::io::Result<()> {
        let saved = self.settings.set_github_token(token);
        self.pull_request_details
            .forget(PullRequestProvider::Github);
        saved
    }

    /// Saves BitBucket's credential pair, the twin of
    /// [`Self::set_github_token`].
    pub fn set_bitbucket_credentials(
        &self,
        username: String,
        api_token: String,
    ) -> std::io::Result<()> {
        let saved = self.settings.set_bitbucket_credentials(username, api_token);
        self.pull_request_details
            .forget(PullRequestProvider::Bitbucket);
        saved
    }

    /// Announces a provider's enabled flag as just set, to every window and
    /// every served tab.
    fn provider_flag_set(&self, provider: PullRequestProvider, enabled: bool) {
        self.notify(ServiceNotice::PullRequestProviderChanged(
            PullRequestProviderChangedPayload { provider, enabled },
        ));
    }

    /// Active (non-archived) logical change count across every tracked entry.
    pub fn active_count(&self) -> usize {
        self.watcher.total_active_logical_count()
    }

    /// One workspace's active changes (from the cache).
    pub fn changes_for(&self, workspace: &Path) -> Vec<ChangeData> {
        self.watcher.changes_for(workspace)
    }

    /// The repo/instance-aware top-level view, with presentation overrides
    /// (display name + tint) joined in so labels match across surfaces.
    ///
    /// This is the *tree pane's* accessor and it excludes disabled rows. It is
    /// the single implementation of that exclusion and of the presentation
    /// join: the desktop shell, the web server, and the terminal UI all serve
    /// their aggregated view from here rather than each filtering and joining
    /// for itself.
    ///
    /// The Dashboard, commit garden, and author sweep deliberately read
    /// `self.watcher.workspace_views()` directly instead, so a parked workspace
    /// keeps contributing to every historical surface — see the *Dashboard
    /// Unaffected by Workspace Disable* requirement in the `dashboard`
    /// capability.
    pub fn workspace_views(&self) -> Vec<WorkspaceView> {
        let mut views = self.watcher.workspace_views();
        views.retain(|v| !v.is_disabled());
        if let Ok(store) = self.presentation.lock() {
            join_presentation(&mut views, &store);
        }
        views
    }

    /// The user-registered workspaces, with presentation overrides joined in.
    pub fn list_workspaces(&self) -> Result<Vec<RegisteredWorkspace>, String> {
        let reg = self.registry.lock().map_err(|e| e.to_string())?;
        let store = self.presentation.lock().map_err(|e| e.to_string())?;
        let mut items: Vec<RegisteredWorkspace> = reg
            .entries()
            .iter()
            .filter(|e| matches!(e.origin, openspec_core::WorkspaceOrigin::UserRegistered))
            .map(|e| {
                let mut ws = RegisteredWorkspace::from_folder(&e.folder);
                let repo_path = e.repo_id.as_ref().map(|r| r.as_path().to_path_buf());
                let key = match &repo_path {
                    Some(r) => PresentationKey::Repo(r.clone()),
                    None => PresentationKey::Flat(e.folder.uri.clone()),
                };
                let (dn, c, disabled) = store.lookup_row(&key);
                ws.display_name = dn;
                ws.color = c;
                ws.disabled = disabled;
                ws.repo_id = repo_path;
                ws
            })
            .collect();
        items.sort_by(|a, b| a.name.cmp(&b.name).then(a.uri.cmp(&b.uri)));
        Ok(items)
    }

    /// Register a workspace folder and wire it into the live watcher set, then
    /// return the user-registered entry (with its repo association and any
    /// presentation overrides joined in). `register` validates the folder
    /// (exists, is a directory, holds `openspec/` or lies inside a git working
    /// tree — whose root is then what gets registered), promotes it if it was
    /// already discovered, and discovers sibling worktrees of the same git
    /// repo; this method starts a watcher for
    /// each newly-tracked folder, installs per-repo monitors, and refreshes the
    /// aggregated view so a subsequent `workspace_views`/`list_workspaces` (and
    /// the watcher's `CacheEvent` subscribers) reflect the addition immediately.
    ///
    /// This is the single orchestration both frontends call — the Tauri command
    /// and the terminal UI — so watcher lifecycle stays owned by the service.
    pub async fn add_workspace(&self, path: PathBuf) -> Result<RegisteredWorkspace, String> {
        // `primary` is the folder that ended up user-registered — the selected
        // path, or the worktree root a subfolder resolved to — whether newly
        // added or promoted from an already-discovered worktree. `added` is
        // everything newly tracked, which excludes a promoted folder: it is
        // already watched.
        let (primary, added) = {
            let mut reg = self.registry.lock().map_err(|e| e.to_string())?;
            reg.register_resolved(path).map_err(|e| e.to_string())?
        };

        // Start watchers for every newly-tracked workspace (the user-registered
        // one and any discovered siblings).
        for folder in &added {
            if folder.uri.is_dir() {
                if let Err(e) = self.watcher.add_workspace(folder.clone()).await {
                    eprintln!("failed to add watcher for {}: {e}", folder.uri.display());
                }
            }
        }
        // Install (or update) per-repo monitors so future runtime worktree
        // adds/removes for this repo are picked up automatically, then refresh
        // the cached aggregated view — `add_workspace` mutates the cache without
        // emitting a raw `CacheEvent`, so the aggregator would otherwise miss
        // this change until an unrelated filesystem event fired.
        self.watcher.sync_repos();
        // Off the async runtime — see the comment on `populate`'s equivalent
        // call.
        let watcher_for_blocking = self.watcher.clone();
        tokio::task::spawn_blocking(move || watcher_for_blocking.aggregate_and_emit())
            .await
            .unwrap();

        // Build the returned entry the same way `list_workspaces` does: carry
        // the repo_id and join any presentation overrides keyed to this row.
        let reg = self.registry.lock().map_err(|e| e.to_string())?;
        let store = self.presentation.lock().map_err(|e| e.to_string())?;
        let mut ws = RegisteredWorkspace::from_folder(&primary);
        if let Some(entry) = reg.entry(&primary.uri) {
            let repo_path = entry.repo_id.as_ref().map(|r| r.as_path().to_path_buf());
            let key = match &repo_path {
                Some(r) => PresentationKey::Repo(r.clone()),
                None => PresentationKey::Flat(primary.uri.clone()),
            };
            let (dn, c, disabled) = store.lookup_row(&key);
            ws.display_name = dn;
            ws.color = c;
            ws.disabled = disabled;
            ws.repo_id = repo_path;
        }
        Ok(ws)
    }

    /// Unregister a workspace and tear down the watchers it implied, cascading to
    /// the discovered worktrees the registry drops with it and cleaning up any
    /// now-orphaned presentation entries. Returns whether anything was removed.
    pub async fn remove_workspace(&self, path: PathBuf) -> Result<bool, String> {
        // Snapshot the entry's repo association before unregister so we can
        // decide which presentation keys to cascade-clean afterwards.
        //
        // Canonicalise through `openspec_core::canonicalize` (dunce) — the same
        // function the registry keys its entries with — and hand that same value
        // to `unregister` below, so the entry we inspect and the entry the
        // registry drops are provably the same one. std's `canonicalize` is not
        // interchangeable here: on Windows it yields verbatim forms (`\\?\C:\…`,
        // `\\?\UNC\…`) that never match a registry key, so the lookup missed, the
        // whole cascade below was skipped, and a `disabled: true` entry outlived
        // its registration to silently re-park the folder on re-registration.
        // Fall back to the input when canonicalisation fails (e.g. the directory
        // was deleted) — the same fallback the registry uses, so the two agree.
        let canonical = openspec_core::canonicalize(&path).unwrap_or_else(|_| path.clone());
        let (was_user_registered, target_repo_id) = {
            let reg = self.registry.lock().map_err(|e| e.to_string())?;
            match reg.entry(&canonical) {
                Some(e) => (
                    matches!(e.origin, WorkspaceOrigin::UserRegistered),
                    e.repo_id.as_ref().map(|r| r.as_path().to_path_buf()),
                ),
                None => (false, None),
            }
        };

        let removed = {
            let mut reg = self.registry.lock().map_err(|e| e.to_string())?;
            reg.unregister(&canonical).map_err(|e| e.to_string())?
        };
        let any_removed = !removed.is_empty();

        // Tear down watchers for every removed path (the user-registered one
        // plus any cascaded discovered worktrees), drop now-empty repo
        // monitors, and refresh the aggregated view once from the settled state.
        for p in &removed {
            self.watcher.remove_workspace(p);
        }
        self.watcher.sync_repos();

        // Evict this repository's lifecycle-cache entry unconditionally —
        // cheap even when another registered worktree of the same repo
        // keeps its `RepoMonitor` alive, and load-bearing when this was the
        // repo's last registered worktree: `sync_repos` just tore down that
        // monitor, so `GraphChanged` (the cache's only invalidation signal)
        // will never arrive for this repo again. Without this eviction, a
        // commit landing while the repo is unregistered would leave a stale
        // `Slot::Done` in place, and re-registering it would keep serving
        // that pre-removal snapshot until some *unrelated* future commit
        // happened to invalidate it.
        if let Some(repo_id) = &target_repo_id {
            let repo = RepoId(repo_id.clone());
            self.lifecycle_cache.invalidate(&repo);
            self.commit_activity_cache.invalidate(&repo);
            // The remembered remotes, for the same reason: their only
            // invalidation signal is the monitor `sync_repos` just tore down,
            // so a `git remote set-url` while unregistered would otherwise be
            // missed after re-registering (`pull-request-worktree-links`).
            self.watcher.invalidate_remotes(&repo);
        }

        // Off the async runtime — see the comment on `populate`'s equivalent
        // call.
        let watcher_for_blocking = self.watcher.clone();
        tokio::task::spawn_blocking(move || watcher_for_blocking.aggregate_and_emit())
            .await
            .unwrap();

        // Cascade presentation cleanup, mirroring the registry's own cascade: a
        // flat workspace drops its own `Flat` entry; a repo-member workspace
        // drops the shared `Repo` entry only once the repository has no
        // remaining user-registered worktree.
        if was_user_registered {
            let still_has_user_for_repo = match target_repo_id.as_ref() {
                Some(repo_id) => {
                    let reg = self.registry.lock().map_err(|e| e.to_string())?;
                    repo_still_has_user_registered(&reg, repo_id)
                }
                None => false,
            };
            let keys = presentation_keys_to_drop(
                &canonical,
                target_repo_id.as_deref(),
                still_has_user_for_repo,
            );
            if !keys.is_empty() {
                let mut store = self.presentation.lock().map_err(|e| e.to_string())?;
                for key in keys {
                    let _ = store.remove(&key);
                }
            }
        }

        Ok(any_removed)
    }

    /// Persist the display-name and palette-colour overrides for a top-level
    /// row. `repo_id` is `Some` for a workspace inside a git repository (the
    /// override is keyed by the repo group) and `None` for a flat workspace.
    /// An empty display name is normalised to absent; an unrecognised colour is
    /// rejected by the store.
    pub fn set_workspace_presentation(
        &self,
        uri: PathBuf,
        repo_id: Option<PathBuf>,
        display_name: Option<String>,
        color: Option<PaletteColor>,
    ) -> Result<(), String> {
        let key = match repo_id {
            Some(r) => PresentationKey::Repo(r),
            None => PresentationKey::Flat(uri),
        };
        let mut store = self.presentation.lock().map_err(|e| e.to_string())?;
        store
            .set(key, display_name, color)
            .map_err(|e| e.to_string())
    }

    /// Park or un-park a top-level row. `repo_id` selects the key the same way
    /// [`Self::set_workspace_presentation`] does, so sibling worktrees of one
    /// repository share a single state.
    ///
    /// The aggregated snapshot is refreshed *before returning*, so the next
    /// `get_workspace_views` already reflects the new state without waiting for
    /// a filesystem event (the *Re-enable Freshness* requirement). Re-enabling
    /// therefore performs the git work the row skipped while parked, and the
    /// caller gets a fully warm row on its next request.
    pub async fn set_workspace_disabled(
        &self,
        uri: PathBuf,
        repo_id: Option<PathBuf>,
        disabled: bool,
    ) -> Result<(), String> {
        let key = match repo_id.clone() {
            Some(r) => PresentationKey::Repo(r),
            None => PresentationKey::Flat(uri),
        };
        {
            let mut store = self.presentation.lock().map_err(|e| e.to_string())?;
            store
                .set_disabled(key, disabled)
                .map_err(|e| e.to_string())?;
        }
        // Off the async runtime, like every other recompute site: re-enabling a
        // row performs the `git status` / `git branch` sweep it skipped while
        // parked, and a tokio worker must not block on that subprocess I/O.
        //
        // Scoped to the repository for a repo row; a flat row has none to scope
        // to and falls back to the full refresh, matching every other caller.
        let watcher = self.watcher.clone();
        tokio::task::spawn_blocking(move || match repo_id {
            Some(r) => watcher.refresh_status_for(&RepoId(r)),
            None => watcher.refresh_status_and_notify(),
        })
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Authorize a caller-supplied repository identifier against the registry:
    /// accepted only when its canonical path matches the canonical git directory
    /// of a registered workspace. Returns the matching `RepoId` on success; on a
    /// miss (or an unresolvable path) returns an error and nothing is read. Keys
    /// on the same `openspec_core::canonicalize` (dunce) the registry uses, so an
    /// equivalently-spelled path is neither wrongly refused nor able to evade the
    /// guard.
    fn ensure_registered_repo(&self, repo_id: &Path) -> Result<RepoId, String> {
        let canonical = openspec_core::canonicalize(repo_id)
            .map_err(|_| "unregistered repository".to_string())?;
        let repos = {
            let reg = self.registry.lock().map_err(|e| e.to_string())?;
            reg.repos()
        };
        repos
            .into_iter()
            .find(|r| {
                openspec_core::canonicalize(r.as_path())
                    .map(|c| c == canonical)
                    .unwrap_or(false)
            })
            .ok_or_else(|| "unregistered repository".to_string())
    }

    /// Authorize a caller-supplied workspace against the registry: accepted only
    /// when its canonical path matches a registered (or registry-discovered)
    /// workspace folder. Returns the registered folder path (already canonical)
    /// on success; on a miss (or an unresolvable path) returns an error and
    /// nothing is read. Keys on the same canonicalization the registry uses so an
    /// equivalently-spelled path resolves to the same membership decision.
    fn ensure_registered_workspace(&self, workspace: &Path) -> Result<PathBuf, String> {
        let canonical = openspec_core::canonicalize(workspace)
            .map_err(|_| "unregistered workspace".to_string())?;
        let folders: Vec<PathBuf> = {
            let reg = self.registry.lock().map_err(|e| e.to_string())?;
            reg.entries().iter().map(|e| e.folder.uri.clone()).collect()
        };
        folders
            .into_iter()
            .find(|f| {
                openspec_core::canonicalize(f)
                    .map(|c| c == canonical)
                    .unwrap_or(false)
            })
            .ok_or_else(|| "unregistered workspace".to_string())
    }

    /// Authorize a caller-supplied *browse root* for the file browser. Accepted
    /// when the root is itself a registered workspace, or when it lives inside a
    /// registered repository — a Repo group browses its main worktree, which
    /// need not itself be registered (the user may have registered only a
    /// worktree of that repository). Anything else is refused, so naming an
    /// arbitrary path on the host cannot enumerate or read it. Returns the
    /// canonical root, which callers use for resolution so the path that was
    /// authorized is the path that is read.
    /// Whether the root failed the workspace test or the repository one is an
    /// implementation detail of the check, so every refusal reports the same
    /// message: the caller asked to browse a root, and it is not one of theirs.
    fn ensure_browse_root(&self, root: &Path) -> Result<PathBuf, String> {
        const REFUSED: &str = "unregistered workspace";
        if let Ok(folder) = self.ensure_registered_workspace(root) {
            return Ok(folder);
        }
        let repo = git_common_dir(root).ok_or_else(|| REFUSED.to_string())?;
        self.ensure_registered_repo(repo.as_path())
            .map_err(|_| REFUSED.to_string())?;
        openspec_core::canonicalize(root).map_err(|_| REFUSED.to_string())
    }

    /// One workspace's archived changes (newest-first), for the Archive browser.
    pub fn list_archived(&self, workspace: &Path) -> Result<Vec<ArchivedChangeSummary>, String> {
        let workspace = self.ensure_registered_workspace(workspace)?;
        list_archived_summaries(&workspace).map_err(|e| e.to_string())
    }

    /// The Archive browser's listing for one top-level row: the **union** of
    /// the archived changes across every tracked worktree of a repository —
    /// user-registered *and* registry-discovered — de-duplicated on the bare
    /// logical id, one row per logical change carrying the copies it collapsed
    /// (`archive-browser`: *Union Archive Listing Across a Repository's
    /// Worktrees*). A flat workspace is the degenerate one-folder case.
    ///
    /// Discovered worktrees are included deliberately: a change archived inside
    /// a feature worktree lives in exactly that worktree until its branch
    /// merges, and such a worktree is auto-discovered rather than registered,
    /// so a union over user-registered folders alone would omit precisely the
    /// changes this exists to reach.
    ///
    /// Authorization is by top-level row, matching the addressing: a repository
    /// through `ensure_registered_repo` (so an unregistered repository is
    /// refused before any worktree is enumerated), a flat workspace through
    /// `ensure_registered_workspace`.
    ///
    /// Runs off the async runtime like the other filesystem-walking
    /// operations, and only when the caller asks — nothing here is reachable
    /// from the watcher's aggregation path (*On-Demand, Off-Hot-Path
    /// Loading*).
    pub async fn list_archived_rows(
        &self,
        scope: ArchiveScope,
    ) -> Result<Vec<ArchivedChangeRow>, String> {
        let mut worktrees: Vec<PathBuf> = match &scope {
            ArchiveScope::Repo { repo_id } => {
                let repo = self.ensure_registered_repo(repo_id)?;
                let reg = self.registry.lock().map_err(|e| e.to_string())?;
                reg.entries()
                    .iter()
                    .filter(|e| e.repo_id.as_ref() == Some(&repo))
                    .map(|e| e.folder.uri.clone())
                    .collect()
            }
            ArchiveScope::Flat { workspace } => {
                vec![self.ensure_registered_workspace(workspace)?]
            }
        };
        // Registry order is already deterministic (insertion-ordered), but the
        // fan-out is sorted so the set of reads does not depend on the order
        // the user happened to register worktrees in.
        worktrees.sort();

        tokio::task::spawn_blocking(move || -> Result<Vec<ArchivedChangeRow>, String> {
            let mut listings = Vec::with_capacity(worktrees.len());
            for worktree in worktrees {
                // One unreadable worktree degrades to contributing nothing —
                // it never takes the whole repository's union down with it.
                // `list_archived_changes` folds only `NotFound` to an empty
                // listing, so a worktree on an unmounted volume (`ENOENT`'s
                // siblings), an `openspec/changes/archive` the user cannot read
                // (`EACCES`), or a regular file where that directory should be
                // (`ENOTDIR`) would otherwise replace every OTHER worktree's
                // archive with an error banner. The aggregation path already
                // takes this stance for the same reason — `repo_view` calls
                // `list_archived_stubs(...).unwrap_or_default()` so one sick
                // worktree cannot blank the repository's row.
                let summaries = list_archived_summaries(&worktree).unwrap_or_else(|e| {
                    eprintln!("archive listing skipped for {}: {e}", worktree.display());
                    Vec::new()
                });
                listings.push((worktree, summaries));
            }
            Ok(group_archived_rows(listings))
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// Which artifacts an archived change has on disk. `dir_name` is one archive
    /// directory entry (`<YYYY-MM-DD>-<id>`), never a path.
    pub fn archived_artifact_status(
        &self,
        workspace: &Path,
        dir_name: &str,
    ) -> Result<ArtifactStatus, String> {
        // Directory-name sanitization stays in force independently of the
        // registration check (archive-browser spec), so a traversal-shaped name
        // is always rejected as invalid.
        if dir_name.contains('/') || dir_name.contains('\\') || dir_name.contains("..") {
            return Err("invalid archive directory name".into());
        }
        let workspace = self.ensure_registered_workspace(workspace)?;
        let change_dir = workspace
            .join("openspec")
            .join("changes")
            .join("archive")
            .join(dir_name);
        Ok(parse_artifact_status(&change_dir))
    }

    /// Raw markdown for one artifact of a change. `artifact_kind` is one of
    /// `proposal`/`design`/`tasks`/`spec`; `capability` is required for `spec`.
    /// A path-traversal guard rejects anything outside `openspec/changes/`.
    pub async fn read_artifact(
        &self,
        workspace: &Path,
        change_id: &str,
        artifact_kind: &str,
        capability: Option<&str>,
    ) -> Result<ArtifactRead, String> {
        let workspace = self.ensure_registered_workspace(workspace)?;
        let resolved = resolve_artifact_path(&workspace, change_id, artifact_kind, capability)?;
        // Metadata BEFORE the body, deliberately — not after, and not from the
        // open handle once the bytes are in hand. A write landing between the
        // two reads then yields a modification time at or before the bytes we
        // actually return, so the header can report the artifact as *older*
        // than it is but never as fresher. Both orderings are corrected by the
        // next watcher batch; they differ in how they read while uncorrected,
        // and a false "just now" is the one a reader acts on.
        let modified_at = artifact_modified_at(&resolved).await;
        let body = tokio::fs::read_to_string(&resolved)
            .await
            .map_err(|e| e.to_string())?;
        Ok(ArtifactRead { body, modified_at })
    }

    /// The workspace's markdown files: `.gitignore`-aware for a git
    /// repository (reads the index via `markdown_files`, so ignored
    /// directories are never walked) or a bounded filesystem walk for a
    /// non-git root. `root` is a repository's main worktree or a flat
    /// workspace folder — the same value `read_workspace_file` resolves reads
    /// against — and is authorized against the registry first, so an
    /// unregistered path is refused rather than enumerated. Runs off the async
    /// runtime, matching the commit-graph pattern.
    pub async fn list_markdown_files(&self, root: PathBuf) -> Result<Vec<String>, String> {
        let root = self.ensure_browse_root(&root)?;
        tokio::task::spawn_blocking(move || -> Result<Vec<String>, String> {
            if git_common_dir(&root).is_some() {
                markdown_files(&root).ok_or_else(|| {
                    "failed to list files: git is unavailable or the repository could not be read"
                        .to_string()
                })
            } else {
                Ok(walk_markdown_files(&root))
            }
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// The file browser's listing for one top-level row: the **union** of the
    /// markdown enumerations of every tracked worktree of a repository —
    /// user-registered *and* registry-discovered — de-duplicated on the
    /// root-relative path, one row per path carrying the copies it collapsed
    /// (`workspace-file-browser`: *Union Markdown Listing Across a
    /// Repository's Worktrees*). A flat workspace is the degenerate
    /// one-folder case and lists exactly what it does today.
    ///
    /// Discovered worktrees are included deliberately: the document written in
    /// a feature worktree this morning lives in exactly that worktree until its
    /// branch merges, and such a worktree is auto-discovered rather than
    /// registered — so a union over user-registered folders alone would omit
    /// precisely the files this exists to reach.
    ///
    /// Authorization is by top-level row, matching the addressing: a repository
    /// through [`Self::ensure_registered_repo`] (so an unregistered repository
    /// is refused before any worktree is enumerated), a flat workspace through
    /// [`Self::ensure_registered_workspace`]. The per-worktree reads that
    /// follow are already permitted — `ensure_browse_root` accepts any path
    /// inside a registered repository — so this adds a scope check rather than
    /// widening what may be read.
    ///
    /// Runs off the async runtime like the other filesystem-walking
    /// operations, and only when the caller asks — nothing here is reachable
    /// from the watcher's aggregation path.
    pub async fn list_workspace_file_rows(
        &self,
        scope: FileScope,
    ) -> Result<Vec<WorkspaceFileRow>, String> {
        let mut worktrees: Vec<PathBuf> = match &scope {
            FileScope::Repo { repo_id } => {
                let repo = self.ensure_registered_repo(repo_id)?;
                let reg = self.registry.lock().map_err(|e| e.to_string())?;
                reg.entries()
                    .iter()
                    .filter(|e| e.repo_id.as_ref() == Some(&repo))
                    .map(|e| e.folder.uri.clone())
                    .collect()
            }
            FileScope::Flat { workspace } => {
                vec![self.ensure_registered_workspace(workspace)?]
            }
        };
        // Registry order is already deterministic (insertion-ordered), but the
        // fan-out is sorted so the set of reads does not depend on the order
        // the user happened to register worktrees in.
        worktrees.sort();

        tokio::task::spawn_blocking(move || -> Result<Vec<WorkspaceFileRow>, String> {
            let mut listings = Vec::with_capacity(worktrees.len());
            for worktree in worktrees {
                // One unreadable worktree degrades to contributing nothing — it
                // never takes the whole repository's union down with it. The
                // single-root `list_markdown_files` propagates a git failure as
                // an error because there is exactly one root and the caller
                // asked for it; here a worktree on an unmounted volume, or a
                // regular file where the directory should be (`ENOTDIR`), would
                // otherwise replace every OTHER worktree's files with an error
                // banner. `list_archived_rows` takes the same stance for the
                // same reason.
                let paths = if git_common_dir(&worktree).is_some() {
                    markdown_files(&worktree).unwrap_or_else(|| {
                        eprintln!("file listing skipped for {}", worktree.display());
                        Vec::new()
                    })
                } else {
                    walk_markdown_files(&worktree)
                };
                listings.push((worktree, paths));
            }
            let mut rows = group_workspace_file_rows(listings);
            // Divergence is determined AFTER the union is built, over the
            // pooled rows only — it never gates the enumeration, and the tree
            // is derived from the path list alone (*The tree renders before
            // divergence is known*).
            mark_divergent_rows(&mut rows);
            Ok(rows)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// Read one markdown file from a workspace-wide browse root. Two
    /// independent guards apply, mirroring `read_artifact`: the root is first
    /// authorized against the registry (*which* roots may be read at all), then
    /// a path guard bounds *where within* the root a read may reach — reject
    /// absolute paths and `..` components up front, canonicalise the resolved
    /// file and require it to stay under the canonical root (this also rejects a
    /// symlink escaping the workspace), require a case-insensitive `.md`
    /// extension, and cap content at 5 MiB. Unlike `read_artifact` the path
    /// guard is not confined to `openspec/changes/`, which is exactly why the
    /// registry check matters here.
    pub async fn read_workspace_file(
        &self,
        root: PathBuf,
        rel_path: String,
    ) -> Result<String, String> {
        let root = self.ensure_browse_root(&root)?;
        tokio::task::spawn_blocking(move || -> Result<String, String> {
            let resolved = guard_workspace_document(&root, &rel_path)?;
            // The guard deliberately does not require the file to exist — a
            // document watch outlives a deleted file. A *read* does, so the
            // existence check lands here, keeping this call's error surface
            // exactly what it was before the guard was shared.
            let metadata =
                std::fs::metadata(&resolved).map_err(|e| format!("file not found: {e}"))?;
            if metadata.len() > MAX_WORKSPACE_FILE_BYTES {
                return Err("file is too large to preview".to_string());
            }
            std::fs::read_to_string(&resolved).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// Register interest in one markdown document, so a surface displaying it
    /// is notified when it changes on disk. Authorises the browse root against
    /// the registry and applies the shared path guard *before* any watch is
    /// established, so a registration can never reach where a read would be
    /// refused.
    ///
    /// Reference-counted: several surfaces may hold the same document, and the
    /// watch is torn down only when the last one releases it.
    pub async fn watch_document(
        &self,
        owner: &str,
        root: PathBuf,
        rel_path: String,
    ) -> Result<(), String> {
        let root = self.ensure_browse_root(&root)?;
        guard_workspace_document(&root, &rel_path)?;
        self.documents
            .acquire(owner, DocumentKey::new(root, rel_path))
            .map_err(|e| e.to_string())
    }

    /// Release one registration taken by [`Self::watch_document`]. Releasing a
    /// document that is not registered is a no-op, so a surface unmounting
    /// twice cannot tear down a watch another surface still holds.
    ///
    /// The root is resolved the same way a registration resolved it so the two
    /// name the same key, but an *unregistered* root is not refused here: a
    /// workspace can be unregistered while a reader still holds a watch on it,
    /// and refusing the release would strand exactly the watch this call exists
    /// to drop. Falling back to a plain canonicalisation keeps that path
    /// working; nothing is read, so there is nothing to authorise.
    pub async fn unwatch_document(&self, owner: &str, root: PathBuf, rel_path: String) {
        if let Ok(canonical) = self.ensure_browse_root(&root) {
            self.documents
                .release(owner, &DocumentKey::new(canonical, rel_path));
            return;
        }
        // The root no longer resolves — unregistered, and its directory moved
        // or removed. There is now no way to reconstruct the canonical root the
        // key was stored under, so fall back to matching this owner's
        // registration by relative path. Without this the release silently
        // finds nothing and the watch survives until the whole owner goes.
        self.documents.release_by_rel_path(owner, &rel_path);
    }

    /// Drop every document watch `owner` holds, because that frontend has gone
    /// away — a reader window destroyed, or a browser tab whose event stream
    /// dropped. This is what keeps the watch count a function of open
    /// documents even when a frontend never gets to clean up after itself.
    pub fn release_document_owner(&self, owner: &str) {
        self.documents.release_owner(owner);
    }

    /// Classify and resolve one anchor href from rendered artifact markdown —
    /// the validated chokepoint every frontend's "open this link" command
    /// funnels through (see the `open-artifact-links` design). `root` is
    /// authorized by the same browse-root rule `list_markdown_files`/
    /// `read_workspace_file` use (a registered workspace, or a repository
    /// main worktree accepted because a worktree of that repository is
    /// registered) — an unauthorized root is refused before any path is
    /// resolved. `base_path` is the root-relative path of the markdown file
    /// being viewed; a relative file href resolves against its parent
    /// directory. Performs no I/O beyond the canonicalising stat calls needed
    /// to resolve and contain a file target — the caller (the Tauri command)
    /// does the actual opening once it holds a classified result.
    pub fn open_artifact_link(
        &self,
        root: &Path,
        base_path: &str,
        href: &str,
    ) -> Result<LinkResolution, String> {
        let root = self.ensure_browse_root(root)?;
        Ok(resolve_artifact_link(&root, base_path, href))
    }

    /// The developer-identity payload (saved config + detected candidate
    /// identities) for the Settings identity section.
    pub fn identity_info(&self) -> Result<IdentityInfo, String> {
        let config = self.settings.identity();
        let folders: Vec<PathBuf> = {
            let reg = self.registry.lock().map_err(|e| e.to_string())?;
            reg.entries().iter().map(|e| e.folder.uri.clone()).collect()
        };
        let candidates = detect_candidate_identities(&folders);
        Ok(IdentityInfo { config, candidates })
    }

    /// The commit graph for a repository (identified by its git common dir),
    /// laid out into lanes/edges. Empty (not an error) when the repo can't be
    /// read, so the rail degrades to empty.
    pub async fn commit_graph(
        &self,
        repo_id: PathBuf,
        limit: usize,
    ) -> Result<CommitGraph, String> {
        let repo = self.ensure_registered_repo(&repo_id)?;
        tokio::task::spawn_blocking(move || {
            let mut commits = commit_log(&repo, limit.saturating_add(1));
            let truncated = commits.len() > limit;
            commits.truncate(limit);
            layout_commit_graph(commits, truncated)
        })
        .await
        .map_err(|e| e.to_string())
    }

    /// The files a commit changed, in the diff model and under its budgets
    /// (`commit-graph`: *Commit Detail View*): every file with its status,
    /// paths, modes and counts, the eager ones with their hunks, and every
    /// other file with a patch withheld. Read in a fixed number of `git`
    /// processes, whatever the number of files.
    pub async fn commit_detail(
        &self,
        repo_id: PathBuf,
        sha: String,
    ) -> Result<Vec<DiffFile>, String> {
        if !is_object_id(&sha) {
            return Err("invalid commit reference".to_string());
        }
        let repo = self.ensure_registered_repo(&repo_id)?;
        tokio::task::spawn_blocking(move || read_commit_detail(&repo, &sha))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())
    }

    /// One file of a commit, read alone on request, as a withheld file's
    /// "Load diff" asks: its hunks, or too large to preview. `path` is the
    /// file's key path, and a renamed file passes its `old_path` too, so it
    /// loads as one renamed file.
    pub async fn commit_diff(
        &self,
        repo_id: PathBuf,
        sha: String,
        path: String,
        old_path: Option<String>,
    ) -> Result<DiffFile, String> {
        if !is_object_id(&sha) {
            return Err("invalid commit reference".to_string());
        }
        let repo = self.ensure_registered_repo(&repo_id)?;
        tokio::task::spawn_blocking(move || {
            read_commit_file(&repo, &sha, &path, old_path.as_deref())
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
    }

    /// An image file's two versions in a commit (`commit-graph`: *Commit
    /// Detail View*), against the same base as its diff, each decided by its
    /// bytes (`diff-view`: *Image Comparison*). `path` and `old_path` are as
    /// [`Self::commit_diff`] takes them. Read in at most four `git`
    /// processes, whatever the file's size.
    pub async fn commit_file_image(
        &self,
        repo_id: PathBuf,
        sha: String,
        path: String,
        old_path: Option<String>,
    ) -> Result<ImageVersions, String> {
        if !is_object_id(&sha) {
            return Err("invalid commit reference".to_string());
        }
        let repo = self.ensure_registered_repo(&repo_id)?;
        tokio::task::spawn_blocking(move || {
            commit_file_blobs(&repo, &sha, &path, old_path.as_deref()).map(|blobs| ImageVersions {
                old: ImageSide::of_blob(blobs.old),
                new: ImageSide::of_blob(blobs.new),
            })
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
    }

    /// The commit garden: one stylized plant per top-level entry, grown from
    /// today's commits. Unconditional — no setting gates it. Returned in the
    /// order the section renders, per [`sort_plots`].
    ///
    /// **The leading sort key inherits [`GARDEN_COMMIT_LIMIT`]'s truncation.**
    /// `commit_log_authored` takes the newest `GARDEN_COMMIT_LIMIT` commits by
    /// *committer* date across every ref, and `compute_garden` then keeps the
    /// ones whose *author* date is today. A fetch or a force-push landing more
    /// than that many commits with newer committer timestamps can therefore
    /// push work authored today out of the window: the plot's count drops, the
    /// entry is demoted, and at zero it becomes dormant and is omitted. Before
    /// the count became an ordering key this only shortened one plot; it can
    /// now reorder the section. Raising the limit or selecting on author date
    /// would fix it properly — this comment exists so the ordering is not read
    /// as an unqualified promise.
    ///
    /// [`sort_plots`]: openspec_core::sort_plots
    pub async fn commit_garden(&self) -> Result<Vec<WorkspaceGarden>, String> {
        let identity = self.settings.identity();
        let mut views = self.watcher.workspace_views();
        {
            let store = self.presentation.lock().map_err(|e| e.to_string())?;
            for view in &mut views {
                match view {
                    WorkspaceView::Repo(r) => {
                        let (dn, _) = store.lookup(&PresentationKey::Repo(r.repo_id.clone()));
                        r.display_name = dn;
                    }
                    WorkspaceView::Flat {
                        workspace,
                        display_name,
                        ..
                    } => {
                        let (dn, _) = store.lookup(&PresentationKey::Flat(workspace.uri.clone()));
                        *display_name = dn;
                    }
                }
            }
        }

        tokio::task::spawn_blocking(move || {
            let today = local_today();
            let mut plants: Vec<WorkspaceGarden> = views
                .iter()
                .map(|view| match view {
                    WorkspaceView::Repo(r) => {
                        let commits =
                            commit_log_authored(&RepoId(r.repo_id.clone()), GARDEN_COMMIT_LIMIT);
                        let mut plant = compute_garden(commits, today, &identity);
                        plant.label = r.display_name.clone().unwrap_or_else(|| r.name.clone());
                        plant.entry_key = r.repo_id.to_string_lossy().into_owned();
                        // Registry-wide, exactly as the removed breakdown's
                        // count was — not the hero's developer-scoped in-flight
                        // tile (`commit-garden`: *Plot Caption*).
                        plant.active_count = r.active.len();
                        plant
                    }
                    WorkspaceView::Flat {
                        workspace,
                        changes,
                        display_name,
                        ..
                    } => WorkspaceGarden {
                        label: display_name
                            .clone()
                            .unwrap_or_else(|| workspace.name.clone()),
                        entry_key: workspace.uri.to_string_lossy().into_owned(),
                        // A flat workspace is always dormant and so never
                        // rendered, but the count is honest rather than left at
                        // a zero that would later read as data.
                        active_count: changes.len(),
                        dormant: true,
                        commits: Vec::new(),
                        edges: Vec::new(),
                        lane_count: 0,
                    },
                })
                .collect();
            // The garden is the Dashboard's only per-repository list now that
            // the analytics band is gone, so it carries its own order rather
            // than inheriting the registry's. The comparator lives in
            // `openspec-core` rather than inline here: `cargo mutants` replaces
            // whole function bodies, so a closure inside this `async fn` would
            // produce no mutants of its own and the gate would be blind to it
            // (`commit-garden`: *Deterministic Plot Order*).
            sort_plots(&mut plants);
            plants
        })
        .await
        .map_err(|e| e.to_string())
    }

    /// Aggregate the global Dashboard payload: cross-workspace analytics plus
    /// the developer's progress layer. The git reads run off the async runtime.
    pub async fn dashboard(&self) -> Result<DashboardData, String> {
        let identity = self.settings.identity();
        let mut views = self.watcher.workspace_views();
        {
            let store = self.presentation.lock().map_err(|e| e.to_string())?;
            join_presentation(&mut views, &store);
        }

        let heatmap_since = format!("{DASHBOARD_HEATMAP_WINDOW_DAYS} days ago");
        let day_axis = day_axis(DASHBOARD_HEATMAP_WINDOW_DAYS as u32);
        let today = today_str();
        let log = self.activity.clone();
        let cache = self.lifecycle_cache.clone();
        let commit_cache = self.commit_activity_cache.clone();

        tokio::task::spawn_blocking(move || {
            let mut lifecycles: std::collections::HashMap<PathBuf, Vec<ChangeLifecycle>> =
                std::collections::HashMap::new();
            for view in &views {
                if let WorkspaceView::Repo(r) = view {
                    let repo_id = RepoId(r.repo_id.clone());
                    // Routed through the cache: a repository whose history
                    // hasn't moved since the last fetch is not re-mined.
                    // `reconcile_lifecycle` stays idempotent whether `lcs`
                    // came from the cache or a fresh mine, so replaying it
                    // against a cache hit records nothing new — the intended
                    // behaviour.
                    let lcs = cache.get_or_compute(&repo_id, change_lifecycle_checked);
                    log.reconcile_lifecycle(&r.main_worktree, &lcs);
                    lifecycles.insert(r.repo_id.clone(), lcs);
                }
            }

            // ONE `git log` per repository, at the widest window any Dashboard
            // section needs (the 371-day heatmap). Every commit-derived surface
            // — the heatmap, the streak — is a filter over these same rows, so
            // no section spawns a second walk of its own.
            let mut commit_pairs: Vec<(String, Author)> = Vec::new();
            for view in &views {
                if let WorkspaceView::Repo(r) = view {
                    let repo_id = RepoId(r.repo_id.clone());
                    // Routed through the cache: a repository whose history
                    // hasn't moved since the last fetch is not re-walked. The
                    // miner cannot fail (the git helper is empty-on-error), so
                    // the error arm is uninhabited in practice — it exists to
                    // satisfy the shared cache's fallible-miner contract.
                    let pairs = commit_cache.get_or_compute(&repo_id, |r| {
                        Ok::<_, std::convert::Infallible>(commit_activity_with_authors(
                            r,
                            &heatmap_since,
                        ))
                    });
                    commit_pairs.extend(pairs);
                }
            }

            let mut data = compute_dashboard(
                &views,
                &today,
                |repo| lifecycles.get(&repo.0).cloned().unwrap_or_default(),
                |worktree_path: &Path, dated_dir: &str| {
                    parse_proposal_title(
                        &worktree_path
                            .join("openspec")
                            .join("changes")
                            .join("archive")
                            .join(dated_dir)
                            .join("proposal.md"),
                    )
                },
            );

            let all_achievements = log.query_window(DASHBOARD_HEATMAP_WINDOW_DAYS as u32);

            let scoped_achievements: Vec<_> = all_achievements
                .iter()
                .filter(|e| event_is_me(e, &identity))
                .cloned()
                .collect();
            let commit_days: Vec<String> = commit_pairs
                .iter()
                .filter(|(_, a)| is_me(a, &identity))
                .filter(|(iso, _)| iso.len() >= 10)
                .map(|(iso, _)| iso[..10].to_string())
                .collect();

            data.progress = compute_progress(&scoped_achievements, &commit_days, &day_axis, &today);
            // The creator set is deliberately read from the WHOLE log, not the
            // windowed slice above: an active change older than the heatmap
            // window would otherwise vanish from this tile while still counting
            // in the Dashboard's own "N active" footnote.
            data.progress.in_flight =
                scoped_in_flight(&views, &log.me_created_change_ids(&identity));

            data
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// A file's modification time as unix seconds, or `None` when the filesystem
/// does not report one this code can use.
///
/// Every failure folds to `None` rather than to a substitute value: absent
/// metadata, a platform that does not track modification times, and a stamp
/// before the unix epoch all mean "no time to show". Falling back to `0` would
/// turn each of them into a confident claim that the artifact was last written
/// in 1970 — the caller cannot tell a real epoch timestamp from a stand-in, so
/// none is offered.
async fn artifact_modified_at(path: &Path) -> Option<u64> {
    tokio::fs::metadata(path)
        .await
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|since_epoch| since_epoch.as_secs())
}

/// Resolve the on-disk path of one artifact of a change, enforcing the
/// path-traversal guard: the canonicalised file must stay under the workspace's
/// `openspec/changes/` subtree. Synchronous and side-effect-free beyond the
/// canonicalize stat, so it is unit-testable without a runtime — and shared by
/// every frontend's `read_artifact` so the guard has one implementation.
///
/// `artifact_kind` is one of `proposal`/`design`/`tasks`/`spec`; `capability`
/// is required for `spec`.
pub fn resolve_artifact_path(
    workspace: &Path,
    change_id: &str,
    artifact_kind: &str,
    capability: Option<&str>,
) -> Result<PathBuf, String> {
    let changes_root = workspace.join("openspec").join("changes");
    let change_dir = changes_root.join(change_id);

    let file_path = match artifact_kind {
        "proposal" => change_dir.join("proposal.md"),
        "design" => change_dir.join("design.md"),
        "tasks" => change_dir.join("tasks.md"),
        "spec" => {
            let cap = capability
                .ok_or_else(|| "spec artifact requires a `capability` name".to_string())?;
            change_dir.join("specs").join(cap).join("spec.md")
        }
        other => return Err(format!("unknown artifact kind: {other}")),
    };

    let changes_root_canonical = openspec_core::canonicalize(&changes_root)
        .map_err(|e| format!("workspace changes directory missing: {e}"))?;
    let resolved =
        openspec_core::canonicalize(&file_path).map_err(|e| format!("artifact not found: {e}"))?;
    if !resolved.starts_with(&changes_root_canonical) {
        return Err("artifact path escapes workspace".to_string());
    }
    Ok(resolved)
}

/// Case-insensitive document-type allow-list the open operation honours —
/// deliberately narrow so a link can never execute a file (Decision 4 of the
/// `open-artifact-links` design): executables, scripts, and anything
/// unrecognised fall to `LinkResolution::Refused`.
const OPENABLE_LINK_EXTENSIONS: &[&str] = &[
    "html", "htm", "png", "jpg", "jpeg", "gif", "svg", "webp", "avif", "css", "pdf", "txt", "json",
    "csv",
];

/// The outcome of classifying and resolving one anchor href from rendered
/// artifact markdown, once its root has already been authorized (see
/// [`AppService::open_artifact_link`], the only caller). A pure classified
/// result — no I/O beyond the canonicalising stat calls needed to resolve and
/// contain a file target — so it's unit-testable without a GUI and reusable
/// by any frontend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkResolution {
    /// `http(s)`/`mailto:`/`tel:` — open the raw URL via the OS handler.
    External(String),
    /// A validated, canonicalised, allow-listed file inside the authorized
    /// root — open via the OS default handler for its type.
    File(PathBuf),
    /// No defined behaviour in v1: a relative markdown link, a fragment-only
    /// href, or any scheme other than the external four. Not an error — the
    /// frontend renders these with a deliberately inert affordance.
    Inert,
    /// Resolved but refused: a `..`/symlink escape, a target outside the
    /// allow-list, a directory, or a target that doesn't exist. Carries a
    /// short human-readable reason; the user-facing treatment is uniformly
    /// "quiet failure" regardless of which.
    Refused(String),
}

/// The URI scheme prefix of `href` (e.g. `"http"` for `"http://example.com"`),
/// lowercased, or `None` for a scheme-less relative reference. Mirrors RFC
/// 3986's `scheme = ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )` grammar so a
/// relative markdown link — which never starts with `ALPHA ":"` — is never
/// misread as a scheme.
fn href_scheme(href: &str) -> Option<String> {
    let colon = href.find(':')?;
    let prefix = &href[..colon];
    let mut chars = prefix.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return None,
    }
    if chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        Some(prefix.to_ascii_lowercase())
    } else {
        None
    }
}

/// Classify and resolve one anchor href from rendered artifact markdown, once
/// `root` is already authorized by the browse-root rule (see
/// [`AppService::open_artifact_link`]). `base_path` is the root-relative path
/// of the markdown file being viewed; a relative file href resolves against
/// its parent directory. Mirrors [`resolve_artifact_path`]'s shape: pure and
/// synchronous beyond the canonicalising stat calls, so it's unit-testable
/// without a runtime.
///
/// Pipeline (design.md, Decision 3): classify scheme → strip fragment/query →
/// classify relative-markdown/fragment-only as inert → percent-decode once →
/// reject an absolute href and guard `base_path` (no absolute paths, no `..`
/// components) → join against `parent(base_path)` and canonicalise → require
/// containment under the canonical root → require a document-type allow-list
/// match and refuse directories.
fn resolve_artifact_link(root: &Path, base_path: &str, href: &str) -> LinkResolution {
    if let Some(scheme) = href_scheme(href) {
        return match scheme.as_str() {
            "http" | "https" | "mailto" | "tel" => LinkResolution::External(href.to_string()),
            _ => LinkResolution::Inert, // javascript:, file:, data:, ...
        };
    }
    if href.is_empty() || href.starts_with('#') {
        return LinkResolution::Inert;
    }

    // Strip fragment and query before classifying-by-extension or decoding,
    // so `./login.html#hero` and `./notes.md?v=2` are judged by their real
    // target rather than the suffix.
    let without_fragment = href.split('#').next().unwrap_or_default();
    let path_part = without_fragment.split('?').next().unwrap_or_default();

    // Relative markdown is reserved for future in-app navigation — inert in
    // v1. Case-insensitive, mirroring `read_workspace_file`'s
    // `eq_ignore_ascii_case`, so `./NOTES.MD` can't slip through as a "file"
    // and open in a text editor.
    if let Some(ext) = Path::new(path_part).extension().and_then(|e| e.to_str()) {
        if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown") {
            return LinkResolution::Inert;
        }
    }

    // Percent-decode exactly once now that scheme/markdown/fragment
    // classification is settled, so `./my%20file.html` resolves to the real
    // file on disk.
    let decoded = percent_encoding::percent_decode_str(path_part).decode_utf8_lossy();

    if Path::new(decoded.as_ref()).is_absolute() {
        return LinkResolution::Refused("absolute links are not allowed".to_string());
    }
    let base = Path::new(base_path);
    if base.is_absolute() {
        return LinkResolution::Refused("the viewed file's path must be relative".to_string());
    }
    if base.components().any(|c| matches!(c, Component::ParentDir)) {
        return LinkResolution::Refused("the viewed file's path must not contain `..`".to_string());
    }
    let base_dir = base.parent().unwrap_or_else(|| Path::new(""));

    // Canonicalising *before* the containment check is what closes both the
    // symlink escape (a symlink inside root pointing outside resolves to its
    // real path) and encoded traversal (`..%2f` decodes and joins like any
    // other `..`, then fails `starts_with` like any other escape).
    let root_canonical = match openspec_core::canonicalize(root) {
        Ok(p) => p,
        Err(_) => return LinkResolution::Refused("workspace root not found".to_string()),
    };
    let candidate = root.join(base_dir).join(decoded.as_ref());
    let resolved = match openspec_core::canonicalize(&candidate) {
        Ok(p) => p,
        Err(_) => return LinkResolution::Refused("target not found".to_string()),
    };
    if !resolved.starts_with(&root_canonical) {
        return LinkResolution::Refused("target escapes the workspace".to_string());
    }
    if resolved.is_dir() {
        return LinkResolution::Refused("directories cannot be opened".to_string());
    }
    let allowed = resolved
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| {
            OPENABLE_LINK_EXTENSIONS
                .iter()
                .any(|allow| e.eq_ignore_ascii_case(allow))
        });
    if !allowed {
        return LinkResolution::Refused("not an openable document type".to_string());
    }
    LinkResolution::File(resolved)
}

/// Join presentation overrides (display name + tint) into the top-level views.
fn join_presentation(views: &mut [WorkspaceView], store: &WorkspacePresentationStore) {
    for view in views.iter_mut() {
        match view {
            WorkspaceView::Repo(r) => {
                let (dn, c) = store.lookup(&PresentationKey::Repo(r.repo_id.clone()));
                r.display_name = dn;
                r.color = c;
            }
            WorkspaceView::Flat {
                workspace,
                display_name,
                color,
                ..
            } => {
                let (dn, c) = store.lookup(&PresentationKey::Flat(workspace.uri.clone()));
                *display_name = dn;
                *color = c;
            }
        }
    }
}

/// The presentation key of the top-level row a workspace belongs to: its
/// repository group when it is inside one, otherwise its own flat key. This is
/// the same selection `list_workspaces` and `set_workspace_presentation` make,
/// factored out so the notification dispatcher resolves rows identically —
/// a change in one worktree of a parked repository must be as silent as one in
/// any other.
///
/// Falls back to the flat key for a path the registry does not know, which is
/// the conservative answer: an unknown row is not parked.
pub fn row_key_for_workspace(
    registry: &Mutex<WorkspaceRegistry>,
    workspace: &Path,
) -> PresentationKey {
    let repo_id = registry
        .lock()
        .ok()
        .and_then(|reg| reg.entry(workspace).and_then(|e| e.repo_id.clone()));
    match repo_id {
        Some(r) => PresentationKey::Repo(r.as_path().to_path_buf()),
        None => PresentationKey::Flat(workspace.to_path_buf()),
    }
}

/// Pure decision function: given the unregistered workspace's canonical path
/// and its repo association (if any), plus whether the repository still has any
/// other user-registered workspace, return the presentation keys to drop.
///
/// Flat workspaces always drop their own `Flat` key. Repo-member workspaces drop
/// the shared `Repo` key only when their cascade fired — i.e. the repository no
/// longer has any user-registered worktree.
fn presentation_keys_to_drop(
    canonical: &Path,
    target_repo_id: Option<&Path>,
    repo_still_has_user_registered: bool,
) -> Vec<PresentationKey> {
    match target_repo_id {
        None => vec![PresentationKey::Flat(canonical.to_path_buf())],
        Some(repo_id) if !repo_still_has_user_registered => {
            vec![PresentationKey::Repo(repo_id.to_path_buf())]
        }
        Some(_) => Vec::new(),
    }
}

fn repo_still_has_user_registered(registry: &WorkspaceRegistry, repo_id: &Path) -> bool {
    registry.entries().iter().any(|e| {
        matches!(e.origin, WorkspaceOrigin::UserRegistered)
            && e.repo_id.as_ref().map(|r| r.as_path()) == Some(repo_id)
    })
}

/// Count active (non-archived) changes the developer created, for the *Me*
/// scope's in-flight tile.
fn scoped_in_flight(
    views: &[WorkspaceView],
    me_created: &std::collections::HashSet<String>,
) -> u32 {
    use std::collections::HashSet;
    let mut active: HashSet<&str> = HashSet::new();
    for view in views {
        match view {
            WorkspaceView::Repo(r) => {
                for lc in &r.active {
                    active.insert(lc.name.as_str());
                }
            }
            WorkspaceView::Flat { changes, .. } => {
                for c in changes {
                    active.insert(c.change_id.as_str());
                }
            }
        }
    }
    active.iter().filter(|id| me_created.contains(**id)).count() as u32
}

/// Seed the activity log from git history on first launch (when the log is
/// empty). Once per distinct repository in the registry. Routes lifecycle
/// mining through `cache` so first launch does not mine every repository
/// twice — once here, once for the first Dashboard fetch. Returns whether
/// anything was actually recorded (not merely whether a backfill pass was
/// attempted — `log.record_all` is itself a no-op for an empty batch, e.g. an
/// empty registry or repos with no recoverable lifecycle/task-history data),
/// so [`AppService::spawn_backfill`] knows whether its post-backfill
/// `GraphChanged` nudge has anything to announce.
fn backfill_activity(
    registry: &Arc<Mutex<WorkspaceRegistry>>,
    log: &Arc<ActivityLog>,
    cache: &LifecycleCache,
) -> bool {
    if !log.is_empty() {
        return false;
    }
    let repo_ids = match registry.lock() {
        Ok(reg) => reg.repos(),
        Err(_) => return false,
    };
    let mut recorded = false;
    for repo_id in repo_ids {
        let main_wt = worktree_list(&repo_id)
            .into_iter()
            .find(|wt| wt.is_main)
            .map(|wt| wt.path)
            .or_else(|| repo_id.as_path().parent().map(Path::to_path_buf))
            .unwrap_or_else(|| repo_id.as_path().to_path_buf());
        let lifecycles = cache.get_or_compute(&repo_id, change_lifecycle_checked);
        let task_history = task_completion_history(&repo_id, BACKFILL_SINCE);
        let events = build_backfill(&main_wt, &lifecycles, &task_history);
        recorded |= !events.is_empty();
        log.record_all(events);
    }
    recorded
}

/// The join behind [`AppService::pull_request_links`], on owned inputs so it
/// can run on the blocking pool. Rows are passed in the order a worktree
/// lists them — BitBucket, then GitHub authored, then GitHub review-requested
/// — each tagged with its provider and role.
fn join_pull_request_links(
    watcher: &WatcherManager,
    bitbucket: &BitbucketPullRequestsState,
    github: &GithubPullRequestsState,
    views: &[WorkspaceView],
) -> crate::pull_request_links::PullRequestLinks {
    use crate::events::PullRequestProvider;
    use crate::pull_request_links::{
        link_pull_requests, PullRequestInput, PullRequestRole, RepoInput,
    };

    fn listed(
        provider: PullRequestProvider,
        role: PullRequestRole,
        rows: &[crate::pull_requests::PullRequestSummary],
    ) -> Vec<PullRequestInput<'_>> {
        rows.iter()
            .map(|row| PullRequestInput {
                provider,
                role,
                row,
            })
            .collect()
    }
    let mut inputs = listed(
        PullRequestProvider::Bitbucket,
        PullRequestRole::Authored,
        &bitbucket.pull_requests,
    );
    inputs.extend(listed(
        PullRequestProvider::Github,
        PullRequestRole::Authored,
        &github.authored,
    ));
    inputs.extend(listed(
        PullRequestProvider::Github,
        PullRequestRole::ReviewRequested,
        &github.review_requested,
    ));

    let repos: Vec<(
        &openspec_core::repo_view::RepoView,
        Vec<openspec_core::Remote>,
    )> = views
        .iter()
        .filter_map(|view| match view {
            WorkspaceView::Repo(repo) => Some(repo),
            WorkspaceView::Flat { .. } => None,
        })
        .map(|repo| (repo, watcher.remotes(&RepoId(repo.repo_id.clone()))))
        .collect();
    let repo_inputs: Vec<RepoInput<'_>> = repos
        .iter()
        .map(|(repo, remotes)| RepoInput {
            repo_id: &repo.repo_id,
            main_worktree: &repo.main_worktree,
            worktrees: &repo.worktree_refs,
            remotes,
        })
        .collect();
    link_pull_requests(&inputs, &repo_inputs)
}

/// The blocking read behind [`AppService::commit_detail`]: the commit's
/// parents, then its file list, the line rule over it, and one streamed patch
/// read for the eager files it has to reach. Each file is assembled from its
/// list record — status, paths, modes and counts — and the content the patch
/// read returned.
fn read_commit_detail(repo: &RepoId, sha: &str) -> Result<Vec<DiffFile>, CommitReadError> {
    let base = commit_base(repo, sha)?;
    let mut files = commit_file_list(repo, &base)?;
    // A binary file has no counts, and no patch to show.
    let mut to_read = eager_by_lines(files.iter().map(|file| {
        file.additions
            .zip(file.deletions)
            .map(|(added, removed)| added.saturating_add(removed))
    }));
    // The line rule counts a file with no hunks to show as nothing, so it is
    // eager and never withheld; its content is known from the list alone, and
    // the patch read stops after the last eager file that has lines.
    for (file, read) in files.iter_mut().zip(&mut to_read) {
        if has_no_hunks(file) {
            file.content = DiffContent::Hunks { hunks: Vec::new() };
            *read = false;
        }
    }
    let contents = commit_patch(repo, &base, &files, &to_read)?;
    Ok(files
        .into_iter()
        .zip(contents)
        .map(|(file, content)| DiffFile { content, ..file })
        .collect())
}

/// Whether the file list alone shows that `file` has no hunks to show: it
/// changed no lines, as a mode-only change, a pure rename, or an empty file
/// added or deleted does. Never a type change, which git writes as a deletion
/// and a creation: a regular file replaced by a symlink to its own text
/// changes no lines by its counts, yet both of its sections have them.
fn has_no_hunks(file: &DiffFile) -> bool {
    file.additions == Some(0) && file.deletions == Some(0) && file.status != FileStatus::TypeChanged
}

/// The blocking read behind [`AppService::commit_diff`]: the commit's parents,
/// then the one-file read against the same base as the rest of its diff. An
/// empty path is refused before git is asked, since as a literal pathspec it
/// would match every file the commit changed.
fn read_commit_file(
    repo: &RepoId,
    sha: &str,
    path: &str,
    old_path: Option<&str>,
) -> Result<DiffFile, CommitReadError> {
    if path.is_empty() || old_path == Some("") {
        return Err(CommitReadError::NoSuchFile);
    }
    let base = commit_base(repo, sha)?;
    commit_file_diff(repo, &base, path, old_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pull_requests::PullRequestsStatus;
    use std::time::Duration;

    /// The in-flight tile counts the developer's own active changes — not every
    /// active change, and not zero.
    #[test]
    fn scoped_in_flight_counts_only_active_changes_the_developer_created() {
        use openspec_core::{ArtifactStatus, ChangeData, WorkspaceFolder};
        use std::collections::HashSet;

        let folder = WorkspaceFolder {
            uri: PathBuf::from("/ws"),
            name: "ws".to_string(),
        };
        let change = |id: &str| ChangeData {
            change_id: id.to_string(),
            title: None,
            sections: Vec::new(),
            total_tasks: 0,
            completed_tasks: 0,
            artifacts: ArtifactStatus::default(),
            workspace: folder.clone(),
        };
        let views = vec![WorkspaceView::Flat {
            workspace: folder.clone(),
            changes: vec![change("mine-a"), change("mine-b"), change("theirs")],
            display_name: None,
            color: None,
            has_open_spec: true,
            disabled: false,
        }];

        // `archived-already` is mine but no longer active, so it must not count:
        // the tile is the intersection of "active now" and "created by me".
        let me: HashSet<String> = ["mine-a", "mine-b", "archived-already"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            scoped_in_flight(&views, &me),
            2,
            "only the two of mine that are still active count"
        );
        assert_eq!(
            scoped_in_flight(&views, &HashSet::new()),
            0,
            "nothing of mine is active yet"
        );
    }

    #[tokio::test]
    async fn commit_detail_and_diff_refuse_non_object_id_ref() {
        // The AppService boundary rejects a non-hex ref before any git call, on
        // both transports — a regression guard for the `is_object_id` check that
        // task 3.2 exercised only at the predicate level.
        let dir = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(dir.path().to_path_buf());
        let repo = PathBuf::from("/nonexistent/repo/.git");

        for bad in ["HEAD", "--output=x", ":/msg", ""] {
            let e = svc
                .commit_detail(repo.clone(), bad.to_string())
                .await
                .unwrap_err();
            assert_eq!(e, "invalid commit reference", "commit_detail({bad:?})");
            let e = svc
                .commit_diff(repo.clone(), bad.to_string(), "f".to_string(), None)
                .await
                .unwrap_err();
            assert_eq!(e, "invalid commit reference", "commit_diff({bad:?})");
            let e = svc
                .commit_file_image(repo.clone(), bad.to_string(), "f.png".to_string(), None)
                .await
                .unwrap_err();
            assert_eq!(e, "invalid commit reference", "commit_file_image({bad:?})");
        }
    }

    #[test]
    fn flat_workspace_drops_its_flat_key() {
        let keys = presentation_keys_to_drop(Path::new("/ws/flat"), None, false);
        assert_eq!(keys, vec![PresentationKey::Flat("/ws/flat".into())]);
    }

    #[test]
    fn repo_member_unregister_with_other_user_registrations_drops_nothing() {
        let keys =
            presentation_keys_to_drop(Path::new("/r/main"), Some(Path::new("/r/.git")), true);
        assert!(
            keys.is_empty(),
            "repo presentation must survive when another user-registered workspace remains"
        );
    }

    #[test]
    fn last_repo_member_unregister_drops_the_repo_key() {
        let keys =
            presentation_keys_to_drop(Path::new("/r/main"), Some(Path::new("/r/.git")), false);
        assert_eq!(keys, vec![PresentationKey::Repo("/r/.git".into())]);
    }

    #[test]
    fn resolve_artifact_path_accepts_in_tree_proposal() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let change_dir = ws.join("openspec").join("changes").join("add-x");
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(change_dir.join("proposal.md"), "# X").unwrap();

        let resolved = resolve_artifact_path(ws, "add-x", "proposal", None).unwrap();
        let changes_root =
            openspec_core::canonicalize(&ws.join("openspec").join("changes")).unwrap();
        assert!(resolved.starts_with(&changes_root));
        assert!(resolved.ends_with("proposal.md"));
    }

    #[test]
    fn resolve_artifact_path_rejects_escape_outside_changes() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let change_dir = ws.join("openspec").join("changes").join("add-x");
        std::fs::create_dir_all(change_dir.join("specs")).unwrap();
        // A real `spec.md` *outside* openspec/changes/ that a crafted capability
        // would reach if the guard were absent.
        std::fs::create_dir_all(ws.join("secret")).unwrap();
        std::fs::write(ws.join("secret").join("spec.md"), "top secret").unwrap();

        // capability climbs out of changes/add-x/specs/ back to ws/secret/.
        let err =
            resolve_artifact_path(ws, "add-x", "spec", Some("../../../../secret")).unwrap_err();
        assert!(
            err.contains("escapes"),
            "expected escape rejection, got: {err}"
        );
    }

    #[test]
    fn resolve_artifact_path_unknown_kind_errs() {
        let dir = tempfile::tempdir().unwrap();
        let err = resolve_artifact_path(dir.path(), "add-x", "bogus", None).unwrap_err();
        assert!(err.contains("unknown artifact kind"));
    }

    #[test]
    fn resolve_artifact_path_spec_requires_capability() {
        let dir = tempfile::tempdir().unwrap();
        let err = resolve_artifact_path(dir.path(), "add-x", "spec", None).unwrap_err();
        assert!(err.contains("capability"));
    }

    #[test]
    fn preserve_corrupt_config_moves_file_aside_without_losing_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspaces.json");
        std::fs::write(&path, "{ corrupt").unwrap();

        preserve_corrupt_config(&path);

        // The corrupt file is moved aside (not deleted), so its data stays
        // recoverable and a later save cannot overwrite the original.
        assert!(
            !path.exists(),
            "corrupt file should be moved out of the way"
        );
        let backup = dir.path().join("workspaces.json.corrupt-0");
        assert!(backup.exists(), "a recoverable backup copy must remain");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), "{ corrupt");
    }

    #[test]
    fn preserve_corrupt_config_is_a_noop_when_file_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspaces.json");
        preserve_corrupt_config(&path); // must not panic or create anything
        assert!(!path.exists());
        assert!(!dir.path().join("workspaces.json.corrupt-0").exists());
    }

    // --- Registry-membership authorization (authorize-command-paths) ---------

    fn git(args: &[&str], cwd: &Path) {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("git invocation");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A git repo carrying an `openspec/changes/` tree and one commit; returns
    /// its canonical root.
    fn init_openspec_repo(root: &Path) -> PathBuf {
        std::fs::create_dir_all(root.join("openspec").join("changes")).unwrap();
        git(&["init", "-b", "main"], root);
        git(&["config", "user.email", "t@t"], root);
        git(&["config", "user.name", "t"], root);
        git(&["commit", "--allow-empty", "-m", "init"], root);
        openspec_core::canonicalize(root).unwrap()
    }

    fn register(svc: &AppService, path: &Path) {
        svc.registry
            .lock()
            .unwrap()
            .register(path.to_path_buf())
            .unwrap();
    }

    /// A 40-hex object id that satisfies `is_object_id`, so commit_detail/diff
    /// reach the registration guard instead of short-circuiting on ref shape.
    const OBJ: &str = "0123456789abcdef0123456789abcdef01234567";

    #[tokio::test]
    async fn commit_reads_require_a_registered_repository() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let registered_root = init_openspec_repo(&roots.path().join("registered"));
        let outsider_root = init_openspec_repo(&roots.path().join("outsider"));
        register(&svc, &registered_root);

        let registered_repo = svc.registry.lock().unwrap().repos()[0]
            .as_path()
            .to_path_buf();
        // A real, readable `.git` that is simply not in the registry.
        let outsider_repo = outsider_root.join(".git");

        assert_eq!(
            svc.commit_graph(outsider_repo.clone(), 10)
                .await
                .unwrap_err(),
            "unregistered repository"
        );
        assert_eq!(
            svc.commit_detail(outsider_repo.clone(), OBJ.to_string())
                .await
                .unwrap_err(),
            "unregistered repository"
        );
        assert_eq!(
            svc.commit_diff(
                outsider_repo.clone(),
                OBJ.to_string(),
                "f".to_string(),
                None
            )
            .await
            .unwrap_err(),
            "unregistered repository"
        );
        // An image read is refused before any `git` process runs.
        {
            use openspec_core::git::invocation_log;
            invocation_log::enable();
            let mark = invocation_log::mark();
            assert_eq!(
                svc.commit_file_image(outsider_repo, OBJ.to_string(), "f.png".to_string(), None)
                    .await
                    .unwrap_err(),
                "unregistered repository"
            );
            assert!(invocation_log::recorded_since(mark)
                .iter()
                .all(|invocation| !invocation.anchor.starts_with(&outsider_root)));
        }

        // The registered repository passes the guard: the graph reads normally,
        // and detail/diff are never refused as unregistered.
        assert!(svc.commit_graph(registered_repo.clone(), 10).await.is_ok());
        assert_ne!(
            svc.commit_detail(registered_repo.clone(), OBJ.to_string())
                .await
                .err()
                .as_deref(),
            Some("unregistered repository")
        );
        assert_ne!(
            svc.commit_diff(registered_repo, OBJ.to_string(), "f".to_string(), None)
                .await
                .err()
                .as_deref(),
            Some("unregistered repository")
        );
    }

    /// `commit-graph`: *An image file reads its versions near the view*, and
    /// `diff-view`: *Bytes decide, not the name*.
    #[tokio::test]
    async fn a_commit_image_read_answers_each_side_by_its_bytes() {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine as _;

        const PNG: &[u8] =
            include_bytes!("../../openspec-core/tests/fixtures/images/three-by-two.png");
        const GIF: &[u8] =
            include_bytes!("../../openspec-core/tests/fixtures/images/three-by-two-87a.gif");
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let root = init_openspec_repo(&roots.path().join("app"));
        register(&svc, &root);
        std::fs::write(root.join("app.png"), PNG).unwrap();
        std::fs::write(root.join("notes.png"), "<html>not found</html>\n").unwrap();
        git(&["add", "-A"], &root);
        git(&["commit", "-m", "add"], &root);
        // A GIF under a `.png` name renders as the GIF it is.
        std::fs::write(root.join("app.png"), GIF).unwrap();
        git(&["commit", "-am", "regenerate"], &root);
        let (repo, ids) = repo_and_commits(&svc, 2);

        let image = |mime: &str, bytes: &[u8]| ImageSide::Image {
            mime: mime.to_string(),
            width: 3,
            height: 2,
            data: STANDARD.encode(bytes),
        };
        assert_eq!(
            svc.commit_file_image(repo.clone(), ids[0].clone(), "app.png".to_string(), None)
                .await,
            Ok(ImageVersions {
                old: image("image/png", PNG),
                new: image("image/gif", GIF),
            })
        );
        assert_eq!(
            svc.commit_file_image(repo, ids[1].clone(), "notes.png".to_string(), None)
                .await,
            Ok(ImageVersions {
                old: ImageSide::Absent,
                new: ImageSide::Refused {
                    reason: openspec_core::ImageRefusal::NotImage
                },
            })
        );
    }

    /// The one registered repository, as the frontend names it, and its
    /// newest `count` commits' ids, newest first.
    fn repo_and_commits(svc: &AppService, count: usize) -> (PathBuf, Vec<String>) {
        let repo = svc.registry.lock().unwrap().repos()[0].clone();
        let ids = commit_log(&repo, count).into_iter().map(|c| c.id).collect();
        (repo.into_path_buf(), ids)
    }

    /// `commit-graph`: *A commit is read in a fixed number of git processes*.
    #[tokio::test]
    async fn a_commit_is_read_in_a_fixed_number_of_git_processes() {
        use openspec_core::git::invocation_log;

        invocation_log::enable();
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let root = init_openspec_repo(&roots.path().join("app"));
        register(&svc, &root);
        std::fs::write(root.join("one.txt"), "one\n").unwrap();
        git(&["add", "-A"], &root);
        git(&["commit", "-m", "one file"], &root);
        for n in 0..200 {
            std::fs::write(root.join(format!("f{n:03}.txt")), format!("{n}\n")).unwrap();
        }
        git(&["add", "-A"], &root);
        git(&["commit", "-m", "two hundred files"], &root);
        let (repo, ids) = repo_and_commits(&svc, 2);

        let mut processes = Vec::new();
        for sha in [&ids[1], &ids[0]] {
            let mark = invocation_log::mark();
            let files = svc.commit_detail(repo.clone(), sha.clone()).await.unwrap();
            assert!(
                files
                    .iter()
                    .all(|file| matches!(file.content, DiffContent::Hunks { .. })),
                "every file arrives with its hunks: {files:?}"
            );
            processes.push((
                files.len(),
                invocation_log::recorded_since(mark)
                    .iter()
                    .filter(|invocation| invocation.anchor.starts_with(&root))
                    .count(),
            ));
        }
        // The parents, the file list and the streamed patch, and none per file.
        assert_eq!(processes, [(1, 3), (200, 3)]);
    }

    /// `commit-graph`: *A commit is read in a fixed number of git processes*,
    /// where the budgets withhold the files after the last eager one, and
    /// *A withheld file loads on request*.
    #[tokio::test]
    async fn a_commit_read_stops_after_its_last_eager_file_and_loads_the_rest_on_request() {
        use openspec_core::git::invocation_log;

        invocation_log::enable();
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let root = init_openspec_repo(&roots.path().join("app"));
        register(&svc, &root);
        std::fs::write(root.join("a.txt"), "small\n").unwrap();
        // Past the line rule's 500, and more patch text than a pipe holds, so
        // `git` is still writing it when the read stops after `a.txt`.
        let long: String = (0..600).map(|n| format!("{n:0199}\n")).collect();
        std::fs::write(root.join("b.txt"), &long).unwrap();
        git(&["add", "-A"], &root);
        git(&["commit", "-m", "small and long"], &root);
        let (repo, ids) = repo_and_commits(&svc, 1);
        let sha = ids[0].clone();

        let files = svc.commit_detail(repo.clone(), sha.clone()).await.unwrap();
        let [small, long] = &files[..] else {
            panic!("two files: {files:?}");
        };
        assert_eq!(small.new_path.as_deref(), Some("a.txt"));
        assert!(
            matches!(small.content, DiffContent::Hunks { .. }),
            "{small:?}"
        );
        // Withheld, and carrying its counts.
        assert_eq!(
            (
                long.new_path.as_deref(),
                long.additions,
                long.deletions,
                &long.content
            ),
            (Some("b.txt"), Some(600), Some(0), &DiffContent::Withheld)
        );

        let loaded = svc
            .commit_diff(repo.clone(), sha.clone(), "b.txt".to_string(), None)
            .await
            .unwrap();
        let DiffContent::Hunks { hunks } = &loaded.content else {
            panic!("a loaded file has its hunks: {loaded:?}");
        };
        assert_eq!(hunks[0].lines.len(), 600);

        // An empty path would match every file as a literal pathspec, so it
        // is refused before any git process runs.
        let mark = invocation_log::mark();
        for (path, old_path) in [("", None), ("b.txt", Some(String::new()))] {
            assert_eq!(
                svc.commit_diff(repo.clone(), sha.clone(), path.to_string(), old_path)
                    .await
                    .unwrap_err(),
                "no such file in commit"
            );
        }
        assert!(invocation_log::recorded_since(mark)
            .iter()
            .all(|invocation| !invocation.anchor.starts_with(&root)));
    }

    /// A file with no changed lines has no hunks to show, so its content is
    /// known from the file list and the patch read never has to reach it: it
    /// stops after the last eager file that has lines, here before a withheld
    /// file with more patch text than a pipe holds and a mode-only change
    /// after it, last in git's order.
    #[tokio::test]
    async fn a_file_with_no_changed_lines_is_known_without_reading_its_patch() {
        use openspec_core::git::invocation_log;

        invocation_log::enable();
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let root = init_openspec_repo(&roots.path().join("app"));
        register(&svc, &root);
        std::fs::write(root.join("z.sh"), "echo\n").unwrap();
        git(&["add", "-A"], &root);
        git(&["commit", "-m", "add z.sh"], &root);
        let long: String = (0..600).map(|n| format!("{n:0199}\n")).collect();
        std::fs::write(root.join("a.txt"), "small\n").unwrap();
        std::fs::write(root.join("b.txt"), &long).unwrap();
        git(&["add", "-A"], &root);
        git(&["update-index", "--chmod=+x", "z.sh"], &root);
        git(&["commit", "-m", "small, long and executable"], &root);
        // Then no eager file has lines: another long file, and z.sh's mode
        // changed back.
        std::fs::write(root.join("c.txt"), &long).unwrap();
        git(&["add", "-A"], &root);
        git(&["update-index", "--chmod=-x", "z.sh"], &root);
        git(&["commit", "-m", "long and no longer executable"], &root);
        let (repo, ids) = repo_and_commits(&svc, 2);
        let processes = |mark: usize| {
            invocation_log::recorded_since(mark)
                .iter()
                .filter(|invocation| invocation.anchor.starts_with(&root))
                .count()
        };
        let no_hunks = DiffContent::Hunks { hunks: Vec::new() };
        let mode = |file: &DiffFile| {
            (
                file.status,
                file.old_mode.clone(),
                file.new_mode.clone(),
                file.content.clone(),
            )
        };

        let mark = invocation_log::mark();
        let files = svc
            .commit_detail(repo.clone(), ids[1].clone())
            .await
            .unwrap();
        let [small, long, executable] = &files[..] else {
            panic!("three files: {files:?}");
        };
        assert!(
            matches!(&small.content, DiffContent::Hunks { hunks } if hunks.len() == 1),
            "{small:?}"
        );
        assert_eq!(
            (long.additions, &long.content),
            (Some(600), &DiffContent::Withheld)
        );
        assert_eq!(
            mode(executable),
            (
                FileStatus::ModeChanged,
                Some("100644".to_string()),
                Some("100755".to_string()),
                no_hunks.clone()
            )
        );
        // The parents, the file list, and the patch read stopped after
        // `a.txt`, its last eager file with lines.
        assert_eq!(processes(mark), 3);

        // With no eager file that has lines, no patch read runs at all.
        let mark = invocation_log::mark();
        let files = svc
            .commit_detail(repo.clone(), ids[0].clone())
            .await
            .unwrap();
        let [long, executable] = &files[..] else {
            panic!("two files: {files:?}");
        };
        assert_eq!(long.content, DiffContent::Withheld);
        assert_eq!(
            mode(executable),
            (
                FileStatus::ModeChanged,
                Some("100755".to_string()),
                Some("100644".to_string()),
                no_hunks
            )
        );
        assert_eq!(processes(mark), 2);
    }

    /// A type change whose counts show no changed lines is still read from
    /// the patch: git writes it as a deletion and a creation, and both have
    /// lines even when a regular file becomes a symlink to its own text.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_type_change_without_changed_lines_is_read_from_the_patch() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let root = init_openspec_repo(&roots.path().join("app"));
        register(&svc, &root);
        std::fs::write(root.join("config"), "target").unwrap();
        git(&["add", "-A"], &root);
        git(&["commit", "-m", "add config"], &root);
        std::fs::remove_file(root.join("config")).unwrap();
        std::os::unix::fs::symlink("target", root.join("config")).unwrap();
        git(&["add", "-A"], &root);
        git(&["commit", "-m", "link config to its own text"], &root);
        let (repo, ids) = repo_and_commits(&svc, 1);

        let files = svc.commit_detail(repo, ids[0].clone()).await.unwrap();
        let [config] = &files[..] else {
            panic!("one file: {files:?}");
        };
        assert_eq!(
            (config.status, config.additions, config.deletions),
            (FileStatus::TypeChanged, Some(0), Some(0))
        );
        let DiffContent::Hunks { hunks } = &config.content else {
            panic!("a type change has its hunks: {config:?}");
        };
        assert_eq!(hunks.len(), 2, "the deletion's and the creation's");
    }

    #[tokio::test]
    async fn artifact_and_archive_reads_require_a_registered_workspace() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let registered = roots.path().join("registered");
        let outsider = roots.path().join("outsider");
        // Both hold a real proposal + archive dir on disk, so only registration —
        // not file absence — decides the outcome.
        for ws in [&registered, &outsider] {
            let change_dir = ws.join("openspec").join("changes").join("add-x");
            std::fs::create_dir_all(ws.join("openspec").join("changes").join("archive")).unwrap();
            std::fs::create_dir_all(&change_dir).unwrap();
            std::fs::write(change_dir.join("proposal.md"), "# X").unwrap();
        }
        register(&svc, &registered);

        // Unregistered workspace: every reader refuses and nothing is read.
        assert_eq!(
            svc.read_artifact(&outsider, "add-x", "proposal", None)
                .await
                .unwrap_err(),
            "unregistered workspace"
        );
        assert_eq!(
            svc.list_archived(&outsider).unwrap_err(),
            "unregistered workspace"
        );
        assert_eq!(
            svc.archived_artifact_status(&outsider, "2025-01-01-x")
                .unwrap_err(),
            "unregistered workspace"
        );

        // Registered workspace: the readers behave as before.
        assert_eq!(
            svc.read_artifact(&registered, "add-x", "proposal", None)
                .await
                .unwrap()
                .body,
            "# X"
        );
        assert!(svc.list_archived(&registered).is_ok());
        assert!(svc
            .archived_artifact_status(&registered, "2025-01-01-x")
            .is_ok());

        // Directory-name sanitization holds independently of registration.
        assert_eq!(
            svc.archived_artifact_status(&registered, "../escape")
                .unwrap_err(),
            "invalid archive directory name"
        );

        // A registry-DISCOVERED worktree is authorized exactly as a
        // user-registered folder is. The union listing depends on this, so it
        // is pinned rather than left as an accident a future tightening of the
        // check could remove: narrowing to user-registered folders would
        // silently empty the union of the worktrees it exists to reach.
        let repo = init_openspec_repo(&roots.path().join("repo"));
        let sibling = roots.path().join("repo-feature");
        git(
            &[
                "worktree",
                "add",
                "-b",
                "feature",
                sibling.to_str().unwrap(),
            ],
            &repo,
        );
        let archive = sibling.join("openspec/changes/archive/2026-09-05-add-thing");
        std::fs::create_dir_all(&archive).unwrap();
        std::fs::write(archive.join("proposal.md"), "# Add thing").unwrap();
        let sibling = openspec_core::canonicalize(&sibling).unwrap();
        // Registering the MAIN worktree is what discovers the sibling.
        register(&svc, &repo);

        assert!(
            !svc.list_workspaces()
                .unwrap()
                .iter()
                .any(|w| w.uri == sibling),
            "precondition: the sibling is discovered, not user-registered, so \
             it is absent from the Settings listing"
        );
        assert_eq!(
            svc.list_archived(&sibling)
                .expect("a discovered worktree's archive is readable")
                .iter()
                .map(|s| s.dir_name.as_str())
                .collect::<Vec<_>>(),
            vec!["2026-09-05-add-thing"]
        );
        assert!(
            svc.archived_artifact_status(&sibling, "2026-09-05-add-thing")
                .expect("a discovered worktree's archived change is inspectable")
                .proposal
        );
    }

    /// `archive-browser`: *Union listing for an unregistered repository is
    /// refused*.
    #[tokio::test]
    async fn union_listing_for_an_unregistered_repository_is_refused() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let registered = init_openspec_repo(&roots.path().join("registered"));
        let outsider = init_openspec_repo(&roots.path().join("outsider"));
        // A real, readable archive in the outsider, so only registration —
        // not absence — can decide the outcome.
        let archive = outsider.join("openspec/changes/archive/2026-09-05-secret");
        std::fs::create_dir_all(&archive).unwrap();
        std::fs::write(archive.join("proposal.md"), "# Secret").unwrap();
        register(&svc, &registered);

        assert_eq!(
            svc.list_archived_rows(ArchiveScope::Repo {
                repo_id: outsider.join(".git"),
            })
            .await
            .unwrap_err(),
            "unregistered repository"
        );
        // A flat scope naming an unregistered folder is refused too.
        assert_eq!(
            svc.list_archived_rows(ArchiveScope::Flat {
                workspace: outsider.clone(),
            })
            .await
            .unwrap_err(),
            "unregistered workspace"
        );

        let repo_id = svc.registry.lock().unwrap().repos()[0]
            .as_path()
            .to_path_buf();
        assert!(svc
            .list_archived_rows(ArchiveScope::Repo { repo_id })
            .await
            .is_ok());
    }

    /// `archive-browser`: *Union Archive Listing Across a Repository's
    /// Worktrees* — the pooling itself, over a mix of a user-registered and a
    /// registry-discovered worktree.
    #[tokio::test]
    async fn union_pools_archived_changes_across_a_repositorys_worktrees() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let main = init_openspec_repo(&roots.path().join("main"));
        let feature = roots.path().join("feature");
        git(
            &[
                "worktree",
                "add",
                "-b",
                "feature",
                feature.to_str().unwrap(),
            ],
            &main,
        );
        let feature = openspec_core::canonicalize(&feature).unwrap();

        let write_archive = |root: &Path, dir: &str, title: &str| {
            let d = root.join("openspec/changes/archive").join(dir);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("proposal.md"), format!("# {title}")).unwrap();
        };
        // `shared` exists in both worktrees under DIFFERENT dates; `only-here`
        // exists solely in the discovered worktree — the case the whole change
        // is for.
        write_archive(&main, "2026-06-04-shared", "Shared");
        write_archive(&feature, "2026-06-05-shared", "Shared");
        write_archive(&feature, "2026-06-06-only-here", "Only here");
        register(&svc, &main);

        let repo_id = svc.registry.lock().unwrap().repos()[0]
            .as_path()
            .to_path_buf();
        let rows = svc
            .list_archived_rows(ArchiveScope::Repo { repo_id })
            .await
            .unwrap();

        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["only-here", "shared"],
            "one row per logical change, newest first: {rows:?}"
        );
        // The change that lives only in the auto-discovered worktree is
        // reachable, addressed by that worktree and its own directory name.
        assert_eq!(rows[0].copies.len(), 1);
        assert_eq!(rows[0].copies[0].worktree_path, feature);
        assert_eq!(rows[0].copies[0].archive_dir, "2026-06-06-only-here");
        // The two dates collapse into one row dated by the newer.
        assert_eq!(rows[1].date.as_deref(), Some("2026-06-05"));
        assert_eq!(
            rows[1]
                .copies
                .iter()
                .map(|c| (c.worktree_path.clone(), c.archive_dir.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (feature.clone(), "2026-06-05-shared"),
                (main.clone(), "2026-06-04-shared"),
            ]
        );
    }

    /// One unreadable worktree contributes nothing; it never takes the whole
    /// repository's union down with it. The aggregation path already takes this
    /// stance (`list_archived_stubs(...).unwrap_or_default()`), and the union
    /// pools MANY worktrees where the old per-workspace listing read exactly the
    /// one the user had selected — so propagating the first error would let a
    /// single sick worktree blank every other worktree's archive.
    #[tokio::test]
    async fn one_unreadable_worktree_does_not_blank_the_repositorys_union() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let main = init_openspec_repo(&roots.path().join("main"));
        let broken = roots.path().join("broken");
        git(
            &["worktree", "add", "-b", "broken", broken.to_str().unwrap()],
            &main,
        );
        let broken = openspec_core::canonicalize(&broken).unwrap();

        let d = main.join("openspec/changes/archive/2026-06-04-healthy");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("proposal.md"), "# Healthy").unwrap();

        // A regular FILE where the archive directory belongs: reading it yields
        // `ENOTDIR`, which `list_archived_changes` does not fold to an empty
        // listing the way it folds `NotFound`. Deterministic and portable —
        // unlike a permissions trick, which a privileged test runner ignores.
        let changes = broken.join("openspec/changes");
        std::fs::create_dir_all(&changes).unwrap();
        std::fs::write(changes.join("archive"), "not a directory").unwrap();

        register(&svc, &main);
        let repo_id = svc.registry.lock().unwrap().repos()[0]
            .as_path()
            .to_path_buf();
        let rows = svc
            .list_archived_rows(ArchiveScope::Repo { repo_id })
            .await
            .expect("a sick worktree must not fail the whole union");

        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["healthy"],
            "the healthy worktree's archive still lists: {rows:?}"
        );
    }

    /// `archive-browser`: *On-Demand, Off-Hot-Path Loading*. The union's
    /// distinguishing work is reading each archived change's `proposal.md`
    /// heading. The watcher's aggregation must do none of it — so the same
    /// fixture yields titled rows through the on-demand union and untitled
    /// stubs through the aggregated snapshot.
    #[tokio::test]
    async fn watcher_aggregation_does_not_do_the_unions_work() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let main = init_openspec_repo(&roots.path().join("main"));
        let archived = main.join("openspec/changes/archive/2026-06-04-add-thing");
        std::fs::create_dir_all(archived.join("specs/payments")).unwrap();
        std::fs::write(archived.join("proposal.md"), "# Add thing").unwrap();
        std::fs::write(archived.join("tasks.md"), "- [x] 1.1 done\n").unwrap();
        std::fs::write(archived.join("specs/payments/spec.md"), "## ADDED\n").unwrap();

        svc.add_workspace(main.clone()).await.unwrap();
        svc.watcher.aggregate_and_emit();

        // Aggregation saw the directory (it must, to tell archived from
        // deleted) but parsed nothing inside it.
        let views = svc.watcher.workspace_views();
        let WorkspaceView::Repo(repo) = &views[0] else {
            panic!("expected a repo row")
        };
        let stub = &repo.archived[0].instances[0].change;
        assert_eq!(stub.change_id, "2026-06-04-add-thing");
        assert_eq!(stub.title, None, "no archived proposal.md was read");
        assert_eq!(stub.total_tasks, 0, "no archived tasks.md was read");
        assert!(
            stub.artifacts.specs.is_empty(),
            "no archived specs/ was walked"
        );

        // The on-demand union does read the heading — which is what makes the
        // assertions above evidence of an exclusion rather than of an empty
        // fixture.
        let repo_id = svc.registry.lock().unwrap().repos()[0]
            .as_path()
            .to_path_buf();
        let rows = svc
            .list_archived_rows(ArchiveScope::Repo { repo_id })
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title.as_deref(), Some("Add thing"));
    }

    /// Stamp a file's modification time at an exact unix second.
    ///
    /// The alternative — write, sleep, write again, assert the second is newer
    /// — makes the assertion depend on how fast the machine runs and on the
    /// filesystem's timestamp granularity, and the usual repair is to widen the
    /// sleep. That is the pattern `watcher.rs`'s `recompute_gate` exists to
    /// avoid. Setting the value outright removes the timing question rather
    /// than tuning it, and lets the tests below assert exact equality.
    fn stamp(path: &Path, unix_secs: u64) {
        let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(unix_secs);
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(at)
            .unwrap();
    }

    /// A registered workspace holding one change with a stamped `proposal.md`
    /// and `tasks.md`, returning the service and the workspace root.
    fn workspace_with_stamped_artifacts(
        cfg: &Path,
        root: &Path,
        proposal_at: u64,
        tasks_at: u64,
    ) -> AppService {
        let svc = AppService::bootstrap(cfg.to_path_buf());
        let change_dir = root.join("openspec").join("changes").join("add-x");
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(change_dir.join("proposal.md"), "# X").unwrap();
        std::fs::write(change_dir.join("tasks.md"), "- [ ] t").unwrap();
        stamp(&change_dir.join("proposal.md"), proposal_at);
        stamp(&change_dir.join("tasks.md"), tasks_at);
        register(&svc, root);
        svc
    }

    #[tokio::test]
    async fn read_artifact_reports_the_artifacts_own_modification_time() {
        // The two artifacts are stamped far apart on purpose. `tasks.md` is the
        // newer, so a read that reported the *directory's* newest mtime — which
        // is what `ChangeInstance::modified_at` carries, and what it would have
        // cost nothing to reuse — would answer TASKS_AT for the proposal and
        // fail here. That substitution is the defect this test exists to catch.
        const PROPOSAL_AT: u64 = 1_700_000_000;
        const TASKS_AT: u64 = 1_800_000_000;

        let cfg = tempfile::tempdir().unwrap();
        let roots = tempfile::tempdir().unwrap();
        let ws = roots.path().join("ws");
        let svc = workspace_with_stamped_artifacts(cfg.path(), &ws, PROPOSAL_AT, TASKS_AT);

        let read = svc
            .read_artifact(&ws, "add-x", "proposal", None)
            .await
            .unwrap();
        assert_eq!(read.body, "# X");
        assert_eq!(
            read.modified_at,
            Some(PROPOSAL_AT),
            "the proposal must report its own stamp, in whole seconds"
        );

        // The sibling carries its own. The pair together pins the value as
        // per-artifact rather than per-change directory.
        assert_eq!(
            svc.read_artifact(&ws, "add-x", "tasks", None)
                .await
                .unwrap()
                .modified_at,
            Some(TASKS_AT)
        );
    }

    #[tokio::test]
    async fn rewriting_an_artifact_reports_a_strictly_newer_modification_time() {
        const FIRST_AT: u64 = 1_700_000_000;
        const SECOND_AT: u64 = FIRST_AT + 60;

        let cfg = tempfile::tempdir().unwrap();
        let roots = tempfile::tempdir().unwrap();
        let ws = roots.path().join("ws");
        let svc = workspace_with_stamped_artifacts(cfg.path(), &ws, FIRST_AT, FIRST_AT);
        let proposal = ws
            .join("openspec")
            .join("changes")
            .join("add-x")
            .join("proposal.md");

        let before = svc
            .read_artifact(&ws, "add-x", "proposal", None)
            .await
            .unwrap();
        assert_eq!(before.modified_at, Some(FIRST_AT));

        // Rewritten with IDENTICAL bytes. The body compares equal across the
        // two reads while the time moves — the exact pairing the detail pane's
        // equality guard has to distinguish, so the service must report the two
        // independently rather than folding the time into "did the body change".
        std::fs::write(&proposal, "# X").unwrap();
        stamp(&proposal, SECOND_AT);

        let after = svc
            .read_artifact(&ws, "add-x", "proposal", None)
            .await
            .unwrap();
        assert_eq!(after.body, before.body, "bytes are deliberately unchanged");
        assert_eq!(after.modified_at, Some(SECOND_AT));
        assert!(
            after.modified_at > before.modified_at,
            "a rewrite must report a strictly newer time: {:?} then {:?}",
            before.modified_at,
            after.modified_at
        );
    }

    #[tokio::test]
    async fn artifact_reads_refuse_before_touching_metadata() {
        // The guards run first: an unregistered workspace and a traversal escape
        // are both refused, and neither refusal leaks a modification time for a
        // file the caller was never allowed to reach.
        const AT: u64 = 1_700_000_000;

        let cfg = tempfile::tempdir().unwrap();
        let roots = tempfile::tempdir().unwrap();
        let registered = roots.path().join("registered");
        let outsider = roots.path().join("outsider");
        let svc = workspace_with_stamped_artifacts(cfg.path(), &registered, AT, AT);
        // The outsider holds a real, stamped artifact too, so only registration
        // — not file absence — decides the outcome.
        let outsider_change = outsider.join("openspec").join("changes").join("add-x");
        std::fs::create_dir_all(&outsider_change).unwrap();
        std::fs::write(outsider_change.join("proposal.md"), "# X").unwrap();
        stamp(&outsider_change.join("proposal.md"), AT);

        assert_eq!(
            svc.read_artifact(&outsider, "add-x", "proposal", None)
                .await
                .unwrap_err(),
            "unregistered workspace"
        );

        // A real, stamped `spec.md` outside `openspec/changes/`, reached through
        // a `specs/` directory that also exists — so the guard is what refuses
        // the crafted capability, not a missing path component along the way.
        std::fs::create_dir_all(
            registered
                .join("openspec")
                .join("changes")
                .join("add-x")
                .join("specs"),
        )
        .unwrap();
        std::fs::create_dir_all(registered.join("secret")).unwrap();
        std::fs::write(registered.join("secret").join("spec.md"), "top secret").unwrap();
        stamp(&registered.join("secret").join("spec.md"), AT);

        let escape = svc
            .read_artifact(&registered, "add-x", "spec", Some("../../../../secret"))
            .await
            .unwrap_err();
        assert!(
            escape.contains("escapes"),
            "expected escape rejection, got: {escape}"
        );
    }

    #[tokio::test]
    async fn membership_is_not_spelling_sensitive() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let root = init_openspec_repo(&roots.path().join("repo"));
        let change_dir = root.join("openspec").join("changes").join("add-x");
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(change_dir.join("proposal.md"), "# X").unwrap();
        register(&svc, &root);

        // Repo membership: a `..` round-trip spelling of the registered git dir
        // is recognized as the same repository and read normally.
        let repo = svc.registry.lock().unwrap().repos()[0]
            .as_path()
            .to_path_buf();
        let equivalent_repo = repo.join("objects").join("..");
        assert!(
            svc.commit_graph(equivalent_repo, 10).await.is_ok(),
            "an equivalent spelling of a registered repo must be accepted"
        );

        // Workspace membership: `openspec/..` round-trips back to the registered
        // folder and is accepted.
        let equivalent_ws = root.join("openspec").join("..");
        assert_eq!(
            svc.read_artifact(&equivalent_ws, "add-x", "proposal", None)
                .await
                .unwrap()
                .body,
            "# X"
        );
    }

    #[tokio::test]
    async fn file_browser_reads_require_a_registered_root() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let registered = roots.path().join("registered");
        // A plain (non-git) outsider and a git outsider, so neither backend of
        // the enumeration can be reached by naming an unregistered path.
        let outsider = roots.path().join("outsider");
        let outsider_repo_root = init_openspec_repo(&roots.path().join("outsider-repo"));
        for ws in [&registered, &outsider, &outsider_repo_root] {
            std::fs::create_dir_all(ws.join("openspec").join("changes")).unwrap();
            std::fs::write(ws.join("secret.md"), "# secret").unwrap();
        }
        register(&svc, &registered);

        // Unregistered roots: neither enumeration nor read is served, even
        // though a real `.md` file sits at each one.
        for bad in [&outsider, &outsider_repo_root] {
            assert_eq!(
                svc.list_markdown_files(bad.clone()).await.unwrap_err(),
                "unregistered workspace",
                "listing must refuse {bad:?}"
            );
            assert_eq!(
                svc.read_workspace_file(bad.clone(), "secret.md".to_string())
                    .await
                    .unwrap_err(),
                "unregistered workspace",
                "read must refuse {bad:?}"
            );
        }

        // The registered flat workspace is served normally.
        assert_eq!(
            svc.list_markdown_files(registered.clone()).await.unwrap(),
            vec!["secret.md".to_string()]
        );
        assert_eq!(
            svc.read_workspace_file(registered.clone(), "secret.md".to_string())
                .await
                .unwrap(),
            "# secret"
        );

        // The path guard still bounds reads *within* an authorized root.
        assert_eq!(
            svc.read_workspace_file(registered.clone(), "../outsider/secret.md".to_string())
                .await
                .unwrap_err(),
            "path must not contain `..`"
        );
    }

    #[tokio::test]
    async fn file_browser_accepts_a_repo_root_registered_only_by_worktree() {
        // A Repo group browses its main worktree, which need not itself be
        // registered — registering only a linked worktree must still authorize
        // the repository's main worktree as a browse root.
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let main = init_openspec_repo(&roots.path().join("main"));
        std::fs::write(main.join("notes.md"), "# notes").unwrap();
        let linked = roots.path().join("linked");
        git(
            &["worktree", "add", linked.to_str().unwrap(), "-b", "side"],
            &main,
        );
        std::fs::create_dir_all(linked.join("openspec").join("changes")).unwrap();
        register(&svc, &linked);

        assert_eq!(
            svc.list_markdown_files(main.clone()).await.unwrap(),
            vec!["notes.md".to_string()]
        );
        assert_eq!(
            svc.read_workspace_file(main, "notes.md".to_string())
                .await
                .unwrap(),
            "# notes"
        );
    }

    // --- the repository-scoped union (workspace-file-browser) ------------

    /// The repository's `repo_id` as the registry keys it, for a service with
    /// exactly one registered repository.
    fn sole_repo_id(svc: &AppService) -> PathBuf {
        svc.registry.lock().unwrap().repos()[0]
            .as_path()
            .to_path_buf()
    }

    /// `workspace-file-browser`: *Union Markdown Listing Across a Repository's
    /// Worktrees* — the pooling itself, over a user-registered worktree and a
    /// registry-discovered one, including the divergence marker.
    #[tokio::test]
    async fn union_pools_markdown_across_a_repositorys_worktrees() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let main = init_openspec_repo(&roots.path().join("main"));
        let feature = roots.path().join("feature");
        git(
            &[
                "worktree",
                "add",
                "-b",
                "feature",
                feature.to_str().unwrap(),
            ],
            &main,
        );
        let feature = openspec_core::canonicalize(&feature).unwrap();
        // `openspec/changes` is untracked in the fixture, so `git worktree add`
        // does not carry it across: the sibling holds no `openspec/` at all.
        // It is tracked anyway, because the repository is registered at a
        // worktree root, and its files join the union like any other's.

        // `shared.md` is in both worktrees with DIFFERENT bytes of the same
        // length — the divergence a size-only comparison would miss.
        // `agreed.md` is in both, identical. The `only-*` files are each in one
        // worktree, which is the case the union exists for.
        std::fs::write(main.join("shared.md"), "# cat").unwrap();
        std::fs::write(feature.join("shared.md"), "# bat").unwrap();
        std::fs::write(main.join("agreed.md"), "# same").unwrap();
        std::fs::write(feature.join("agreed.md"), "# same").unwrap();
        std::fs::write(main.join("only-main.md"), "# main").unwrap();
        std::fs::write(feature.join("only-feature.md"), "# feature").unwrap();
        // Registering the MAIN worktree is what discovers the sibling.
        register(&svc, &main);

        let rows = svc
            .list_workspace_file_rows(FileScope::Repo {
                repo_id: sole_repo_id(&svc),
            })
            .await
            .unwrap();

        assert_eq!(
            rows.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(),
            vec!["agreed.md", "only-feature.md", "only-main.md", "shared.md"],
            "one row per root-relative path, path-ascending: {rows:?}"
        );
        // Copies are worktree-path ascending. Both worktrees share a parent
        // directory, so `feature` precedes `main` by construction.
        let both = vec![feature.clone(), main.clone()];
        let copies = |i: usize| {
            rows[i]
                .copies
                .iter()
                .map(|c| c.worktree_path.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(copies(0), both, "agreed.md records both copies");
        assert_eq!(copies(3), both, "shared.md records both copies");
        // The file that lives only in the auto-discovered worktree is
        // reachable, addressed by that worktree plus the row's path.
        assert_eq!(copies(1), vec![feature.clone()]);
        assert_eq!(copies(2), vec![main.clone()]);

        assert_eq!(
            rows.iter()
                .map(|r| (r.path.as_str(), r.differs))
                .collect::<Vec<_>>(),
            vec![
                ("agreed.md", false),
                ("only-feature.md", false),
                ("only-main.md", false),
                ("shared.md", true),
            ],
            "only the row whose copies actually differ is marked"
        );
    }

    /// `workspace-file-browser`: *An unregistered repository is refused* — the
    /// scope check happens before any worktree is touched, so a real, readable
    /// repository full of markdown is enumerated only when it is registered.
    #[tokio::test]
    async fn file_union_listing_for_an_unregistered_repository_is_refused() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let registered = init_openspec_repo(&roots.path().join("registered"));
        let outsider = init_openspec_repo(&roots.path().join("outsider"));
        std::fs::write(registered.join("mine.md"), "# mine").unwrap();
        std::fs::write(outsider.join("secret.md"), "# secret").unwrap();
        register(&svc, &registered);

        assert_eq!(
            svc.list_workspace_file_rows(FileScope::Repo {
                repo_id: outsider.join(".git"),
            })
            .await
            .unwrap_err(),
            "unregistered repository"
        );
        // A flat scope naming an unregistered folder is refused too.
        assert_eq!(
            svc.list_workspace_file_rows(FileScope::Flat {
                workspace: outsider.clone(),
            })
            .await
            .unwrap_err(),
            "unregistered workspace"
        );

        let rows = svc
            .list_workspace_file_rows(FileScope::Repo {
                repo_id: sole_repo_id(&svc),
            })
            .await
            .expect("the registered repository lists normally");
        assert_eq!(
            rows.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(),
            vec!["mine.md"],
            "and the refusal enumerated nothing of the outsider's: {rows:?}"
        );
    }

    /// `workspace-file-browser`: *One unreadable worktree does not blank the
    /// listing*. The single-root listing propagates a git failure as an error
    /// because the caller asked for exactly that root; the union pools MANY
    /// worktrees, so propagating the first failure would let one sick worktree
    /// blank every other worktree's files.
    #[tokio::test]
    async fn one_unreadable_worktree_does_not_blank_the_repositorys_file_union() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let main = init_openspec_repo(&roots.path().join("main"));
        std::fs::write(main.join("healthy.md"), "# healthy").unwrap();
        let broken = roots.path().join("broken");
        git(
            &["worktree", "add", "-b", "broken", broken.to_str().unwrap()],
            &main,
        );
        let broken = openspec_core::canonicalize(&broken).unwrap();
        std::fs::create_dir_all(broken.join("openspec").join("changes")).unwrap();
        register(&svc, &main);

        // A regular FILE where the worktree directory belongs: every path under
        // it yields `ENOTDIR`. Deterministic and portable — unlike a
        // permissions trick, which a privileged test runner ignores.
        std::fs::remove_dir_all(&broken).unwrap();
        std::fs::write(&broken, "not a directory").unwrap();

        let rows = svc
            .list_workspace_file_rows(FileScope::Repo {
                repo_id: sole_repo_id(&svc),
            })
            .await
            .expect("a sick worktree must not fail the whole union");

        assert_eq!(
            rows.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(),
            vec!["healthy.md"],
            "the healthy worktree's files still list: {rows:?}"
        );
        assert_eq!(
            rows[0]
                .copies
                .iter()
                .map(|c| c.worktree_path.clone())
                .collect::<Vec<_>>(),
            vec![main.clone()]
        );
    }

    /// `workspace-file-browser`: *A registry-discovered worktree is an
    /// acceptable browse root*. The union hands the frontend a worktree the
    /// user never registered and expects the per-copy read to be served from
    /// it, so this pins both halves — a future tightening of `ensure_browse_root`
    /// to user-registered folders would empty the union's whole point.
    #[tokio::test]
    async fn a_discovered_worktree_is_browsable_and_readable() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let main = init_openspec_repo(&roots.path().join("main"));
        let sibling = roots.path().join("sibling");
        git(
            &["worktree", "add", "-b", "side", sibling.to_str().unwrap()],
            &main,
        );
        let sibling = openspec_core::canonicalize(&sibling).unwrap();
        std::fs::create_dir_all(sibling.join("openspec").join("changes")).unwrap();
        std::fs::write(sibling.join("draft.md"), "# draft").unwrap();
        // Registering the MAIN worktree is what discovers the sibling.
        register(&svc, &main);

        assert!(
            !svc.list_workspaces()
                .unwrap()
                .iter()
                .any(|w| w.uri == sibling),
            "precondition: the sibling is discovered, not user-registered, so \
             it is absent from the Settings listing"
        );
        assert_eq!(
            svc.list_markdown_files(sibling.clone())
                .await
                .expect("a discovered worktree is an acceptable browse root"),
            vec!["draft.md".to_string()]
        );
        assert_eq!(
            svc.read_workspace_file(sibling, "draft.md".to_string())
                .await
                .expect("and its files are readable through the same guard"),
            "# draft"
        );
    }

    /// The union's work — enumerating a whole worktree's markdown and reading
    /// bytes to compare copies — must never happen on the watcher's
    /// aggregation path. The same fixture therefore yields the file through the
    /// on-demand union and nothing at all through the aggregated snapshot,
    /// which is what makes the absence evidence of an exclusion rather than of
    /// an empty fixture.
    #[tokio::test]
    async fn watcher_aggregation_does_not_do_the_file_unions_work() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let main = init_openspec_repo(&roots.path().join("main"));
        std::fs::create_dir_all(main.join("docs")).unwrap();
        std::fs::write(main.join("docs/uniquely-named-guide.md"), "# Guide").unwrap();

        svc.add_workspace(main.clone()).await.unwrap();
        svc.watcher.aggregate_and_emit();

        let views = serde_json::to_string(&svc.watcher.workspace_views()).unwrap();
        assert!(
            !views.contains("uniquely-named-guide.md"),
            "aggregation must not enumerate workspace markdown: {views}"
        );

        let rows = svc
            .list_workspace_file_rows(FileScope::Repo {
                repo_id: sole_repo_id(&svc),
            })
            .await
            .unwrap();
        assert_eq!(
            rows.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(),
            vec!["docs/uniquely-named-guide.md"],
            "the on-demand union does list it: {rows:?}"
        );
    }

    // --- document watch (document-watch) ---------------------------------

    /// Every refusal must happen *before* a watch exists — asserting only that
    /// an error came back would pass even if the watch had already been armed.
    #[tokio::test]
    async fn document_watch_refusals_arm_no_watch() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let registered = roots.path().join("registered");
        let outsider = roots.path().join("outsider");
        for ws in [&registered, &outsider] {
            std::fs::create_dir_all(ws.join("openspec").join("changes")).unwrap();
            std::fs::write(ws.join("secret.md"), "# secret").unwrap();
            std::fs::write(ws.join("notes.txt"), "plain").unwrap();
        }
        register(&svc, &registered);

        let cases: Vec<(PathBuf, &str, &str)> = vec![
            (outsider.clone(), "secret.md", "unregistered workspace"),
            (
                registered.clone(),
                "../outsider/secret.md",
                "path must not contain `..`",
            ),
            (
                registered.clone(),
                "notes.txt",
                "only .md files can be read",
            ),
        ];
        for (root, rel, expected) in cases {
            let err = svc
                .watch_document("w", root.clone(), rel.to_string())
                .await
                .unwrap_err();
            assert_eq!(err, expected, "watching {rel:?} under {root:?}");
            assert_eq!(
                svc.documents.watched_dir_count(),
                0,
                "a refused registration must arm no filesystem watch"
            );
        }
    }

    /// A symlink that leaves the workspace is refused for a watch exactly as it
    /// is for a read — the containment check resolves what exists on disk.
    #[cfg(unix)]
    #[tokio::test]
    async fn document_watch_refuses_a_symlink_escape() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let registered = roots.path().join("registered");
        std::fs::create_dir_all(registered.join("openspec").join("changes")).unwrap();
        let outside = roots.path().join("outside.md");
        std::fs::write(&outside, "# outside").unwrap();
        std::os::unix::fs::symlink(&outside, registered.join("link.md")).unwrap();
        register(&svc, &registered);

        let err = svc
            .watch_document("w", registered.clone(), "link.md".to_string())
            .await
            .unwrap_err();
        assert_eq!(err, "path escapes workspace");
        assert_eq!(svc.documents.watched_dir_count(), 0);

        // The read agrees, because both go through the same guard.
        assert_eq!(
            svc.read_workspace_file(registered, "link.md".to_string())
                .await
                .unwrap_err(),
            "path escapes workspace"
        );
    }

    /// A reader keeps watching a document that has been deleted, so that it can
    /// resume when the file comes back. Registration must therefore not require
    /// the file to exist — while the *read* still does.
    #[tokio::test]
    async fn a_document_watch_outlives_the_file() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let registered = roots.path().join("registered");
        std::fs::create_dir_all(registered.join("openspec").join("changes")).unwrap();
        register(&svc, &registered);

        svc.watch_document("w", registered.clone(), "gone.md".to_string())
            .await
            .expect("a watch on a not-yet-existing document is allowed");
        assert_eq!(svc.documents.registration_count(), 1);

        let err = svc
            .read_workspace_file(registered, "gone.md".to_string())
            .await
            .unwrap_err();
        assert!(
            err.starts_with("file not found"),
            "the read still requires the file to exist, got {err:?}"
        );
    }

    #[tokio::test]
    async fn releasing_an_owner_drops_its_document_watches() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let registered = roots.path().join("registered");
        std::fs::create_dir_all(registered.join("openspec").join("changes")).unwrap();
        std::fs::write(registered.join("a.md"), "# a").unwrap();
        std::fs::write(registered.join("b.md"), "# b").unwrap();
        register(&svc, &registered);

        svc.watch_document("reader-1", registered.clone(), "a.md".to_string())
            .await
            .unwrap();
        svc.watch_document("reader-1", registered.clone(), "b.md".to_string())
            .await
            .unwrap();
        assert_eq!(svc.documents.registration_count(), 2);

        svc.release_document_owner("reader-1");

        assert_eq!(svc.documents.registration_count(), 0);
        assert_eq!(svc.documents.watched_dir_count(), 0);
    }

    /// Unregistering a workspace must not strand the watches a reader still
    /// holds on it — the release path deliberately does not re-authorise.
    #[tokio::test]
    async fn unwatching_still_works_after_the_workspace_is_unregistered() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let registered = roots.path().join("registered");
        std::fs::create_dir_all(registered.join("openspec").join("changes")).unwrap();
        std::fs::write(registered.join("a.md"), "# a").unwrap();
        register(&svc, &registered);

        svc.watch_document("w", registered.clone(), "a.md".to_string())
            .await
            .unwrap();
        assert_eq!(svc.documents.watched_dir_count(), 1);

        svc.registry.lock().unwrap().unregister(&registered).ok();
        svc.unwatch_document("w", registered.clone(), "a.md".to_string())
            .await;

        assert_eq!(
            svc.documents.watched_dir_count(),
            0,
            "a release must not be refused just because the root is no longer registered"
        );
    }

    /// The harder half: the workspace is unregistered AND its directory is
    /// gone, so the canonical root the key was stored under cannot be
    /// reconstructed at all. Without the relative-path fallback the release
    /// finds nothing and the watch survives.
    #[tokio::test]
    async fn unwatching_still_works_after_the_workspace_directory_is_removed() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let registered = roots.path().join("registered");
        std::fs::create_dir_all(registered.join("openspec").join("changes")).unwrap();
        std::fs::write(registered.join("a.md"), "# a").unwrap();
        register(&svc, &registered);

        svc.watch_document("w", registered.clone(), "a.md".to_string())
            .await
            .unwrap();
        assert_eq!(svc.documents.watched_dir_count(), 1);

        svc.registry.lock().unwrap().unregister(&registered).ok();
        std::fs::remove_dir_all(&registered).unwrap();
        svc.unwatch_document("w", registered.clone(), "a.md".to_string())
            .await;

        assert_eq!(
            svc.documents.registration_count(),
            0,
            "a release must still find its registration when the root cannot be resolved"
        );
        assert_eq!(svc.documents.watched_dir_count(), 0);
    }

    // --- open_artifact_link (open-artifact-links) -----------------------

    /// A flat registered workspace with an `openspec/changes/` tree — enough
    /// to authorize as a browse root, no git required. Returns the
    /// *canonical* path (mirroring `init_openspec_repo`) so a test's
    /// expected `File(...)` values — built by joining onto this return value
    /// — compare equal to `resolve_artifact_link`'s always-canonical result
    /// (on macOS a bare tempdir path is `/var/...`, which canonicalizes to
    /// `/private/var/...`).
    fn registered_flat_workspace(svc: &AppService, roots: &Path, name: &str) -> PathBuf {
        let ws = roots.join(name);
        std::fs::create_dir_all(ws.join("openspec").join("changes")).unwrap();
        register(svc, &ws);
        openspec_core::canonicalize(&ws).unwrap()
    }

    #[tokio::test]
    async fn open_artifact_link_refuses_unauthorized_root() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let outsider = roots.path().join("outsider");
        std::fs::create_dir_all(outsider.join("openspec").join("changes")).unwrap();

        // Unauthorized, so refused before any path is resolved — even for an
        // href (external) that would otherwise need no resolution at all.
        let err = svc
            .open_artifact_link(&outsider, "proposal.md", "https://example.com")
            .unwrap_err();
        assert_eq!(err, "unregistered workspace");
    }

    /// `workspace-file-browser`: *Preview Link Handling* — the resolution base
    /// and the containment root are the worktree of the copy being previewed,
    /// not the repository and not its main worktree.
    ///
    /// Both halves matter and they fail differently. Where the target exists in
    /// BOTH worktrees, resolving against the wrong root silently opens the
    /// wrong file — no error, just different bytes. Where it exists only
    /// alongside its source, resolving against the wrong root refuses a link
    /// that is perfectly valid in the copy being read. This pins the root that
    /// decides both.
    #[tokio::test]
    async fn preview_links_resolve_within_the_previewed_copys_worktree() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let main = init_openspec_repo(&roots.path().join("main"));
        let feature = roots.path().join("feature");
        git(
            &[
                "worktree",
                "add",
                "-b",
                "feature",
                feature.to_str().unwrap(),
            ],
            &main,
        );
        let feature = openspec_core::canonicalize(&feature).unwrap();
        register(&svc, &main);

        for wt in [&main, &feature] {
            std::fs::create_dir_all(wt.join("docs")).unwrap();
            // In BOTH worktrees, so only the root decides which one opens.
            std::fs::write(wt.join("docs/mockup.html"), "<p>mockup</p>").unwrap();
        }
        // In the feature worktree ONLY — the draft written alongside the note
        // that links to it, which is the case the union exists for.
        std::fs::write(feature.join("docs/draft.html"), "<p>draft</p>").unwrap();

        // A link whose target exists in both resolves inside the copy being
        // read — a DIFFERENT file per copy, not one of them for both.
        assert_eq!(
            svc.open_artifact_link(&feature, "docs/note.md", "./mockup.html")
                .unwrap(),
            LinkResolution::File(feature.join("docs/mockup.html"))
        );
        assert_eq!(
            svc.open_artifact_link(&main, "docs/note.md", "./mockup.html")
                .unwrap(),
            LinkResolution::File(main.join("docs/mockup.html"))
        );

        // A link whose target exists only alongside its source opens from that
        // worktree...
        assert_eq!(
            svc.open_artifact_link(&feature, "docs/note.md", "./draft.html")
                .unwrap(),
            LinkResolution::File(feature.join("docs/draft.html"))
        );
        // ...and is refused after switching the preview to the main worktree's
        // copy, rather than reaching across into the feature worktree.
        assert_eq!(
            svc.open_artifact_link(&main, "docs/note.md", "./draft.html")
                .unwrap(),
            LinkResolution::Refused("target not found".to_string())
        );

        // And the containment root still refuses an escape out of the copy,
        // including one aimed at the sibling worktree by name.
        assert_eq!(
            svc.open_artifact_link(&main, "docs/note.md", "../../feature/docs/draft.html")
                .unwrap(),
            LinkResolution::Refused("target escapes the workspace".to_string())
        );
    }

    #[tokio::test]
    async fn open_artifact_link_accepts_main_worktree_registered_only_by_worktree() {
        // Mirrors `file_browser_accepts_a_repo_root_registered_only_by_worktree`:
        // a Repo group browses its main worktree, which need not itself be
        // registered.
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let main = init_openspec_repo(&roots.path().join("main"));
        std::fs::create_dir_all(main.join("mockups")).unwrap();
        std::fs::write(main.join("mockups").join("login.html"), "<html></html>").unwrap();
        let linked = roots.path().join("linked");
        git(
            &["worktree", "add", linked.to_str().unwrap(), "-b", "side"],
            &main,
        );
        std::fs::create_dir_all(linked.join("openspec").join("changes")).unwrap();
        register(&svc, &linked);

        let resolution = svc
            .open_artifact_link(&main, "notes.md", "./mockups/login.html")
            .unwrap();
        assert_eq!(
            resolution,
            LinkResolution::File(main.join("mockups").join("login.html"))
        );
    }

    #[tokio::test]
    async fn open_artifact_link_refuses_dotdot_traversal_plain_and_percent_encoded() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let ws = registered_flat_workspace(&svc, roots.path(), "registered");
        std::fs::create_dir_all(roots.path().join("secret_outside")).unwrap();
        std::fs::write(
            roots.path().join("secret_outside").join("secret.html"),
            "<html></html>",
        )
        .unwrap();
        std::fs::create_dir_all(ws.join("openspec").join("changes").join("x")).unwrap();
        let base_path = "openspec/changes/x/proposal.md";

        for href in [
            "../../../../secret_outside/secret.html",
            "%2e%2e%2f%2e%2e%2f%2e%2e%2f%2e%2e%2fsecret_outside/secret.html",
        ] {
            let resolution = svc.open_artifact_link(&ws, base_path, href).unwrap();
            assert!(
                matches!(resolution, LinkResolution::Refused(_)),
                "{href} must be refused, got {resolution:?}"
            );
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn open_artifact_link_refuses_symlink_escaping_root() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let ws = registered_flat_workspace(&svc, roots.path(), "registered");
        std::fs::create_dir_all(roots.path().join("outside")).unwrap();
        std::fs::write(
            roots.path().join("outside").join("secret.html"),
            "<html></html>",
        )
        .unwrap();
        std::os::unix::fs::symlink(
            roots.path().join("outside").join("secret.html"),
            ws.join("escape.html"),
        )
        .unwrap();

        let resolution = svc
            .open_artifact_link(&ws, "notes.md", "./escape.html")
            .unwrap();
        assert!(
            matches!(resolution, LinkResolution::Refused(_)),
            "a symlink pointing outside the root must be refused, got {resolution:?}"
        );
    }

    #[tokio::test]
    async fn open_artifact_link_allows_target_inside_root_outside_change_dir() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let ws = registered_flat_workspace(&svc, roots.path(), "registered");
        std::fs::create_dir_all(ws.join("openspec").join("changes").join("x")).unwrap();
        std::fs::create_dir_all(ws.join("mockups")).unwrap();
        std::fs::write(ws.join("mockups").join("login.html"), "<html></html>").unwrap();

        // Climbs from the change dir back to a root-level `mockups/`, outside
        // `openspec/changes/` entirely — wider than the artifact-read
        // boundary, exactly as design.md's Decision 3 intends.
        let resolution = svc
            .open_artifact_link(
                &ws,
                "openspec/changes/x/proposal.md",
                "../../../mockups/login.html",
            )
            .unwrap();
        assert_eq!(
            resolution,
            LinkResolution::File(ws.join("mockups").join("login.html"))
        );
    }

    #[tokio::test]
    async fn open_artifact_link_reports_nonexistent_target_as_refused() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let ws = registered_flat_workspace(&svc, roots.path(), "registered");

        let resolution = svc
            .open_artifact_link(&ws, "notes.md", "./missing.html")
            .unwrap();
        assert!(
            matches!(resolution, LinkResolution::Refused(_)),
            "a dangling link must refuse quietly, not panic — got {resolution:?}"
        );
    }

    #[tokio::test]
    async fn open_artifact_link_resolves_percent_encoded_space_in_filename() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let ws = registered_flat_workspace(&svc, roots.path(), "registered");
        std::fs::write(ws.join("my file.html"), "<html></html>").unwrap();

        let resolution = svc
            .open_artifact_link(&ws, "notes.md", "./my%20file.html")
            .unwrap();
        assert_eq!(resolution, LinkResolution::File(ws.join("my file.html")));
    }

    #[tokio::test]
    async fn open_artifact_link_strips_fragment_and_query() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let ws = registered_flat_workspace(&svc, roots.path(), "registered");
        std::fs::write(ws.join("login.html"), "<html></html>").unwrap();

        for href in ["./login.html#hero", "./login.html?v=2"] {
            let resolution = svc.open_artifact_link(&ws, "notes.md", href).unwrap();
            assert_eq!(
                resolution,
                LinkResolution::File(ws.join("login.html")),
                "{href} must resolve to the underlying file"
            );
        }
    }

    #[tokio::test]
    async fn open_artifact_link_classifies_markdown_extension_case_insensitive_as_inert() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let ws = registered_flat_workspace(&svc, roots.path(), "registered");

        // No file need exist on disk: markdown classification happens before
        // any resolution or existence check.
        for href in ["./notes.md", "./NOTES.MD", "./notes.markdown"] {
            assert_eq!(
                svc.open_artifact_link(&ws, "proposal.md", href).unwrap(),
                LinkResolution::Inert,
                "{href} must be inert"
            );
        }
    }

    #[tokio::test]
    async fn open_artifact_link_classifies_script_and_fragment_schemes_as_inert() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let ws = registered_flat_workspace(&svc, roots.path(), "registered");

        for href in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "#just-a-fragment",
        ] {
            assert_eq!(
                svc.open_artifact_link(&ws, "proposal.md", href).unwrap(),
                LinkResolution::Inert,
                "{href} must be inert"
            );
        }
    }

    #[tokio::test]
    async fn open_artifact_link_classifies_external_schemes_without_touching_disk() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        // Note: no workspace directory is even created beyond the registry
        // requirement — an External classification does no path resolution.
        let ws = registered_flat_workspace(&svc, roots.path(), "registered");

        for href in [
            "http://example.com",
            "https://example.com/path",
            "mailto:a@example.com",
            "tel:+1-555-0100",
        ] {
            assert_eq!(
                svc.open_artifact_link(&ws, "proposal.md", href).unwrap(),
                LinkResolution::External(href.to_string())
            );
        }
    }

    #[tokio::test]
    async fn open_artifact_link_refuses_non_allowlisted_extension_and_directory() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let ws = registered_flat_workspace(&svc, roots.path(), "registered");
        std::fs::write(ws.join("run.sh"), "#!/bin/sh\necho hi\n").unwrap();
        std::fs::create_dir_all(ws.join("adir")).unwrap();

        for href in ["./run.sh", "./adir"] {
            let resolution = svc.open_artifact_link(&ws, "notes.md", href).unwrap();
            assert!(
                matches!(resolution, LinkResolution::Refused(_)),
                "{href} must be refused, got {resolution:?}"
            );
        }
    }

    /// Containment relies entirely on comparing *canonicalised* paths
    /// (`openspec_core::canonicalize`, dunce-backed) rather than a literal
    /// string prefix — the same mechanism that collapses a `\\wsl.localhost`
    /// workspace's verbatim and simplified UNC spellings into one identity on
    /// Windows. This proxies that with a `..`-round-trip spelling (real on
    /// every platform, exercising the identical canonicalize-then-`starts_with`
    /// code path); the actual UNC-form runtime behaviour still needs a real
    /// Windows+WSL2 box to verify (see CLAUDE.md's WSL notes).
    #[tokio::test]
    async fn open_artifact_link_containment_is_canonical_not_literal() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let ws = registered_flat_workspace(&svc, roots.path(), "registered");
        std::fs::create_dir_all(ws.join("mockups")).unwrap();
        std::fs::write(ws.join("mockups").join("login.html"), "<html></html>").unwrap();

        // An equivalent-but-differently-spelled root (round-trips through a
        // child dir and back) must authorize and resolve identically to the
        // canonical spelling.
        let equivalent_root = ws.join("openspec").join("..");
        let resolution = svc
            .open_artifact_link(&equivalent_root, "notes.md", "./mockups/login.html")
            .unwrap();
        assert_eq!(
            resolution,
            LinkResolution::File(ws.join("mockups").join("login.html"))
        );
    }

    // --- Disabling a workspace (disable-workspaces) --------------------------

    /// Adds `name` as an active change directory in `root`.
    fn add_change(root: &Path, name: &str) {
        let dir = root.join("openspec").join("changes").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("proposal.md"), "# x\n").unwrap();
    }

    #[tokio::test]
    async fn a_disabled_row_leaves_the_tree_but_stays_in_the_listing_and_the_record() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let parked = init_openspec_repo(&roots.path().join("parked"));
        let kept = init_openspec_repo(&roots.path().join("kept"));
        add_change(&parked, "parked-change");
        add_change(&kept, "kept-change");

        let parked_ws = svc.add_workspace(parked.clone()).await.unwrap();
        svc.add_workspace(kept.clone()).await.unwrap();
        let parked_repo = parked_ws
            .repo_id
            .clone()
            .expect("parked repo is git-backed");

        assert_eq!(
            svc.workspace_views().len(),
            2,
            "both rows start in the tree"
        );
        assert_eq!(svc.active_count(), 2, "both changes start in the badge");

        svc.set_workspace_disabled(parked.clone(), Some(parked_repo.clone()), true)
            .await
            .unwrap();

        // The tree drops it — and the freshness contract means this holds on the
        // very next call, with no intervening filesystem event.
        let views = svc.workspace_views();
        assert_eq!(views.len(), 1, "the parked row leaves the tree");
        assert!(
            !views.iter().any(|v| matches!(
                v,
                WorkspaceView::Repo(r) if r.repo_id == parked_repo
            )),
            "and it is specifically the parked one that left"
        );
        assert_eq!(svc.active_count(), 1, "the badge drops its active change");

        // Settings keeps it, flagged — that is where the toggle back lives.
        let listed = svc.list_workspaces().unwrap();
        assert_eq!(listed.len(), 2, "the listing keeps every registration");
        assert!(
            listed.iter().find(|w| w.uri == parked).unwrap().disabled,
            "the parked row is flagged in the listing"
        );
        assert!(!listed.iter().find(|w| w.uri == kept).unwrap().disabled);

        // The record keeps it: the raw snapshot the Dashboard reads is unfiltered,
        // and the parked row still carries its change.
        let record = svc.watcher.workspace_views();
        assert_eq!(record.len(), 2, "the Dashboard's snapshot keeps both rows");
        let parked_row = record
            .iter()
            .find_map(|v| match v {
                WorkspaceView::Repo(r) if r.repo_id == parked_repo => Some(r),
                _ => None,
            })
            .expect("parked row present in the unfiltered snapshot");
        assert!(parked_row.disabled);
        assert_eq!(
            parked_row.active.len(),
            1,
            "a parked row keeps its change count, so Dashboard totals stay whole"
        );

        // Re-enabling restores it in one shot.
        svc.set_workspace_disabled(parked.clone(), Some(parked_repo), false)
            .await
            .unwrap();
        assert_eq!(
            svc.workspace_views().len(),
            2,
            "re-enabling restores the row"
        );
        assert_eq!(svc.active_count(), 2);
        assert!(!svc.list_workspaces().unwrap().iter().any(|w| w.disabled));
    }

    #[tokio::test]
    async fn sibling_worktrees_of_one_repository_share_a_single_disabled_state() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let main = init_openspec_repo(&roots.path().join("main"));
        let sibling = roots.path().join("sibling");
        git(
            &[
                "worktree",
                "add",
                "-b",
                "feature",
                sibling.to_str().unwrap(),
            ],
            &main,
        );
        std::fs::create_dir_all(sibling.join("openspec").join("changes")).unwrap();
        let sibling = openspec_core::canonicalize(&sibling).unwrap();

        // Both worktrees user-registered, so Settings lists two rows for one
        // repo. The sibling is auto-discovered by the first registration, so
        // the second `add_workspace` promotes it.
        let main_ws = svc.add_workspace(main.clone()).await.unwrap();
        let sibling_ws = svc.add_workspace(sibling.clone()).await.unwrap();
        assert_eq!(sibling_ws.uri, sibling, "the promoted folder is returned");
        let repo_id = main_ws.repo_id.clone().expect("git-backed");

        let listed = svc.list_workspaces().unwrap();
        assert_eq!(listed.len(), 2, "both worktrees are user-registered");
        assert!(listed.iter().all(|w| !w.disabled));

        // Disable from *one* of the rows.
        svc.set_workspace_disabled(main.clone(), Some(repo_id.clone()), true)
            .await
            .unwrap();

        let listed = svc.list_workspaces().unwrap();
        assert!(
            listed.iter().all(|w| w.disabled),
            "the repo group is one row, so both of its Settings entries report disabled: {:?}",
            listed
                .iter()
                .map(|w| (&w.name, w.disabled))
                .collect::<Vec<_>>()
        );
        assert!(
            svc.workspace_views().is_empty(),
            "the whole repository group leaves the tree, not just one worktree"
        );

        // Re-enabling from the *other* row brings the group back.
        svc.set_workspace_disabled(sibling, Some(repo_id), false)
            .await
            .unwrap();
        assert!(svc.list_workspaces().unwrap().iter().all(|w| !w.disabled));
        assert_eq!(svc.workspace_views().len(), 1, "one repo group row");
    }

    #[tokio::test]
    async fn disabling_a_flat_workspace_uses_its_own_key() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());

        let roots = tempfile::tempdir().unwrap();
        let flat = roots.path().join("flat");
        std::fs::create_dir_all(flat.join("openspec").join("changes")).unwrap();
        add_change(&flat, "c1");
        let flat = openspec_core::canonicalize(&flat).unwrap();

        let ws = svc.add_workspace(flat.clone()).await.unwrap();
        assert!(ws.repo_id.is_none(), "precondition: not a git workspace");
        assert_eq!(svc.workspace_views().len(), 1);

        svc.set_workspace_disabled(flat.clone(), None, true)
            .await
            .unwrap();
        assert!(
            svc.workspace_views().is_empty(),
            "the flat row leaves the tree"
        );
        assert_eq!(svc.active_count(), 0);
        assert!(svc.list_workspaces().unwrap()[0].disabled);

        svc.set_workspace_disabled(flat, None, false).await.unwrap();
        assert_eq!(svc.workspace_views().len(), 1);
    }

    /// A snapshot listing one pull request per URL.
    fn listing(urls: &[&str]) -> BitbucketPullRequestsState {
        BitbucketPullRequestsState {
            status: PullRequestsStatus::Ok,
            stale: false,
            fetched_at_unix: Some(1_700_000_000),
            pull_requests: rows(urls),
            skipped_workspaces: Vec::new(),
        }
    }

    /// One row per URL, the shape both providers' snapshots hold.
    fn rows(urls: &[&str]) -> Vec<crate::pull_requests::PullRequestSummary> {
        urls.iter()
            .enumerate()
            .map(|(i, url)| crate::pull_requests::PullRequestSummary {
                id: i as u64 + 1,
                title: format!("PR {i}"),
                repo_full_name: "acme/app".to_string(),
                source_branch: "feature".to_string(),
                destination_branch: "main".to_string(),
                url: url.to_string(),
                draft: false,
                updated_at_unix: 1_700_000_000,
                review: None,
                open_tasks: 0,
                author: None,
                checks: None,
                conflicting: false,
                unresolved_threads: 0,
                source_repo_full_name: "acme/app".to_string(),
            })
            .collect()
    }

    /// A GitHub snapshot with the given authored and review-requested URLs.
    fn github_listing(authored: &[&str], review_requested: &[&str]) -> GithubPullRequestsState {
        GithubPullRequestsState {
            status: PullRequestsStatus::Ok,
            stale: false,
            fetched_at_unix: Some(1_700_000_000),
            authored: rows(authored),
            review_requested: rows(review_requested),
            withheld: 0,
        }
    }

    #[test]
    fn open_pull_request_accepts_a_url_listed_in_the_current_snapshot() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let url = "https://bitbucket.org/acme/app/pull-requests/7";

        // Nothing is listed while the feature is off — even a real pull
        // request's URL is refused.
        assert_eq!(
            svc.bitbucket_pull_requests(),
            BitbucketPullRequestsState::disabled(),
            "the snapshot starts disabled"
        );
        assert!(svc.open_pull_request(url).is_err());

        svc.bitbucket.set(listing(&[url]));

        assert_eq!(svc.bitbucket_pull_requests(), listing(&[url]));
        assert_eq!(svc.open_pull_request(url), Ok(url.to_string()));
    }

    #[test]
    fn open_pull_request_refuses_anything_not_listed() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        svc.bitbucket
            .set(listing(&["https://bitbucket.org/acme/app/pull-requests/7"]));

        for other in [
            "https://bitbucket.org/acme/app/pull-requests/8",
            "https://bitbucket.org/acme/app/pull-requests/7/diff",
            "https://bitbucket.org/acme/app/pull-requests/",
            "https://evil.example/",
            "file:///etc/passwd",
            "",
        ] {
            assert!(svc.open_pull_request(other).is_err(), "{other:?}");
        }
    }

    /// A row whose response carried no `https://` link has an empty URL; that
    /// must not make the empty string openable.
    #[test]
    fn open_pull_request_refuses_the_empty_url_of_a_linkless_row() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        svc.bitbucket.set(listing(&[""]));

        assert!(svc.open_pull_request("").is_err());
    }

    /// Rows from either GitHub list open; a URL in neither snapshot does not,
    /// and the BitBucket rows keep opening beside them
    /// (`github-pull-requests`: *Opening a GitHub Pull Request*).
    #[test]
    fn open_pull_request_accepts_rows_from_both_github_lists() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let mine = "https://github.com/acme/app/pull/1";
        let theirs = "https://github.com/acme/app/pull/2";
        let bitbucket = "https://bitbucket.org/acme/app/pull-requests/7";

        assert_eq!(
            svc.github_pull_requests(),
            GithubPullRequestsState::disabled(),
            "the GitHub snapshot starts disabled"
        );
        assert!(svc.open_pull_request(mine).is_err());

        svc.github.set(github_listing(&[mine], &[theirs]));
        svc.bitbucket.set(listing(&[bitbucket]));

        assert_eq!(
            svc.github_pull_requests(),
            github_listing(&[mine], &[theirs])
        );
        assert_eq!(svc.open_pull_request(mine), Ok(mine.to_string()));
        assert_eq!(svc.open_pull_request(theirs), Ok(theirs.to_string()));
        assert_eq!(svc.open_pull_request(bitbucket), Ok(bitbucket.to_string()));
        for other in [
            "https://github.com/acme/app/pull/3",
            "https://github.com/acme/app/pull/1/files",
            "",
        ] {
            assert!(svc.open_pull_request(other).is_err(), "{other:?}");
        }
    }

    /// End to end through the service (`pull-request-worktree-links`: *The
    /// Pull-Request Links Snapshot*): a registered repository whose `origin`
    /// is on GitHub, its main worktree on the pull request's head branch, and
    /// a GitHub snapshot listing that pull request — linked both ways. With no
    /// rows the join is empty, and a disabled GitHub snapshot contributes
    /// nothing.
    #[tokio::test]
    async fn pull_request_links_join_the_snapshots_with_the_registered_worktrees() {
        use crate::pull_request_links::{PullRequestLinks, PullRequestRole};

        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let main = init_openspec_repo(&roots.path().join("api"));
        git(
            &["remote", "add", "origin", "git@github.com:acme/app.git"],
            &main,
        );
        git(&["checkout", "-b", "feature"], &main);
        svc.add_workspace(main.clone()).await.unwrap();
        svc.watcher.aggregate_and_emit();

        assert_eq!(
            svc.pull_request_links().await,
            PullRequestLinks::default(),
            "no rows, nothing to link"
        );

        let url = "https://github.com/acme/app/pull/1";
        svc.github.set(github_listing(&[url], &[]));
        let links = svc.pull_request_links().await;
        assert_eq!(links.pull_requests.len(), 1, "{links:?}");
        assert_eq!(links.pull_requests[0].url, url);
        assert_eq!(links.pull_requests[0].worktrees.len(), 1);
        let linked = &links.pull_requests[0].worktrees[0];
        assert_eq!(linked.worktree_path, main);
        assert_eq!(linked.branch.as_deref(), Some("feature"));
        assert_eq!(links.worktrees.len(), 1);
        assert_eq!(links.worktrees[0].worktree_path, main);
        assert_eq!(links.worktrees[0].pull_requests[0].url, url);
        assert_eq!(
            links.worktrees[0].pull_requests[0].role,
            PullRequestRole::Authored
        );

        // Disabling GitHub collapses its snapshot, and its links go with it.
        svc.github.set(GithubPullRequestsState::disabled());
        assert_eq!(svc.pull_request_links().await, PullRequestLinks::default());
    }

    /// The service tags and orders its join inputs: a worktree linked to a
    /// BitBucket row, a GitHub authored row and a GitHub review request lists
    /// them in that order, each with its provider and role.
    #[tokio::test]
    async fn pull_request_links_list_bitbucket_then_github_authored_then_review_requests() {
        use crate::events::PullRequestProvider;
        use crate::pull_request_links::PullRequestRole;

        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let main = init_openspec_repo(&roots.path().join("app"));
        git(
            &["remote", "add", "origin", "git@github.com:acme/app.git"],
            &main,
        );
        git(
            &["remote", "add", "bb", "git@bitbucket.org:acme/app.git"],
            &main,
        );
        git(&["checkout", "-b", "feature"], &main);
        svc.add_workspace(main.clone()).await.unwrap();
        svc.watcher.aggregate_and_emit();

        let bb = "https://bitbucket.org/acme/app/pull-requests/1";
        let mine = "https://github.com/acme/app/pull/2";
        let theirs = "https://github.com/acme/app/pull/3";
        // Set out of order on purpose: the service must impose the order.
        svc.github.set(github_listing(&[mine], &[theirs]));
        svc.bitbucket.set(listing(&[bb]));

        let links = svc.pull_request_links().await;
        assert_eq!(links.worktrees.len(), 1, "{links:?}");
        let listed: Vec<(PullRequestProvider, PullRequestRole, &str)> = links.worktrees[0]
            .pull_requests
            .iter()
            .map(|p| (p.provider, p.role, p.url.as_str()))
            .collect();
        assert_eq!(
            listed,
            vec![
                (
                    PullRequestProvider::Bitbucket,
                    PullRequestRole::Authored,
                    bb
                ),
                (PullRequestProvider::Github, PullRequestRole::Authored, mine),
                (
                    PullRequestProvider::Github,
                    PullRequestRole::ReviewRequested,
                    theirs
                ),
            ]
        );
    }

    /// Where the guarantee is enforced: a disabled repository never reaches
    /// the remotes reader, so computing links spawns no `git remote -v` for it;
    /// re-enabling reads its remotes once and the link appears
    /// (`pull-request-worktree-links`: *Repository Remote Identities*).
    #[tokio::test]
    async fn a_disabled_repository_reads_no_remotes_for_the_links() {
        use openspec_core::git::invocation_log;

        invocation_log::enable();
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let main = init_openspec_repo(&roots.path().join("parked"));
        git(
            &["remote", "add", "origin", "git@github.com:acme/app.git"],
            &main,
        );
        git(&["checkout", "-b", "feature"], &main);
        let ws = svc.add_workspace(main.clone()).await.unwrap();
        svc.watcher.aggregate_and_emit();
        let repo_id = ws.repo_id.clone().expect("git-backed");
        svc.github
            .set(github_listing(&["https://github.com/acme/app/pull/1"], &[]));
        let remote_reads = |mark: usize| {
            invocation_log::recorded_since(mark)
                .iter()
                .filter(|inv| {
                    inv.anchor.starts_with(&main) && inv.args.iter().any(|a| a == "remote")
                })
                .count()
        };

        svc.set_workspace_disabled(main.clone(), Some(repo_id.clone()), true)
            .await
            .unwrap();
        let mark = invocation_log::mark();
        let links = svc.pull_request_links().await;
        assert!(links.pull_requests.is_empty(), "{links:?}");
        assert_eq!(
            remote_reads(mark),
            0,
            "a disabled repository reads no remotes"
        );

        svc.set_workspace_disabled(main.clone(), Some(repo_id), false)
            .await
            .unwrap();
        let mark = invocation_log::mark();
        let links = svc.pull_request_links().await;
        assert_eq!(links.pull_requests.len(), 1, "{links:?}");
        assert_eq!(remote_reads(mark), 1, "re-enabled: read once");
    }

    /// Removing a repository drops its remembered remotes: a `set-url` made
    /// while it was unregistered is seen after re-registering, because the
    /// monitor that would have reported it was torn down with the removal.
    #[tokio::test]
    async fn a_re_registered_repository_reads_its_remotes_afresh() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let roots = tempfile::tempdir().unwrap();
        let main = init_openspec_repo(&roots.path().join("app"));
        git(
            &["remote", "add", "origin", "git@github.com:acme/app.git"],
            &main,
        );
        git(&["checkout", "-b", "feature"], &main);
        svc.add_workspace(main.clone()).await.unwrap();
        svc.watcher.aggregate_and_emit();
        svc.github
            .set(github_listing(&["https://github.com/acme/app/pull/1"], &[]));
        assert_eq!(svc.pull_request_links().await.pull_requests.len(), 1);

        svc.remove_workspace(main.clone()).await.unwrap();
        git(
            &[
                "remote",
                "set-url",
                "origin",
                "git@github.com:acme/other.git",
            ],
            &main,
        );
        svc.add_workspace(main.clone()).await.unwrap();
        svc.watcher.aggregate_and_emit();

        let links = svc.pull_request_links().await;
        assert!(
            links.pull_requests.is_empty(),
            "the new origin no longer agrees with the pull request: {links:?}"
        );
    }

    /// The GitHub twin of the poller test below: with the feature on and no
    /// token it reports unauthenticated and announces it on its own variant.
    /// Steps aside rather than reach GitHub when a token is in the
    /// environment.
    #[tokio::test]
    async fn the_github_poller_announces_a_missing_token_once_spawned() {
        if std::env::var_os("GH_TOKEN").is_some() || std::env::var_os("GITHUB_TOKEN").is_some() {
            return;
        }
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        svc.settings.set_github_enabled(true).unwrap();
        let mut events = svc.subscribe();
        assert_eq!(
            svc.github_pull_requests().status,
            PullRequestsStatus::Disabled,
            "nothing runs before the poller is spawned"
        );

        svc.spawn_github_poller();

        let announced = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match events.recv().await {
                    Ok(CacheEvent::GithubPullRequestsUpdated) => break,
                    Ok(_) => continue,
                    Err(e) => panic!("cache stream closed: {e}"),
                }
            }
        })
        .await;
        assert!(
            announced.is_ok(),
            "the poller must announce its first snapshot within a few ticks"
        );
        assert_eq!(
            svc.github_pull_requests().status,
            PullRequestsStatus::Unauthenticated
        );
        assert_eq!(
            svc.bitbucket_pull_requests().status,
            PullRequestsStatus::Disabled,
            "the BitBucket snapshot is untouched"
        );

        // Disabling stops tracking: the snapshot collapses to Disabled and the
        // collapse is announced, so the panel disappears (`github-pull-requests`:
        // *Opt-in GitHub Pull-Request Tracking*, "Disabling stops tracking").
        svc.settings.set_github_enabled(false).unwrap();
        let collapsed = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match events.recv().await {
                    Ok(CacheEvent::GithubPullRequestsUpdated) => break,
                    Ok(_) => continue,
                    Err(e) => panic!("cache stream closed: {e}"),
                }
            }
        })
        .await;
        assert!(collapsed.is_ok(), "the collapse must be announced");
        assert_eq!(
            svc.github_pull_requests(),
            GithubPullRequestsState::disabled()
        );
    }

    /// The spawned poller is real: with the feature on and no credential it
    /// reports the unauthenticated state within a tick and announces it on
    /// the cache stream, which is what the panel's first paint relies on. No
    /// network is involved on this path — there is nothing to authenticate
    /// with — so the test is hermetic as long as the artifex environment
    /// variables are not set, and it steps aside rather than reach BitBucket
    /// when they are.
    #[tokio::test]
    async fn the_bitbucket_poller_announces_missing_credentials_once_spawned() {
        if std::env::var_os("BITBUCKET_USERNAME").is_some()
            || std::env::var_os("BITBUCKET_API_TOKEN").is_some()
        {
            return;
        }
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        svc.settings.set_bitbucket_enabled(true).unwrap();
        let mut events = svc.subscribe();
        assert_eq!(
            svc.bitbucket_pull_requests().status,
            PullRequestsStatus::Disabled,
            "nothing runs before the poller is spawned"
        );

        svc.spawn_bitbucket_poller();

        let announced = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match events.recv().await {
                    Ok(CacheEvent::BitbucketPullRequestsUpdated) => break,
                    Ok(_) => continue,
                    Err(e) => panic!("cache stream closed: {e}"),
                }
            }
        })
        .await;
        assert!(
            announced.is_ok(),
            "the poller must announce its first snapshot within a few ticks"
        );
        assert_eq!(
            svc.bitbucket_pull_requests().status,
            PullRequestsStatus::Unauthenticated
        );
    }

    // ------------------------------------------------- notices and limits

    /// A notice raised through any clone of the service reaches every
    /// subscriber — the desktop forwarder and each served tab's stream hold
    /// one — and never rides the cache stream (`pull-request-viewer`:
    /// *Provider Enabled Flags Stay Current*).
    #[test]
    fn a_notice_reaches_every_subscriber_and_not_the_cache_stream() {
        use crate::events::{PullRequestProvider, PullRequestProviderChangedPayload};

        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        // Raising one with nobody listening is not an error.
        svc.notify(ServiceNotice::PullRequestProviderChanged(
            PullRequestProviderChangedPayload {
                provider: PullRequestProvider::Github,
                enabled: true,
            },
        ));

        let mut window = svc.subscribe_notices();
        let mut tab = svc.clone().subscribe_notices();
        let mut cache = svc.subscribe();
        let notice = ServiceNotice::PullRequestProviderChanged(PullRequestProviderChangedPayload {
            provider: PullRequestProvider::Bitbucket,
            enabled: false,
        });
        svc.clone().notify(notice.clone());

        assert_eq!(window.try_recv().unwrap(), notice);
        assert_eq!(tab.try_recv().unwrap(), notice);
        assert!(window.try_recv().is_err(), "raised once, heard once");
        assert!(cache.try_recv().is_err(), "never a cache event");
    }

    /// The current wall clock, as the pollers read it.
    fn wall_clock() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Waits for the next announcement of `wanted` on the cache stream.
    async fn announced(
        events: &mut broadcast::Receiver<CacheEvent>,
        wanted: fn(&CacheEvent) -> bool,
    ) {
        let heard = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match events.recv().await {
                    Ok(event) if wanted(&event) => break,
                    Ok(_) => continue,
                    Err(e) => panic!("cache stream closed: {e}"),
                }
            }
        })
        .await;
        assert!(heard.is_ok(), "announced within a few ticks");
    }

    /// `pull-request-viewer`: *Toggling the provider resets nothing*. Each
    /// provider's deadlines and spent budget survive disabling it, enabling
    /// it again and saving its credential.
    #[test]
    fn provider_limits_survive_toggling_and_a_saved_credential() {
        use crate::github::{GithubRequest, RateLimit};
        use crate::pull_request_limits::{admit_or_fail, Admission};
        const NOW: u64 = 1_800_000_000;

        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (bitbucket, github) = (&svc.bitbucket_limits, &svc.github_limits);
        let Admission::Admitted(read) = admit_or_fail(bitbucket, true, NOW) else {
            panic!("an idle provider admits a read");
        };
        for _ in 0..bitbucket.budget() {
            read.request(NOW).unwrap();
        }
        drop(read);
        bitbucket.rate_limited(Some(600), NOW);
        github.rate_limited(
            GithubRequest::Files,
            RateLimit {
                delay: Some(900),
                secondary: true,
            },
            NOW,
        );
        let before = (
            bitbucket.deadlines(),
            bitbucket.spent(NOW),
            github.deadlines(),
        );
        assert_eq!(before.1, crate::bitbucket::DETAIL_BUDGET);

        svc.settings.set_bitbucket_enabled(true).unwrap();
        svc.settings.set_bitbucket_enabled(false).unwrap();
        svc.settings.set_bitbucket_enabled(true).unwrap();
        svc.settings
            .set_bitbucket_credentials("ada".to_string(), "new-token".to_string())
            .unwrap();
        svc.settings.set_github_enabled(true).unwrap();
        svc.settings.set_github_enabled(false).unwrap();
        svc.settings.set_github_enabled(true).unwrap();
        svc.settings
            .set_github_token("ghp_new".to_string())
            .unwrap();

        assert_eq!(
            (
                bitbucket.deadlines(),
                bitbucket.spent(NOW),
                github.deadlines()
            ),
            before
        );
        assert!(matches!(
            admit_or_fail(bitbucket, true, NOW + 600),
            Admission::Deferred { .. }
        ));
    }

    /// `github-pull-requests`: *A deadline survives switching the feature off
    /// and on*, through the running poller. Enabled inside a GraphQL deadline
    /// it publishes and announces `unavailable` at once, sending nothing;
    /// switched off it collapses to `disabled` and keeps the deadline;
    /// switched on again it reads `unavailable` again at once. No token is
    /// stored, so a poller that did send would read unauthenticated, never
    /// reach GitHub — and the test steps aside when the environment supplies
    /// one.
    #[tokio::test]
    async fn the_github_poller_waits_out_a_deadline_through_a_toggle() {
        use crate::github::{GithubRequest, RateLimit};

        if std::env::var_os("GH_TOKEN").is_some() || std::env::var_os("GITHUB_TOKEN").is_some() {
            return;
        }
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        svc.github_limits.rate_limited(
            GithubRequest::Query,
            RateLimit {
                delay: Some(1_200),
                secondary: false,
            },
            wall_clock(),
        );
        let deadlines = svc.github_limits.deadlines();
        let github = |event: &CacheEvent| matches!(event, CacheEvent::GithubPullRequestsUpdated);
        let unavailable = |state: GithubPullRequestsState| {
            state.status == PullRequestsStatus::Unavailable
                && !state.stale
                && state.rows().next().is_none()
        };
        svc.settings.set_github_enabled(true).unwrap();
        let mut events = svc.subscribe();

        svc.spawn_github_poller();
        announced(&mut events, github).await;
        assert!(unavailable(svc.github_pull_requests()));

        svc.settings.set_github_enabled(false).unwrap();
        announced(&mut events, github).await;
        assert_eq!(
            svc.github_pull_requests(),
            GithubPullRequestsState::disabled()
        );
        assert_eq!(svc.github_limits.deadlines(), deadlines);

        svc.settings.set_github_enabled(true).unwrap();
        announced(&mut events, github).await;
        assert!(unavailable(svc.github_pull_requests()));
        assert_eq!(svc.github_limits.deadlines(), deadlines);
    }

    /// `bitbucket-pull-requests`: *A deadline survives switching the feature
    /// off and on*, the twin of the GitHub test above. No credential is
    /// stored, so a poller that did send would read unauthenticated.
    #[tokio::test]
    async fn the_bitbucket_poller_waits_out_a_deadline_through_a_toggle() {
        if std::env::var_os("BITBUCKET_USERNAME").is_some()
            || std::env::var_os("BITBUCKET_API_TOKEN").is_some()
        {
            return;
        }
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        svc.bitbucket_limits.rate_limited(Some(600), wall_clock());
        let deadline = svc.bitbucket_limits.deadlines();
        let bitbucket =
            |event: &CacheEvent| matches!(event, CacheEvent::BitbucketPullRequestsUpdated);
        let unavailable = |state: BitbucketPullRequestsState| {
            state.status == PullRequestsStatus::Unavailable
                && !state.stale
                && state.pull_requests.is_empty()
        };
        svc.settings.set_bitbucket_enabled(true).unwrap();
        let mut events = svc.subscribe();

        svc.spawn_bitbucket_poller();
        announced(&mut events, bitbucket).await;
        assert!(unavailable(svc.bitbucket_pull_requests()));

        svc.settings.set_bitbucket_enabled(false).unwrap();
        announced(&mut events, bitbucket).await;
        assert_eq!(
            svc.bitbucket_pull_requests(),
            BitbucketPullRequestsState::disabled()
        );
        assert_eq!(svc.bitbucket_limits.deadlines(), deadline);

        svc.settings.set_bitbucket_enabled(true).unwrap();
        announced(&mut events, bitbucket).await;
        assert!(unavailable(svc.bitbucket_pull_requests()));
        assert_eq!(svc.bitbucket_limits.deadlines(), deadline);
    }

    // ------------------------------------------------- pull-request detail reads

    use crate::pull_request_detail::FileReadFailure;
    use crate::pull_request_detail::PullRequestDetail;
    use crate::pull_request_limits::Deadlines as _;
    use crate::pull_request_read::fake::{self, FakeIo};
    use crate::pull_requests::{ChecksState, PullRequestSummary};
    use PullRequestProvider::{Bitbucket, Github};

    /// The scripted transport's clock when a test begins.
    const READ_AT: u64 = 1_800_000_000;

    /// Pull request `id` of `repo_full_name`, listed with a URL on its
    /// provider's own site.
    fn listed_pull_request(
        provider: PullRequestProvider,
        repo_full_name: &str,
        id: u64,
    ) -> PullRequestSummary {
        let url = match provider {
            Github => format!("https://github.com/{repo_full_name}/pull/{id}"),
            Bitbucket => format!("https://bitbucket.org/{repo_full_name}/pull-requests/{id}"),
        };
        PullRequestSummary {
            id,
            title: format!("PR {id}"),
            repo_full_name: repo_full_name.to_string(),
            source_branch: "feature".to_string(),
            destination_branch: "main".to_string(),
            url,
            draft: false,
            updated_at_unix: 1_700_000_000,
            review: None,
            open_tasks: 0,
            author: None,
            checks: None,
            conflicting: false,
            unresolved_threads: 0,
            source_repo_full_name: repo_full_name.to_string(),
        }
    }

    fn pull_request(
        provider: PullRequestProvider,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> PullRequestReference {
        PullRequestReference {
            provider,
            owner: owner.to_string(),
            repo: repo.to_string(),
            number,
        }
    }

    /// Sets `provider`'s snapshot to list `rows`, fresh.
    fn list(svc: &AppService, provider: PullRequestProvider, rows: Vec<PullRequestSummary>) {
        match provider {
            Github => svc.github.set(GithubPullRequestsState {
                status: PullRequestsStatus::Ok,
                stale: false,
                fetched_at_unix: Some(READ_AT),
                authored: rows,
                review_requested: Vec::new(),
                withheld: 0,
            }),
            Bitbucket => svc.bitbucket.set(BitbucketPullRequestsState {
                status: PullRequestsStatus::Ok,
                stale: false,
                fetched_at_unix: Some(READ_AT),
                pull_requests: rows,
                skipped_workspaces: Vec::new(),
            }),
        }
    }

    /// A service with `provider` enabled and listing `rows`, beside a
    /// scripted transport whose clock reads `READ_AT`.
    fn serving(
        provider: PullRequestProvider,
        rows: Vec<PullRequestSummary>,
    ) -> (tempfile::TempDir, AppService, Arc<FakeIo>) {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        match provider {
            Github => svc.settings.set_github_enabled(true),
            Bitbucket => svc.settings.set_bitbucket_enabled(true),
        }
        .unwrap();
        list(&svc, provider, rows);
        (cfg, svc, FakeIo::new(READ_AT))
    }

    /// A service listing GitHub's `acme/api#42`, and that reference.
    fn serving_acme() -> (
        tempfile::TempDir,
        AppService,
        Arc<FakeIo>,
        PullRequestReference,
    ) {
        let (cfg, svc, io) = serving(Github, vec![listed_pull_request(Github, "acme/api", 42)]);
        (cfg, svc, io, pull_request(Github, "acme", "api", 42))
    }

    async fn ask(
        svc: &AppService,
        reference: &PullRequestReference,
        manual: bool,
        cached_only: bool,
        io: &Arc<FakeIo>,
    ) -> PullRequestDetailOutcome {
        svc.pull_request_detail_with(reference.clone(), manual, cached_only, io.clone())
            .await
    }

    fn detail(outcome: PullRequestDetailOutcome) -> PullRequestDetail {
        match outcome {
            PullRequestDetailOutcome::Detail { detail } => *detail,
            other => panic!("expected a detail, got {other:?}"),
        }
    }

    /// How many reads `io` has seen begin: a GitHub read begins with its
    /// query, the only POST.
    fn reads(io: &FakeIo) -> usize {
        io.requests()
            .iter()
            .filter(|request| request.starts_with("POST "))
            .count()
    }

    /// `pull-request-viewer`: *A pull request outside the snapshot spends
    /// nothing*, on either provider, a manual refresh included.
    #[tokio::test]
    async fn a_reference_in_no_snapshot_sends_nothing_and_answers_not_listed() {
        for provider in [Github, Bitbucket] {
            let (_cfg, svc, io) = serving(
                provider,
                vec![listed_pull_request(provider, "acme/api", 42)],
            );
            for unlisted in [
                pull_request(provider, "acme", "api", 43),
                pull_request(provider, "acme", "web", 42),
            ] {
                for manual in [false, true] {
                    assert_eq!(
                        ask(&svc, &unlisted, manual, false, &io).await,
                        PullRequestDetailOutcome::NotListed,
                        "{unlisted:?}"
                    );
                }
            }
            assert!(io.requests().is_empty(), "{provider:?}");
        }
    }

    /// `pull-request-viewer`: *A pull request that left the list keeps its
    /// last detail*.
    #[tokio::test]
    async fn a_pull_request_that_left_its_list_keeps_its_detail_marked_no_longer_listed() {
        let (_cfg, svc, io, acme) = serving_acme();
        let read = detail(ask(&svc, &acme, false, false, &io).await);
        assert!(!read.no_longer_listed);
        let sent = io.requests().len();

        list(&svc, Github, Vec::new());
        io.set_now(READ_AT + 3_600);
        let kept = detail(ask(&svc, &acme, true, false, &io).await);
        assert!(kept.no_longer_listed);
        assert_eq!(
            PullRequestDetail {
                no_longer_listed: false,
                ..kept
            },
            read
        );
        assert_eq!(io.requests().len(), sent, "no request");
    }

    /// `pull-request-viewer`: *A cache-only call answers whatever the
    /// entry's age*, and with nothing cached answers not cached.
    #[tokio::test]
    async fn a_cache_only_call_answers_a_five_minute_old_detail_and_sends_nothing() {
        let (_cfg, svc, io, acme) = serving_acme();
        assert_eq!(
            ask(&svc, &acme, false, true, &io).await,
            PullRequestDetailOutcome::NotCached
        );
        assert!(io.requests().is_empty());

        detail(ask(&svc, &acme, false, false, &io).await);
        let sent = io.requests().len();
        io.set_now(READ_AT + 300);
        for manual in [false, true] {
            let cached = detail(ask(&svc, &acme, manual, true, &io).await);
            assert_eq!(cached.read_at_unix, READ_AT);
            assert!(!cached.no_longer_listed);
        }
        assert_eq!(io.requests().len(), sent, "nothing sent");

        list(&svc, Github, Vec::new());
        assert!(detail(ask(&svc, &acme, false, true, &io).await).no_longer_listed);
        let elsewhere = pull_request(Github, "acme", "api", 7);
        assert_eq!(
            ask(&svc, &elsewhere, false, true, &io).await,
            PullRequestDetailOutcome::NotCached
        );
    }

    /// `pull-request-viewer`: *A listed pull request is read through its
    /// row*: `ACME/Api` asked of a row spelt `acme/api` sends requests naming
    /// `acme/api`, on either provider, and the detail is spelt as the row.
    #[tokio::test]
    async fn a_reference_spelt_otherwise_is_read_through_the_rows_spelling() {
        let (_cfg, svc, io) = serving(Github, vec![listed_pull_request(Github, "acme/api", 42)]);
        let read = detail(
            ask(
                &svc,
                &pull_request(Github, "ACME", "Api", 42),
                false,
                false,
                &io,
            )
            .await,
        );
        let requests = io.requests();
        assert_eq!(requests.len(), 2, "{requests:?}");
        let body = requests[0]
            .strip_prefix("POST https://api.github.com/graphql ")
            .expect("the query first");
        let body: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(
            body["variables"],
            serde_json::json!({ "owner": "acme", "name": "api", "number": 42 })
        );
        assert_eq!(
            requests[1],
            "GET https://api.github.com/repos/acme/api/pulls/42/files?per_page=50&page=1"
        );
        assert_eq!(
            (read.reference.owner.as_str(), read.reference.repo.as_str()),
            ("acme", "api")
        );

        let (_cfg, svc, io) = serving(
            Bitbucket,
            vec![listed_pull_request(Bitbucket, "acme/api", 7)],
        );
        let read = detail(
            ask(
                &svc,
                &pull_request(Bitbucket, "Acme", "API", 7),
                false,
                false,
                &io,
            )
            .await,
        );
        assert_eq!(
            io.requests()[0],
            "GET https://api.bitbucket.org/2.0/repositories/acme/api/pullrequests/7"
        );
        assert_eq!(io.requests().len(), 5);
        assert_eq!(
            (read.reference.owner.as_str(), read.reference.repo.as_str()),
            ("acme", "api")
        );
        assert_eq!(read.head_commit, fake::HEAD);
    }

    /// `pull-request-viewer`: *A disabled provider serves nothing*: its
    /// cached detail is dropped, and every reference is refused, cache-only
    /// calls and withheld files included, until it is enabled again with
    /// nothing cached.
    #[tokio::test]
    async fn a_disabled_provider_refuses_every_reference_cache_only_calls_included() {
        for provider in [Github, Bitbucket] {
            let (_cfg, svc, io) = serving(
                provider,
                vec![listed_pull_request(provider, "acme/api", 42)],
            );
            let acme = pull_request(provider, "acme", "api", 42);
            let read = detail(ask(&svc, &acme, false, false, &io).await);
            let path = read.files[0].new_path.clone().unwrap();
            let set_enabled = |enabled| match provider {
                Github => svc.set_github_enabled(enabled),
                Bitbucket => svc.set_bitbucket_enabled(enabled),
            };
            set_enabled(false).unwrap();
            let sent = io.requests().len();
            for reference in [acme.clone(), pull_request(provider, "acme", "api", 7)] {
                for (manual, cached_only) in
                    [(false, false), (true, false), (false, true), (true, true)]
                {
                    assert_eq!(
                        ask(&svc, &reference, manual, cached_only, &io).await,
                        PullRequestDetailOutcome::Refused,
                        "{reference:?}, manual {manual}, cached only {cached_only}"
                    );
                }
            }
            assert_eq!(
                svc.pull_request_file_with(&acme, &path, fake::HEAD, fake::BASE, io.clone())
                    .await,
                PullRequestFileOutcome::Failed {
                    reason: FileReadFailure::Refused,
                    until_unix: None,
                }
            );
            assert_eq!(io.requests().len(), sent, "nothing sent");

            set_enabled(true).unwrap();
            assert_eq!(
                ask(&svc, &acme, false, true, &io).await,
                PullRequestDetailOutcome::NotCached,
                "dropped when it was switched off"
            );
        }
    }

    /// `pull-request-viewer`: *A credential saved mid-read discards the
    /// read*: it sends no further request, caches and returns nothing, and
    /// BitBucket's cached details are dropped at once.
    #[tokio::test]
    async fn a_bitbucket_credential_saved_between_two_requests_ends_the_read() {
        let (_cfg, svc, io) = serving(
            Bitbucket,
            vec![
                listed_pull_request(Bitbucket, "acme/api", 7),
                listed_pull_request(Bitbucket, "acme/api", 8),
            ],
        );
        let (cached, reading) = (
            pull_request(Bitbucket, "acme", "api", 7),
            pull_request(Bitbucket, "acme", "api", 8),
        );
        detail(ask(&svc, &cached, false, false, &io).await);
        let sent = io.requests().len();
        let saver = svc.clone();
        io.before_request(move |number| {
            if number == sent + 1 {
                saver
                    .set_bitbucket_credentials("ada".to_string(), "new-token".to_string())
                    .unwrap();
            }
        });

        assert_eq!(
            ask(&svc, &reading, false, false, &io).await,
            PullRequestDetailOutcome::Transient
        );
        assert_eq!(io.requests().len(), sent + 1, "no further request");
        for reference in [&reading, &cached] {
            assert_eq!(
                ask(&svc, reference, false, true, &io).await,
                PullRequestDetailOutcome::NotCached,
                "{reference:?}"
            );
        }
    }

    /// `github-pull-requests`: *Disabling stops a detail read between
    /// requests*: it sends no further request, and is refused.
    #[tokio::test]
    async fn disabling_github_between_two_requests_refuses_the_read() {
        let (_cfg, svc, io, acme) = serving_acme();
        let disabler = svc.clone();
        io.before_request(move |number| {
            if number == 1 {
                disabler.set_github_enabled(false).unwrap();
            }
        });
        assert_eq!(
            ask(&svc, &acme, false, false, &io).await,
            PullRequestDetailOutcome::Refused
        );
        assert_eq!(io.requests().len(), 1);
    }

    /// A flag written straight to the settings, as the transports' commands
    /// still write it, stops a read between two requests too: the read
    /// checks the flag itself, not only the credential generation, which
    /// this write leaves where it was.
    #[tokio::test]
    async fn a_flag_switched_off_in_the_settings_mid_read_stops_it() {
        let (_cfg, svc, io, acme) = serving_acme();
        let settings = svc.settings.clone();
        io.before_request(move |number| {
            if number == 1 {
                settings.set_github_enabled(false).unwrap();
            }
        });
        assert_eq!(
            ask(&svc, &acme, false, false, &io).await,
            PullRequestDetailOutcome::Refused
        );
        assert_eq!(io.requests().len(), 1);
        assert_eq!(svc.pull_request_details.generation(Github), 0);
    }

    /// No credential is a credential problem, and nothing is sent.
    #[tokio::test]
    async fn without_a_credential_a_read_is_unauthenticated_and_sends_nothing() {
        for provider in [Github, Bitbucket] {
            let (_cfg, svc, io) = serving(
                provider,
                vec![listed_pull_request(provider, "acme/api", 42)],
            );
            io.withhold_credentials();
            let acme = pull_request(provider, "acme", "api", 42);
            assert_eq!(
                ask(&svc, &acme, false, false, &io).await,
                PullRequestDetailOutcome::Unauthenticated
            );
            assert!(io.requests().is_empty());
        }
    }

    /// `pull-request-viewer`: *A poller's rate limit holds back detail
    /// reads*: a deferred read sends nothing, names when a read becomes
    /// possible, and carries any cached detail; at the deadline it reads.
    #[tokio::test]
    async fn a_held_deadline_defers_a_read_and_carries_the_cached_detail() {
        let (_cfg, svc, io) = serving(
            Bitbucket,
            vec![listed_pull_request(Bitbucket, "acme/api", 7)],
        );
        let acme = pull_request(Bitbucket, "acme", "api", 7);
        svc.bitbucket_limits.rate_limited(Some(600), READ_AT);
        assert_eq!(
            ask(&svc, &acme, false, false, &io).await,
            PullRequestDetailOutcome::Deferred {
                until_unix: READ_AT + 600,
                detail: None,
            }
        );
        assert!(io.requests().is_empty());

        io.set_now(READ_AT + 600);
        let read = detail(ask(&svc, &acme, false, false, &io).await);
        svc.bitbucket_limits.rate_limited(Some(600), READ_AT + 600);
        io.set_now(READ_AT + 700);
        assert_eq!(
            ask(&svc, &acme, false, false, &io).await,
            PullRequestDetailOutcome::Deferred {
                until_unix: READ_AT + 1_200,
                detail: Some(Box::new(read)),
            }
        );
    }

    // ------------------------------------------------- the cache, through the service

    /// `pull-request-viewer`: *A quick reopen sends nothing*.
    #[tokio::test]
    async fn a_reopen_twenty_seconds_after_a_read_sends_nothing() {
        let (_cfg, svc, io, acme) = serving_acme();
        detail(ask(&svc, &acme, false, false, &io).await);
        io.set_now(READ_AT + 20);
        assert_eq!(
            detail(ask(&svc, &acme, false, false, &io).await).read_at_unix,
            READ_AT
        );
        assert_eq!(reads(&io), 1);
    }

    #[tokio::test]
    async fn an_entry_sixty_seconds_old_reads_again_and_one_fifty_nine_seconds_old_does_not() {
        let (_cfg, svc, io, acme) = serving_acme();
        detail(ask(&svc, &acme, false, false, &io).await);
        io.set_now(READ_AT + 59);
        detail(ask(&svc, &acme, false, false, &io).await);
        assert_eq!(reads(&io), 1);
        io.set_now(READ_AT + 60);
        assert_eq!(
            detail(ask(&svc, &acme, false, false, &io).await).read_at_unix,
            READ_AT + 60
        );
        assert_eq!(reads(&io), 2);
    }

    /// `pull-request-viewer`: *A changed row brings a read*.
    #[tokio::test]
    async fn a_changed_checks_signature_reads_again() {
        let (_cfg, svc, io, acme) = serving_acme();
        detail(ask(&svc, &acme, false, false, &io).await);
        let mut failing = listed_pull_request(Github, "acme/api", 42);
        failing.checks = Some(ChecksState::Failing);
        list(&svc, Github, vec![failing]);
        io.set_now(READ_AT + 1);
        detail(ask(&svc, &acme, false, false, &io).await);
        assert_eq!(reads(&io), 2);
    }

    /// `pull-request-viewer`: *Manual refreshes are bounded per pull
    /// request*.
    #[tokio::test]
    async fn manual_refreshes_ten_seconds_apart_send_one_read() {
        let (_cfg, svc, io, acme) = serving_acme();
        detail(ask(&svc, &acme, true, false, &io).await);
        io.set_now(READ_AT + 10);
        detail(ask(&svc, &acme, true, false, &io).await);
        assert_eq!(reads(&io), 1);
    }

    /// `pull-request-viewer`: *A manual read of a stale entry starts the
    /// bound*.
    #[tokio::test]
    async fn a_manual_refresh_of_a_seventy_second_old_entry_reads_and_one_ten_seconds_later_does_not(
    ) {
        let (_cfg, svc, io, acme) = serving_acme();
        detail(ask(&svc, &acme, false, false, &io).await);
        io.set_now(READ_AT + 70);
        detail(ask(&svc, &acme, true, false, &io).await);
        assert_eq!(reads(&io), 2);
        io.set_now(READ_AT + 80);
        detail(ask(&svc, &acme, true, false, &io).await);
        assert_eq!(reads(&io), 2);
    }

    #[tokio::test]
    async fn a_manual_refresh_thirty_seconds_after_the_last_manual_read_reads_and_twenty_nine_does_not(
    ) {
        let (_cfg, svc, io, acme) = serving_acme();
        detail(ask(&svc, &acme, true, false, &io).await);
        io.set_now(READ_AT + 29);
        detail(ask(&svc, &acme, true, false, &io).await);
        assert_eq!(reads(&io), 1);
        io.set_now(READ_AT + 30);
        detail(ask(&svc, &acme, true, false, &io).await);
        assert_eq!(reads(&io), 2);
    }

    /// `pull-request-viewer`: *The cache keeps the 32 most recently used*.
    #[tokio::test]
    async fn a_thirty_third_read_drops_the_least_recently_used_detail() {
        let rows = (1..=33)
            .map(|id| listed_pull_request(Github, "acme/api", id))
            .collect();
        let (_cfg, svc, io) = serving(Github, rows);
        let numbered = |number| pull_request(Github, "acme", "api", number);
        for number in 1..=32 {
            detail(ask(&svc, &numbered(number), false, false, &io).await);
        }
        for number in 1..=32 {
            detail(ask(&svc, &numbered(number), false, true, &io).await);
        }
        detail(ask(&svc, &numbered(33), false, false, &io).await);
        assert_eq!(
            ask(&svc, &numbered(1), false, true, &io).await,
            PullRequestDetailOutcome::NotCached
        );
        for number in 2..=33 {
            detail(ask(&svc, &numbered(number), false, true, &io).await);
        }
    }

    /// `pull-request-viewer`: *Two presentations share one read*.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn two_concurrent_asks_for_one_reference_send_one_read() {
        let (_cfg, svc, io, acme) = serving_acme();
        let (entered, entered_rx) = std::sync::mpsc::channel();
        let (release, released) = std::sync::mpsc::channel::<()>();
        io.before_request(move |number| {
            if number == 1 {
                entered.send(()).unwrap();
                released.recv().unwrap();
            }
        });
        let asking = || {
            let (svc, io, acme) = (svc.clone(), io.clone(), acme.clone());
            tokio::spawn(async move { ask(&svc, &acme, false, false, &io).await })
        };
        let first = asking();
        entered_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the first read sends");
        let second = asking();
        tokio::time::timeout(Duration::from_secs(10), async {
            while svc.pull_request_details.waiting(&acme.key()) < 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("both asks wait on the one read");
        release.send(()).unwrap();

        let joined = tokio::time::timeout(Duration::from_secs(10), async {
            (first.await.unwrap(), second.await.unwrap())
        })
        .await
        .expect("both asks are answered");
        assert_eq!(detail(joined.0), detail(joined.1));
        assert_eq!(reads(&io), 1);
    }

    // ------------------------------------------------- withheld files

    /// `pull-request-viewer`: *A withheld file is served from the cache*.
    #[tokio::test]
    async fn a_withheld_file_is_served_from_the_cache_with_no_request() {
        let (_cfg, svc, io, acme) = serving_acme();
        let read = detail(ask(&svc, &acme, false, false, &io).await);
        let big = read
            .files
            .iter()
            .find(|file| file.new_path.as_deref() == Some("big.rs"))
            .unwrap();
        assert_eq!(big.content, DiffContent::Withheld);
        let sent = io.requests().len();
        io.before_request(|_| panic!("a withheld file is served with no request"));

        let served = file_of(
            svc.pull_request_file(&acme, "big.rs", fake::HEAD, fake::BASE)
                .await,
        );
        let DiffContent::Hunks { hunks } = &served.content else {
            panic!("its hunks, got {:?}", served.content);
        };
        assert_eq!(hunks[0].lines.len(), 600);
        assert_eq!(served.additions, Some(600));
        assert_eq!(io.requests().len(), sent);
    }

    /// `pull-request-viewer`: *A withheld-file request after a push is
    /// refused*, as is one against another base or for an unknown path.
    #[tokio::test]
    async fn a_withheld_file_asked_at_another_commit_is_refused() {
        let (_cfg, svc, io, acme) = serving_acme();
        detail(ask(&svc, &acme, false, false, &io).await);
        let pushed = "3333333333333333333333333333333333333333";
        let uncached = pull_request(Github, "acme", "api", 7);
        for (reference, path, head, base) in [
            (&acme, "big.rs", pushed, fake::BASE),
            (&acme, "big.rs", fake::HEAD, pushed),
            (&acme, "missing.rs", fake::HEAD, fake::BASE),
            (&uncached, "big.rs", fake::HEAD, fake::BASE),
        ] {
            assert_eq!(
                svc.pull_request_file(reference, path, head, base).await,
                PullRequestFileOutcome::Changed,
                "{reference:?} {path} {head} {base}"
            );
        }
    }

    /// The file a load answers, which must be one.
    fn file_of(outcome: PullRequestFileOutcome) -> DiffFile {
        match outcome {
            PullRequestFileOutcome::File { file } => file,
            other => panic!("expected a file, got {other:?}"),
        }
    }

    /// The hunks of a file a load answers.
    fn hunks_of(outcome: PullRequestFileOutcome) -> Vec<openspec_core::Hunk> {
        match file_of(outcome).content {
            DiffContent::Hunks { hunks } => hunks,
            other => panic!("expected hunks, got {other:?}"),
        }
    }

    /// A service listing `acme/api#42`, whose GitHub files include `page.tsx`,
    /// modified and sent without its patch, read once so its detail is cached.
    async fn serving_a_patchless_page() -> (
        tempfile::TempDir,
        AppService,
        Arc<FakeIo>,
        PullRequestReference,
    ) {
        let (cfg, svc, io, acme) = serving_acme();
        io.push(|pushed| {
            fake::patchless(
                pushed,
                "page.tsx",
                "modified",
                Some(b"a\nb\nc\n"),
                Some(b"a\nB\nc\n"),
            )
        });
        let read = detail(ask(&svc, &acme, false, false, &io).await);
        let page = read
            .files
            .iter()
            .find(|file| file.new_path.as_deref() == Some("page.tsx"))
            .unwrap();
        assert_eq!(
            page.content,
            DiffContent::Withheld,
            "withheld, not too large"
        );
        (cfg, svc, io, acme)
    }

    async fn load(
        svc: &AppService,
        reference: &PullRequestReference,
        path: &str,
        io: &Arc<FakeIo>,
    ) -> PullRequestFileOutcome {
        svc.pull_request_file_with(reference, path, fake::HEAD, fake::BASE, io.clone())
            .await
    }

    /// `pull-request-viewer`: *A patchless file with lines is read on
    /// request*, *A file read is read once and kept*.
    #[tokio::test]
    async fn a_file_sent_without_its_patch_is_read_once_and_kept() {
        let (_cfg, svc, io, acme) = serving_a_patchless_page().await;
        let sent = io.requests().len();
        let hunks = hunks_of(load(&svc, &acme, "page.tsx", &io).await);
        let changed: Vec<_> = hunks[0]
            .lines
            .iter()
            .map(|line| (line.kind, line.text.as_str()))
            .collect();
        assert_eq!(
            changed,
            [
                (openspec_core::LineKind::Context, "a"),
                (openspec_core::LineKind::Removed, "b"),
                (openspec_core::LineKind::Added, "B"),
                (openspec_core::LineKind::Context, "c"),
            ]
        );
        let api = "https://api.github.com/repos/acme/api";
        assert_eq!(
            io.requests()[sent..],
            [
                format!(
                    "GET {api}/compare/{}...{}?per_page=1",
                    fake::BASE,
                    fake::HEAD
                ),
                format!("GET {api}/contents/page.tsx?ref={}", fake::MERGE_BASE),
                format!("GET {api}/contents/page.tsx?ref={}", fake::HEAD),
            ]
        );
        io.before_request(|_| panic!("a file read once is kept"));
        assert_eq!(hunks_of(load(&svc, &acme, "page.tsx", &io).await), hunks);
    }

    /// `pull-request-viewer`: *The merge base is read once per detail*, *An
    /// added file reads only its new version*.
    #[tokio::test]
    async fn a_second_file_sends_no_compare_and_an_added_one_reads_its_new_version_only() {
        let (_cfg, svc, io, acme) = serving_a_patchless_page().await;
        io.push(|pushed| fake::patchless(pushed, "new.tsx", "added", None, Some(b"x\n")));
        detail(ask(&svc, &acme, true, false, &io).await);
        hunks_of(load(&svc, &acme, "page.tsx", &io).await);
        let sent = io.requests().len();
        let hunks = hunks_of(load(&svc, &acme, "new.tsx", &io).await);
        assert_eq!(hunks[0].lines.len(), 1);
        assert_eq!(
            io.requests()[sent..],
            [format!(
                "GET https://api.github.com/repos/acme/api/contents/new.tsx?ref={}",
                fake::HEAD
            )]
        );
    }

    /// `pull-request-viewer`: *A rate-limited file read sets the REST
    /// deadline*; and while it holds, a load sends nothing.
    #[tokio::test]
    async fn a_rate_limited_version_defers_the_load_and_sets_the_rest_deadline() {
        let (_cfg, svc, io, acme) = serving_a_patchless_page().await;
        io.push(|pushed| pushed.contents_status = 429);
        let outcome = load(&svc, &acme, "page.tsx", &io).await;
        let PullRequestFileOutcome::Failed {
            reason: FileReadFailure::Deferred,
            until_unix: Some(until),
        } = outcome
        else {
            panic!("deferred, got {outcome:?}");
        };
        assert!(until > READ_AT);
        assert_eq!(svc.github_limits.deadlines().held_until(), until);
        let sent = io.requests().len();
        assert_eq!(load(&svc, &acme, "page.tsx", &io).await, outcome);
        assert_eq!(io.requests().len(), sent, "nothing sent while it holds");
    }

    /// `pull-request-viewer`: *A file read counts against the budget*.
    #[tokio::test]
    async fn a_spent_budget_defers_a_file_read_without_a_request() {
        use crate::pull_request_limits::{admit_or_fail, Admission};
        let (_cfg, svc, io, acme) = serving_a_patchless_page().await;
        let limits = &svc.github_limits;
        let Admission::Admitted(read) = admit_or_fail(limits, true, READ_AT) else {
            panic!("an idle provider admits a read");
        };
        while limits.spent(READ_AT) < limits.budget() {
            read.request(READ_AT).unwrap();
        }
        drop(read);
        let sent = io.requests().len();
        assert!(matches!(
            load(&svc, &acme, "page.tsx", &io).await,
            PullRequestFileOutcome::Failed {
                reason: FileReadFailure::Deferred,
                until_unix: Some(_),
            }
        ));
        assert_eq!(io.requests().len(), sent);
    }

    /// A version GitHub no longer has is unavailable, and nothing is kept, so
    /// a later load asks again.
    #[tokio::test]
    async fn a_missing_version_is_unavailable() {
        let (_cfg, svc, io, acme) = serving_a_patchless_page().await;
        io.push(|pushed| pushed.versions.clear());
        assert_eq!(
            load(&svc, &acme, "page.tsx", &io).await,
            PullRequestFileOutcome::Failed {
                reason: FileReadFailure::Unavailable,
                until_unix: None,
            }
        );
        let sent = io.requests().len();
        load(&svc, &acme, "page.tsx", &io).await;
        assert!(io.requests().len() > sent, "asked again");
    }

    /// `pull-request-viewer`: *Credential changes*, for a file read: a
    /// credential saved between its requests ends it, keeping nothing.
    #[tokio::test]
    async fn a_credential_saved_mid_file_read_keeps_nothing() {
        let (_cfg, svc, io, acme) = serving_a_patchless_page().await;
        let saver = svc.clone();
        let sent = io.requests().len();
        io.before_request(move |number| {
            if number == sent + 2 {
                saver.set_github_token("ghp_another".to_string()).unwrap();
            }
        });
        assert_eq!(
            load(&svc, &acme, "page.tsx", &io).await,
            PullRequestFileOutcome::Failed {
                reason: FileReadFailure::Transient,
                until_unix: None,
            }
        );
        assert_eq!(io.requests().len(), sent + 2, "nothing sent after the save");
        assert_eq!(
            load(&svc, &acme, "page.tsx", &io).await,
            PullRequestFileOutcome::Changed,
            "the save dropped the cached detail"
        );
    }

    /// `github-pull-requests`: *A pull request outside the snapshot is not
    /// read*, for a file read: a pull request that left its list keeps its
    /// cached detail, and its file read sends nothing.
    #[tokio::test]
    async fn a_file_read_goes_only_to_a_listed_pull_request() {
        let (_cfg, svc, io, acme) = serving_a_patchless_page().await;
        list(&svc, Github, Vec::new());
        let sent = io.requests().len();
        assert_eq!(
            load(&svc, &acme, "page.tsx", &io).await,
            PullRequestFileOutcome::Failed {
                reason: FileReadFailure::Unavailable,
                until_unix: None,
            }
        );
        assert_eq!(io.requests().len(), sent);
    }

    // ------------------------------------------------------------ image reads

    const PNG: &[u8] = include_bytes!("../../openspec-core/tests/fixtures/images/three-by-two.png");
    const GIF: &[u8] =
        include_bytes!("../../openspec-core/tests/fixtures/images/three-by-two-87a.gif");

    async fn see(
        svc: &AppService,
        reference: &PullRequestReference,
        path: &str,
        io: &Arc<FakeIo>,
    ) -> PullRequestImageOutcome {
        svc.pull_request_file_image_with(reference, path, fake::HEAD, fake::BASE, io.clone())
            .await
    }

    /// A service listing `provider`'s `acme/api#42`, whose files include
    /// `icons/app.png`, modified from a PNG to a GIF, and `icons/new.png`,
    /// added, read once so its detail is cached.
    async fn serving_images(
        provider: PullRequestProvider,
    ) -> (
        tempfile::TempDir,
        AppService,
        Arc<FakeIo>,
        PullRequestReference,
    ) {
        let (cfg, svc, io) = serving(
            provider,
            vec![listed_pull_request(provider, "acme/api", 42)],
        );
        io.push(|pushed| {
            fake::image(pushed, "icons/app.png", "modified", Some(PNG), Some(GIF));
            fake::image(pushed, "icons/new.png", "added", None, Some(PNG));
        });
        let acme = pull_request(provider, "acme", "api", 42);
        let read = detail(ask(&svc, &acme, false, false, &io).await);
        let paths: Vec<_> = read
            .files
            .iter()
            .filter_map(|f| f.new_path.as_deref())
            .collect();
        assert!(paths.contains(&"icons/app.png"), "{paths:?}");
        (cfg, svc, io, acme)
    }

    fn images(old: ImageSide, new: ImageSide) -> PullRequestImageOutcome {
        PullRequestImageOutcome::Images { old, new }
    }

    fn image_failed(reason: FileReadFailure) -> PullRequestImageOutcome {
        PullRequestImageOutcome::Failed {
            reason,
            until_unix: None,
        }
    }

    /// `pull-request-viewer`: *A GitHub image read sends at most three
    /// requests*, *The service keeps no bytes*: the compare and both contents
    /// GETs once, then only the contents GETs.
    #[tokio::test]
    async fn a_github_image_read_sends_three_requests_and_then_two() {
        let (_cfg, svc, io, acme) = serving_images(Github).await;
        let api = "https://api.github.com/repos/acme/api";
        let contents = |commit: &str| format!("GET {api}/contents/icons/app.png?ref={commit}");
        let sent = io.requests().len();
        let both = images(ImageSide::of(PNG), ImageSide::of(GIF));
        assert_eq!(see(&svc, &acme, "icons/app.png", &io).await, both);
        assert_eq!(
            io.requests()[sent..],
            [
                format!(
                    "GET {api}/compare/{}...{}?per_page=1",
                    fake::BASE,
                    fake::HEAD
                ),
                contents(fake::MERGE_BASE),
                contents(fake::HEAD),
            ]
        );
        let sent = io.requests().len();
        assert_eq!(see(&svc, &acme, "icons/app.png", &io).await, both);
        assert_eq!(
            io.requests()[sent..],
            [contents(fake::MERGE_BASE), contents(fake::HEAD)]
        );
    }

    /// `pull-request-viewer`: *A merge base already learned is reused*, by a
    /// file read of the same detail; and *An added image reads only its new
    /// version*.
    #[tokio::test]
    async fn an_image_read_reuses_a_file_reads_merge_base_and_an_added_one_reads_once() {
        let (_cfg, svc, io, acme) = serving_a_patchless_page().await;
        io.push(|pushed| fake::image(pushed, "icons/new.png", "added", None, Some(PNG)));
        detail(ask(&svc, &acme, true, false, &io).await);
        hunks_of(load(&svc, &acme, "page.tsx", &io).await);
        let sent = io.requests().len();
        assert_eq!(
            see(&svc, &acme, "icons/new.png", &io).await,
            images(ImageSide::Absent, ImageSide::of(PNG))
        );
        assert_eq!(
            io.requests()[sent..],
            [format!(
                "GET https://api.github.com/repos/acme/api/contents/icons/new.png?ref={}",
                fake::HEAD
            )]
        );
    }

    /// `pull-request-viewer`: *A BitBucket image read reads the merge base and
    /// both versions*, all to `api.bitbucket.org`, and then no `merge-base`.
    #[tokio::test]
    async fn a_bitbucket_image_read_reads_the_merge_base_and_both_versions() {
        let (_cfg, svc, io, acme) = serving_images(Bitbucket).await;
        let api = "https://api.bitbucket.org/2.0/repositories/acme/api";
        let src = |commit: &str| format!("GET {api}/src/{commit}/icons/app.png");
        let sent = io.requests().len();
        let both = images(ImageSide::of(PNG), ImageSide::of(GIF));
        assert_eq!(see(&svc, &acme, "icons/app.png", &io).await, both);
        assert_eq!(
            io.requests()[sent..],
            [
                format!("GET {api}/merge-base/{}..{}", fake::HEAD, fake::BASE),
                src(fake::MERGE_BASE),
                src(fake::HEAD),
            ]
        );
        let sent = io.requests().len();
        assert_eq!(see(&svc, &acme, "icons/app.png", &io).await, both);
        assert_eq!(
            io.requests()[sent..],
            [src(fake::MERGE_BASE), src(fake::HEAD)]
        );
    }

    /// `pull-request-viewer`: *A BitBucket redirect is not followed*: the read
    /// answers `redirected` and requests nothing more.
    #[tokio::test]
    async fn a_bitbucket_redirect_answers_redirected_and_requests_nothing_more() {
        let (_cfg, svc, io, acme) = serving_images(Bitbucket).await;
        io.push(|pushed| pushed.contents_status = 302);
        let sent = io.requests().len();
        assert_eq!(
            see(&svc, &acme, "icons/app.png", &io).await,
            image_failed(FileReadFailure::Redirected)
        );
        let requests = &io.requests()[sent..];
        assert_eq!(
            requests.len(),
            2,
            "the merge base and one version: {requests:?}"
        );
        assert!(requests[1].contains("/src/"), "{requests:?}");
    }

    /// Spends the hourly budget of `limits` through one admitted read, which
    /// then ends.
    fn spend_budget<D: crate::pull_request_limits::Deadlines>(
        limits: &crate::pull_request_limits::ProviderLimits<D>,
    ) {
        use crate::pull_request_limits::{admit_or_fail, Admission};
        let Admission::Admitted(read) = admit_or_fail(limits, true, READ_AT) else {
            panic!("an idle provider admits a read");
        };
        while limits.spent(READ_AT) < limits.budget() {
            read.request(READ_AT).unwrap();
        }
    }

    /// `pull-request-viewer`: *An image read counts against the budget*, on
    /// either provider.
    #[tokio::test]
    async fn a_spent_budget_defers_an_image_read_without_a_request() {
        for provider in [Github, Bitbucket] {
            let (_cfg, svc, io, acme) = serving_images(provider).await;
            match provider {
                Github => spend_budget(&svc.github_limits),
                Bitbucket => spend_budget(&svc.bitbucket_limits),
            }
            let sent = io.requests().len();
            let outcome = see(&svc, &acme, "icons/app.png", &io).await;
            assert!(
                matches!(
                    outcome,
                    PullRequestImageOutcome::Failed {
                        reason: FileReadFailure::Deferred,
                        until_unix: Some(_),
                    }
                ),
                "{provider:?}: {outcome:?}"
            );
            assert_eq!(io.requests().len(), sent, "{provider:?}");
        }
    }

    /// A 429 sets GitHub's REST deadline, or BitBucket's shared one, and
    /// defers the read until it.
    #[tokio::test]
    async fn a_rate_limited_version_sets_the_providers_deadline() {
        for provider in [Github, Bitbucket] {
            let (_cfg, svc, io, acme) = serving_images(provider).await;
            io.push(|pushed| pushed.contents_status = 429);
            let outcome = see(&svc, &acme, "icons/app.png", &io).await;
            let PullRequestImageOutcome::Failed {
                reason: FileReadFailure::Deferred,
                until_unix: Some(until),
            } = outcome
            else {
                panic!("{provider:?}: deferred, got {outcome:?}");
            };
            assert!(until > READ_AT, "{provider:?}");
            let held = match provider {
                Github => svc.github_limits.deadlines().held_until(),
                Bitbucket => svc.bitbucket_limits.deadlines().held_until(),
            };
            assert_eq!(held, until, "{provider:?}");
        }
    }

    /// A version the provider no longer has is unavailable, on either
    /// provider; and `pull-request-viewer`: *An image read after a push
    /// answers changed*, with no request.
    #[tokio::test]
    async fn a_missing_version_is_unavailable_and_another_head_is_changed() {
        for provider in [Github, Bitbucket] {
            let (_cfg, svc, io, acme) = serving_images(provider).await;
            io.push(|pushed| pushed.versions.clear());
            assert_eq!(
                see(&svc, &acme, "icons/app.png", &io).await,
                image_failed(FileReadFailure::Unavailable),
                "{provider:?}"
            );
            let sent = io.requests().len();
            let pushed = svc
                .pull_request_file_image_with(
                    &acme,
                    "icons/app.png",
                    "5555555555555555555555555555555555555555",
                    fake::BASE,
                    io.clone(),
                )
                .await;
            assert_eq!(pushed, PullRequestImageOutcome::Changed, "{provider:?}");
            assert_eq!(
                see(&svc, &acme, "icons/none.png", &io).await,
                PullRequestImageOutcome::Changed,
                "{provider:?}"
            );
            assert_eq!(io.requests().len(), sent, "{provider:?}");
        }
    }

    /// `pull-request-viewer`: *Credential changes*, for an image read: a
    /// credential saved between its requests ends it, keeping nothing.
    #[tokio::test]
    async fn a_credential_saved_mid_image_read_keeps_nothing() {
        let (_cfg, svc, io, acme) = serving_images(Github).await;
        let saver = svc.clone();
        let sent = io.requests().len();
        io.before_request(move |number| {
            if number == sent + 2 {
                saver.set_github_token("ghp_another".to_string()).unwrap();
            }
        });
        assert_eq!(
            see(&svc, &acme, "icons/app.png", &io).await,
            image_failed(FileReadFailure::Transient)
        );
        assert_eq!(io.requests().len(), sent + 2, "nothing sent after the save");
        assert_eq!(
            see(&svc, &acme, "icons/app.png", &io).await,
            PullRequestImageOutcome::Changed,
            "the save dropped the cached detail"
        );
    }

    /// `pull-request-viewer`: *The provider's word is not trusted*: HTML text
    /// is refused as not an image.
    #[tokio::test]
    async fn an_image_read_checks_each_version_by_its_bytes() {
        let (_cfg, svc, io, acme) = serving_images(Github).await;
        io.push(|pushed| {
            for (path, _, bytes) in &mut pushed.versions {
                if path == "icons/new.png" {
                    *bytes = b"<!DOCTYPE html><title>Moved</title>".to_vec();
                }
            }
        });
        assert_eq!(
            see(&svc, &acme, "icons/new.png", &io).await,
            images(
                ImageSide::Absent,
                ImageSide::Refused {
                    reason: openspec_core::ImageRefusal::NotImage,
                }
            )
        );
    }

    /// `pull-request-viewer`: *A disabled provider reads no image*; and an
    /// image read goes only to a listed pull request.
    #[tokio::test]
    async fn a_disabled_or_unlisted_provider_reads_no_image() {
        let (_cfg, svc, io, acme) = serving_images(Github).await;
        let sent = io.requests().len();
        list(&svc, Github, Vec::new());
        assert_eq!(
            see(&svc, &acme, "icons/app.png", &io).await,
            image_failed(FileReadFailure::Unavailable)
        );
        svc.settings.set_github_enabled(false).unwrap();
        assert_eq!(
            see(&svc, &acme, "icons/app.png", &io).await,
            image_failed(FileReadFailure::Refused)
        );
        assert_eq!(io.requests().len(), sent);
    }

    // ------------------------------------------------- the provider setters

    /// `github-pull-requests`: *A deadline survives switching the feature off
    /// and on*, through the service's setter: re-enabling inside a 20-minute
    /// GraphQL deadline publishes and announces `unavailable` before it
    /// returns. Enabling with no deadline publishes nothing.
    #[test]
    fn re_enabling_inside_a_deadline_publishes_unavailable_at_once() {
        use crate::github::{GithubRequest, RateLimit};

        let unavailable = |status, rows: usize, stale: bool| {
            status == PullRequestsStatus::Unavailable && rows == 0 && !stale
        };
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let mut events = svc.subscribe();
        svc.set_github_enabled(true).unwrap();
        svc.set_bitbucket_enabled(true).unwrap();
        assert_eq!(
            svc.github_pull_requests(),
            GithubPullRequestsState::disabled()
        );
        assert!(events.try_recv().is_err(), "no deadline, nothing published");

        let deadline = RateLimit {
            delay: Some(1_200),
            secondary: false,
        };
        svc.github_limits
            .rate_limited(GithubRequest::Query, deadline, now_unix());
        svc.bitbucket_limits.rate_limited(Some(1_200), now_unix());
        svc.set_github_enabled(false).unwrap();
        svc.set_bitbucket_enabled(false).unwrap();
        svc.set_github_enabled(true).unwrap();
        let github = svc.github_pull_requests();
        assert!(unavailable(
            github.status,
            github.rows().count(),
            github.stale
        ));
        assert!(matches!(
            events.try_recv(),
            Ok(CacheEvent::GithubPullRequestsUpdated)
        ));
        svc.set_bitbucket_enabled(true).unwrap();
        let bitbucket = svc.bitbucket_pull_requests();
        assert!(unavailable(
            bitbucket.status,
            bitbucket.pull_requests.len(),
            bitbucket.stale
        ));
        assert!(matches!(
            events.try_recv(),
            Ok(CacheEvent::BitbucketPullRequestsUpdated)
        ));
    }

    /// `pull-request-viewer`: *Provider Enabled Flags Stay Current*: each
    /// flag write raises exactly one notice naming the provider and the flag
    /// as written, an unchanged one included; a saved credential raises
    /// none.
    #[test]
    fn each_flag_write_raises_one_notice_naming_the_provider_and_its_flag() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let mut notices = svc.subscribe_notices();
        svc.set_github_enabled(true).unwrap();
        svc.set_github_enabled(true).unwrap();
        svc.set_bitbucket_enabled(false).unwrap();
        svc.set_github_token("ghp_new".to_string()).unwrap();
        svc.set_bitbucket_credentials("ada".to_string(), "ATBB-new".to_string())
            .unwrap();
        svc.set_github_enabled(false).unwrap();
        let mut heard = Vec::new();
        while let Ok(notice) = notices.try_recv() {
            heard.push(notice);
        }
        let changed = |provider, enabled| {
            ServiceNotice::PullRequestProviderChanged(PullRequestProviderChangedPayload {
                provider,
                enabled,
            })
        };
        assert_eq!(
            heard,
            [
                changed(Github, true),
                changed(Github, true),
                changed(Bitbucket, false),
                changed(Github, false),
            ]
        );
        assert!(svc.settings.github_config_view().token_set);
    }

    /// `pull-request-viewer`: *Toggling the provider resets nothing*,
    /// through the service's own setters.
    #[test]
    fn the_service_setters_keep_the_deadlines_and_the_spent_budget() {
        use crate::github::{GithubRequest, RateLimit};
        use crate::pull_request_limits::{admit_or_fail, Admission};

        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let Admission::Admitted(read) = admit_or_fail(&svc.bitbucket_limits, true, READ_AT) else {
            panic!("an idle provider admits a read");
        };
        for _ in 0..svc.bitbucket_limits.budget() {
            read.request(READ_AT).unwrap();
        }
        drop(read);
        svc.bitbucket_limits.rate_limited(Some(600), READ_AT);
        let limit = RateLimit {
            delay: Some(900),
            secondary: true,
        };
        svc.github_limits
            .rate_limited(GithubRequest::Files, limit, READ_AT);
        let limits = |svc: &AppService| {
            (
                svc.bitbucket_limits.deadlines(),
                svc.bitbucket_limits.spent(READ_AT),
                svc.github_limits.deadlines(),
            )
        };
        let before = limits(&svc);

        for enabled in [true, false, true] {
            svc.set_bitbucket_enabled(enabled).unwrap();
            svc.set_github_enabled(enabled).unwrap();
        }
        svc.set_bitbucket_credentials("ada".to_string(), "ATBB-new".to_string())
            .unwrap();
        svc.set_github_token("ghp_new".to_string()).unwrap();
        assert_eq!(limits(&svc), before);
    }

    /// Disabling a provider or saving its credential drops its cached details
    /// at once and advances its credential generation; enabling it does
    /// neither, and the other provider is untouched.
    #[tokio::test]
    async fn disabling_or_saving_a_credential_drops_the_cache_and_advances_the_generation() {
        let (_cfg, svc, io) = serving(Github, vec![listed_pull_request(Github, "acme/api", 42)]);
        svc.settings.set_bitbucket_enabled(true).unwrap();
        list(
            &svc,
            Bitbucket,
            vec![listed_pull_request(Bitbucket, "acme/api", 7)],
        );
        let (on_github, on_bitbucket) = (
            pull_request(Github, "acme", "api", 42),
            pull_request(Bitbucket, "acme", "api", 7),
        );
        let cached = |reference: &PullRequestReference| {
            let (svc, io, reference) = (svc.clone(), io.clone(), reference.clone());
            async move {
                matches!(
                    ask(&svc, &reference, false, true, &io).await,
                    PullRequestDetailOutcome::Detail { .. }
                )
            }
        };
        let generation = |provider| svc.pull_request_details.generation(provider);
        detail(ask(&svc, &on_github, false, false, &io).await);
        detail(ask(&svc, &on_bitbucket, false, false, &io).await);

        svc.set_github_enabled(true).unwrap();
        assert!(cached(&on_github).await, "enabling drops nothing");
        assert_eq!(generation(Github), 0);

        svc.set_github_token("ghp_new".to_string()).unwrap();
        assert!(!cached(&on_github).await);
        assert!(cached(&on_bitbucket).await);
        assert_eq!((generation(Github), generation(Bitbucket)), (1, 0));

        detail(ask(&svc, &on_github, false, false, &io).await);
        svc.set_github_enabled(false).unwrap();
        svc.set_github_enabled(true).unwrap();
        assert!(!cached(&on_github).await);
        assert_eq!(generation(Github), 2);

        svc.set_bitbucket_credentials("ada".to_string(), "ATBB-new".to_string())
            .unwrap();
        assert!(!cached(&on_bitbucket).await);
        assert_eq!(generation(Bitbucket), 1);
        detail(ask(&svc, &on_bitbucket, false, false, &io).await);
        svc.set_bitbucket_enabled(false).unwrap();
        svc.set_bitbucket_enabled(true).unwrap();
        assert!(!cached(&on_bitbucket).await);
        assert_eq!(generation(Bitbucket), 2);
    }

    // ------------------------------------------------- review progress

    use crate::review_progress::{FileReviewProgress, FileReviewState};

    /// The head commit a scripted push moves to.
    const PUSHED: &str = "3333333333333333333333333333333333333333";

    /// The progress of `path` in `reference`'s review progress.
    fn progress_of(
        svc: &AppService,
        reference: &PullRequestReference,
        path: &str,
    ) -> FileReviewProgress {
        svc.review_progress(reference)
            .unwrap()
            .files
            .into_iter()
            .find(|file| file.path == path)
            .unwrap_or_else(|| panic!("{path} is a file of the detail"))
    }

    fn state_of(svc: &AppService, reference: &PullRequestReference, path: &str) -> FileReviewState {
        progress_of(svc, reference, path).state
    }

    /// Every notice heard so far.
    fn heard(notices: &mut broadcast::Receiver<ServiceNotice>) -> Vec<ServiceNotice> {
        let mut heard = Vec::new();
        while let Ok(notice) = notices.try_recv() {
            heard.push(notice);
        }
        heard
    }

    fn review_store_of(cfg: &tempfile::TempDir) -> PathBuf {
        cfg.path().join("review-progress.json")
    }

    /// Reads the detail again after a push: past the freshness rule, as the
    /// row a push updates would make it.
    async fn read_after_push(
        svc: &AppService,
        reference: &PullRequestReference,
        io: &Arc<FakeIo>,
        at: u64,
    ) -> PullRequestDetail {
        io.set_now(at);
        detail(ask(svc, reference, false, false, io).await)
    }

    /// `pull-request-viewer`: *Marking needs a cached detail*, *A mark
    /// against a different head is refused* and *A path outside the detail
    /// is refused*: each refusal, of a mark or an unmark, stores nothing and
    /// announces nothing.
    #[tokio::test]
    async fn every_refused_mark_stores_nothing_and_announces_nothing() {
        let (cfg, svc, io, acme) = serving_acme();
        let mut notices = svc.subscribe_notices();
        assert!(svc
            .set_file_viewed(&acme, "src/lib.rs", true, fake::HEAD, fake::BASE)
            .is_err());
        detail(ask(&svc, &acme, false, false, &io).await);
        for (path, head, base) in [
            ("src/lib.rs", PUSHED, fake::BASE),
            ("src/lib.rs", fake::HEAD, PUSHED),
            ("missing.rs", fake::HEAD, fake::BASE),
        ] {
            for viewed in [true, false] {
                assert!(
                    svc.set_file_viewed(&acme, path, viewed, head, base)
                        .is_err(),
                    "{path} at {head}..{base}, viewed {viewed}"
                );
            }
        }
        let uncached = pull_request(Github, "acme", "api", 7);
        assert!(svc
            .set_file_viewed(&uncached, "src/lib.rs", true, fake::HEAD, fake::BASE)
            .is_err());
        assert!(!review_store_of(&cfg).exists(), "nothing stored");
        assert_eq!(heard(&mut notices), []);
        let progress = svc.review_progress(&acme).unwrap();
        assert_eq!((progress.viewed, progress.total), (0, 2));
    }

    /// `pull-request-viewer`: *Unmarking creates nothing*.
    #[tokio::test]
    async fn unmarking_a_pull_request_with_no_entry_creates_none() {
        let (cfg, svc, io, acme) = serving_acme();
        detail(ask(&svc, &acme, false, false, &io).await);
        let mut notices = svc.subscribe_notices();
        svc.set_file_viewed(&acme, "src/lib.rs", false, fake::HEAD, fake::BASE)
            .unwrap();
        assert!(!review_store_of(&cfg).exists());
        assert_eq!(heard(&mut notices), [], "nothing stored, nothing announced");
        assert_eq!(
            state_of(&svc, &acme, "src/lib.rs"),
            FileReviewState::Unviewed
        );
    }

    /// `pull-request-viewer`: *A mark in one window reaches every view*: each
    /// stored mark and unmark raises one notice, heard by every subscriber,
    /// carrying the pull request as its detail spells it, whatever spelling
    /// marked it.
    #[tokio::test]
    async fn each_stored_mark_or_unmark_raises_one_notice_for_every_view() {
        let (_cfg, svc, io, acme) = serving_acme();
        let spelt = pull_request(Github, "ACME", "Api", 42);
        detail(ask(&svc, &spelt, false, false, &io).await);
        let (mut window, mut tab) = (svc.subscribe_notices(), svc.subscribe_notices());
        svc.set_file_viewed(&spelt, "src/lib.rs", true, fake::HEAD, fake::BASE)
            .unwrap();
        svc.set_file_viewed(&acme, "big.rs", true, fake::HEAD, fake::BASE)
            .unwrap();
        svc.set_file_viewed(&spelt, "src/lib.rs", false, fake::HEAD, fake::BASE)
            .unwrap();
        for heard in [heard(&mut window), heard(&mut tab)] {
            assert_eq!(heard.len(), 3);
            for notice in heard {
                let ServiceNotice::ReviewProgressChanged(reference) = notice else {
                    panic!("a review notice, got {notice:?}");
                };
                assert_eq!(
                    (reference.owner.as_str(), reference.repo.as_str()),
                    ("acme", "api")
                );
                assert_eq!(reference, acme);
            }
        }
        let progress = svc.review_progress(&spelt).unwrap();
        assert_eq!((progress.viewed, progress.total), (1, 2));
        assert_eq!(progress.last_marked_head.as_deref(), Some(fake::HEAD));
        assert_eq!(state_of(&svc, &acme, "big.rs"), FileReviewState::Viewed);
        assert_eq!(
            state_of(&svc, &acme, "src/lib.rs"),
            FileReviewState::Unviewed
        );
    }

    /// `pull-request-viewer`: *A mark survives a restart and never leaves
    /// the machine*: a fresh service over the same directory answers nothing
    /// until the detail is read again, then shows the mark; marking sent no
    /// request.
    #[tokio::test]
    async fn a_mark_survives_a_fresh_service_once_the_detail_is_cached_again() {
        let (cfg, svc, io, acme) = serving_acme();
        detail(ask(&svc, &acme, false, false, &io).await);
        let sent = io.requests().len();
        svc.set_file_viewed(&acme, "big.rs", true, fake::HEAD, fake::BASE)
            .unwrap();
        assert_eq!(io.requests().len(), sent, "the mark goes to no host");
        drop(svc);

        let restarted = AppService::bootstrap(cfg.path().to_path_buf());
        assert!(restarted.settings.github_enabled(), "the flag persisted");
        list(
            &restarted,
            Github,
            vec![listed_pull_request(Github, "acme/api", 42)],
        );
        assert!(
            restarted.review_progress(&acme).is_err(),
            "nothing cached yet"
        );
        let io = FakeIo::new(READ_AT + 3_600);
        detail(ask(&restarted, &acme, false, false, &io).await);
        assert_eq!(
            state_of(&restarted, &acme, "big.rs"),
            FileReviewState::Viewed
        );
        assert_eq!(
            state_of(&restarted, &acme, "src/lib.rs"),
            FileReviewState::Unviewed
        );
    }

    /// `pull-request-viewer`: *Progress is answered only while the provider
    /// is enabled*. Switched off in the settings alone, which leaves the
    /// cached detail where it was, the flag itself refuses progress and
    /// marks; switched on again, the earlier mark is there.
    #[tokio::test]
    async fn a_disabled_provider_refuses_progress_and_marks() {
        let (_cfg, svc, io, acme) = serving_acme();
        detail(ask(&svc, &acme, false, false, &io).await);
        svc.set_file_viewed(&acme, "big.rs", true, fake::HEAD, fake::BASE)
            .unwrap();
        svc.settings.set_github_enabled(false).unwrap();
        assert!(svc.review_progress(&acme).is_err());
        assert!(svc
            .set_file_viewed(&acme, "src/lib.rs", true, fake::HEAD, fake::BASE)
            .is_err());
        svc.settings.set_github_enabled(true).unwrap();
        let progress = svc.review_progress(&acme).unwrap();
        assert_eq!(progress.viewed, 1);
        assert_eq!(
            state_of(&svc, &acme, "src/lib.rs"),
            FileReviewState::Unviewed
        );
    }

    /// `pull-request-viewer`: *A push that changes a file flags it* and *A
    /// push that leaves a file alone keeps its mark*, a withheld file
    /// included; marked again at the new head, the count dates from there.
    #[tokio::test]
    async fn a_push_flags_the_file_it_changed_and_keeps_the_mark_it_left_alone() {
        let (_cfg, svc, io, acme) = serving_acme();
        let read = detail(ask(&svc, &acme, false, false, &io).await);
        assert_eq!(read.files[1].new_path.as_deref(), Some("big.rs"));
        assert_eq!(read.files[1].content, DiffContent::Withheld);
        for path in ["src/lib.rs", "big.rs"] {
            svc.set_file_viewed(&acme, path, true, fake::HEAD, fake::BASE)
                .unwrap();
        }
        io.push(|pushed| {
            pushed.head = PUSHED.to_string();
            pushed.github_files[0]["patch"] = serde_json::json!(fake::added(2));
            pushed.github_files[0]["additions"] = serde_json::json!(2);
        });
        assert_eq!(
            read_after_push(&svc, &acme, &io, READ_AT + 60)
                .await
                .head_commit,
            PUSHED
        );
        let progress = svc.review_progress(&acme).unwrap();
        assert_eq!(
            progress
                .files
                .iter()
                .map(|file| (file.path.as_str(), file.state))
                .collect::<Vec<_>>(),
            [
                ("src/lib.rs", FileReviewState::ChangedSinceViewed),
                ("big.rs", FileReviewState::Viewed),
            ]
        );
        assert_eq!(
            (
                progress.viewed,
                progress.changed_since_viewed,
                progress.total
            ),
            (1, 1, 2)
        );
        assert_eq!(progress.last_marked_head.as_deref(), Some(fake::HEAD));

        svc.set_file_viewed(&acme, "src/lib.rs", true, PUSHED, fake::BASE)
            .unwrap();
        let progress = svc.review_progress(&acme).unwrap();
        assert_eq!((progress.viewed, progress.changed_since_viewed), (2, 0));
        assert_eq!(progress.last_marked_head.as_deref(), Some(PUSHED));
    }

    /// A patch of two hunks far apart, the second adding `second`.
    fn two_hunks(second: &str) -> String {
        format!("@@ -1,2 +1,3 @@\n a\n+b\n c\n@@ -40,2 +41,3 @@\n x\n+{second}\n z")
    }

    /// `pull-request-viewer`: *A hunk mark needs the file's hunks* and *A
    /// hunk past the last is refused*, beside every refusal a file mark has:
    /// none stores anything or announces anything.
    #[tokio::test]
    async fn every_refused_hunk_mark_stores_nothing_and_announces_nothing() {
        let (cfg, svc, _io, acme) = serving_a_patchless_page().await;
        let mut notices = svc.subscribe_notices();
        let uncached = pull_request(Github, "acme", "api", 7);
        assert!(svc
            .set_hunk_viewed(&uncached, "src/lib.rs", 0, true, fake::HEAD, fake::BASE)
            .is_err());
        for (path, hunk, head, base) in [
            ("src/lib.rs", 0, PUSHED, fake::BASE),
            ("src/lib.rs", 0, fake::HEAD, PUSHED),
            ("missing.rs", 0, fake::HEAD, fake::BASE),
            ("src/lib.rs", 1, fake::HEAD, fake::BASE),
            ("page.tsx", 0, fake::HEAD, fake::BASE),
        ] {
            for viewed in [true, false] {
                assert!(
                    svc.set_hunk_viewed(&acme, path, hunk, viewed, head, base)
                        .is_err(),
                    "{path} hunk {hunk} at {head}..{base}, viewed {viewed}"
                );
            }
        }
        svc.settings.set_github_enabled(false).unwrap();
        assert!(svc
            .set_hunk_viewed(&acme, "src/lib.rs", 0, true, fake::HEAD, fake::BASE)
            .is_err());
        assert!(!review_store_of(&cfg).exists(), "nothing stored");
        assert_eq!(heard(&mut notices), []);
    }

    /// `pull-request-viewer`: *Marking a hunk leaves the others alone*,
    /// *Marking the last hunk marks the file*, *A hunk mark reaches every
    /// view*: hunk by hunk the file becomes viewed, an unmark makes it partly
    /// viewed again, and each stored write is announced.
    #[tokio::test]
    async fn hunk_marks_build_up_to_the_file_and_each_is_announced() {
        let (_cfg, svc, io, acme) = serving_acme();
        io.push(|pushed| pushed.github_files[0]["patch"] = serde_json::json!(two_hunks("y")));
        detail(ask(&svc, &acme, false, false, &io).await);
        let mut notices = svc.subscribe_notices();
        svc.set_hunk_viewed(&acme, "src/lib.rs", 1, true, fake::HEAD, fake::BASE)
            .unwrap();
        let partly = progress_of(&svc, &acme, "src/lib.rs");
        assert_eq!(
            (partly.state, partly.hunks),
            (FileReviewState::PartlyViewed, Some(vec![false, true]))
        );
        let progress = svc.review_progress(&acme).unwrap();
        assert_eq!((progress.viewed, progress.changed_since_viewed), (0, 0));
        assert_eq!(progress.last_marked_head.as_deref(), Some(fake::HEAD));
        assert_eq!(
            (progress.head_commit.as_str(), progress.base_commit.as_str()),
            (fake::HEAD, fake::BASE)
        );

        svc.set_hunk_viewed(&acme, "src/lib.rs", 0, true, fake::HEAD, fake::BASE)
            .unwrap();
        assert_eq!(state_of(&svc, &acme, "src/lib.rs"), FileReviewState::Viewed);
        svc.set_hunk_viewed(&acme, "src/lib.rs", 0, false, fake::HEAD, fake::BASE)
            .unwrap();
        let unmarked = progress_of(&svc, &acme, "src/lib.rs");
        assert_eq!(
            (unmarked.state, unmarked.hunks),
            (FileReviewState::PartlyViewed, Some(vec![false, true]))
        );
        let heard = heard(&mut notices);
        assert_eq!(heard.len(), 3);
        assert!(heard
            .iter()
            .all(|notice| *notice == ServiceNotice::ReviewProgressChanged(acme.clone())));
    }

    /// `pull-request-viewer`: *A push that changes a file flags it*, with one
    /// hunk to review and the other still viewed; marking that hunk at the
    /// new head views the file, and *Unmarking a file clears its hunks*.
    #[tokio::test]
    async fn a_push_reopens_only_the_hunk_it_changed() {
        let (_cfg, svc, io, acme) = serving_acme();
        io.push(|pushed| pushed.github_files[0]["patch"] = serde_json::json!(two_hunks("y")));
        detail(ask(&svc, &acme, false, false, &io).await);
        svc.set_file_viewed(&acme, "src/lib.rs", true, fake::HEAD, fake::BASE)
            .unwrap();
        assert_eq!(
            progress_of(&svc, &acme, "src/lib.rs").hunks,
            Some(vec![true, true])
        );

        io.push(|pushed| {
            pushed.head = PUSHED.to_string();
            pushed.github_files[0]["patch"] = serde_json::json!(two_hunks("Y"));
        });
        read_after_push(&svc, &acme, &io, READ_AT + 60).await;
        let changed = progress_of(&svc, &acme, "src/lib.rs");
        assert_eq!(
            (changed.state, changed.hunks),
            (FileReviewState::ChangedSinceViewed, Some(vec![true, false]))
        );
        assert_eq!(svc.review_progress(&acme).unwrap().changed_since_viewed, 1);

        svc.set_hunk_viewed(&acme, "src/lib.rs", 1, true, PUSHED, fake::BASE)
            .unwrap();
        assert_eq!(state_of(&svc, &acme, "src/lib.rs"), FileReviewState::Viewed);
        svc.set_file_viewed(&acme, "src/lib.rs", false, PUSHED, fake::BASE)
            .unwrap();
        let cleared = progress_of(&svc, &acme, "src/lib.rs");
        assert_eq!(
            (cleared.state, cleared.hunks),
            (FileReviewState::Unviewed, Some(vec![false, false]))
        );
    }

    /// `pull-request-viewer`: *A loaded file's hunks become known*: a file
    /// GitHub sent without its patch has no hunk states, and refuses a hunk
    /// mark, until its file read is kept; then its one hunk marks it whole.
    #[tokio::test]
    async fn a_patchless_files_hunks_are_marked_once_its_file_read_is_kept() {
        let (_cfg, svc, io, acme) = serving_a_patchless_page().await;
        assert_eq!(progress_of(&svc, &acme, "page.tsx").hunks, None);
        assert!(svc
            .set_hunk_viewed(&acme, "page.tsx", 0, true, fake::HEAD, fake::BASE)
            .is_err());
        hunks_of(load(&svc, &acme, "page.tsx", &io).await);
        assert_eq!(
            progress_of(&svc, &acme, "page.tsx").hunks,
            Some(vec![false])
        );
        svc.set_hunk_viewed(&acme, "page.tsx", 0, true, fake::HEAD, fake::BASE)
            .unwrap();
        assert_eq!(state_of(&svc, &acme, "page.tsx"), FileReviewState::Viewed);
    }

    /// One file of BitBucket's diff, `menu.txt`, whose added line ends in
    /// `byte`.
    fn menu_diff(byte: u8) -> Vec<u8> {
        let mut diff =
            b"diff --git a/menu.txt b/menu.txt\n--- a/menu.txt\n+++ b/menu.txt\n@@ -1 +1 @@\n-cafe\n+caf"
                .to_vec();
        diff.extend([byte, b'\n']);
        diff
    }

    /// A BitBucket file with text is keyed by its bytes as received, not by
    /// the head: a push that leaves them keeps its mark, and one that turns
    /// a Latin-1 `é` (0xE9) into `è` (0xE8), alike once decoded, flags it.
    #[tokio::test]
    async fn a_bitbucket_patch_changed_in_one_latin1_byte_flags_its_file() {
        let (_cfg, svc, io) = serving(
            Bitbucket,
            vec![listed_pull_request(Bitbucket, "acme/api", 7)],
        );
        let acme = pull_request(Bitbucket, "acme", "api", 7);
        io.push(|pushed| pushed.bitbucket_diff = menu_diff(0xe9));
        detail(ask(&svc, &acme, false, false, &io).await);
        svc.set_file_viewed(&acme, "menu.txt", true, fake::HEAD, fake::BASE)
            .unwrap();

        io.push(|pushed| pushed.head = PUSHED.to_string());
        read_after_push(&svc, &acme, &io, READ_AT + 60).await;
        let kept = progress_of(&svc, &acme, "menu.txt");
        assert_eq!(kept.state, FileReviewState::Viewed);
        assert!(!kept.keyed_by_head);

        io.push(|pushed| pushed.bitbucket_diff = menu_diff(0xe8));
        read_after_push(&svc, &acme, &io, READ_AT + 120).await;
        assert_eq!(
            state_of(&svc, &acme, "menu.txt"),
            FileReviewState::ChangedSinceViewed
        );
    }

    /// `pull-request-viewer`: *A file keyed by the head commit says why it
    /// changed*: a BitBucket binary file, viewed at one head, is changed
    /// since viewed after a push, and keyed by the head.
    #[tokio::test]
    async fn a_bitbucket_binary_file_is_flagged_by_a_push_and_says_why() {
        let (_cfg, svc, io) = serving(
            Bitbucket,
            vec![listed_pull_request(Bitbucket, "acme/api", 7)],
        );
        let acme = pull_request(Bitbucket, "acme", "api", 7);
        io.push(|pushed| {
            pushed.bitbucket_diff = b"diff --git a/logo.png b/logo.png\nindex 1111111..2222222 100644\nBinary files a/logo.png and b/logo.png differ\n".to_vec();
        });
        detail(ask(&svc, &acme, false, false, &io).await);
        svc.set_file_viewed(&acme, "logo.png", true, fake::HEAD, fake::BASE)
            .unwrap();
        let viewed = progress_of(&svc, &acme, "logo.png");
        assert_eq!(viewed.state, FileReviewState::Viewed);
        assert!(viewed.keyed_by_head);

        io.push(|pushed| pushed.head = PUSHED.to_string());
        read_after_push(&svc, &acme, &io, READ_AT + 60).await;
        let pushed = progress_of(&svc, &acme, "logo.png");
        assert_eq!(pushed.state, FileReviewState::ChangedSinceViewed);
        assert!(pushed.keyed_by_head, "the view says why");
    }

    /// `pull-request-viewer`: *A file without a patch is keyed by GitHub's
    /// blob*: a push that leaves its blob alone keeps the mark, and one that
    /// changes the blob flags it.
    #[tokio::test]
    async fn a_github_file_without_a_patch_keeps_its_mark_while_its_blob_holds() {
        let (_cfg, svc, io, acme) = serving_acme();
        let logo = |sha: &str| {
            serde_json::json!([{ "filename": "logo.png", "status": "modified",
                "additions": 0, "deletions": 0, "sha": sha }])
        };
        io.push(|pushed| pushed.github_files = logo("blob-1"));
        detail(ask(&svc, &acme, false, false, &io).await);
        svc.set_file_viewed(&acme, "logo.png", true, fake::HEAD, fake::BASE)
            .unwrap();

        io.push(|pushed| pushed.head = PUSHED.to_string());
        read_after_push(&svc, &acme, &io, READ_AT + 60).await;
        let kept = progress_of(&svc, &acme, "logo.png");
        assert_eq!(kept.state, FileReviewState::Viewed);
        assert!(!kept.keyed_by_head);

        io.push(|pushed| pushed.github_files = logo("blob-2"));
        read_after_push(&svc, &acme, &io, READ_AT + 120).await;
        assert_eq!(
            state_of(&svc, &acme, "logo.png"),
            FileReviewState::ChangedSinceViewed
        );
    }

    /// `pull-request-viewer`: *Pruning waits for the lists*, *A disabled
    /// provider's entries are kept*, through the subscriber `bootstrap`
    /// installs. Nothing is pruned at load; once GitHub's list arrives, the
    /// 120-day entry it no longer lists goes, announced, while the one it
    /// lists and the disabled BitBucket's stay.
    #[tokio::test]
    async fn review_progress_is_pruned_once_the_lists_arrive_and_never_at_load() {
        let cfg = tempfile::tempdir().unwrap();
        let old = now_unix() - 120 * 24 * 60 * 60;
        let entry =
            serde_json::json!({ "lastMarkedHead": fake::HEAD, "files": {}, "touchedAt": old });
        let stored = serde_json::json!({
            "github/acme/api/42": entry,
            "github/acme/api/7": entry,
            "bitbucket/acme/api/9": entry,
        });
        std::fs::write(review_store_of(&cfg), stored.to_string()).unwrap();
        let names = || {
            let raw: serde_json::Value =
                serde_json::from_slice(&std::fs::read(review_store_of(&cfg)).unwrap()).unwrap();
            let mut names: Vec<String> = raw.as_object().unwrap().keys().cloned().collect();
            names.sort();
            names
        };

        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        svc.settings.set_github_enabled(true).unwrap();
        let mut notices = svc.subscribe_notices();
        assert_eq!(names().len(), 3, "nothing pruned at load");

        list(
            &svc,
            Github,
            vec![listed_pull_request(Github, "acme/api", 7)],
        );
        svc.watcher.emit(CacheEvent::GithubPullRequestsUpdated);
        let pruned = tokio::time::timeout(Duration::from_secs(10), notices.recv())
            .await
            .expect("pruned within the bound")
            .unwrap();
        assert_eq!(
            pruned,
            ServiceNotice::ReviewProgressChanged(pull_request(Github, "acme", "api", 42))
        );
        assert_eq!(names(), ["bitbucket/acme/api/9", "github/acme/api/7"]);
    }

    // ------------------------------------------------- the link opener

    /// `pull-request-viewer`: *An external link opens in the system browser*,
    /// *Other schemes are refused* and *Without a cached detail nothing
    /// opens*: the href comes back only for a cached detail of an enabled
    /// provider, and only over `http` or `https` with a host; nothing is
    /// ever fetched.
    #[tokio::test]
    async fn a_link_opens_only_for_a_cached_detail_and_only_over_http_or_https() {
        let (_cfg, svc, io, acme) = serving_acme();
        let href = "https://example.com/docs#setup";
        assert!(
            svc.open_pull_request_link(&acme, href).is_err(),
            "nothing cached"
        );
        detail(ask(&svc, &acme, false, false, &io).await);
        let sent = io.requests().len();
        assert_eq!(
            svc.open_pull_request_link(&acme, href),
            Ok(href.to_string())
        );
        for refused in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "data:text/html,<p>",
            "mailto:ada@example.com",
            "vscode://file/etc/passwd",
            "https:///hostless",
            "docs/setup.md",
        ] {
            assert!(
                svc.open_pull_request_link(&acme, refused).is_err(),
                "{refused}"
            );
        }
        let uncached = pull_request(Github, "acme", "api", 7);
        assert!(svc.open_pull_request_link(&uncached, href).is_err());
        assert_eq!(io.requests().len(), sent, "the href is never fetched");

        svc.settings.set_github_enabled(false).unwrap();
        assert!(
            svc.open_pull_request_link(&acme, href).is_err(),
            "the provider is off"
        );
    }
}
