//! Aggregates registered workspaces + git-mined history into the global
//! Dashboard payload the frontend renders as its home surface.
//!
//! Mirrors the `repo_view` split: pure transforms over [`WorkspaceView`]s and
//! injected git data, so the whole thing is unit-testable from `cargo test`
//! with no GUI and no real git. The Tauri `get_dashboard` command supplies the
//! git closures (commit activity + change lifecycle); tests inject fixtures.

use crate::activity_log::{Achievement, AchievementKind};
use crate::git::{ChangeLifecycle, RepoId};
use crate::repo_view::{LogicalChange, WorkspaceView};
use crate::types::ChangeData;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Everything the Dashboard renders, aggregated across all workspaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardData {
    pub summary: SummaryMetrics,
    pub repos: Vec<RepoBreakdown>,
    pub activity: Vec<ActivityBucket>,
    /// How many days the activity window spans. The frontend builds its day
    /// axis (newest = today, viewer-local) of this length and looks up each
    /// day's count from `activity`, zero-filling the gaps.
    pub activity_window_days: u64,
    pub lifecycle: LifecycleMetrics,
    pub recent: Vec<RecentEntry>,
    /// Gamified progress layer — today's haul, streak, heatmap, milestones —
    /// derived from the activity log. Defaulted by [`compute_dashboard`]; the
    /// IPC layer fills it via [`compute_progress`] from the activity log.
    #[serde(default)]
    pub progress: ProgressData,
}

/// The progress-focused layer the Dashboard renders above its analytics.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProgressData {
    pub today: TodayProgress,
    pub streak: StreakInfo,
    /// One cell per local calendar day over the heatmap window, ascending
    /// (oldest first, today last).
    pub heatmap: Vec<HeatmapCell>,
    pub milestones: Vec<Milestone>,
}

/// What was achieved on the current local calendar day, with a comparison to
/// the trailing-30-day daily average (stored ×100 so the type stays `Eq`; the
/// frontend divides by 100). Change *creation* is intentionally absent: the
/// Dashboard's second hero tile shows the live in-flight (active-change) count
/// from the summary metrics, not a today-flow created count. Per-day created
/// counts still feed the heatmap drill-down via [`HeatmapCell::created`].
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TodayProgress {
    pub tasks_completed: u32,
    pub changes_archived: u32,
    pub commits_landed: u32,
    pub tasks_avg_centi: u32,
    pub changes_archived_avg_centi: u32,
    pub commits_avg_centi: u32,
}

/// Consecutive-active-day streak (ending today) and the longest run within the
/// heatmap window. A day is active if it has any achievement or any commit.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreakInfo {
    pub current: u32,
    pub longest: u32,
}

/// One day's activity for the contribution heatmap. `count` is the combined
/// total that drives the cell's intensity; the per-kind fields back the
/// drill-down detail shown when a cell is selected.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HeatmapCell {
    pub day: String,
    pub count: u32,
    pub tasks: u32,
    pub ships: u32,
    pub commits: u32,
    pub created: u32,
}

/// A threshold achievement. `achieved_at` is the timestamp of the event that
/// crossed the threshold (None for streak milestones, which are not pinned to
/// a single event). `backfilled` is true when the crossing was reconstructed
/// from git history — the frontend never fires live celebration for those.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Milestone {
    pub id: String,
    pub label: String,
    /// One of `tasks` | `ships` | `firstShip` | `streak`.
    pub kind: String,
    pub threshold: u32,
    pub achieved: bool,
    pub achieved_at: Option<i64>,
    pub backfilled: bool,
}

/// Global counts across every registered workspace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SummaryMetrics {
    /// Active (non-archived) changes; a logical change spanning worktrees
    /// counts once, matching the tray badge.
    pub active_changes: usize,
    pub completed_tasks: usize,
    pub total_tasks: usize,
    /// 0..=100; defined as 0 when `total_tasks == 0` (never divides by zero).
    pub task_percent: u8,
    pub specs_touching: usize,
    pub repo_count: usize,
    /// Distinct worktrees carrying at least one tracked change, summed across
    /// repos. A view-derived proxy for the repo's worktree footprint that
    /// needs no extra git call.
    pub worktree_count: usize,
    pub flat_count: usize,
}

/// One top-level entry's active/archived change counts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoBreakdown {
    pub label: String,
    pub active_count: usize,
    pub archived_count: usize,
}

