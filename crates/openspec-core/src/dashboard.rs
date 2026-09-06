//! Aggregates registered workspaces + git-mined history into the global
//! Dashboard payload the frontend renders as its home surface.
//!
//! Mirrors the `repo_view` split: pure transforms over [`WorkspaceView`]s and
//! injected git data, so the whole thing is unit-testable from `cargo test`
//! with no GUI and no real git. The Tauri `get_dashboard` command supplies the
//! git closures (commit activity + change lifecycle); tests inject fixtures.

use crate::activity_log::{Achievement, AchievementKind};
use crate::git::{ChangeLifecycle, RepoId};
use crate::parser::archive_dir_date;
use crate::repo_view::{LogicalChange, RepoView, WorkspaceView};
use crate::types::ChangeData;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// Everything the Dashboard renders, aggregated across all workspaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardData {
    pub summary: SummaryMetrics,
    pub repos: Vec<RepoBreakdown>,
    /// How many days the lifecycle throughput window spans. The frontend
    /// presents it alongside the figures it bounds — since the commits chart
    /// was removed, nothing else on screen defines the window.
    pub lifecycle_window_days: u64,
    pub lifecycle: LifecycleMetrics,
    pub todays_ships: Vec<ShipEntry>,
    /// Progress layer — today's haul, streak, heatmap — derived from the
    /// activity log. Defaulted by [`compute_dashboard`]; the IPC layer fills it
    /// via [`compute_progress`] from the activity log.
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
    /// In-flight (active, non-archived) change count for the hero's second
    /// tile: the changes the developer created, per the personal frame. A live
    /// state level (not a today-flow count), so it carries no average.
    #[serde(default)]
    pub in_flight: u32,
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

/// One change archived today, with enough identity to deep-link into the
/// Archive browser.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShipEntry {
    /// Bare logical change id (date prefix stripped) — for display and to
    /// address the change in navigation.
    pub change_id: String,
    pub title: Option<String>,
    /// Display name of the owning repo or flat workspace.
    pub workspace_label: String,
    /// Git common dir of the owning repository — the top-level row identity,
    /// matching `RepoView::repo_id` and `RegisteredWorkspace::repo_id`.
    pub repo_id: PathBuf,
    /// The registered workspace (worktree) path whose `openspec/changes/archive/`
    /// holds the change — the Archive browser is opened scoped to it.
    pub worktree_path: PathBuf,
    /// The dated `YYYY-MM-DD-<id>` archive directory name, addressing the
    /// archive entry so the Archive reader can open it.
    pub archive_dir: String,
    /// Git-recovered archival instant (Unix epoch seconds); `None` when git
    /// could not supply it. Drives the "archived 2h ago" label and intra-day
    /// ordering, and is omitted under graceful degradation.
    pub archived_at: Option<u64>,
}

/// Aggregate the Dashboard payload. Pure given the injected git closures:
/// `lifecycle_for` returns a repo's change lifecycles, and `ship_title_for`
/// resolves an archived change's title from `(worktree_path, dated_dir)` — it
/// is called only for the handful of changes shipped today. `now_unix` is the
/// current time in epoch seconds; `window_days` bounds throughput; and `today`
/// is the viewer's local `YYYY-MM-DD`, which scopes the today's-ships feed.
/// Flat (non-git) workspaces contribute to the counts but not to the
/// git-derived sections or the ships feed (they carry no archive section).
pub fn compute_dashboard(
    views: &[WorkspaceView],
    now_unix: u64,
    window_days: u64,
    today: &str,
    lifecycle_for: impl Fn(&RepoId) -> Vec<ChangeLifecycle>,
    ship_title_for: impl Fn(&Path, &str) -> Option<String>,
) -> DashboardData {
    let summary = summary_metrics(views);
    let repos = repo_breakdowns(views);

    let mut lifecycles: Vec<ChangeLifecycle> = Vec::new();
    let mut todays_ships: Vec<ShipEntry> = Vec::new();
    for view in views {
        if let WorkspaceView::Repo(repo) = view {
            let repo_id = RepoId(repo.repo_id.clone());
            // Mine the repo's lifecycle once, then reuse it for both the
            // throughput metrics and the ships' archival instants.
            let lcs = lifecycle_for(&repo_id);
            todays_ships.extend(repo_ships(repo, today, &lcs, &ship_title_for));
            lifecycles.extend(lcs);
        }
    }
    // Interleave ships from every repo by archival instant, newest first.
    // A missing instant (`None`) sinks below the timed ships and is broken by
    // the dated directory name so the order stays deterministic without git.
    todays_ships.sort_by(|a, b| {
        b.archived_at
            .cmp(&a.archived_at)
            .then_with(|| b.archive_dir.cmp(&a.archive_dir))
    });

    DashboardData {
        summary,
        repos,
        lifecycle_window_days: window_days,
        lifecycle: lifecycle_metrics(&lifecycles, now_unix, window_days),
        todays_ships,
        progress: ProgressData::default(),
    }
}

