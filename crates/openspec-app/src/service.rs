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
    build_backfill, change_lifecycle_checked, commit_activity_with_authors, commit_diff,
    commit_files, commit_log, commit_log_authored, compute_dashboard, compute_garden,
    compute_leaderboard, compute_progress, day_axis, detect_candidate_identities, event_is_me,
    git_common_dir, is_me, is_object_id, layout_commit_graph, list_archived_summaries, local_today,
    markdown_files, normalized_key, parse_artifact_status, parse_proposal_title,
    task_completion_history, today_str, walk_markdown_files, worktree_list, ActivityLog,
    ArchivedChangeSummary, ArtifactStatus, Author, CacheEvent, ChangeData, ChangeLifecycle,
    CommitActivityCache, CommitFile, CommitGraph, DashboardData, DocumentKey, DocumentWatcher,
    IdentityConfig, LifecycleCache, PaletteColor, Person, PresentationKey, RegisteredWorkspace,
    RepoId, WatcherManager, WorkspaceGarden, WorkspaceOrigin, WorkspacePresentationStore,
    WorkspaceRegistry, WorkspaceView,
};
use serde::Serialize;
use tokio::sync::broadcast;

use crate::chatgpt_quota::{ChatGptQuotaHandle, ChatGptQuotaState};
use crate::quota::{ClaudeQuotaState, QuotaHandle};
use crate::settings::SettingsStore;

/// How many days the Dashboard's git-mined activity + throughput window spans.
pub const DASHBOARD_ACTIVITY_WINDOW_DAYS: u64 = 14;
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
/// configuration, the contributor roster (named people other than "me"), and the
/// distinct git identities detected across registered workspaces, offered as
/// alias suggestions. Lives here (rather than behind a `#[tauri::command]`) so
/// every frontend — the shell, the terminal UI, and the web server — returns the
/// identical shape.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInfo {
    pub config: IdentityConfig,
    pub people: Vec<Person>,
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
    /// Per-repository cache of mined [`openspec_core::ChangeLifecycle`] data
    /// (see `openspec_core::LifecycleCache`), so `dashboard()` and the
    /// first-launch backfill mine a repository's history at most once per
    /// change to it rather than once per fetch. Kept correct by
    /// [`Self::spawn_lifecycle_cache_invalidator`], installed by
    /// [`Self::bootstrap`], which invalidates a repository's entry on
    /// `CacheEvent::GraphChanged`.
    pub lifecycle_cache: LifecycleCache,
    /// Per-repository cache of the year-long commit walk backing the heatmap,
    /// streak and leaderboard. Invalidated by the same `GraphChanged` signal as
    /// `lifecycle_cache`, in the same subscriber.
    pub commit_activity_cache: CommitActivityCache,
}