/// Commits on one calendar day, summed across all git-backed repos.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActivityBucket {
    /// `YYYY-MM-DD` — the date prefix of the commit's author date (`%aI`).
    pub day: String,
    pub commit_count: usize,
}

/// Change throughput + mean time-to-archive, derived from lifecycle commits.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleMetrics {
    /// Changes whose archive commit falls within the window.
    pub archived_in_window: usize,
    /// Mean `archive − creation` in seconds over changes with both dates
    /// recoverable from git; `None` when none are recoverable.
    pub avg_time_to_archive_secs: Option<u64>,
}

/// One recently-active change, with enough identity to navigate to it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecentEntry {
    pub change_id: String,
    pub title: Option<String>,
    /// Display name of the owning repo or flat workspace.
    pub workspace_label: String,
    /// The worktree (or flat workspace) path the artifacts are read from —
    /// the frontend opens this change's `proposal.md` from here.
    pub worktree_path: PathBuf,
    pub modified_at: u64,
}

/// Aggregate the Dashboard payload. Pure given the injected git closures:
/// `activity_for` returns a repo's commit author-dates (ISO-8601) within the
/// window, `lifecycle_for` returns its change lifecycles. `now_unix` is the
/// current time in epoch seconds; `window_days` bounds throughput; and
/// `recent_limit` caps the recent feed. Flat (non-git) workspaces contribute
/// to the counts and feed but nothing to the git-derived sections.
pub fn compute_dashboard(
    views: &[WorkspaceView],
    now_unix: u64,
    window_days: u64,
    recent_limit: usize,
    activity_for: impl Fn(&RepoId) -> Vec<String>,
    lifecycle_for: impl Fn(&RepoId) -> Vec<ChangeLifecycle>,
) -> DashboardData {
    let summary = summary_metrics(views);
    let repos = repo_breakdowns(views);
    let recent = recent_entries(views, recent_limit);

    let mut activity_dates: Vec<String> = Vec::new();
    let mut lifecycles: Vec<ChangeLifecycle> = Vec::new();
    for view in views {
        if let WorkspaceView::Repo(repo) = view {
            let repo_id = RepoId(repo.repo_id.clone());
            activity_dates.extend(activity_for(&repo_id));
            lifecycles.extend(lifecycle_for(&repo_id));
        }
    }

    DashboardData {
        summary,
        repos,
        activity: bucket_activity(&activity_dates),
        activity_window_days: window_days,
        lifecycle: lifecycle_metrics(&lifecycles, now_unix, window_days),
        recent,
        progress: ProgressData::default(),
    }
}

/// Milestone thresholds, in ascending order per family. Derived purely from
/// the activity log's cumulative totals so unlock state needs no second store.
const TASK_MILESTONES: &[u32] = &[10, 50, 100, 250, 500];
const SHIP_MILESTONES: &[u32] = &[5, 10, 25, 50];
const STREAK_MILESTONES: &[u32] = &[3, 7, 30, 100];

