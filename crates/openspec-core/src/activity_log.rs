//! Append-only activity log: a private record of observed achievements.
//!
//! The log lives in the application's data directory (its path is injected by
//! the Tauri shell, mirroring [`crate`]'s settings pattern), never inside any
//! workspace `openspec/` tree — so SpecForge stays a read-only *viewer* of
//! workspaces while keeping its own diary of what it observed. The Dashboard
//! derives its today, streak, heatmap, and milestone views from this log.
//!
//! Events are append-only: once recorded they are never rewritten or removed,
//! even if the underlying workspace later loses tasks or changes. The log is
//! persisted as a JSON array and rewritten in full on each append; event volume
//! is modest and queries are window-bounded.

use crate::types::{ArtifactStatus, ChangeData};
use chrono::{Duration as ChronoDuration, Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The kind of achievement observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AchievementKind {
    /// One or more tasks were checked off in a change's `tasks.md`.
    TaskCompleted,
    /// A change reached a new artifact status (proposal/design/tasks/specs).
    ArtifactReached,
    /// A new change directory appeared.
    ChangeCreated,
    /// A change moved to the archive ("shipped").
    ChangeArchived,
}

/// A single recorded achievement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Achievement {
    pub kind: AchievementKind,
    /// Unix epoch seconds when the achievement was observed (live) or occurred
    /// (backfilled from git).
    pub timestamp: i64,
    /// The workspace this achievement belongs to.
    pub workspace: PathBuf,
    /// The change involved, when applicable.
    pub change_id: Option<String>,
    /// Units this event represents (e.g. number of tasks checked off at once).
    pub magnitude: u32,
    /// True when reconstructed from git history rather than observed live.
    pub backfilled: bool,
}

impl Achievement {
    /// A live achievement observed at `timestamp`.
    pub fn new(
        kind: AchievementKind,
        timestamp: i64,
        workspace: PathBuf,
        change_id: Option<String>,
        magnitude: u32,
    ) -> Self {
        Self {
            kind,
            timestamp,
            workspace,
            change_id,
            magnitude,
            backfilled: false,
        }
    }

    /// Mark this achievement as reconstructed from history.
    pub fn as_backfilled(mut self) -> Self {
        self.backfilled = true;
        self
    }
}

/// Cumulative per-kind totals, summed by `magnitude` — the basis for milestones.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementTotals {
    pub tasks_completed: u32,
    pub artifacts_reached: u32,
    pub changes_created: u32,
    pub changes_archived: u32,
}

/// Append-only store of [`Achievement`]s, persisted as a JSON array.
pub struct ActivityLog {
    events: Mutex<Vec<Achievement>>,
    path: PathBuf,
}

impl ActivityLog {
    /// Load the log from `path`, falling back to an empty log if it is missing
    /// or unparseable.
    pub fn load(path: PathBuf) -> Self {
        let events = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<Achievement>>(&s).ok())
            .unwrap_or_default();
        Self {
            events: Mutex::new(events),
            path,
        }
    }

    /// Append one achievement and persist.
    pub fn record(&self, event: Achievement) {
        {
            let mut events = self.events.lock().unwrap();
            events.push(event);
        }
        self.persist();
    }

    /// Append many achievements and persist once.
    pub fn record_all(&self, mut new_events: Vec<Achievement>) {
        if new_events.is_empty() {
            return;
        }
        {
            let mut events = self.events.lock().unwrap();
            events.append(&mut new_events);
        }
        self.persist();
    }

    /// True when no achievement has ever been recorded.
    pub fn is_empty(&self) -> bool {
        self.events.lock().unwrap().is_empty()
    }

    /// True when at least one achievement exists for `workspace`. Backfill is
    /// skipped for a workspace that already has history.
    pub fn has_workspace(&self, workspace: &Path) -> bool {
        self.events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.workspace == workspace)
    }

    /// A clone of every recorded achievement (newest order not guaranteed).
    pub fn snapshot(&self) -> Vec<Achievement> {
        self.events.lock().unwrap().clone()
    }

    /// Achievements at or after `cutoff_ts` (unix seconds).
    pub fn query_since(&self, cutoff_ts: i64) -> Vec<Achievement> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.timestamp >= cutoff_ts)
            .cloned()
            .collect()
    }

    /// Achievements within the trailing `window_days` calendar days (including
    /// today), in the viewer's local time zone.
    pub fn query_window(&self, window_days: u32) -> Vec<Achievement> {
        self.query_since(window_start_ts(window_days))
    }

    /// Cumulative per-kind totals across the whole log.
    pub fn totals(&self) -> AchievementTotals {
        totals_of(&self.events.lock().unwrap())
    }

    fn persist(&self) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&*self.events.lock().unwrap()) {
            let _ = std::fs::write(&self.path, json);
        }
    }
}

