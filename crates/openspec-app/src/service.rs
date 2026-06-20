//! The headless application service shared by both frontends.
//!
//! `AppService` owns the stateful handles (registry, settings, presentation,
//! activity log, watcher) and exposes the read surface both the Tauri shell and
//! the terminal frontend render. The orchestration that previously lived behind
//! `#[tauri::command]` in the shell — most importantly the ~270-line dashboard
//! assembly — lives here as plain methods, so it is callable in-process by
//! either frontend and reachable from `cargo test`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use openspec_core::{
    build_backfill, change_lifecycle, commit_activity, commit_activity_with_authors, commit_diff,
    commit_files, commit_log, commit_log_authored, compute_dashboard, compute_garden,
    compute_leaderboard, compute_progress, compute_season, current_season_index, day_axis,
    detect_candidate_identities, event_is_me, in_season, is_me, layout_commit_graph,
    list_archived_summaries, local_today, normalized_key, parse_artifact_status,
    parse_proposal_title, season_baseline, season_info, season_recap, task_completion_history,
    today_str, treatment_from_id, unlocked_treatments, worktree_list, Achievement, AchievementKind,
    ActivityLog, ArchivedChangeSummary, ArtifactStatus, Author, CacheEvent, ChangeData,
    ChangeLifecycle, CommitFile, CommitGraph, DashboardData, IdentityConfig, PaletteColor,
    Person, PresentationKey, RegisteredWorkspace, RepoId, TreatmentDescriptor, WatcherManager,
    WorkspaceGarden, WorkspaceOrigin, WorkspacePresentationStore, WorkspaceRegistry, WorkspaceView,
};
use serde::Serialize;
use tokio::sync::broadcast;

use crate::quota::{ClaudeQuotaState, QuotaHandle};
use crate::settings::SettingsStore;

/// How many days the Dashboard's git-mined activity + throughput window spans.
pub const DASHBOARD_ACTIVITY_WINDOW_DAYS: u64 = 14;
/// The gamified heatmap / streak window — 53 weeks of local calendar days, so
/// the contribution grid reads as a full-year GitHub-style band. Bounded.
pub const DASHBOARD_HEATMAP_WINDOW_DAYS: u64 = 371;
/// How many commits per repo the garden reads before filtering to today.
const GARDEN_COMMIT_LIMIT: usize = 500;
/// Bounded window for the one-time git backfill of historical achievements.
/// Matches the heatmap window so a year of contribution cells has data to show.
const BACKFILL_SINCE: &str = "54 weeks ago";
/// Debounce for the filesystem watcher.
const WATCH_DEBOUNCE_MS: u64 = 200;

/// The treatment **wardrobe** for Settings: every finish unlocked across all
/// seasons (rebuilt from the persisted locker ids, newest first) plus the
/// equipped one.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreatmentLocker {
    pub unlocked: Vec<TreatmentDescriptor>,
    pub equipped: Option<TreatmentDescriptor>,
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
    /// Latest opt-in Claude usage-quota snapshot, written by the quota poller
    /// and read by both frontends. `Disabled` until the poller runs with the
    /// feature enabled.
    pub quota: QuotaHandle,
}

impl AppService {
    /// Build the service against an application config directory: load the
    /// persisted stores, construct the watcher, and seed first-run defaults
    /// (developer identity and the season-recap bookmark). Does **not** start
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

        let registry = WorkspaceRegistry::load(workspaces_path.clone())
            .unwrap_or_else(|_| WorkspaceRegistry::new(workspaces_path));
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

        // Seed the season rollover bookmark on first launch to the current
        // season, so the imminent git backfill (which reconstructs months of
        // history) does not fire a recap for every past month.
        if settings.season_state().last_recapped_season_index.is_none() {
            let _ = settings.set_last_recapped_season(current_season_index());
        }

        let watcher = WatcherManager::with_registry(
            std::time::Duration::from_millis(WATCH_DEBOUNCE_MS),
            Some(shared_registry.clone()),
        );
        #[cfg(target_os = "windows")]
        watcher.set_poll_interval(std::time::Duration::from_secs(
            settings.wsl_poll_interval_secs(),
        ));