/// Compute the gamified progress layer from the activity log. Pure given the
/// inputs: `achievements` is the full log, `commit_days` are the local
/// calendar-day strings (`YYYY-MM-DD`) on which commits landed across all
/// repos (one entry per commit), and `day_axis` is the ascending heatmap
/// window ending today (newest last). `today` is the current local day.
///
/// A day counts toward the streak if it carries any achievement OR any commit.
/// Milestones are crossed off the cumulative achievement totals; the crossing
/// event's `backfilled` flag and timestamp are surfaced so the UI can suppress
/// celebration for history recovered from git.
pub fn compute_progress(
    achievements: &[Achievement],
    commit_days: &[String],
    day_axis: &[String],
    today: &str,
) -> ProgressData {
    // Per-day achievement magnitude by kind, plus a combined per-day count
    // (achievements + commits) for the heatmap and streak.
    let mut commits_by_day: BTreeMap<&str, u32> = BTreeMap::new();
    for d in commit_days {
        *commits_by_day.entry(d.as_str()).or_insert(0) += 1;
    }

    let mut tasks_by_day: BTreeMap<String, u32> = BTreeMap::new();
    let mut ships_by_day: BTreeMap<String, u32> = BTreeMap::new();
    let mut created_by_day: BTreeMap<String, u32> = BTreeMap::new();
    let mut combined_by_day: BTreeMap<String, u32> = BTreeMap::new();
    for a in achievements {
        let day = crate::activity_log::local_day(a.timestamp);
        *combined_by_day.entry(day.clone()).or_insert(0) += a.magnitude;
        match a.kind {
            AchievementKind::TaskCompleted => *tasks_by_day.entry(day).or_insert(0) += a.magnitude,
            AchievementKind::ChangeArchived => *ships_by_day.entry(day).or_insert(0) += a.magnitude,
            AchievementKind::ChangeCreated => {
                *created_by_day.entry(day).or_insert(0) += a.magnitude
            }
            AchievementKind::ArtifactReached => {}
        }
    }
    for (day, n) in &commits_by_day {
        *combined_by_day.entry(day.to_string()).or_insert(0) += n;
    }

    let today_progress = TodayProgress {
        tasks_completed: tasks_by_day.get(today).copied().unwrap_or(0),
        changes_archived: ships_by_day.get(today).copied().unwrap_or(0),
        commits_landed: commits_by_day.get(today).copied().unwrap_or(0),
        tasks_avg_centi: trailing_avg_centi(&tasks_by_day, day_axis, today),
        changes_archived_avg_centi: trailing_avg_centi(&ships_by_day, day_axis, today),
        commits_avg_centi: commits_trailing_avg_centi(&commits_by_day, day_axis, today),
    };

    let heatmap: Vec<HeatmapCell> = day_axis
        .iter()
        .map(|day| HeatmapCell {
            day: day.clone(),
            count: combined_by_day.get(day).copied().unwrap_or(0),
            tasks: tasks_by_day.get(day).copied().unwrap_or(0),
            ships: ships_by_day.get(day).copied().unwrap_or(0),
            commits: commits_by_day.get(day.as_str()).copied().unwrap_or(0),
            created: created_by_day.get(day).copied().unwrap_or(0),
        })
        .collect();

    let streak = compute_streak(&combined_by_day, day_axis, today);
    let milestones = compute_milestones(achievements, streak.current);

    ProgressData {
        today: today_progress,
        streak,
        heatmap,
        milestones,
    }
}

/// Mean over the trailing-30 *active* days (days with a nonzero count),
/// excluding today, scaled ×100. Resting days don't depress the bar.
fn trailing_avg_centi(by_day: &BTreeMap<String, u32>, day_axis: &[String], today: &str) -> u32 {
    let recent: Vec<u32> = day_axis
        .iter()
        .filter(|d| d.as_str() != today)
        .rev()
        .take(30)
        .filter_map(|d| by_day.get(d).copied())
        .filter(|&n| n > 0)
        .collect();
    if recent.is_empty() {
        return 0;
    }
    let sum: u32 = recent.iter().sum();
    (sum as u64 * 100 / recent.len() as u64) as u32
}

/// As [`trailing_avg_centi`] but for the `&str`-keyed commit map.
fn commits_trailing_avg_centi(
    by_day: &BTreeMap<&str, u32>,
    day_axis: &[String],
    today: &str,
) -> u32 {
    let recent: Vec<u32> = day_axis
        .iter()
        .filter(|d| d.as_str() != today)
        .rev()
        .take(30)
        .filter_map(|d| by_day.get(d.as_str()).copied())
        .filter(|&n| n > 0)
        .collect();
    if recent.is_empty() {
        return 0;
    }
    let sum: u32 = recent.iter().sum();
    (sum as u64 * 100 / recent.len() as u64) as u32
}

