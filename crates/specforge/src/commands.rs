//! `#[tauri::command]` handlers exposed to the frontend.
//!
//! Each handler converts errors to `String` since Tauri serialises the
//! return type to JSON for the JS side. Most handlers borrow shared state
//! via `State<'_, T>`; async handlers release any `std::sync::Mutex`
//! guards before crossing `await` boundaries.

use crate::events::EVENT_WORKSPACE_PRESENTATION_UPDATED;
use openspec_app::{
    AppService, ClaudeQuotaState, SettingsStore, TreatmentLocker, DASHBOARD_HEATMAP_WINDOW_DAYS,
};
use openspec_core::{
    commit_activity_with_authors, commit_diff, commit_files, commit_log,
    detect_candidate_identities, layout_commit_graph, list_archived_summaries, normalized_key,
    ArchivedChangeSummary, Author, ChangeData, CommitFile, CommitGraph, DashboardData,
    IdentityConfig, PaletteColor, Person, PresentationKey, RegisteredWorkspace, RepoId,
    WatcherManager, WorkspaceGarden, WorkspaceOrigin, WorkspacePresentationStore,
    WorkspaceRegistry, WorkspaceView,
};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, State};
use tauri_plugin_autostart::ManagerExt;

type SharedRegistry = Arc<Mutex<WorkspaceRegistry>>;
type SharedSettings = Arc<SettingsStore>;
type SharedPresentation = Arc<Mutex<WorkspacePresentationStore>>;

#[tauri::command]
pub async fn register_workspace(
    path: String,
    svc: State<'_, AppService>,
) -> Result<RegisteredWorkspace, String> {
    // The whole orchestration — register + watcher wiring + aggregate refresh —
    // lives in `AppService` so both frontends share one tested path.
    svc.add_workspace(PathBuf::from(path)).await
}

#[tauri::command]
pub async fn unregister_workspace(
    path: String,
    svc: State<'_, AppService>,
) -> Result<bool, String> {
    svc.remove_workspace(PathBuf::from(path)).await
}

