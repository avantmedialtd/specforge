//! `#[tauri::command]` handlers exposed to the frontend.
//!
//! Each handler converts errors to `String` since Tauri serialises the
//! return type to JSON for the JS side. Most handlers borrow shared state
//! via `State<'_, T>`; async handlers release any `std::sync::Mutex`
//! guards before crossing `await` boundaries.

use crate::events::EVENT_WORKSPACE_PRESENTATION_UPDATED;
use crate::settings::SettingsStore;
use openspec_core::{
    change_lifecycle, commit_activity, commit_activity_with_authors, commit_diff, commit_files,
    commit_log, compute_dashboard, compute_leaderboard, compute_progress, day_axis,
    detect_candidate_identities, event_is_me, layout_commit_graph, today_str, AchievementKind,
    ActivityLog, Author, ChangeData, ChangeLifecycle, CommitFile, CommitGraph, DashboardData,
    IdentityConfig, PaletteColor, PresentationKey, RegisteredWorkspace, RepoId, WatcherManager,
    WorkspaceOrigin, WorkspacePresentationStore, WorkspaceRegistry, WorkspaceView,
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
    registry: State<'_, SharedRegistry>,
    watcher: State<'_, WatcherManager>,
) -> Result<RegisteredWorkspace, String> {
    // `register` returns the user-registered entry plus any auto-discovered
    // sibling worktrees of the same git repo. The first element is always
    // the user-registered folder.
    let added = {
        let mut reg = registry.lock().map_err(|e| e.to_string())?;
        reg.register(PathBuf::from(path))
            .map_err(|e| e.to_string())?
    };

    let primary = added
        .first()
        .cloned()
        .ok_or_else(|| "register returned no folders".to_string())?;

    // Start watchers for every newly-tracked workspace (the user-registered
    // one and any discovered siblings).
    for folder in &added {
        if folder.uri.is_dir() {
            if let Err(e) = watcher.add_workspace(folder.clone()).await {
                eprintln!("failed to add watcher for {}: {e}", folder.uri.display());
            }
        }
    }

    // Install (or update) per-repo monitors so future runtime worktree
    // adds/removes for this repo are picked up automatically.
    watcher.sync_repos();

    // Refresh the cached aggregated view so the next `get_workspace_views`
    // request reflects the new registration. `add_workspace` mutates the
    // cache directly without emitting a raw `CacheEvent`, so the aggregator
    // task that normally drives `last_views` would otherwise miss this
    // change until an unrelated filesystem event fired.
    watcher.aggregate_and_emit();

    // Look up the new entry's repo_id (set when the path is inside a git
    // repository) so the frontend can address the correct presentation key
    // for this row. Presentation overrides themselves stay `None` on a fresh
    // registration — `list_workspaces` is the canonical join site.
    let repo_id = {
        let reg = registry.lock().map_err(|e| e.to_string())?;
        reg.entry(&primary.uri)
            .and_then(|e| e.repo_id.as_ref().map(|r| r.as_path().to_path_buf()))
    };

    Ok(RegisteredWorkspace {
        uri: primary.uri,
        name: primary.name,
        is_missing: false,
        display_name: None,
        color: None,
        repo_id,
    })
}

#[tauri::command]
pub async fn unregister_workspace(
    path: String,
    registry: State<'_, SharedRegistry>,
    watcher: State<'_, WatcherManager>,
    presentation: State<'_, SharedPresentation>,
) -> Result<bool, String> {
    let path_buf = PathBuf::from(path);

    // Snapshot the entry's repo association before unregister so we can
    // decide which presentation keys to cascade-clean afterwards. Use the
    // canonicalised path because that's what the registry stores; fall back
    // to the input when canonicalisation fails (e.g., the directory was
    // deleted), which still lets us unregister but skips presentation
    // cleanup for that pathological case.
    let canonical = path_buf.canonicalize().unwrap_or_else(|_| path_buf.clone());
    let (was_user_registered, target_repo_id) = {
        let reg = registry.lock().map_err(|e| e.to_string())?;
        match reg.entry(&canonical) {
            Some(e) => (
                matches!(e.origin, WorkspaceOrigin::UserRegistered),
                e.repo_id.as_ref().map(|r| r.as_path().to_path_buf()),
            ),
            None => (false, None),
        }
    };

    let removed = {
        let mut reg = registry.lock().map_err(|e| e.to_string())?;
        reg.unregister(&path_buf).map_err(|e| e.to_string())?
    };

    // Tear down watchers for every removed path (the user-registered one
    // plus any cascaded discovered worktrees of the same repo).
    let any_removed = !removed.is_empty();
    for p in &removed {
        watcher.remove_workspace(p);
    }
    // Drop any repo monitors whose repo no longer has tracked workspaces.
    // Must be `async` so `sync_repos` → `RepoMonitor::install` (which calls
    // `tokio::spawn`) has an active runtime.
    watcher.sync_repos();

    // Refresh the cached aggregated view so the next `get_workspace_views`
    // request reflects the removal. A single call after the loop covers the
    // cascade case — `last_views` is recomputed once from the final settled
    // state. Idempotent when `removed` is empty.
    watcher.aggregate_and_emit();

    // Cascade presentation cleanup. Mirrors the registry's own cascade: a
    // flat workspace drops its own `Flat` entry; a repo-member workspace
    // drops the shared `Repo` entry only when the repository no longer has
    // any user-registered worktree.
    if was_user_registered {
        let still_has_user_for_repo = target_repo_id.as_ref().map(|repo_id| {
            let reg = registry.lock().map_err(|e| e.to_string()).ok();
            match reg {
                Some(reg) => repo_still_has_user_registered(&reg, repo_id),
                None => false,
            }
        });
        let keys = presentation_keys_to_drop(
            &canonical,
            target_repo_id.as_deref(),
            still_has_user_for_repo.unwrap_or(false),
        );
        if !keys.is_empty() {
            let mut store = presentation.lock().map_err(|e| e.to_string())?;
            for key in keys {
                let _ = store.remove(&key);
            }
        }
    }

    Ok(any_removed)
}