/// Move a corrupt `workspaces.json` aside to the first free
/// `workspaces.json.corrupt-<n>` sibling, so its data stays recoverable and a
/// later save does not overwrite it. Best-effort: if the file is gone or the
/// rename fails, leave things as-is.
fn preserve_corrupt_config(path: &std::path::Path) {
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
            lifecycle_cache: LifecycleCache::new(),
            commit_activity_cache: CommitActivityCache::new(),
        };

        // Keep `lifecycle_cache` correct as git history moves. Installed here
        // — before any frontend calls `populate`/`spawn_backfill` — so no
        // early `GraphChanged` (e.g. from `spawn_backfill` on first launch)
        // can land before the subscriber is listening.
        svc.spawn_lifecycle_cache_invalidator();

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

    /// Subscribe to the watcher's `CacheEvent` stream. Callers re-read the
    /// aggregated view on each event rather than caching it themselves.
    pub fn subscribe(&self) -> broadcast::Receiver<CacheEvent> {
        self.watcher.subscribe()
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
    /// (exists, is a directory, has an `openspec/` subdirectory) and discovers
    /// sibling worktrees of the same git repo; this method starts a watcher for
    /// each newly-tracked folder, installs per-repo monitors, and refreshes the
    /// aggregated view so a subsequent `workspace_views`/`list_workspaces` (and
    /// the watcher's `CacheEvent` subscribers) reflect the addition immediately.
    ///
    /// This is the single orchestration both frontends call — the Tauri command
    /// and the terminal UI — so watcher lifecycle stays owned by the service.
    pub async fn add_workspace(&self, path: PathBuf) -> Result<RegisteredWorkspace, String> {
        // `register` returns the user-registered entry plus any auto-discovered
        // sibling worktrees of the same git repo. The first element is always
        // the user-registered folder.
        let added = {
            let mut reg = self.registry.lock().map_err(|e| e.to_string())?;
            reg.register(path).map_err(|e| e.to_string())?
        };
        let primary = added
            .first()
            .cloned()
            .ok_or_else(|| "register returned no folders".to_string())?;

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

    /// The developer-identity payload (saved config + contributor roster +
    /// detected candidate identities) for the Settings identity section.
    pub fn identity_info(&self) -> Result<IdentityInfo, String> {
        let config = self.settings.identity();
        let people = self.settings.people();
        let folders: Vec<PathBuf> = {
            let reg = self.registry.lock().map_err(|e| e.to_string())?;
            reg.entries().iter().map(|e| e.folder.uri.clone()).collect()
        };
        let candidates = detect_candidate_identities(&folders);
        Ok(IdentityInfo {
            config,
            people,
            candidates,
        })
    }

    /// The distinct non-"me" authors observed across registered repositories
    /// within the dashboard window, deduped by normalised key in first-seen
    /// order — the candidate pool the roster UI offers for naming and merging.
    /// Authors that resolve as the developer, or that have no usable key, are
    /// excluded. Read-only: shells `git log` per repo, bounded by the window.
    pub fn observed_authors(&self) -> Vec<Author> {
        let identity = self.settings.identity();
        let since = format!("{DASHBOARD_HEATMAP_WINDOW_DAYS} days ago");
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut out: Vec<Author> = Vec::new();
        for view in self.watcher.workspace_views() {
            if let WorkspaceView::Repo(r) = view {
                let repo_id = RepoId(r.repo_id.clone());
                for (_, author) in commit_activity_with_authors(&repo_id, &since) {
                    if is_me(&author, &identity) {
                        continue;
                    }
                    if let Some(key) = normalized_key(&author) {
                        if seen.insert(key) {
                            out.push(author);
                        }
                    }
                }
            }
        }
        out
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

    /// The files a commit changed, with per-file added/removed counts.
    pub async fn commit_detail(
        &self,
        repo_id: PathBuf,
        sha: String,
    ) -> Result<Vec<CommitFile>, String> {
        if !is_object_id(&sha) {
            return Err("invalid commit reference".to_string());
        }
        let repo = self.ensure_registered_repo(&repo_id)?;
        tokio::task::spawn_blocking(move || commit_files(&repo, &sha))
            .await
            .map_err(|e| e.to_string())
    }

    /// The raw unified diff for one file of a commit.
    pub async fn commit_diff(
        &self,
        repo_id: PathBuf,
        sha: String,
        path: String,
    ) -> Result<String, String> {
        if !is_object_id(&sha) {
            return Err("invalid commit reference".to_string());
        }
        let repo = self.ensure_registered_repo(&repo_id)?;
        tokio::task::spawn_blocking(move || commit_diff(&repo, &sha, &path))
            .await
            .map_err(|e| e.to_string())
    }

    /// The commit garden: one stylized plant per top-level entry, grown from
    /// today's commits. Unconditional — no setting gates it.
    pub async fn commit_garden(&self) -> Result<Vec<WorkspaceGarden>, String> {
        let identity = self.settings.identity();
        let people = self.settings.people();
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
            views
                .iter()
                .map(|view| match view {
                    WorkspaceView::Repo(r) => {
                        let commits =
                            commit_log_authored(&RepoId(r.repo_id.clone()), GARDEN_COMMIT_LIMIT);
                        let mut plant = compute_garden(commits, today, &identity, &people);
                        plant.label = r.display_name.clone().unwrap_or_else(|| r.name.clone());
                        plant
                    }
                    WorkspaceView::Flat {
                        workspace,
                        display_name,
                        ..
                    } => WorkspaceGarden {
                        label: display_name
                            .clone()
                            .unwrap_or_else(|| workspace.name.clone()),
                        dormant: true,
                        commits: Vec::new(),
                        edges: Vec::new(),
                        lane_count: 0,
                    },
                })
                .collect()
        })
        .await
        .map_err(|e| e.to_string())
    }

    /// Aggregate the global Dashboard payload: cross-workspace analytics plus
    /// the developer's progress layer and the per-author leaderboard. The git
    /// reads run off the async runtime.
    pub async fn dashboard(&self) -> Result<DashboardData, String> {
        let identity = self.settings.identity();
        let people = self.settings.people();
        let mut views = self.watcher.workspace_views();
        {
            let store = self.presentation.lock().map_err(|e| e.to_string())?;
            join_presentation(&mut views, &store);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let heatmap_since = format!("{DASHBOARD_HEATMAP_WINDOW_DAYS} days ago");
        // Inclusive lower bound of the activity chart, as a local calendar day.
        // The frontend renders exactly this many local days ending today, so
        // bounding by the axis (rather than git's `--since` approxidate, which
        // is a rolling 14x24h window and therefore time-of-day dependent) makes
        // the backend's buckets and the rendered axis agree by construction.
        let activity_cutoff = day_axis(DASHBOARD_ACTIVITY_WINDOW_DAYS as u32)
            .first()
            .cloned()
            .unwrap_or_default();

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
            // section needs (the 371-day heatmap). The 14-day activity chart is
            // derived by filtering these same rows: its `%aI` output is a strict
            // subset of this call's, so spawning a second `git log` per repo for
            // it walked the same history twice.
            let mut commit_pairs: Vec<(String, Author)> = Vec::new();
            let mut activity_by_repo: std::collections::HashMap<PathBuf, Vec<String>> =
                std::collections::HashMap::new();
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
                    activity_by_repo.insert(
                        r.repo_id.clone(),
                        activity_dates_since(&pairs, &activity_cutoff),
                    );
                    commit_pairs.extend(pairs);
                }
            }

            let mut data = compute_dashboard(
                &views,
                now,
                DASHBOARD_ACTIVITY_WINDOW_DAYS,
                &today,
                |repo| activity_by_repo.get(&repo.0).cloned().unwrap_or_default(),
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

            let commit_authors: Vec<Author> = commit_pairs.iter().map(|(_, a)| a.clone()).collect();
            data.leaderboard =
                compute_leaderboard(&all_achievements, &commit_authors, &identity, &people);

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
/// The activity chart's commit dates, narrowed out of the wider heatmap walk.
///
/// `pairs` is the year-long `(author-date, author)` walk; `cutoff` is the first
/// local calendar day the chart renders. Comparing the `YYYY-MM-DD` prefixes
/// lexicographically is an ordering comparison on ISO-8601 dates, and the
/// length guard skips any malformed row rather than panicking on a short slice.
///
/// Extracted as a pure function because it is the whole reason the Dashboard no
/// longer spawns a second `git log` per repository: its boundary behaviour is
/// the thing worth pinning.
fn activity_dates_since(pairs: &[(String, Author)], cutoff: &str) -> Vec<String> {
    pairs
        .iter()
        .filter(|(iso, _)| iso.len() >= 10 && iso[..10] >= *cutoff)
        .map(|(iso, _)| iso.clone())
        .collect()
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(iso: &str) -> (String, Author) {
        (
            iso.to_string(),
            Author {
                name: Some("t".into()),
                email: Some("t@t".into()),
            },
        )
    }

    /// The activity chart's window is now carved out of the wider heatmap walk
    /// rather than mined by its own `git log`, so the cutoff comparison is the
    /// only thing standing between the chart and the wrong span. Fixed strings,
    /// no clock: the boundary cases are pinned exactly, including the inclusive
    /// `>=` edge that a `>` would silently drop.
    #[test]
    fn activity_dates_since_is_inclusive_of_the_cutoff_day() {
        let pairs = vec![
            pair("2026-08-12T09:00:00+01:00"), // after the cutoff
            pair("2026-07-30T23:59:59+01:00"), // exactly ON the cutoff day
            pair("2026-07-29T23:59:59+01:00"), // one day before — excluded
            pair("2025-01-01T00:00:00+01:00"), // far outside
        ];
        let kept = activity_dates_since(&pairs, "2026-07-30");
        assert_eq!(
            kept,
            vec![
                "2026-08-12T09:00:00+01:00".to_string(),
                "2026-07-30T23:59:59+01:00".to_string(),
            ],
            "the cutoff day itself is inside the window; the day before is not"
        );
    }

    /// A malformed row is skipped, not panicked on: the filter slices `[..10]`,
    /// so the length guard is load-bearing rather than defensive decoration.
    #[test]
    fn activity_dates_since_skips_rows_too_short_to_carry_a_date() {
        let pairs = vec![pair("2026-08-12T09:00:00+01:00"), pair("oops"), pair("")];
        let kept = activity_dates_since(&pairs, "2026-07-30");
        assert_eq!(kept, vec!["2026-08-12T09:00:00+01:00".to_string()]);
    }

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
                .commit_diff(repo.clone(), bad.to_string(), "f".to_string())
                .await
                .unwrap_err();
            assert_eq!(e, "invalid commit reference", "commit_diff({bad:?})");
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
            svc.commit_diff(outsider_repo, OBJ.to_string(), "f".to_string())
                .await
                .unwrap_err(),
            "unregistered repository"
        );

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
            svc.commit_diff(registered_repo, OBJ.to_string(), "f".to_string())
                .await
                .err()
                .as_deref(),
            Some("unregistered repository")
        );
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
        // repo. Registered through the registry directly rather than through
        // `add_workspace`: the sibling is auto-discovered by the first
        // registration, and `add_workspace` cannot currently promote an
        // already-discovered worktree — `register` returns only *newly*
        // discovered folders on that path, and `add_workspace` treats the empty
        // list as an error. That is a pre-existing defect on the promotion path,
        // unrelated to disabling; this test routes around it rather than
        // asserting it.
        let main_ws = svc.add_workspace(main.clone()).await.unwrap();
        svc.registry
            .lock()
            .unwrap()
            .register(sibling.clone())
            .unwrap();
        svc.watcher.sync_repos();
        svc.watcher.aggregate_and_emit();
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
}