#[tauri::command]
pub fn list_workspaces(
    registry: State<'_, SharedRegistry>,
    presentation: State<'_, SharedPresentation>,
) -> Result<Vec<RegisteredWorkspace>, String> {
    let reg = registry.lock().map_err(|e| e.to_string())?;
    let store = presentation.lock().map_err(|e| e.to_string())?;

    // Walk the registry directly (rather than calling `reg.list()`) so each
    // listed row also carries its repo_id and presentation overrides without
    // a second pass. The sort order matches the registry's `list()` helper.
    let mut items: Vec<RegisteredWorkspace> = reg
        .entries()
        .iter()
        .filter(|e| matches!(e.origin, WorkspaceOrigin::UserRegistered))
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

#[tauri::command]
pub fn get_changes(
    workspace: String,
    watcher: State<'_, WatcherManager>,
) -> Result<Vec<ChangeData>, String> {
    Ok(watcher.changes_for(&PathBuf::from(workspace)))
}

/// Lists one workspace's archived changes for the Archive browser — a
/// lightweight `{ id, date, title }` per archive directory, newest-first.
/// Called on demand when the Archive view opens or its selected workspace
/// changes; never on the watcher's aggregation path, so the archive stays off
/// the hot path entirely.
#[tauri::command]
pub fn list_archived(workspace: String) -> Result<Vec<ArchivedChangeSummary>, String> {
    list_archived_summaries(&PathBuf::from(workspace)).map_err(|e| e.to_string())
}

/// Reports which artifacts an archived change has on disk, so the Archive view
/// can offer per-artifact navigation (proposal / design / tasks / capability
/// specs). On-demand and per-change — only when a change is opened — so it
/// stays off the watcher's aggregation path. `dir_name` is one archive
/// directory entry (`<YYYY-MM-DD>-<id>`), never a path.
#[tauri::command]
pub fn archived_artifact_status(
    workspace: String,
    dir_name: String,
) -> Result<openspec_core::ArtifactStatus, String> {
    if dir_name.contains('/') || dir_name.contains('\\') || dir_name.contains("..") {
        return Err("invalid archive directory name".into());
    }
    let change_dir = PathBuf::from(workspace)
        .join("openspec")
        .join("changes")
        .join("archive")
        .join(&dir_name);
    Ok(openspec_core::parse_artifact_status(&change_dir))
}

/// Returns one entry per tracked top-level workspace: either an aggregated
/// [`WorkspaceView::Repo`] grouping all worktrees of a git repository, or
/// a [`WorkspaceView::Flat`] for a non-git workspace. This is the command
/// the new repo/instance-aware tree consumes.
///
/// Presentation overrides (display name + tint) are joined in here so the
/// pure aggregator stays unaware of the presentation store.
#[tauri::command]
pub fn get_workspace_views(
    watcher: State<'_, WatcherManager>,
    presentation: State<'_, SharedPresentation>,
) -> Result<Vec<WorkspaceView>, String> {
    let mut views = watcher.workspace_views();
    let store = presentation.lock().map_err(|e| e.to_string())?;
    for view in &mut views {
        match view {
            WorkspaceView::Repo(r) => {
                let key = PresentationKey::Repo(r.repo_id.clone());
                let (dn, c) = store.lookup(&key);
                r.display_name = dn;
                r.color = c;
            }
            WorkspaceView::Flat {
                workspace,
                display_name,
                color,
                ..
            } => {
                let key = PresentationKey::Flat(workspace.uri.clone());
                let (dn, c) = store.lookup(&key);
                *display_name = dn;
                *color = c;
            }
        }
    }
    Ok(views)
}

/// Persists the display-name and tint-colour overrides for a top-level row.
/// The frontend passes `repo_id` when editing a repository group's row and
/// omits it for a flat workspace; the command picks the appropriate
/// presentation key from there. Emits `workspace-presentation-updated` on
/// success so the frontend refetches.
#[tauri::command]
pub fn set_workspace_presentation(
    uri: PathBuf,
    repo_id: Option<PathBuf>,
    display_name: Option<String>,
    color: Option<PaletteColor>,
    svc: State<'_, AppService>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    svc.set_workspace_presentation(uri, repo_id, display_name, color)?;
    let _ = app.emit(EVENT_WORKSPACE_PRESENTATION_UPDATED, ());
    Ok(())
}

#[tauri::command]
pub fn get_active_count(watcher: State<'_, WatcherManager>) -> Result<usize, String> {
    // Counts non-archived logical changes across every tracked entry. A
    // logical change touched by multiple worktrees contributes 1.
    Ok(watcher.total_active_logical_count())
}

/// Aggregate the global Dashboard payload. Delegates to the shared
/// [`openspec_app::AppService`] so the assembly stays unit-testable and
/// identical to the terminal frontend.
#[tauri::command]
pub async fn get_dashboard(svc: State<'_, AppService>) -> Result<DashboardData, String> {
    svc.dashboard().await
}

/// The commit garden: one stylized plant per top-level entry. Delegates to
/// [`openspec_app::AppService`].
#[tauri::command]
pub async fn get_commit_garden(svc: State<'_, AppService>) -> Result<Vec<WorkspaceGarden>, String> {
    svc.commit_garden().await
}

/// Equip a treatment finish by its id (pass `null` to clear it). The only
/// season-state mutation the frontend drives; persisted in app-data.
#[tauri::command]
pub fn set_equipped_treatment(
    treatment_id: Option<String>,
    settings: State<'_, SharedSettings>,
) -> Result<(), String> {
    settings
        .set_equipped_treatment(treatment_id)
        .map_err(|e| e.to_string())
}

/// The treatment wardrobe for Settings. Delegates to
/// [`openspec_app::AppService`].
#[tauri::command]
pub fn get_treatment_locker(svc: State<'_, AppService>) -> Result<TreatmentLocker, String> {
    Ok(svc.treatment_locker())
}

/// Returns the raw markdown for one artifact of a change.
///
/// `artifact_kind` must be one of `"proposal"`, `"design"`, `"tasks"`, or
/// `"spec"`. The `capability` argument is required (and only used) when
/// kind is `"spec"`.
///
/// A path-traversal guard canonicalises the resolved file and rejects
/// anything outside the workspace's `openspec/changes/` subtree.
#[tauri::command]
pub async fn read_artifact(
    workspace: String,
    change_id: String,
    artifact_kind: String,
    capability: Option<String>,
) -> Result<String, String> {
    let workspace_path = PathBuf::from(workspace);
    let changes_root = workspace_path.join("openspec").join("changes");
    let change_dir = changes_root.join(&change_id);

    let file_path = match artifact_kind.as_str() {
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

    tokio::fs::read_to_string(&resolved)
        .await
        .map_err(|e| e.to_string())
}

/// Build the commit-graph for a repository, identified by its git common
/// directory (`repo_id`, as carried on `RepoView.repoId`). Reads up to
/// `limit` commits across all refs and lays them out into lanes/edges. Runs
/// the blocking `git` calls off the async runtime. Returns an empty graph
/// (not an error) when the repo can't be read, so the rail degrades to empty.
#[tauri::command]
pub async fn get_commit_graph(repo_id: PathBuf, limit: usize) -> Result<CommitGraph, String> {
    tokio::task::spawn_blocking(move || {
        let repo = RepoId(repo_id);
        // Fetch one extra commit to detect truncation without a second pass.
        let mut commits = commit_log(&repo, limit.saturating_add(1));
        let truncated = commits.len() > limit;
        commits.truncate(limit);
        layout_commit_graph(commits, truncated)
    })
    .await
    .map_err(|e| e.to_string())
}

/// The files a commit changed, with per-file added/removed counts. Drives the
/// commit-detail view's file list.
#[tauri::command]
pub async fn get_commit_detail(repo_id: PathBuf, sha: String) -> Result<Vec<CommitFile>, String> {
    tokio::task::spawn_blocking(move || commit_files(&RepoId(repo_id), &sha))
        .await
        .map_err(|e| e.to_string())
}

/// The raw unified diff for one file of a commit.
#[tauri::command]
pub async fn get_commit_diff(
    repo_id: PathBuf,
    sha: String,
    path: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || commit_diff(&RepoId(repo_id), &sha, &path))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_launch_on_login(app: tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_launch_on_login(enabled: bool, app: tauri::AppHandle) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())?;
    } else {
        manager.disable().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_gamification_enabled(settings: State<'_, SharedSettings>) -> Result<bool, String> {
    Ok(settings.gamification_enabled())
}

/// The latest opt-in Claude usage-quota snapshot. Delegates to
/// [`openspec_app::AppService`]; returns the `Disabled` snapshot when the
/// feature is off. The frontend re-reads this on each `quota-updated` event.
#[tauri::command]
pub fn get_claude_quota(svc: State<'_, AppService>) -> Result<ClaudeQuotaState, String> {
    Ok(svc.claude_quota())
}

#[tauri::command]
pub fn get_claude_quota_enabled(settings: State<'_, SharedSettings>) -> Result<bool, String> {
    Ok(settings.claude_quota_enabled())
}

/// Toggle the opt-in quota feature. The background poller re-reads this flag on
/// its next tick (within a couple of seconds), so no explicit restart is needed.
#[tauri::command]
pub fn set_claude_quota_enabled(
    enabled: bool,
    settings: State<'_, SharedSettings>,
) -> Result<(), String> {
    settings
        .set_claude_quota_enabled(enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_gamification_enabled(
    enabled: bool,
    settings: State<'_, SharedSettings>,
) -> Result<(), String> {
    settings
        .set_gamification_enabled(enabled)
        .map_err(|e| e.to_string())
}

/// The WSL polling-watcher interval (seconds), or `None` on platforms where WSL
/// workspaces cannot occur (macOS, Linux). The frontend uses `None` to hide the
/// control entirely — so the setting is "absent off Windows" by contract,
/// without the frontend needing a platform plugin.
#[tauri::command]
pub fn get_wsl_poll_interval_secs(
    settings: State<'_, SharedSettings>,
) -> Result<Option<u64>, String> {
    #[cfg(target_os = "windows")]
    {
        Ok(Some(settings.wsl_poll_interval_secs()))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = settings;
        Ok(None)
    }
}

/// Persist a new WSL polling-watcher interval and apply it to the live watcher
/// (takes effect for watchers established afterwards).
#[tauri::command]
pub fn set_wsl_poll_interval_secs(
    secs: u64,
    settings: State<'_, SharedSettings>,
    watcher: State<'_, WatcherManager>,
) -> Result<(), String> {
    settings
        .set_wsl_poll_interval_secs(secs)
        .map_err(|e| e.to_string())?;
    watcher.set_poll_interval(std::time::Duration::from_secs(secs));
    Ok(())
}

#[tauri::command]
pub fn get_notifications_enabled(settings: State<'_, SharedSettings>) -> Result<bool, String> {
    Ok(settings.snapshot().notifications_enabled)
}

#[tauri::command]
pub fn set_notifications_enabled(
    enabled: bool,
    settings: State<'_, SharedSettings>,
) -> Result<(), String> {
    settings
        .set_notifications_enabled(enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_collapsed_tree_node_ids(
    settings: State<'_, SharedSettings>,
) -> Result<Vec<String>, String> {
    Ok(settings.snapshot().collapsed_tree_node_ids)
}

#[tauri::command]
pub fn set_collapsed_tree_node_ids(
    ids: Vec<String>,
    settings: State<'_, SharedSettings>,
) -> Result<(), String> {
    settings
        .set_collapsed_tree_node_ids(ids)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_expanded_tree_node_ids(
    settings: State<'_, SharedSettings>,
) -> Result<Vec<String>, String> {
    Ok(settings.snapshot().expanded_tree_node_ids)
}

#[tauri::command]
pub fn set_expanded_tree_node_ids(
    ids: Vec<String>,
    settings: State<'_, SharedSettings>,
) -> Result<(), String> {
    settings
        .set_expanded_tree_node_ids(ids)
        .map_err(|e| e.to_string())
}

/// The developer-identity payload for the Settings → Identity section: the saved
/// configuration, the contributor roster (named people other than "me"), and the
/// distinct git identities detected across registered workspaces, offered as
/// alias suggestions.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInfo {
    pub config: IdentityConfig,
    pub people: Vec<Person>,
    pub candidates: Vec<Author>,
}

#[tauri::command]
pub fn get_identity(
    settings: State<'_, SharedSettings>,
    registry: State<'_, SharedRegistry>,
) -> Result<IdentityInfo, String> {
    let config = settings.identity();
    let people = settings.people();
    let folders: Vec<PathBuf> = {
        let reg = registry.lock().map_err(|e| e.to_string())?;
        reg.entries().iter().map(|e| e.folder.uri.clone()).collect()
    };
    let candidates = detect_candidate_identities(&folders);
    Ok(IdentityInfo {
        config,
        people,
        candidates,
    })
}

#[tauri::command]
pub fn set_display_name(
    name: Option<String>,
    settings: State<'_, SharedSettings>,
) -> Result<(), String> {
    settings.set_display_name(name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_identity_aliases(
    aliases: Vec<Author>,
    settings: State<'_, SharedSettings>,
) -> Result<(), String> {
    settings
        .set_identity_aliases(aliases)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_people(people: Vec<Person>, settings: State<'_, SharedSettings>) -> Result<(), String> {
    settings.set_people(people).map_err(|e| e.to_string())
}

/// The distinct non-"me" authors observed across registered repositories within
/// the Dashboard window, deduped by normalised key in first-seen order — the
/// candidate pool the Settings roster UI offers for naming and merging. Authors
/// that resolve as the developer, or that have no usable key, are excluded.
/// Read-only: shells `git log` per repo, bounded by the window.
#[tauri::command]
pub fn observed_authors(
    watcher: State<'_, WatcherManager>,
    settings: State<'_, SharedSettings>,
) -> Result<Vec<Author>, String> {
    let identity = settings.identity();
    let since = format!("{DASHBOARD_HEATMAP_WINDOW_DAYS} days ago");
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<Author> = Vec::new();
    for view in watcher.workspace_views() {
        if let WorkspaceView::Repo(r) = view {
            let repo_id = RepoId(r.repo_id.clone());
            for (_, author) in commit_activity_with_authors(&repo_id, &since) {
                if openspec_core::is_me(&author, &identity) {
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
    Ok(out)
}
