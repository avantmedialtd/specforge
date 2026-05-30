//! Aggregates registered workspaces + git-mined history into the global
//! Dashboard payload the frontend renders as its home surface.
//!
//! Mirrors the `repo_view` split: pure transforms over [`WorkspaceView`]s and
//! injected git data, so the whole thing is unit-testable from `cargo test`
//! with no GUI and no real git. The Tauri `get_dashboard` command supplies the
//! git closures (commit activity + change lifecycle); tests inject fixtures.

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
    }
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
    }
}