/// Compute the progress layer from the activity log. Pure given the
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

    ProgressData {
        today: today_progress,
        streak,
        heatmap,
        // The IPC layer overwrites this with the scope-aware active count; the
        // pure progress computation has no view of active changes.
        in_flight: 0,
    }
}

/// Mean over the active days (nonzero count) among the 30 most-recent calendar
/// days strictly *before* `anchor`, scaled ×100. Resting days don't depress the
/// bar. Backs the Today's-Progress hero's ▲/▼ comparison badges, which are the
/// only consumer — see `today_averages_cover_active_days_before_today_only`,
/// the test that guards this helper against being deleted with a caller.
fn trailing_avg_centi(by_day: &BTreeMap<String, u32>, day_axis: &[String], anchor: &str) -> u32 {
    let recent: Vec<u32> = day_axis
        .iter()
        .filter(|d| d.as_str() < anchor)
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
    anchor: &str,
) -> u32 {
    let recent: Vec<u32> = day_axis
        .iter()
        .filter(|d| d.as_str() < anchor)
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
///
/// Rows are returned ordered by active count descending, then archived count
/// descending, then label ascending, so the Dashboard leads with the entries
/// carrying work in flight rather than with whatever was registered first.
/// All three keys matter: in a registry where most entries are quiet the active
/// count ties at zero for the majority and the archived count becomes the
/// effective order, while the label is what stops two entries with identical
/// counts trading places between refreshes — a swap the frontend would render
/// and no membership assertion would catch.
///
/// The ordering lives here rather than in the frontend because a permutation is
/// invisible to any sum, so it costs the payload's other consumers nothing. The
/// frontend caps the list for height (`dashboard`: *Per-Repository Breakdown*);
/// it does not re-sort, and it keeps the whole vector for the registry-wide
/// archived total in its footnote.
pub fn repo_breakdowns(views: &[WorkspaceView]) -> Vec<RepoBreakdown> {
    let mut rows: Vec<RepoBreakdown> = views
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
        .collect();
    rows.sort_by(|a, b| {
        b.active_count
            .cmp(&a.active_count)
            .then_with(|| b.archived_count.cmp(&a.archived_count))
            .then_with(|| a.label.cmp(&b.label))
    });
    rows
}

/// One repo's "today's ships": the logical changes with an archived instance
/// whose dated directory (`archive/<YYYY-MM-DD>-<id>/`) matches `today`, each
/// joined to the repo's lifecycles for the git-recovered archival instant.
/// Membership comes from the dated directory alone (no git); the instant is
/// enrichment, absent when git could not recover it. Returned unsorted — the
/// caller interleaves ships from every repo.
///
/// **Both sections are scanned, not just `archived`.** A `LogicalChange` is
/// keyed on the bare logical id, so a change archived inside a feature worktree
/// while the main worktree still holds it active is ONE logical change with a
/// mix of instances — which buckets into `active`. That is the ordinary shape
/// of an archival in this project's workflow, and reading only `archived` would
/// drop precisely those ships until the branch merged.
///
/// The dated directory and the worktree both come from the SAME archived
/// instance, because with the bare-id key two worktrees' copies can carry
/// different directory names; taking them from different places would name a
/// directory that does not exist in the named worktree.
fn repo_ships(
    repo: &RepoView,
    today: &str,
    lcs: &[ChangeLifecycle],
    ship_title_for: &impl Fn(&Path, &str) -> Option<String>,
) -> Vec<ShipEntry> {
    let label = repo
        .display_name
        .clone()
        .unwrap_or_else(|| repo.name.clone());
    // Archival instant keyed by the BARE logical id, which is how both
    // `change_lifecycle` and `LogicalChange::name` now name an archived change.
    let archived_at_by_id: HashMap<&str, i64> = lcs
        .iter()
        .filter_map(|lc| lc.archived_at.map(|at| (lc.change_name.as_str(), at)))
        .collect();
    repo.active
        .iter()
        .chain(repo.archived.iter())
        .filter_map(|lc| {
            // Instances are ordered most-recently-modified first, so this is
            // the freshest copy archived today.
            let inst = lc.instances.iter().find(|i| {
                i.is_archived_here && archive_dir_date(&i.change.change_id) == Some(today)
            })?;
            let archive_dir = inst.change.change_id.clone();
            let worktree_path = inst.worktree_path.clone();
            let archived_at = archived_at_by_id
                .get(lc.name.as_str())
                .copied()
                .filter(|at| *at >= 0)
                .map(|at| at as u64);
            let title = ship_title_for(&worktree_path, &archive_dir);
            Some(ShipEntry {
                // Already the bare logical id — `build_repo_view` keys every
                // instance, archived or not, on it. Re-stripping here would be
                // a second date strip on a name that has had exactly one.
                change_id: lc.name.clone(),
                title,
                workspace_label: label.clone(),
                repo_id: repo.repo_id.clone(),
                worktree_path,
                archive_dir,
                archived_at,
            })
        })
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
    use crate::identity::{Author, IdentityConfig};
    use crate::parser::archive_dir_logical_id;
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
            spec_commit_state: crate::git::SpecCommitState::Committed,
        }
    }

    fn repo_view(
        name: &str,
        active: Vec<LogicalChange>,
        archived: Vec<LogicalChange>,
    ) -> WorkspaceView {
        WorkspaceView::Repo(RepoView {
            disabled: false,
            repo_id: PathBuf::from(format!("/{name}/.git")),
            main_worktree: PathBuf::from(format!("/{name}")),
            name: name.to_string(),
            default_branch: Some("main".into()),
            active,
            archived,
            display_name: None,
            color: None,
            dirty: false,
            dirty_worktrees: vec![],
            has_uncommitted_specs: false,
            worktrees: vec![],
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
            disabled: false,
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
        // beta leads on active count (2 > 1) despite being registered second.
        assert_eq!(b[0].label, "beta");
        assert_eq!(b[0].active_count, 2);
        assert_eq!(b[0].archived_count, 0);
        assert_eq!(b[1].label, "alpha");
        assert_eq!(b[1].active_count, 1);
        assert_eq!(b[1].archived_count, 2);
    }

    /// The comparator is three keys deep and only the first is obvious, so this
    /// fixture ties deliberately on the first two: dropping the archived key or
    /// the label key must fail here, not merely reorder something invisibly.
    ///
    /// - `busiest` (2 active) leads on the primary key alone.
    /// - `rich` and the three 1-active entries tie on active; `rich` wins on
    ///   archived count.
    /// - `dup-a`, `dup-b` and `poor` tie on BOTH counts, so only the label
    ///   separates them — the key whose absence shows up as rows swapping
    ///   places between refreshes rather than as a wrong-looking list.
    ///
    /// Registration order is the reverse of the expected output, so an absent
    /// sort cannot pass by coincidence.
    #[test]
    fn breakdown_orders_by_active_then_archived_then_label() {
        let one_active = || {
            vec![logical(
                "a",
                vec![instance("/w", change("a", 0, 0, &[]), 1, false)],
            )]
        };
        let views = vec![
            flat("poor", vec![change("c", 0, 0, &[])]),
            flat("dup-b", vec![change("c", 0, 0, &[])]),
            flat("dup-a", vec![change("c", 0, 0, &[])]),
            repo_view(
                "rich",
                one_active(),
                vec![
                    logical("y", vec![instance("/w", change("y", 0, 0, &[]), 1, true)]),
                    logical("z", vec![instance("/w", change("z", 0, 0, &[]), 1, true)]),
                ],
            ),
            flat(
                "busiest",
                vec![change("c", 0, 0, &[]), change("d", 0, 0, &[])],
            ),
        ];

        let labels: Vec<String> = repo_breakdowns(&views)
            .into_iter()
            .map(|b| b.label)
            .collect();
        assert_eq!(
            labels,
            vec!["busiest", "rich", "dup-a", "dup-b", "poor"],
            "ordering is active desc, then archived desc, then label asc"
        );
    }

    /// Ordering must be a pure function of the counts, so the same registry in a
    /// different registration order produces the identical list. This is the
    /// property the frontend depends on when it slices the front of the vector.
    #[test]
    fn breakdown_order_is_independent_of_registration_order() {
        let mk = |labels: [&str; 3]| -> Vec<WorkspaceView> {
            labels
                .iter()
                .map(|l| flat(l, vec![change("c", 0, 0, &[])]))
                .collect()
        };
        let forward: Vec<String> = repo_breakdowns(&mk(["a", "b", "c"]))
            .into_iter()
            .map(|b| b.label)
            .collect();
        let reversed: Vec<String> = repo_breakdowns(&mk(["c", "b", "a"]))
            .into_iter()
            .map(|b| b.label)
            .collect();
        assert_eq!(forward, reversed);
        assert_eq!(forward, vec!["a", "b", "c"]);
    }

    /// A `RepoView` carrying only an archived section — the input `repo_ships`
    /// reads.
    fn ship_repo(name: &str, archived: Vec<LogicalChange>) -> RepoView {
        let WorkspaceView::Repo(rv) = repo_view(name, vec![], archived) else {
            unreachable!()
        };
        rv
    }

    /// A wholly-archived logical change exactly as `build_repo_view` emits
    /// one: the row is named by the BARE logical id while its instance keeps
    /// the dated directory that addresses the read.
    fn arch(dated_dir: &str) -> LogicalChange {
        logical(
            archive_dir_logical_id(dated_dir),
            vec![instance("/alpha", change(dated_dir, 0, 0, &[]), 0, true)],
        )
    }

    /// A lifecycle whose archival instant the ship builder joins by the bare
    /// logical id (how `change_lifecycle` names an archived change).
    fn life(bare_id: &str, archived_at: i64) -> ChangeLifecycle {
        ChangeLifecycle {
            change_name: bare_id.into(),
            created_at: None,
            archived_at: Some(archived_at),
            ..Default::default()
        }
    }

    #[test]
    fn ships_filter_to_today_and_join_clock_and_title() {
        let repo = ship_repo(
            "alpha",
            vec![arch("2026-06-08-foo"), arch("2026-06-07-bar")],
        );
        // Only `foo` has a recovered instant; `bar` is yesterday's archive.
        let lcs = vec![life("foo", 1_700)];
        let ships = repo_ships(&repo, "2026-06-08", &lcs, &|_p, dir| {
            Some(format!("T-{dir}"))
        });
        assert_eq!(ships.len(), 1); // bar excluded — not today
        let s = &ships[0];
        assert_eq!(s.change_id, "foo"); // bare id, date prefix stripped
        assert_eq!(s.archive_dir, "2026-06-08-foo");
        assert_eq!(s.archived_at, Some(1_700));
        assert_eq!(s.title.as_deref(), Some("T-2026-06-08-foo"));
        assert_eq!(s.worktree_path, PathBuf::from("/alpha"));
        // The owning row's identity, not the worktree's — this is what lets a
        // consumer tell whether a ship's repository is currently parked.
        assert_eq!(s.repo_id, PathBuf::from("/alpha/.git"));
    }

    #[test]
    fn ship_without_lifecycle_lists_without_clock() {
        let repo = ship_repo("alpha", vec![arch("2026-06-08-foo")]);
        // No git instant available, and the title resolver finds nothing.
        let ships = repo_ships(&repo, "2026-06-08", &[], &|_p, _d| None);
        assert_eq!(ships.len(), 1);
        assert_eq!(ships[0].archived_at, None); // renders without the clock
        assert_eq!(ships[0].title, None); // frontend falls back to the id
        assert_eq!(ships[0].change_id, "foo");
    }

    #[test]
    fn a_change_archived_in_one_worktree_while_active_in_another_still_ships() {
        // The ordinary shape of an archival in this project: the feature
        // worktree archives, the main worktree still holds the change active
        // until the branch merges. Keyed on the bare logical id those are ONE
        // logical change with a mixed instance set, which buckets into
        // `active` — so reading only `repo.archived` would drop the ship.
        let WorkspaceView::Repo(repo) = repo_view(
            "alpha",
            vec![logical(
                "foo",
                vec![
                    instance("/alpha/wt", change("2026-06-08-foo", 0, 0, &[]), 200, true),
                    instance("/alpha", change("foo", 0, 0, &[]), 100, false),
                ],
            )],
            vec![],
        ) else {
            unreachable!()
        };
        let ships = repo_ships(&repo, "2026-06-08", &[life("foo", 1_700)], &|p, dir| {
            Some(format!("{}:{dir}", p.display()))
        });

        assert_eq!(ships.len(), 1);
        assert_eq!(ships[0].change_id, "foo");
        assert_eq!(ships[0].archive_dir, "2026-06-08-foo");
        // Directory and worktree come from the SAME archived instance, so the
        // pair names something that exists: with the bare-id key two worktrees'
        // copies can carry different directory names.
        assert_eq!(ships[0].worktree_path, PathBuf::from("/alpha/wt"));
        assert_eq!(ships[0].title.as_deref(), Some("/alpha/wt:2026-06-08-foo"));
        assert_eq!(ships[0].archived_at, Some(1_700));
    }

    #[test]
    fn an_active_instance_named_like_a_dated_directory_is_not_a_ship() {
        // Membership is "has an ARCHIVED instance dated today", not "is named
        // like a dated directory" — an active change whose own id starts with
        // a date must not be reported as shipped.
        let WorkspaceView::Repo(repo) = repo_view(
            "alpha",
            vec![logical(
                "2026-06-08-foo",
                vec![instance(
                    "/alpha",
                    change("2026-06-08-foo", 0, 0, &[]),
                    100,
                    false,
                )],
            )],
            vec![],
        ) else {
            unreachable!()
        };
        assert!(repo_ships(&repo, "2026-06-08", &[], &|_p, _d| None).is_empty());
    }

    #[test]
    fn nothing_archived_today_yields_no_ships() {
        let repo = ship_repo("alpha", vec![arch("2026-06-01-old")]);
        let ships = repo_ships(&repo, "2026-06-08", &[life("old", 100)], &|_p, _d| None);
        assert!(ships.is_empty());
    }

    #[test]
    fn compute_dashboard_orders_todays_ships_newest_first() {
        let views = vec![repo_view(
            "alpha",
            vec![],
            vec![arch("2026-06-08-a"), arch("2026-06-08-b")],
        )];
        let data = compute_dashboard(
            &views,
            1_000_000,
            14,
            "2026-06-08",
            |_repo| vec![life("a", 100), life("b", 300)],
            |_p, _d| None,
        );
        assert_eq!(data.todays_ships.len(), 2);
        assert_eq!(data.todays_ships[0].change_id, "b"); // 300 newest first
        assert_eq!(data.todays_ships[1].change_id, "a");
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
                ..Default::default()
            },
            // Archived before the window — excluded from throughput; lifecycle
            // of 300s still counts toward the average.
            ChangeLifecycle {
                change_name: "old".into(),
                created_at: Some(500_000),
                archived_at: Some(500_300),
                ..Default::default()
            },
            // Never archived — contributes to neither.
            ChangeLifecycle {
                change_name: "active".into(),
                created_at: Some(900_000),
                archived_at: None,
                ..Default::default()
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
            ..Default::default()
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
            "2026-05-29",
            |repo| {
                assert!(repo.as_path().to_string_lossy().contains("alpha"));
                vec![ChangeLifecycle {
                    change_name: "a".into(),
                    created_at: Some(999_000),
                    archived_at: Some(999_500),
                    ..Default::default()
                }]
            },
            |_p, _d| None,
        );
        assert_eq!(data.summary.active_changes, 2);
        assert_eq!(data.lifecycle_window_days, 14);
        assert_eq!(data.lifecycle.archived_in_window, 1);
        // No archived changes in the fixture, so nothing shipped today.
        assert!(data.todays_ships.is_empty());
        // compute_dashboard leaves progress at its default; it's filled by the
        // IPC layer from the activity log.
        assert_eq!(data.progress, ProgressData::default());
    }

    /// Payload equivalence (cache-change-lifecycle task 5.3): `DashboardData`
    /// computed with the `lifecycle_for` closure backed by `LifecycleCache`
    /// must be identical to the same computation calling the same underlying
    /// miner directly — over a fixture registry, no real git involved.
    #[test]
    fn compute_dashboard_via_cache_matches_direct_computation() {
        let views = vec![
            repo_view(
                "alpha",
                vec![logical(
                    "a",
                    vec![instance("/alpha", change("a", 1, 2, &[]), 100, false)],
                )],
                vec![arch("2026-06-08-shipped")],
            ),
            flat("beta", vec![change("c", 0, 0, &[])]),
        ];

        let lifecycle_for = |repo: &RepoId| -> Vec<ChangeLifecycle> {
            if repo.as_path().to_string_lossy().contains("alpha") {
                vec![
                    ChangeLifecycle {
                        change_name: "a".into(),
                        created_at: Some(900_000),
                        archived_at: None,
                        ..Default::default()
                    },
                    life("shipped", 999_500),
                ]
            } else {
                vec![]
            }
        };
        let ship_title_for = |_p: &Path, dir: &str| Some(format!("T-{dir}"));

        let direct = compute_dashboard(
            &views,
            1_000_000,
            14,
            "2026-06-08",
            lifecycle_for,
            ship_title_for,
        );

        let cache = crate::LifecycleCache::new();
        let cached = compute_dashboard(
            &views,
            1_000_000,
            14,
            "2026-06-08",
            |repo| {
                cache.get_or_compute(repo, |r| {
                    Ok::<_, crate::git::LifecycleError>(lifecycle_for(r))
                })
            },
            ship_title_for,
        );
        assert_eq!(
            direct, cached,
            "DashboardData computed through the cache must match the direct computation"
        );

        // A second cached read (now warm, served from the cache hit rather
        // than the miner) must still match — the cache itself must not
        // perturb the payload.
        let cached_again = compute_dashboard(
            &views,
            1_000_000,
            14,
            "2026-06-08",
            |repo| {
                cache.get_or_compute(repo, |r| {
                    Ok::<_, crate::git::LifecycleError>(lifecycle_for(r))
                })
            },
            ship_title_for,
        );
        assert_eq!(direct, cached_again);
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

    /// The Today's-Progress hero's ▲/▼ comparison badges are driven entirely by
    /// the three `*_avg_centi` fields, which come from the private
    /// `trailing_avg_centi` / `commits_trailing_avg_centi` helpers. Those helpers
    /// are shared, so deleting a caller must not take them with it — nothing else
    /// asserts these fields, and losing them is a silent UI regression rather
    /// than a type error (`dashboard`: *Today's Progress Hero*, scenario
    /// *Comparison to recent daily average*).
    #[test]
    fn today_averages_cover_active_days_before_today_only() {
        let (axis, today) = axis_and_today(14);
        let achievements = vec![
            // Today's own activity must NOT feed the trailing averages.
            ach(AchievementKind::TaskCompleted, 0, 9),
            ach(AchievementKind::ChangeArchived, 0, 5),
            // Tasks on two prior active days: (2 + 4) / 2 = 3.00
            ach(AchievementKind::TaskCompleted, 1, 2),
            ach(AchievementKind::TaskCompleted, 3, 4),
            // Ships on two prior active days: (1 + 3) / 2 = 2.00
            ach(AchievementKind::ChangeArchived, 2, 1),
            ach(AchievementKind::ChangeArchived, 4, 3),
        ];
        // Commits on two prior active days: (2 + 1) / 2 = 1.50. Two land today
        // and are likewise excluded.
        let commits = vec![
            local_day_at(0),
            local_day_at(0),
            local_day_at(1),
            local_day_at(1),
            local_day_at(5),
        ];

        let p = compute_progress(&achievements, &commits, &axis, &today);

        assert_eq!(
            p.today.tasks_completed, 9,
            "today's own count is unaffected"
        );
        assert_eq!(p.today.changes_archived, 5);
        assert_eq!(p.today.commits_landed, 2);

        // Averages are over *active* days strictly before today — a quiet day
        // is skipped rather than dragging the mean to zero.
        assert_eq!(p.today.tasks_avg_centi, 300);
        assert_eq!(p.today.changes_archived_avg_centi, 200);
        assert_eq!(p.today.commits_avg_centi, 150);
    }

    /// With no history before today, every comparison is defined as zero — the
    /// frontend renders no badge rather than dividing by an empty window.
    #[test]
    fn today_averages_are_zero_without_prior_history() {
        let (axis, today) = axis_and_today(14);
        let achievements = vec![
            ach(AchievementKind::TaskCompleted, 0, 3),
            ach(AchievementKind::ChangeArchived, 0, 1),
        ];
        let p = compute_progress(&achievements, &[local_day_at(0)], &axis, &today);
        assert_eq!(p.today.tasks_avg_centi, 0);
        assert_eq!(p.today.changes_archived_avg_centi, 0);
        assert_eq!(p.today.commits_avg_centi, 0);
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

    fn author(name: Option<&str>, email: Option<&str>) -> Author {
        Author::new(name.map(str::to_string), email.map(str::to_string))
    }

    #[test]
    fn event_is_me_filter_narrows_today_totals_to_developer() {
        let (axis, today) = axis_and_today(14);
        let me = author(Some("Me"), Some("me@x.com"));
        let cfg = IdentityConfig {
            display_name: None,
            aliases: vec![me.clone()],
        };
        let all = vec![
            ach(AchievementKind::TaskCompleted, 0, 3).with_author(Some(me)),
            ach(AchievementKind::TaskCompleted, 0, 5)
                .with_author(Some(author(Some("Them"), Some("them@x.com")))),
        ];
        // The progress layer always filters the log via `event_is_me`, as the
        // IPC layer does, yielding the developer's subset — a smaller today
        // total than the unfiltered log.
        let me_only: Vec<_> = all
            .iter()
            .filter(|e| crate::activity_log::event_is_me(e, &cfg))
            .cloned()
            .collect();
        let unfiltered = compute_progress(&all, &[], &axis, &today);
        let mine = compute_progress(&me_only, &[], &axis, &today);
        assert_eq!(unfiltered.today.tasks_completed, 8);
        assert_eq!(mine.today.tasks_completed, 3);
    }

    // The fixed instant `ach`/`axis_and_today` anchor on, for deriving anchors.
    const TODAY_NOON: i64 = 1_700_000_000;

    fn local_day_at(day_offset: i64) -> String {
        crate::activity_log::local_day(TODAY_NOON - day_offset * 86_400)
    }
}