/// Unix-seconds start (local midnight) of the trailing `window_days` window.
pub fn window_start_ts(window_days: u32) -> i64 {
    let days = window_days.max(1) as i64;
    let start_day = Local::now().date_naive() - ChronoDuration::days(days - 1);
    let start = start_day.and_hms_opt(0, 0, 0).unwrap();
    Local
        .from_local_datetime(&start)
        .earliest()
        .map(|dt| dt.timestamp())
        .unwrap_or(0)
}

/// The local calendar day (`YYYY-MM-DD`) a unix timestamp falls on.
pub fn local_day(timestamp: i64) -> String {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

/// Today's local calendar day (`YYYY-MM-DD`).
pub fn today_str() -> String {
    Local::now().date_naive().format("%Y-%m-%d").to_string()
}

/// Ascending list of local calendar-day strings spanning the trailing
/// `window_days` days and ending today.
pub fn day_axis(window_days: u32) -> Vec<String> {
    let days = window_days.max(1) as i64;
    let today = Local::now().date_naive();
    (0..days)
        .rev()
        .map(|i| {
            (today - ChronoDuration::days(i))
                .format("%Y-%m-%d")
                .to_string()
        })
        .collect()
}

/// Sum achievement magnitudes per kind across `events`.
pub fn totals_of(events: &[Achievement]) -> AchievementTotals {
    let mut t = AchievementTotals::default();
    for e in events {
        match e.kind {
            AchievementKind::TaskCompleted => t.tasks_completed += e.magnitude,
            AchievementKind::ArtifactReached => t.artifacts_reached += e.magnitude,
            AchievementKind::ChangeCreated => t.changes_created += e.magnitude,
            AchievementKind::ChangeArchived => t.changes_archived += e.magnitude,
        }
    }
    t
}

/// Count achievement magnitudes per local calendar day across `events`.
pub fn count_by_day(events: &[Achievement]) -> BTreeMap<String, u32> {
    let mut by_day: BTreeMap<String, u32> = BTreeMap::new();
    for e in events {
        *by_day.entry(local_day(e.timestamp)).or_insert(0) += e.magnitude;
    }
    by_day
}

/// Current unix time in seconds.
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// How many artifacts advanced from absent to present between two statuses —
/// each of proposal/design/tasks plus each newly-present capability spec.
/// Never negative.
fn new_artifact_count(prev: &ArtifactStatus, cur: &ArtifactStatus) -> u32 {
    let mut n = 0u32;
    if cur.proposal && !prev.proposal {
        n += 1;
    }
    if cur.design && !prev.design {
        n += 1;
    }
    if cur.tasks && !prev.tasks {
        n += 1;
    }
    let prev_specs: std::collections::HashSet<&str> =
        prev.specs.iter().map(String::as_str).collect();
    for s in &cur.specs {
        if !prev_specs.contains(s.as_str()) {
            n += 1;
        }
    }
    n
}

/// Net-positive achievements from comparing a workspace's previous active
/// changes against its new ones, stamped at `timestamp`. Only forward progress
/// produces events: a rising completed-task count (magnitude = the increase),
/// an artifact becoming present, or a brand-new change. Unchecking a task,
/// deleting a task line, or removing a change yields nothing. Archival is
/// detected by the watcher (it needs the archive-directory check) and recorded
/// there, not here.
pub fn diff_achievements(
    previous: &[ChangeData],
    current: &[ChangeData],
    workspace: &Path,
    timestamp: i64,
) -> Vec<Achievement> {
    let prev: std::collections::HashMap<&str, &ChangeData> =
        previous.iter().map(|c| (c.change_id.as_str(), c)).collect();
    let mut out = Vec::new();
    let mk = |kind, change_id: &str, mag| {
        Achievement::new(
            kind,
            timestamp,
            workspace.to_path_buf(),
            Some(change_id.to_string()),
            mag,
        )
    };
    for c in current {
        match prev.get(c.change_id.as_str()) {
            None => {
                out.push(mk(AchievementKind::ChangeCreated, &c.change_id, 1));
                if c.completed_tasks > 0 {
                    out.push(mk(
                        AchievementKind::TaskCompleted,
                        &c.change_id,
                        c.completed_tasks as u32,
                    ));
                }
                let arts = new_artifact_count(&ArtifactStatus::default(), &c.artifacts);
                if arts > 0 {
                    out.push(mk(AchievementKind::ArtifactReached, &c.change_id, arts));
                }
            }
            Some(p) => {
                if c.completed_tasks > p.completed_tasks {
                    out.push(mk(
                        AchievementKind::TaskCompleted,
                        &c.change_id,
                        (c.completed_tasks - p.completed_tasks) as u32,
                    ));
                }
                let arts = new_artifact_count(&p.artifacts, &c.artifacts);
                if arts > 0 {
                    out.push(mk(AchievementKind::ArtifactReached, &c.change_id, arts));
                }
            }
        }
    }
    out
}

/// Assemble backfilled achievements for one git-backed workspace from already
/// mined git inputs, all flagged `backfilled = true`. `lifecycles` supplies
/// change creation/archival timestamps (each `created_at` → a ChangeCreated,
/// each `archived_at` → a ChangeArchived); `task_history` is the
/// `(timestamp, change_id, delta)` stream from
/// [`crate::git::task_completion_history`]. Commit activity is *not* turned into
/// achievement events here — the dashboard already counts commits per day
/// directly from git for the heatmap and streak. Pure and deterministic.
pub fn build_backfill(
    workspace: &Path,
    lifecycles: &[crate::git::ChangeLifecycle],
    task_history: &[(i64, String, u32)],
) -> Vec<Achievement> {
    let mut out = Vec::new();
    for lc in lifecycles {
        if let Some(created) = lc.created_at {
            out.push(
                Achievement::new(
                    AchievementKind::ChangeCreated,
                    created,
                    workspace.to_path_buf(),
                    Some(lc.change_name.clone()),
                    1,
                )
                .as_backfilled(),
            );
        }
        if let Some(archived) = lc.archived_at {
            out.push(
                Achievement::new(
                    AchievementKind::ChangeArchived,
                    archived,
                    workspace.to_path_buf(),
                    Some(lc.change_name.clone()),
                    1,
                )
                .as_backfilled(),
            );
        }
    }
    for (ts, change_id, delta) in task_history {
        out.push(
            Achievement::new(
                AchievementKind::TaskCompleted,
                *ts,
                workspace.to_path_buf(),
                Some(change_id.clone()),
                *delta,
            )
            .as_backfilled(),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    fn ts(days_ago: i64) -> i64 {
        (Local::now() - ChronoDuration::days(days_ago)).timestamp()
    }

    fn ev(kind: AchievementKind, timestamp: i64, mag: u32) -> Achievement {
        Achievement::new(kind, timestamp, PathBuf::from("/ws"), None, mag)
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("specforge-actlog-{}", std::process::id()));
        let path = dir.join("activity.json");
        let _ = std::fs::remove_file(&path);

        let log = ActivityLog::load(path.clone());
        assert!(log.is_empty());
        log.record(ev(AchievementKind::TaskCompleted, ts(0), 3));
        log.record(ev(AchievementKind::ChangeArchived, ts(1), 1));

        // Reload from disk and confirm both events survive.
        let reloaded = ActivityLog::load(path.clone());
        let snap = reloaded.snapshot();
        assert_eq!(snap.len(), 2);
        assert_eq!(reloaded.totals().tasks_completed, 3);
        assert_eq!(reloaded.totals().changes_archived, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn query_window_is_bounded() {
        let dir = std::env::temp_dir().join(format!("specforge-actlog-win-{}", std::process::id()));
        let path = dir.join("activity.json");
        let _ = std::fs::remove_file(&path);

        let log = ActivityLog::load(path);
        log.record(ev(AchievementKind::TaskCompleted, ts(0), 1)); // today
        log.record(ev(AchievementKind::TaskCompleted, ts(2), 1)); // 2 days ago
        log.record(ev(AchievementKind::TaskCompleted, ts(40), 1)); // far outside

        // A 7-day window keeps today + 2-days-ago, drops the 40-day-old event.
        let within = log.query_window(7);
        assert_eq!(within.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unchecks_are_never_recorded_as_negatives() {
        // The log only stores what callers append; magnitudes are u32 and the
        // watcher diff only appends net-positive deltas. Totals therefore only
        // ever grow.
        let events = vec![
            ev(AchievementKind::TaskCompleted, ts(0), 2),
            ev(AchievementKind::TaskCompleted, ts(0), 1),
        ];
        assert_eq!(totals_of(&events).tasks_completed, 3);
    }

    #[test]
    fn buckets_events_by_local_day() {
        let today = Local::now();
        let yesterday = today - ChronoDuration::days(1);
        let events = vec![
            ev(AchievementKind::TaskCompleted, today.timestamp(), 2),
            ev(AchievementKind::ChangeArchived, today.timestamp(), 1),
            ev(AchievementKind::TaskCompleted, yesterday.timestamp(), 1),
        ];
        let by_day = count_by_day(&events);
        let today_key = today.format("%Y-%m-%d").to_string();
        let yest_key = yesterday.format("%Y-%m-%d").to_string();
        assert_eq!(by_day.get(&today_key), Some(&3));
        assert_eq!(by_day.get(&yest_key), Some(&1));
    }

    #[test]
    fn local_day_matches_chrono() {
        let now = Local::now();
        let day = local_day(now.timestamp());
        assert!(day.starts_with(&format!("{:04}", now.year())));
        assert_eq!(day.len(), 10);
    }

    fn change_for_diff(id: &str, completed: usize, artifacts: ArtifactStatus) -> ChangeData {
        ChangeData {
            change_id: id.to_string(),
            title: None,
            sections: vec![],
            total_tasks: completed,
            completed_tasks: completed,
            artifacts,
            workspace: crate::types::WorkspaceFolder {
                uri: PathBuf::from("/ws"),
                name: "ws".into(),
            },
        }
    }

    fn arts(proposal: bool, design: bool, tasks: bool, specs: &[&str]) -> ArtifactStatus {
        ArtifactStatus {
            proposal,
            design,
            tasks,
            specs: specs.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn diff_records_task_increase_as_delta() {
        let prev = vec![change_for_diff("a", 1, arts(true, false, true, &[]))];
        let cur = vec![change_for_diff("a", 4, arts(true, false, true, &[]))];
        let evs = diff_achievements(&prev, &cur, Path::new("/ws"), 100);
        let tasks: Vec<_> = evs
            .iter()
            .filter(|e| e.kind == AchievementKind::TaskCompleted)
            .collect();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].magnitude, 3);
    }

    #[test]
    fn diff_ignores_task_decrease() {
        let prev = vec![change_for_diff("a", 4, arts(true, false, true, &[]))];
        let cur = vec![change_for_diff("a", 2, arts(true, false, true, &[]))];
        let evs = diff_achievements(&prev, &cur, Path::new("/ws"), 100);
        assert!(evs.is_empty());
    }

    #[test]
    fn diff_flags_new_change_as_created() {
        let cur = vec![change_for_diff("a", 0, arts(true, false, false, &[]))];
        let evs = diff_achievements(&[], &cur, Path::new("/ws"), 100);
        assert!(evs
            .iter()
            .any(|e| e.kind == AchievementKind::ChangeCreated));
    }

    #[test]
    fn diff_flags_artifact_advance() {
        let prev = vec![change_for_diff("a", 0, arts(true, false, false, &[]))];
        let cur = vec![change_for_diff("a", 0, arts(true, true, true, &["cap"]))];
        let evs = diff_achievements(&prev, &cur, Path::new("/ws"), 100);
        let reached: Vec<_> = evs
            .iter()
            .filter(|e| e.kind == AchievementKind::ArtifactReached)
            .collect();
        assert_eq!(reached.len(), 1);
        assert_eq!(reached[0].magnitude, 3); // design + tasks + one new spec
    }

    #[test]
    fn backfill_maps_lifecycles_and_task_history_as_backfilled() {
        let lifecycles = vec![
            crate::git::ChangeLifecycle {
                change_name: "foo".into(),
                created_at: Some(1000),
                archived_at: Some(2000),
            },
            crate::git::ChangeLifecycle {
                change_name: "bar".into(),
                created_at: Some(1500),
                archived_at: None, // still active — no ship event
            },
        ];
        let task_history = vec![(1200i64, "foo".to_string(), 3u32)];
        let evs = build_backfill(Path::new("/ws"), &lifecycles, &task_history);

        assert!(evs.iter().all(|e| e.backfilled));
        let created = evs
            .iter()
            .filter(|e| e.kind == AchievementKind::ChangeCreated)
            .count();
        let archived = evs
            .iter()
            .filter(|e| e.kind == AchievementKind::ChangeArchived)
            .count();
        let tasks: u32 = evs
            .iter()
            .filter(|e| e.kind == AchievementKind::TaskCompleted)
            .map(|e| e.magnitude)
            .sum();
        assert_eq!(created, 2); // foo + bar
        assert_eq!(archived, 1); // foo only
        assert_eq!(tasks, 3);
    }

    #[test]
    fn backfill_empty_for_no_inputs() {
        let evs = build_backfill(Path::new("/ws"), &[], &[]);
        assert!(evs.is_empty());
    }
}