/// Current streak (consecutive active days ending today) and the longest run
/// anywhere in the window. `today` need not be active — a zero today simply
/// yields a current streak of 0.
fn compute_streak(
    combined_by_day: &BTreeMap<String, u32>,
    day_axis: &[String],
    today: &str,
) -> StreakInfo {
    let active = |day: &str| combined_by_day.get(day).copied().unwrap_or(0) > 0;

    // Current: walk back from today while days are active.
    let mut current = 0u32;
    for day in day_axis.iter().rev() {
        if day.as_str() == today && !active(day) {
            // Today not yet active: the streak is whatever ran up to yesterday.
            continue;
        }
        if active(day) {
            current += 1;
        } else {
            break;
        }
    }

    // Longest run anywhere in the axis.
    let mut longest = 0u32;
    let mut run = 0u32;
    for day in day_axis {
        if active(day) {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }

    StreakInfo { current, longest }
}

/// Cross milestones off cumulative totals. Task and ship families walk their
/// thresholds against the running magnitude sum; the first-ship milestone is a
/// dedicated one-shot. Streak milestones are evaluated against the supplied
/// current streak (they aren't pinned to a single event).
fn compute_milestones(achievements: &[Achievement], current_streak: u32) -> Vec<Milestone> {
    // Chronological order so the crossing event is the earliest one that tips
    // the cumulative total past the threshold.
    let mut chrono: Vec<&Achievement> = achievements.iter().collect();
    chrono.sort_by_key(|a| a.timestamp);

    let mut out = Vec::new();
    let mut task_total = 0u32;
    let mut ship_total = 0u32;
    let mut task_idx = 0usize;
    let mut ship_idx = 0usize;
    let mut first_ship: Option<&Achievement> = None;

    for a in &chrono {
        match a.kind {
            AchievementKind::TaskCompleted => {
                task_total += a.magnitude;
                while task_idx < TASK_MILESTONES.len() && task_total >= TASK_MILESTONES[task_idx] {
                    let t = TASK_MILESTONES[task_idx];
                    out.push(Milestone {
                        id: format!("tasks-{t}"),
                        label: format!("{t} tasks completed"),
                        kind: "tasks".into(),
                        threshold: t,
                        achieved: true,
                        achieved_at: Some(a.timestamp),
                        backfilled: a.backfilled,
                    });
                    task_idx += 1;
                }
            }
            AchievementKind::ChangeArchived => {
                ship_total += a.magnitude;
                if first_ship.is_none() {
                    first_ship = Some(a);
                }
                while ship_idx < SHIP_MILESTONES.len() && ship_total >= SHIP_MILESTONES[ship_idx] {
                    let t = SHIP_MILESTONES[ship_idx];
                    out.push(Milestone {
                        id: format!("ships-{t}"),
                        label: format!("{t} changes shipped"),
                        kind: "ships".into(),
                        threshold: t,
                        achieved: true,
                        achieved_at: Some(a.timestamp),
                        backfilled: a.backfilled,
                    });
                    ship_idx += 1;
                }
            }
            _ => {}
        }
    }

    if let Some(fs) = first_ship {
        out.push(Milestone {
            id: "first-ship".into(),
            label: "First change shipped".into(),
            kind: "firstShip".into(),
            threshold: 1,
            achieved: true,
            achieved_at: Some(fs.timestamp),
            backfilled: fs.backfilled,
        });
    }

    for &t in STREAK_MILESTONES {
        if current_streak >= t {
            out.push(Milestone {
                id: format!("streak-{t}"),
                label: format!("{t}-day streak"),
                kind: "streak".into(),
                threshold: t,
                achieved: true,
                achieved_at: None,
                backfilled: false,
            });
        }
    }

    // Most-recently-crossed first; streak milestones (no timestamp) sort last.
    out.sort_by_key(|m| std::cmp::Reverse(m.achieved_at));
    out
}

/// The representative change for a logical change: its primary (most-recently
/// modified) instance, which the instances are already sorted to put first.
fn primary_change(lc: &LogicalChange) -> Option<&ChangeData> {
    lc.instances.first().map(|i| &i.change)
}

/// Fold every view into the global summary counts.
pub fn summary_metrics(views: &[WorkspaceView]) -> SummaryMetrics {
    let mut active_changes = 0;
    let mut completed_tasks = 0;
    let mut total_tasks = 0;
    let mut specs_touching = 0;
    let mut repo_count = 0;
    let mut worktree_count = 0;
    let mut flat_count = 0;

    for view in views {
        match view {
            WorkspaceView::Repo(repo) => {
                repo_count += 1;
                let mut worktrees = std::collections::HashSet::new();
                for lc in repo.active.iter().chain(repo.archived.iter()) {
                    for inst in &lc.instances {
                        worktrees.insert(inst.worktree_path.clone());
                    }
                }
                worktree_count += worktrees.len();
                for lc in &repo.active {
                    active_changes += 1;
                    if let Some(change) = primary_change(lc) {
                        completed_tasks += change.completed_tasks;
                        total_tasks += change.total_tasks;
                        if !change.artifacts.specs.is_empty() {
                            specs_touching += 1;
                        }
                    }
                }
            }
            WorkspaceView::Flat { changes, .. } => {
                flat_count += 1;
                for change in changes {
                    active_changes += 1;
                    completed_tasks += change.completed_tasks;
                    total_tasks += change.total_tasks;
                    if !change.artifacts.specs.is_empty() {
                        specs_touching += 1;
                    }
                }
            }
        }
    }

    let task_percent = if total_tasks == 0 {
        0
    } else {
        ((completed_tasks as f64 / total_tasks as f64) * 100.0).round() as u8
    };

    SummaryMetrics {
        active_changes,
        completed_tasks,
        total_tasks,
        task_percent,
        specs_touching,
        repo_count,
        worktree_count,
        flat_count,
    }
}

/// One breakdown row per top-level entry, labelled with the tree's display
/// name. Flat workspaces report `archived_count = 0` — the flat view does not
/// carry an archived section.
pub fn repo_breakdowns(views: &[WorkspaceView]) -> Vec<RepoBreakdown> {
    views
        .iter()
        .map(|view| match view {
            WorkspaceView::Repo(repo) => RepoBreakdown {
                label: repo
                    .display_name
                    .clone()
                    .unwrap_or_else(|| repo.name.clone()),
                active_count: repo.active.len(),
                archived_count: repo.archived.len(),
            },
            WorkspaceView::Flat {
                workspace,
                changes,
                display_name,
                ..
            } => RepoBreakdown {
                label: display_name
                    .clone()
                    .unwrap_or_else(|| workspace.name.clone()),
                active_count: changes.len(),
                archived_count: 0,
            },
        })
        .collect()
}

/// The recent-activity feed: active changes across all workspaces, most-recent
/// first by modification time, capped to `limit`. Repo instances carry a real
/// mtime; flat changes have none in the view and sort last (mtime 0).
pub fn recent_entries(views: &[WorkspaceView], limit: usize) -> Vec<RecentEntry> {
    let mut entries: Vec<RecentEntry> = Vec::new();
    for view in views {
        match view {
            WorkspaceView::Repo(repo) => {
                let label = repo
                    .display_name
                    .clone()
                    .unwrap_or_else(|| repo.name.clone());
                for lc in &repo.active {
                    if let Some(inst) = lc.instances.first() {
                        entries.push(RecentEntry {
                            change_id: lc.name.clone(),
                            title: inst.change.title.clone(),
                            workspace_label: label.clone(),
                            worktree_path: inst.worktree_path.clone(),
                            modified_at: inst.modified_at,
                        });
                    }
                }
            }
            WorkspaceView::Flat {
                workspace,
                changes,
                display_name,
                ..
            } => {
                let label = display_name
                    .clone()
                    .unwrap_or_else(|| workspace.name.clone());
                for change in changes {
                    entries.push(RecentEntry {
                        change_id: change.change_id.clone(),
                        title: change.title.clone(),
                        workspace_label: label.clone(),
                        worktree_path: workspace.uri.clone(),
                        modified_at: 0,
                    });
                }
            }
        }
    }
    entries.sort_by(|a, b| {
        b.modified_at
            .cmp(&a.modified_at)
            .then(a.change_id.cmp(&b.change_id))
    });
    entries.truncate(limit);
    entries
}

/// Bucket ISO-8601 author dates by calendar day (the `YYYY-MM-DD` prefix),
/// summed across repos. The prefix is the commit's own offset-local date,
/// which equals the viewer's local date for locally-authored commits (the
/// desktop committer is the viewer) — matching the commit-graph rail's day
/// grouping in the common case.
pub fn bucket_activity(iso_dates: &[String]) -> Vec<ActivityBucket> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for date in iso_dates {
        if date.len() >= 10 {
            *counts.entry(date[..10].to_string()).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .map(|(day, commit_count)| ActivityBucket { day, commit_count })
        .collect()
}

/// Throughput (archives within the window) and mean time-to-archive. Only
/// changes with both a recoverable creation and archive date contribute to the
/// average; `None` when there are none.
pub fn lifecycle_metrics(
    lifecycles: &[ChangeLifecycle],
    now_unix: u64,
    window_days: u64,
) -> LifecycleMetrics {
    let window_secs = window_days.saturating_mul(86_400) as i64;
    let cutoff = (now_unix as i64).saturating_sub(window_secs);
    let mut archived_in_window = 0;
    let mut durations: Vec<i64> = Vec::new();
    for lc in lifecycles {
        let Some(archived) = lc.archived_at else {
            continue;
        };
        if archived >= cutoff {
            archived_in_window += 1;
        }
        if let Some(created) = lc.created_at {
            if archived >= created {
                durations.push(archived - created);
            }
        }
    }
    let avg_time_to_archive_secs = if durations.is_empty() {
        None
    } else {
        let sum: i64 = durations.iter().sum();
        Some((sum / durations.len() as i64) as u64)
    };
    LifecycleMetrics {
        archived_in_window,
        avg_time_to_archive_secs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo_view::{ChangeInstance, RepoView};
    use crate::types::{ArtifactStatus, ChangeData, WorkspaceFolder};

    fn change(id: &str, completed: usize, total: usize, specs: &[&str]) -> ChangeData {
        ChangeData {
            change_id: id.to_string(),
            title: Some(format!("Title {id}")),
            sections: vec![],
            total_tasks: total,
            completed_tasks: completed,
            artifacts: ArtifactStatus {
                proposal: true,
                specs: specs.iter().map(|s| s.to_string()).collect(),
                design: false,
                tasks: total > 0,
            },
            workspace: WorkspaceFolder {
                uri: PathBuf::from(format!("/r/{id}")),
                name: "ws".into(),
            },
        }
    }

    fn instance(
        worktree: &str,
        change: ChangeData,
        modified_at: u64,
        archived: bool,
    ) -> ChangeInstance {
        ChangeInstance {
            worktree_path: PathBuf::from(worktree),
            branch: Some("main".into()),
            is_main_worktree: true,
            is_default_branch: true,
            is_archived_here: archived,
            change,
            modified_at,
            divergence: None,
        }
    }

    fn repo_view(
        name: &str,
        active: Vec<LogicalChange>,
        archived: Vec<LogicalChange>,
    ) -> WorkspaceView {
        WorkspaceView::Repo(RepoView {
            repo_id: PathBuf::from(format!("/{name}/.git")),
            main_worktree: PathBuf::from(format!("/{name}")),
            name: name.to_string(),
            default_branch: Some("main".into()),
            active,
            archived,
            display_name: None,
            color: None,
        })
    }

    fn logical(name: &str, instances: Vec<ChangeInstance>) -> LogicalChange {
        LogicalChange {
            name: name.to_string(),
            instances,
        }
    }

    fn flat(name: &str, changes: Vec<ChangeData>) -> WorkspaceView {
        WorkspaceView::Flat {
            workspace: WorkspaceFolder {
                uri: PathBuf::from(format!("/flat/{name}")),
                name: name.to_string(),
            },
            changes,
            display_name: None,
            color: None,
        }
    }

    #[test]
    fn summary_sums_across_repos_and_flats() {
        let views = vec![
            repo_view(
                "alpha",
                vec![
                    logical(
                        "a",
                        vec![instance("/alpha", change("a", 2, 4, &["cap"]), 100, false)],
                    ),
                    logical(
                        "b",
                        vec![instance("/alpha", change("b", 1, 1, &[]), 90, false)],
                    ),
                ],
                vec![logical(
                    "z",
                    vec![instance("/alpha", change("z", 0, 0, &[]), 1, true)],
                )],
            ),
            flat("beta", vec![change("c", 3, 3, &["cap2"])]),
        ];
        let s = summary_metrics(&views);
        assert_eq!(s.active_changes, 3); // a, b (repo) + c (flat)
        assert_eq!(s.completed_tasks, 2 + 1 + 3);
        assert_eq!(s.total_tasks, 4 + 1 + 3);
        assert_eq!(s.task_percent, 75); // 6/8
        assert_eq!(s.specs_touching, 2); // a + c
        assert_eq!(s.repo_count, 1);
        assert_eq!(s.flat_count, 1);
        assert_eq!(s.worktree_count, 1); // one distinct worktree path in alpha
    }

    #[test]
    fn summary_zero_tasks_yields_zero_percent_not_division_by_zero() {
        let views = vec![flat("beta", vec![change("c", 0, 0, &[])])];
        let s = summary_metrics(&views);
        assert_eq!(s.total_tasks, 0);
        assert_eq!(s.task_percent, 0);
    }

    #[test]
    fn breakdown_reports_active_and_archived_per_entry() {
        let views = vec![
            repo_view(
                "alpha",
                vec![logical(
                    "a",
                    vec![instance("/alpha", change("a", 0, 0, &[]), 1, false)],
                )],
                vec![
                    logical(
                        "y",
                        vec![instance("/alpha", change("y", 0, 0, &[]), 1, true)],
                    ),
                    logical(
                        "z",
                        vec![instance("/alpha", change("z", 0, 0, &[]), 1, true)],
                    ),
                ],
            ),
            flat("beta", vec![change("c", 0, 0, &[]), change("d", 0, 0, &[])]),
        ];
        let b = repo_breakdowns(&views);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].label, "alpha");
        assert_eq!(b[0].active_count, 1);
        assert_eq!(b[0].archived_count, 2);
        assert_eq!(b[1].label, "beta");
        assert_eq!(b[1].active_count, 2);
        assert_eq!(b[1].archived_count, 0);
    }

    #[test]
    fn recent_feed_orders_by_mtime_and_caps() {
        let views = vec![repo_view(
            "alpha",
            vec![
                logical(
                    "old",
                    vec![instance("/alpha", change("old", 0, 0, &[]), 100, false)],
                ),
                logical(
                    "new",
                    vec![instance("/alpha", change("new", 0, 0, &[]), 300, false)],
                ),
                logical(
                    "mid",
                    vec![instance("/alpha", change("mid", 0, 0, &[]), 200, false)],
                ),
            ],
            vec![],
        )];
        let r = recent_entries(&views, 2);
        assert_eq!(r.len(), 2); // capped
        assert_eq!(r[0].change_id, "new");
        assert_eq!(r[1].change_id, "mid");
    }

    #[test]
    fn bucket_activity_groups_by_day_across_repos() {
        let dates = vec![
            "2026-05-29T10:00:00-07:00".to_string(),
            "2026-05-29T23:30:00-07:00".to_string(),
            "2026-05-28T08:00:00-07:00".to_string(),
        ];
        let buckets = bucket_activity(&dates);
        assert_eq!(buckets.len(), 2);
        // BTreeMap → ascending day order.
        assert_eq!(buckets[0].day, "2026-05-28");
        assert_eq!(buckets[0].commit_count, 1);
        assert_eq!(buckets[1].day, "2026-05-29");
        assert_eq!(buckets[1].commit_count, 2);
    }

    #[test]
    fn lifecycle_metrics_window_and_average() {
        // now = 1_000_000; window = 1 day (86_400s); cutoff = 913_600.
        let now = 1_000_000u64;
        let lifecycles = vec![
            // Archived inside the window, full lifecycle of 100s.
            ChangeLifecycle {
                change_name: "in".into(),
                created_at: Some(950_000),
                archived_at: Some(950_100),
            },
            // Archived before the window — excluded from throughput; lifecycle
            // of 300s still counts toward the average.
            ChangeLifecycle {
                change_name: "old".into(),
                created_at: Some(500_000),
                archived_at: Some(500_300),
            },
            // Never archived — contributes to neither.
            ChangeLifecycle {
                change_name: "active".into(),
                created_at: Some(900_000),
                archived_at: None,
            },
        ];
        let m = lifecycle_metrics(&lifecycles, now, 1);
        assert_eq!(m.archived_in_window, 1); // only "in"
        assert_eq!(m.avg_time_to_archive_secs, Some((100 + 300) / 2));
    }

    #[test]
    fn lifecycle_metrics_no_recoverable_average_is_none() {
        let now = 1_000_000u64;
        let lifecycles = vec![ChangeLifecycle {
            change_name: "x".into(),
            created_at: None,
            archived_at: Some(950_000),
        }];
        let m = lifecycle_metrics(&lifecycles, now, 1);
        assert_eq!(m.archived_in_window, 1);
        assert_eq!(m.avg_time_to_archive_secs, None);
    }

    #[test]
    fn compute_dashboard_uses_git_only_for_repos() {
        let views = vec![
            repo_view(
                "alpha",
                vec![logical(
                    "a",
                    vec![instance("/alpha", change("a", 1, 2, &[]), 100, false)],
                )],
                vec![],
            ),
            flat("beta", vec![change("c", 0, 0, &[])]),
        ];
        // The closures are invoked only for the Repo view; the flat view must
        // not reach them (it has no RepoId / history).
        let data = compute_dashboard(
            &views,
            1_000_000,
            14,
            10,
            |repo| {
                assert!(repo.as_path().to_string_lossy().contains("alpha"));
                vec!["2026-05-29T10:00:00-07:00".to_string()]
            },
            |_repo| {
                vec![ChangeLifecycle {
                    change_name: "a".into(),
                    created_at: Some(999_000),
                    archived_at: Some(999_500),
                }]
            },
        );
        assert_eq!(data.summary.active_changes, 2);
        assert_eq!(data.activity_window_days, 14);
        assert_eq!(data.activity.len(), 1);
        assert_eq!(data.activity[0].commit_count, 1);
        assert_eq!(data.lifecycle.archived_in_window, 1);
        assert_eq!(data.recent.len(), 2);
        // compute_dashboard leaves progress at its default; it's filled by the
        // IPC layer from the activity log.
        assert_eq!(data.progress, ProgressData::default());
    }

    fn ach(kind: AchievementKind, day_offset: i64, mag: u32) -> Achievement {
        // day_offset days before a fixed noon "today" so local_day is stable.
        let today_noon = 1_700_000_000i64; // arbitrary fixed instant
        Achievement::new(
            kind,
            today_noon - day_offset * 86_400,
            PathBuf::from("/ws"),
            None,
            mag,
        )
    }

    /// Build an ascending day axis of `n` days ending at the local day of the
    /// fixed "today" instant used by `ach`.
    fn axis_and_today(n: usize) -> (Vec<String>, String) {
        let today_noon = 1_700_000_000i64;
        let today = crate::activity_log::local_day(today_noon);
        let mut axis: Vec<String> = (0..n as i64)
            .rev()
            .map(|i| crate::activity_log::local_day(today_noon - i * 86_400))
            .collect();
        axis.dedup();
        (axis, today)
    }

    #[test]
    fn progress_counts_today_and_streak() {
        let (axis, today) = axis_and_today(14);
        let achievements = vec![
            ach(AchievementKind::TaskCompleted, 0, 3), // today: 3 tasks
            ach(AchievementKind::ChangeArchived, 0, 1), // today: 1 ship
            ach(AchievementKind::TaskCompleted, 1, 2), // yesterday active
            ach(AchievementKind::TaskCompleted, 2, 1), // 2 days ago active
        ];
        let commit_days: Vec<String> = vec![]; // no commits
        let p = compute_progress(&achievements, &commit_days, &axis, &today);
        assert_eq!(p.today.tasks_completed, 3);
        assert_eq!(p.today.changes_archived, 1);
        assert_eq!(p.today.commits_landed, 0);
        // today + yesterday + 2-days-ago all active → streak of 3.
        assert_eq!(p.streak.current, 3);
        assert_eq!(p.heatmap.len(), 14);
        assert_eq!(p.heatmap.last().unwrap().day, today);
    }

    #[test]
    fn progress_gap_breaks_streak() {
        let (axis, today) = axis_and_today(14);
        let achievements = vec![
            ach(AchievementKind::TaskCompleted, 0, 1), // today
            // gap at day 1 (no event)
            ach(AchievementKind::TaskCompleted, 2, 1), // 2 days ago
        ];
        let p = compute_progress(&achievements, &[], &axis, &today);
        assert_eq!(p.streak.current, 1); // only today
    }

    #[test]
    fn progress_commits_sustain_a_day() {
        let (axis, today) = axis_and_today(14);
        // No achievements at all; a commit today keeps the streak alive.
        let commit_days = vec![today.clone()];
        let p = compute_progress(&[], &commit_days, &axis, &today);
        assert_eq!(p.today.commits_landed, 1);
        assert_eq!(p.streak.current, 1);
    }

    #[test]
    fn progress_milestones_cross_on_cumulative_totals() {
        let (axis, today) = axis_and_today(14);
        // 10 tasks today crosses the first task milestone; one ship crosses
        // first-ship.
        let achievements = vec![
            ach(AchievementKind::TaskCompleted, 0, 10),
            ach(AchievementKind::ChangeArchived, 0, 1),
        ];
        let p = compute_progress(&achievements, &[], &axis, &today);
        assert!(p
            .milestones
            .iter()
            .any(|m| m.id == "tasks-10" && m.achieved));
        assert!(p.milestones.iter().any(|m| m.id == "first-ship"));
        // 50-task milestone not reached.
        assert!(!p.milestones.iter().any(|m| m.id == "tasks-50"));
    }

    #[test]
    fn progress_backfilled_milestone_is_flagged() {
        let (axis, today) = axis_and_today(14);
        let archived = ach(AchievementKind::ChangeArchived, 0, 1).as_backfilled();
        let p = compute_progress(&[archived], &[], &axis, &today);
        let fs = p.milestones.iter().find(|m| m.id == "first-ship").unwrap();
        assert!(fs.backfilled);
    }
}