/// Pure decision function: given the unregistered workspace's canonical path
/// and its repo association (if any), plus whether the repository still has
/// any other user-registered workspace, return the presentation keys that
/// should be dropped from the store.
///
/// Flat workspaces always drop their own `Flat` key. Repo-member workspaces
/// drop the shared `Repo` key only when their cascade fired — i.e. the
/// repository no longer has any user-registered worktree.
pub(crate) fn presentation_keys_to_drop(
    canonical: &std::path::Path,
    target_repo_id: Option<&std::path::Path>,
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

fn repo_still_has_user_registered(registry: &WorkspaceRegistry, repo_id: &std::path::Path) -> bool {
    registry.entries().iter().any(|e| {
        matches!(e.origin, WorkspaceOrigin::UserRegistered)
            && e.repo_id.as_ref().map(|r| r.as_path()) == Some(repo_id)
    })
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
    presentation: State<'_, SharedPresentation>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let key = match repo_id {
        Some(r) => PresentationKey::Repo(r),
        None => PresentationKey::Flat(uri),
    };
    {
        let mut store = presentation.lock().map_err(|e| e.to_string())?;
        store
            .set(key, display_name, color)
            .map_err(|e| e.to_string())?;
    }
    let _ = app.emit(EVENT_WORKSPACE_PRESENTATION_UPDATED, ());
    Ok(())
}

#[tauri::command]
pub fn get_active_count(watcher: State<'_, WatcherManager>) -> Result<usize, String> {
    // Counts non-archived logical changes across every tracked entry. A
    // logical change touched by multiple worktrees contributes 1.
    Ok(watcher.total_active_logical_count())
}

/// How many days the Dashboard's git-mined activity + throughput window spans,
/// and the cap on the recent-activity feed. Kept here so the contract lives in
/// one place.
const DASHBOARD_ACTIVITY_WINDOW_DAYS: u64 = 14;
const DASHBOARD_RECENT_LIMIT: usize = 12;
/// The gamified heatmap / streak window — 53 weeks of local calendar days, so
/// the contribution grid reads as a full-year GitHub-style band. Bounded.
const DASHBOARD_HEATMAP_WINDOW_DAYS: u64 = 371;