        let activity = Arc::new(ActivityLog::load(activity_path));
        watcher.set_activity_log(activity.clone());

        Self {
            registry: shared_registry,
            settings,
            presentation: shared_presentation,
            activity,
            watcher,
            quota: QuotaHandle::new(),
        }
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
        self.watcher.aggregate_and_emit();
    }

    /// Seed the activity log from git history on first launch (when the log is
    /// empty), once per distinct repository. Bounded git scans, so it runs on a
    /// background thread; when done it nudges each repo's graph so an open
    /// Dashboard refetches the now-seeded log.
    pub fn spawn_backfill(&self) {
        let registry = self.registry.clone();
        let activity = self.activity.clone();
        let watcher = self.watcher.clone();
        std::thread::spawn(move || {
            backfill_activity(&registry, &activity);
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
    pub fn workspace_views(&self) -> Vec<WorkspaceView> {
        let mut views = self.watcher.workspace_views();
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
                let (dn, c) = store.lookup(&key);
                ws.display_name = dn;
                ws.color = c;
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
        self.watcher.aggregate_and_emit();

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
            let (dn, c) = store.lookup(&key);
            ws.display_name = dn;
            ws.color = c;
            ws.repo_id = repo_path;
        }
        Ok(ws)
    }

    /// Unregister a workspace and tear down the watchers it implied, cascading to
    /// the discovered worktrees the registry drops with it and cleaning up any
    /// now-orphaned presentation entries. Returns whether anything was removed.
    pub async fn remove_workspace(&self, path: PathBuf) -> Result<bool, String> {
        // Snapshot the entry's repo association before unregister so we can
        // decide which presentation keys to cascade-clean afterwards. Use the
        // canonicalised path because that's what the registry stores; fall back
        // to the input when canonicalisation fails (e.g. the directory was
        // deleted), which still unregisters but skips presentation cleanup.
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
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
            reg.unregister(&path).map_err(|e| e.to_string())?
        };
        let any_removed = !removed.is_empty();

        // Tear down watchers for every removed path (the user-registered one
        // plus any cascaded discovered worktrees), drop now-empty repo
        // monitors, and refresh the aggregated view once from the settled state.
        for p in &removed {
            self.watcher.remove_workspace(p);
        }
        self.watcher.sync_repos();
        self.watcher.aggregate_and_emit();

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

    /// One workspace's archived changes (newest-first), for the Archive browser.
    pub fn list_archived(&self, workspace: &Path) -> Result<Vec<ArchivedChangeSummary>, String> {
        list_archived_summaries(workspace).map_err(|e| e.to_string())
    }

    /// Which artifacts an archived change has on disk. `dir_name` is one archive
    /// directory entry (`<YYYY-MM-DD>-<id>`), never a path.
    pub fn archived_artifact_status(
        &self,
        workspace: &Path,
        dir_name: &str,
    ) -> Result<ArtifactStatus, String> {
        if dir_name.contains('/') || dir_name.contains('\\') || dir_name.contains("..") {
            return Err("invalid archive directory name".into());
        }
        let change_dir = workspace
            .join("openspec")
            .join("changes")
            .join("archive")
            .join(dir_name);
        Ok(parse_artifact_status(&change_dir))
    }

    /// The treatment wardrobe (unlocked finishes + the equipped one). Reads only
    /// persisted season state — no activity log or git mining.
    pub fn treatment_locker(&self) -> TreatmentLocker {
        let st = self.settings.season_state();
        let unlocked = st
            .unlocked
            .iter()
            .rev()
            .filter_map(|id| treatment_from_id(id))
            .collect();
        let equipped = st.equipped.as_deref().and_then(treatment_from_id);
        TreatmentLocker { unlocked, equipped }
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
    ) -> Result<String, String> {
        let resolved = resolve_artifact_path(workspace, change_id, artifact_kind, capability)?;
        tokio::fs::read_to_string(&resolved)
            .await
            .map_err(|e| e.to_string())
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
        tokio::task::spawn_blocking(move || {
            let repo = RepoId(repo_id);
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
        tokio::task::spawn_blocking(move || commit_files(&RepoId(repo_id), &sha))
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
        tokio::task::spawn_blocking(move || commit_diff(&RepoId(repo_id), &sha, &path))
            .await
            .map_err(|e| e.to_string())
    }

    /// The commit garden: one stylized plant per top-level entry, grown from
    /// today's commits. Empty when gamification is disabled.
    pub async fn commit_garden(&self) -> Result<Vec<WorkspaceGarden>, String> {
        if !self.settings.gamification_enabled() {
            return Ok(Vec::new());
        }
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

    /// Aggregate the global Dashboard payload: cross-workspace analytics plus,
    /// when gamification is enabled, the developer's progress, season standing,
    /// leaderboards, treatment unlocks, and any rollover recap. The git reads
    /// run off the async runtime.
    pub async fn dashboard(&self) -> Result<DashboardData, String> {
        let gamification = self.settings.gamification_enabled();
        let identity = self.settings.identity();
        let people = self.settings.people();
        let settings_arc = self.settings.clone();
        let mut views = self.watcher.workspace_views();
        {
            let store = self.presentation.lock().map_err(|e| e.to_string())?;
            join_presentation(&mut views, &store);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let since = format!("{DASHBOARD_ACTIVITY_WINDOW_DAYS} days ago");
        let heatmap_since = format!("{DASHBOARD_HEATMAP_WINDOW_DAYS} days ago");

        let day_axis = day_axis(DASHBOARD_HEATMAP_WINDOW_DAYS as u32);
        let today = today_str();
        let log = self.activity.clone();

        tokio::task::spawn_blocking(move || {
            let mut lifecycles: std::collections::HashMap<PathBuf, Vec<ChangeLifecycle>> =
                std::collections::HashMap::new();
            for view in &views {
                if let WorkspaceView::Repo(r) = view {
                    let repo_id = RepoId(r.repo_id.clone());
                    let lcs = change_lifecycle(&repo_id);
                    log.reconcile_lifecycle(&r.main_worktree, &lcs);
                    lifecycles.insert(r.repo_id.clone(), lcs);
                }
            }

            let mut data = compute_dashboard(
                &views,
                now,
                DASHBOARD_ACTIVITY_WINDOW_DAYS,
                &today,
                |repo| commit_activity(repo, &since),
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

            if !gamification {
                return data;
            }

            let mut commit_pairs: Vec<(String, Author)> = Vec::new();
            for view in &views {
                if let WorkspaceView::Repo(r) = view {
                    let repo_id = RepoId(r.repo_id.clone());
                    commit_pairs.extend(commit_activity_with_authors(&repo_id, &heatmap_since));
                }
            }

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

            let base_progress =
                compute_progress(&scoped_achievements, &commit_days, &day_axis, &today);

            let season_index = current_season_index();
            let info = season_info(season_index);
            let season_ym = format!("{:04}-{:02}", info.year, info.month);

            let season_anchor = format!("{:04}-{:02}-01", info.year, info.month);
            let baseline = season_baseline(
                &scoped_achievements,
                &commit_days,
                &day_axis,
                &season_anchor,
            );

            data.progress = base_progress;
            data.progress.in_flight = scoped_in_flight(&views, &all_achievements, &identity);

            let season_events: Vec<_> = all_achievements
                .iter()
                .filter(|e| in_season(season_index, e.timestamp) && event_is_me(e, &identity))
                .cloned()
                .collect();
            let season_commits = commit_pairs
                .iter()
                .filter(|(iso, a)| iso.starts_with(&season_ym) && is_me(a, &identity))
                .count() as u32;
            let totals = log.totals();
            let standing = compute_season(
                season_index,
                &season_events,
                season_commits,
                &baseline,
                &totals,
            );

            let crossed: Vec<String> = unlocked_treatments(season_index, &standing.ladder)
                .iter()
                .map(|t| t.id.clone())
                .collect();
            let _ = settings_arc.unlock_treatments(crossed);
            data.locker = unlocked_treatments(season_index, &standing.ladder);
            let sstate = settings_arc.season_state();
            data.equipped = sstate.equipped.as_deref().and_then(treatment_from_id);
            data.season = Some(standing);

            let season_all_events: Vec<_> = all_achievements
                .iter()
                .filter(|e| in_season(season_index, e.timestamp))
                .cloned()
                .collect();
            let season_commit_authors: Vec<Author> = commit_pairs
                .iter()
                .filter(|(iso, _)| iso.starts_with(&season_ym))
                .map(|(_, a)| a.clone())
                .collect();
            data.season_leaderboard = compute_leaderboard(
                &season_all_events,
                &season_commit_authors,
                &identity,
                &people,
            );

            match sstate.last_recapped_season_index {
                None => {
                    let _ = settings_arc.set_last_recapped_season(season_index);
                }
                Some(last) if last < season_index => {
                    let prev = season_index - 1;
                    let pinfo = season_info(prev);
                    let pym = format!("{:04}-{:02}", pinfo.year, pinfo.month);
                    let pevents: Vec<_> = all_achievements
                        .iter()
                        .filter(|e| in_season(prev, e.timestamp) && event_is_me(e, &identity))
                        .cloned()
                        .collect();
                    let pcommits = commit_pairs
                        .iter()
                        .filter(|(iso, a)| iso.starts_with(&pym) && is_me(a, &identity))
                        .count() as u32;
                    let prev_anchor = format!("{:04}-{:02}-01", pinfo.year, pinfo.month);
                    let prev_baseline = season_baseline(
                        &scoped_achievements,
                        &commit_days,
                        &day_axis,
                        &prev_anchor,
                    );
                    data.recap = Some(season_recap(prev, &pevents, pcommits, &prev_baseline));
                    let _ = settings_arc.set_last_recapped_season(season_index);
                }
                _ => {}
            }

            data.gamification_enabled = true;
            data
        })
        .await
        .map_err(|e| e.to_string())
    }
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
    let resolved = openspec_core::canonicalize(&file_path)
        .map_err(|e| format!("artifact not found: {e}"))?;
    if !resolved.starts_with(&changes_root_canonical) {
        return Err("artifact path escapes workspace".to_string());
    }
    Ok(resolved)
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
    achievements: &[Achievement],
    identity: &IdentityConfig,
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
    let me_created: HashSet<&str> = achievements
        .iter()
        .filter(|e| e.kind == AchievementKind::ChangeCreated && event_is_me(e, identity))
        .filter_map(|e| e.change_id.as_deref())
        .collect();
    active.iter().filter(|id| me_created.contains(*id)).count() as u32
}

/// Seed the activity log from git history on first launch (when the log is
/// empty). Once per distinct repository in the registry.
fn backfill_activity(registry: &Arc<Mutex<WorkspaceRegistry>>, log: &Arc<ActivityLog>) {
    if !log.is_empty() {
        return;
    }
    let repo_ids = match registry.lock() {
        Ok(reg) => reg.repos(),
        Err(_) => return,
    };
    for repo_id in repo_ids {
        let main_wt = worktree_list(&repo_id)
            .into_iter()
            .find(|wt| wt.is_main)
            .map(|wt| wt.path)
            .or_else(|| repo_id.as_path().parent().map(Path::to_path_buf))
            .unwrap_or_else(|| repo_id.as_path().to_path_buf());
        let lifecycles = change_lifecycle(&repo_id);
        let task_history = task_completion_history(&repo_id, BACKFILL_SINCE);
        log.record_all(build_backfill(&main_wt, &lifecycles, &task_history));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let err = resolve_artifact_path(ws, "add-x", "spec", Some("../../../../secret"))
            .unwrap_err();
        assert!(err.contains("escapes"), "expected escape rejection, got: {err}");
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
}