/// Aggregate the global Dashboard payload: cross-workspace summary metrics,
/// per-repo breakdown, git-mined commits-per-day activity, change-lifecycle
/// throughput + time-to-archive, and a recent-activity feed. Presentation
/// display-names are joined in (mirroring `get_workspace_views`) so labels
/// match the tree. The git reads run off the async runtime; flat workspaces
/// and git-less repos degrade to counts-only.
#[tauri::command]
pub async fn get_dashboard(
    scope: Option<String>,
    watcher: State<'_, WatcherManager>,
    presentation: State<'_, SharedPresentation>,
    activity_log: State<'_, Arc<ActivityLog>>,
    settings: State<'_, SharedSettings>,
) -> Result<DashboardData, String> {
    // `scope` selects which achievements feed the gamified layer: "everyone"
    // counts all authors, anything else (default) counts only the developer's.
    // The git mining is identical across scopes; only the in-memory log filter
    // and the in-flight attribution differ.
    let only_me = !matches!(scope.as_deref(), Some("everyone"));
    let identity = settings.identity();
    let mut views = watcher.workspace_views();
    {
        let store = presentation.lock().map_err(|e| e.to_string())?;
        for view in &mut views {
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

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let since = format!("{DASHBOARD_ACTIVITY_WINDOW_DAYS} days ago");
    let heatmap_since = format!("{DASHBOARD_HEATMAP_WINDOW_DAYS} days ago");

    let day_axis = day_axis(DASHBOARD_HEATMAP_WINDOW_DAYS as u32);
    let today = today_str();
    let log = activity_log.inner().clone();

    tokio::task::spawn_blocking(move || {
        // Mine each repo's change lifecycle ONCE, then reuse it twice: to
        // reconcile the activity log and as the `lifecycle_for` source for the
        // throughput metrics (avoiding a second `git log` per repo).
        //
        // Reconciling here is what keeps "shipped" honest. The live watcher only
        // records an archival when it observes an `active → archived` transition
        // inside a watched workspace; a change archived on a branch or worktree
        // reaches the main workspace already-archived, so that transition never
        // fires and the one-shot launch backfill (gated on an empty log) can't
        // recover it on restart. Folding git history — the source of truth for
        // archivals — back into the log on each fetch closes that gap. It is
        // idempotent (dedup by change id), so steady-state fetches write nothing.
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
            DASHBOARD_RECENT_LIMIT,
            |repo| commit_activity(repo, &since),
            |repo| lifecycles.get(&repo.0).cloned().unwrap_or_default(),
        );

        // Commit (date, author) pairs across the heatmap window — used both for
        // the scoped streak/today commit days and the leaderboard's commits.
        let mut commit_pairs: Vec<(String, Author)> = Vec::new();
        for view in &views {
            if let WorkspaceView::Repo(r) = view {
                let repo_id = RepoId(r.repo_id.clone());
                commit_pairs.extend(commit_activity_with_authors(&repo_id, &heatmap_since));
            }
        }

        // Read the achievement window AFTER reconciliation so freshly-recovered
        // archivals land in today's haul, the heatmap, and milestones. This is
        // the full (all-author) set; the leaderboard ranks across it, while the
        // gamified layer below is filtered to the active scope.
        let all_achievements = log.query_window(DASHBOARD_HEATMAP_WINDOW_DAYS as u32);

        // Per-author leaderboard from every author's achievements + commits. The
        // frontend renders it only when it holds more than one distinct author.
        let commit_authors: Vec<Author> = commit_pairs.iter().map(|(_, a)| a.clone()).collect();
        data.leaderboard = compute_leaderboard(&all_achievements, &commit_authors, &identity);

        // Scope the gamified layer: under "me", keep only the developer's
        // achievements and commits (author-less events count as the
        // developer's); under "everyone", keep all.
        let scoped_achievements: Vec<_> = if only_me {
            all_achievements
                .iter()
                .filter(|e| event_is_me(e, &identity))
                .cloned()
                .collect()
        } else {
            all_achievements.clone()
        };
        let commit_days: Vec<String> = commit_pairs
            .iter()
            .filter(|(_, a)| !only_me || openspec_core::is_me(a, &identity))
            .filter(|(iso, _)| iso.len() >= 10)
            .map(|(iso, _)| iso[..10].to_string())
            .collect();

        data.progress = compute_progress(&scoped_achievements, &commit_days, &day_axis, &today);

        // The hero's in-flight tile is scope-aware: under "me" it counts active
        // changes the developer created (by the change-created author); under
        // "everyone" it is the global active-change count.
        data.progress.in_flight = if only_me {
            scoped_in_flight(&views, &all_achievements, &identity)
        } else {
            data.summary.active_changes as u32
        };

        data
    })
    .await
    .map_err(|e| e.to_string())
}

/// Count active (non-archived) changes the developer created, for the *Me*
/// scope's in-flight tile: the active change ids whose `ChangeCreated`
/// achievement resolves to the developer (author-less created events count as
/// the developer's). A change with no recoverable created event is not counted.
fn scoped_in_flight(
    views: &[WorkspaceView],
    achievements: &[openspec_core::Achievement],
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

    let changes_root_canonical = changes_root
        .canonicalize()
        .map_err(|e| format!("workspace changes directory missing: {e}"))?;
    let resolved = file_path
        .canonicalize()
        .map_err(|e| format!("artifact not found: {e}"))?;
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
/// configuration plus the distinct git identities detected across registered
/// workspaces, offered as alias suggestions.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInfo {
    pub config: IdentityConfig,
    pub candidates: Vec<Author>,
}

#[tauri::command]
pub fn get_identity(
    settings: State<'_, SharedSettings>,
    registry: State<'_, SharedRegistry>,
) -> Result<IdentityInfo, String> {
    let config = settings.identity();
    let folders: Vec<PathBuf> = {
        let reg = registry.lock().map_err(|e| e.to_string())?;
        reg.entries().iter().map(|e| e.folder.uri.clone()).collect()
    };
    let candidates = detect_candidate_identities(&folders);
    Ok(IdentityInfo { config, candidates })
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

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
            "Repo presentation must survive when another user-registered workspace remains"
        );
    }

    #[test]
    fn last_repo_member_unregister_drops_the_repo_key() {
        let keys =
            presentation_keys_to_drop(Path::new("/r/main"), Some(Path::new("/r/.git")), false);
        assert_eq!(keys, vec![PresentationKey::Repo("/r/.git".into())]);
    }
}
