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

use crate::identity::{Author, IdentityConfig};
use crate::types::{ArtifactStatus, ChangeData};
use chrono::{Duration as ChronoDuration, Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
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
    /// The raw author this achievement was observed with — the watched repo's
    /// local git identity for live events, the commit author for backfilled
    /// ones. Stored verbatim (never pre-resolved to "me") so that adding an
    /// alias later retroactively reclaims it. `None` for legacy events recorded
    /// before authorship was captured, and for live events in a workspace with
    /// no resolvable git identity; both resolve as the local developer's (see
    /// [`event_is_me`]). `#[serde(default)]` keeps existing logs parseable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<Author>,
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
            author: None,
        }
    }

    /// Mark this achievement as reconstructed from history.
    pub fn as_backfilled(mut self) -> Self {
        self.backfilled = true;
        self
    }

    /// Attach the observed author. A fully-empty author is normalised to
    /// `None` so a `(None, None)` identity never masquerades as authored.
    pub fn with_author(mut self, author: Option<Author>) -> Self {
        self.author = author.filter(|a| crate::identity::normalized_key(a).is_some());
        self
    }
}

/// Whether an event resolves to the canonical developer under `config`. An
/// author-less event (legacy, or a flat workspace with no git identity) counts
/// as the local developer's, since before identity the app was single-user.
pub fn event_is_me(event: &Achievement, config: &IdentityConfig) -> bool {
    match &event.author {
        None => true,
        Some(author) => crate::identity::is_me(author, config),
    }
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

    /// Change ids the canonical developer created, across the **whole** log
    /// rather than any bounded window.
    ///
    /// "Did I create this change" is a lifetime fact, not a windowed one: an
    /// active change can easily be older than the heatmap window, and deriving
    /// the in-flight count from a windowed slice silently drops it — leaving the
    /// hero's "in flight" tile reading `0` directly above a footnote that counts
    /// the very same change as active.
    pub fn me_created_change_ids(&self, config: &IdentityConfig) -> HashSet<String> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.kind == AchievementKind::ChangeCreated && event_is_me(e, config))
            .filter_map(|e| e.change_id.clone())
            .collect()
    }

    /// Reconcile git-derived lifecycle facts into the log: append any
    /// `ChangeArchived` / `ChangeCreated` the log is missing for these
    /// `lifecycles`, stamped under `workspace` and flagged backfilled. Returns
    /// the number appended.
    ///
    /// Unlike the one-shot launch backfill, this is safe to call repeatedly:
    /// dedup is by `(kind, change_id)` (see [`missing_lifecycle_events`]), so a
    /// re-run with the same git history adds nothing, and an archival the live
    /// watcher already recorded is never duplicated. This is how a change
    /// archived on a branch or worktree — which the main workspace only ever
    /// observes already-archived, so the live `active → archived` transition
    /// never fires — still reaches the Dashboard's "shipped" haul.
    pub fn reconcile_lifecycle(
        &self,
        workspace: &Path,
        lifecycles: &[crate::git::ChangeLifecycle],
    ) -> usize {
        let missing = {
            let events = self.events.lock().unwrap();
            missing_lifecycle_events(&events, workspace, lifecycles)
        };
        let n = missing.len();
        self.record_all(missing);
        n
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
///
/// Every recorded event is stamped with `author` — the watched repository's
/// local git identity, read once per batch by the watcher. `None` (a flat
/// workspace with no resolvable git identity) records author-less events, which
/// resolve as the local developer's.
pub fn diff_achievements(
    previous: &[ChangeData],
    current: &[ChangeData],
    workspace: &Path,
    timestamp: i64,
    author: Option<Author>,
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
        .with_author(author.clone())
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
/// change creation/archival timestamps + authors (each `created_at` → a
/// ChangeCreated stamped with `created_by`, each `archived_at` → a
/// ChangeArchived stamped with `archived_by`); `task_history` is the
/// `(timestamp, change_id, delta, author)` stream from
/// [`crate::git::task_completion_history`]. Each event carries its real commit
/// author so shared history is attributed to whoever performed it. Commit
/// activity is *not* turned into achievement events here — the dashboard already
/// counts commits per day directly from git for the heatmap and streak. Pure
/// and deterministic.
pub fn build_backfill(
    workspace: &Path,
    lifecycles: &[crate::git::ChangeLifecycle],
    task_history: &[(i64, String, u32, Option<Author>)],
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
                .as_backfilled()
                .with_author(lc.created_by.clone()),
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
                .as_backfilled()
                .with_author(lc.archived_by.clone()),
            );
        }
    }
    for (ts, change_id, delta, author) in task_history {
        out.push(
            Achievement::new(
                AchievementKind::TaskCompleted,
                *ts,
                workspace.to_path_buf(),
                Some(change_id.clone()),
                *delta,
            )
            .as_backfilled()
            .with_author(author.clone()),
        );
    }
    out
}

/// The `ChangeArchived` / `ChangeCreated` achievements `lifecycles` implies but
/// `existing` does not yet contain, stamped under `workspace` and flagged
/// backfilled. Each `created_at` yields a `ChangeCreated`, each `archived_at` a
/// `ChangeArchived`.
///
/// Dedup is by `(kind, change_id)`: a change that already has a created (resp.
/// archived) event *anywhere* in the log is considered covered, so this is
/// idempotent and never double-counts an archival a prior backfill or the live
/// watcher already recorded — even though the live event may carry a different
/// workspace path (a worktree) or timestamp (observation time vs. git author
/// date). Change ids are repo-unique and date-prefixed, so cross-repo id
/// collisions are not a practical concern.
pub fn missing_lifecycle_events(
    existing: &[Achievement],
    workspace: &Path,
    lifecycles: &[crate::git::ChangeLifecycle],
) -> Vec<Achievement> {
    let mut have_created: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut have_archived: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for e in existing {
        if let Some(id) = e.change_id.as_deref() {
            match e.kind {
                AchievementKind::ChangeCreated => {
                    have_created.insert(id);
                }
                AchievementKind::ChangeArchived => {
                    have_archived.insert(id);
                    // A `ChangeArchived` already in the log may name the change
                    // by its DATED archive directory: that is what a backfill
                    // recorded before `change_lifecycle` was corrected to yield
                    // the bare logical id, while the live watcher has always
                    // recorded the bare one. Coverage is by logical change, so
                    // an entry under either spelling covers both — otherwise a
                    // log written by an older build would gain a second
                    // archival for every historical change on the next
                    // reconcile, silently inflating the shipped haul.
                    have_archived.insert(crate::parser::archive_dir_logical_id(id));
                }
                _ => {}
            }
        }
    }

    let mut out = Vec::new();
    for lc in lifecycles {
        if let Some(created) = lc.created_at {
            if !have_created.contains(lc.change_name.as_str()) {
                out.push(
                    Achievement::new(
                        AchievementKind::ChangeCreated,
                        created,
                        workspace.to_path_buf(),
                        Some(lc.change_name.clone()),
                        1,
                    )
                    .as_backfilled()
                    .with_author(lc.created_by.clone()),
                );
            }
        }
        if let Some(archived) = lc.archived_at {
            if !have_archived.contains(lc.change_name.as_str()) {
                out.push(
                    Achievement::new(
                        AchievementKind::ChangeArchived,
                        archived,
                        workspace.to_path_buf(),
                        Some(lc.change_name.clone()),
                        1,
                    )
                    .as_backfilled()
                    .with_author(lc.archived_by.clone()),
                );
            }
        }
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

    /// Sum magnitudes for one kind across the whole log. A test-local stand-in
    /// for the production `totals()` accessor, which was removed when the
    /// season system took its last caller — these tests use it as an assertion
    /// vehicle for disk round-tripping and reconcile idempotency, not as a
    /// subject, so it does not justify keeping public API alive.
    fn total_for(log: &ActivityLog, kind: AchievementKind) -> u32 {
        log.snapshot()
            .iter()
            .filter(|e| e.kind == kind)
            .map(|e| e.magnitude)
            .sum()
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
        assert_eq!(total_for(&reloaded, AchievementKind::TaskCompleted), 3);
        assert_eq!(total_for(&reloaded, AchievementKind::ChangeArchived), 1);

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
        let evs = diff_achievements(&prev, &cur, Path::new("/ws"), 100, None);
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
        let evs = diff_achievements(&prev, &cur, Path::new("/ws"), 100, None);
        assert!(evs.is_empty());
    }

    #[test]
    fn diff_flags_new_change_as_created() {
        let cur = vec![change_for_diff("a", 0, arts(true, false, false, &[]))];
        let evs = diff_achievements(&[], &cur, Path::new("/ws"), 100, None);
        assert!(evs.iter().any(|e| e.kind == AchievementKind::ChangeCreated));
    }

    #[test]
    fn diff_flags_artifact_advance() {
        let prev = vec![change_for_diff("a", 0, arts(true, false, false, &[]))];
        let cur = vec![change_for_diff("a", 0, arts(true, true, true, &["cap"]))];
        let evs = diff_achievements(&prev, &cur, Path::new("/ws"), 100, None);
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
                ..Default::default()
            },
            crate::git::ChangeLifecycle {
                change_name: "bar".into(),
                created_at: Some(1500),
                archived_at: None, // still active — no ship event
                ..Default::default()
            },
        ];
        let task_history = vec![(1200i64, "foo".to_string(), 3u32, None)];
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

    fn lc(name: &str, created: Option<i64>, archived: Option<i64>) -> crate::git::ChangeLifecycle {
        crate::git::ChangeLifecycle {
            change_name: name.into(),
            created_at: created,
            archived_at: archived,
            ..Default::default()
        }
    }

    #[test]
    fn missing_lifecycle_events_only_emits_uncovered_changes() {
        // The log already knows "foo" was created and archived; "bar" is new.
        let existing = vec![
            Achievement::new(
                AchievementKind::ChangeCreated,
                100,
                PathBuf::from("/ws"),
                Some("foo".into()),
                1,
            ),
            Achievement::new(
                AchievementKind::ChangeArchived,
                200,
                PathBuf::from("/ws"),
                Some("foo".into()),
                1,
            ),
        ];
        let lifecycles = vec![
            lc("foo", Some(100), Some(200)),
            lc("bar", Some(300), Some(400)),
        ];
        let out = missing_lifecycle_events(&existing, Path::new("/ws"), &lifecycles);
        // Only bar's create + archive are missing; foo is fully covered.
        assert_eq!(out.len(), 2);
        assert!(out
            .iter()
            .all(|e| e.change_id.as_deref() == Some("bar") && e.backfilled));
        assert_eq!(
            out.iter()
                .filter(|e| e.kind == AchievementKind::ChangeArchived)
                .count(),
            1
        );
        assert_eq!(
            out.iter()
                .filter(|e| e.kind == AchievementKind::ChangeCreated)
                .count(),
            1
        );
    }

    #[test]
    fn missing_lifecycle_events_dedups_against_a_live_event() {
        // A live (non-backfilled) archival for "foo" must suppress a duplicate
        // backfilled one, even though it carries a different workspace path and
        // timestamp than the git lifecycle would.
        let existing = vec![Achievement::new(
            AchievementKind::ChangeArchived,
            999,
            PathBuf::from("/ws/.worktree/foo"),
            Some("foo".into()),
            1,
        )];
        let lifecycles = vec![lc("foo", None, Some(200))];
        let out = missing_lifecycle_events(&existing, Path::new("/ws"), &lifecycles);
        assert!(out.is_empty());
    }

    #[test]
    fn missing_lifecycle_events_dedups_a_legacy_dated_archive_entry() {
        // The migration case: a log written before `change_lifecycle` was
        // corrected names the archival by its DATED directory. The lifecycle
        // now names the same archival by the bare logical id, and coverage is
        // by logical change — so nothing is appended. Without this, every
        // historical archive would be re-recorded once on the next reconcile.
        let existing = vec![Achievement::new(
            AchievementKind::ChangeArchived,
            999,
            PathBuf::from("/ws"),
            Some("2026-06-04-foo".into()),
            1,
        )
        .as_backfilled()];
        let lifecycles = vec![lc("foo", None, Some(200))];
        assert!(missing_lifecycle_events(&existing, Path::new("/ws"), &lifecycles).is_empty());

        // An unrelated change is still appended — the alias covers one logical
        // change, it does not blanket-suppress.
        let other = vec![lc("bar", None, Some(200))];
        assert_eq!(
            missing_lifecycle_events(&existing, Path::new("/ws"), &other).len(),
            1
        );
    }

    #[test]
    fn reconcile_lifecycle_appends_then_is_idempotent() {
        let dir =
            std::env::temp_dir().join(format!("specforge-actlog-recon-{}", std::process::id()));
        let path = dir.join("activity.json");
        let _ = std::fs::remove_file(&path);

        let log = ActivityLog::load(path.clone());
        let lifecycles = vec![
            lc("foo", Some(100), Some(200)),
            lc("bar", Some(300), None), // created, still active — no ship event
        ];
        // First pass seeds foo(create+archive) + bar(create) = 3 events.
        assert_eq!(log.reconcile_lifecycle(Path::new("/ws"), &lifecycles), 3);
        assert_eq!(total_for(&log, AchievementKind::ChangeArchived), 1);
        assert_eq!(total_for(&log, AchievementKind::ChangeCreated), 2);
        // Second pass with the same git history adds nothing.
        assert_eq!(log.reconcile_lifecycle(Path::new("/ws"), &lifecycles), 0);

        // A newly-archived change is picked up on the next reconcile — the
        // restart-survives behaviour the one-shot backfill lacked.
        let later = vec![
            lc("foo", Some(100), Some(200)),
            lc("bar", Some(300), Some(500)), // bar now archived too
        ];
        assert_eq!(log.reconcile_lifecycle(Path::new("/ws"), &later), 1);
        assert_eq!(total_for(&log, AchievementKind::ChangeArchived), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn author(name: Option<&str>, email: Option<&str>) -> Author {
        Author::new(name.map(str::to_string), email.map(str::to_string))
    }

    fn config_with(aliases: Vec<Author>) -> IdentityConfig {
        IdentityConfig {
            display_name: None,
            aliases,
        }
    }

    #[test]
    fn author_less_event_resolves_as_the_local_developer() {
        // A legacy event with no author counts as the developer's, under any
        // config (including an empty one).
        let ev = ev(AchievementKind::TaskCompleted, ts(0), 1);
        assert!(ev.author.is_none());
        assert!(event_is_me(&ev, &IdentityConfig::default()));
        assert!(event_is_me(
            &ev,
            &config_with(vec![author(None, Some("me@x.com"))])
        ));
    }

    #[test]
    fn adding_an_alias_reclaims_a_past_event_at_query_time() {
        // An event authored by an identity not yet claimed is not "me"…
        let past = ev(AchievementKind::ChangeArchived, ts(1), 1)
            .with_author(Some(author(Some("Old Me"), Some("old@me.com"))));
        let before = config_with(vec![author(None, Some("new@me.com"))]);
        assert!(!event_is_me(&past, &before));
        // …until that identity is added as an alias, when the same stored event
        // resolves as "me" — no rewrite of the event.
        let after = config_with(vec![
            author(None, Some("new@me.com")),
            author(None, Some("OLD@me.com")), // case-insensitive match
        ]);
        assert!(event_is_me(&past, &after));
    }

    #[test]
    fn live_diff_stamps_the_supplied_author() {
        let me = author(Some("Me"), Some("me@x.com"));
        let cur = vec![change_for_diff("a", 2, arts(true, false, true, &[]))];
        let evs = diff_achievements(&[], &cur, Path::new("/ws"), 100, Some(me.clone()));
        assert!(!evs.is_empty());
        assert!(evs.iter().all(|e| e.author.as_ref() == Some(&me)));
    }

    #[test]
    fn backfill_carries_each_commit_author() {
        let alice = author(Some("Alice"), Some("alice@x.com"));
        let bob = author(Some("Bob"), Some("bob@x.com"));
        let lifecycles = vec![crate::git::ChangeLifecycle {
            change_name: "foo".into(),
            created_at: Some(1000),
            archived_at: Some(2000),
            created_by: Some(alice.clone()),
            archived_by: Some(bob.clone()),
        }];
        let task_history = vec![(1200i64, "foo".to_string(), 3u32, Some(alice.clone()))];
        let evs = build_backfill(Path::new("/ws"), &lifecycles, &task_history);

        let created = evs
            .iter()
            .find(|e| e.kind == AchievementKind::ChangeCreated)
            .unwrap();
        let archived = evs
            .iter()
            .find(|e| e.kind == AchievementKind::ChangeArchived)
            .unwrap();
        let task = evs
            .iter()
            .find(|e| e.kind == AchievementKind::TaskCompleted)
            .unwrap();
        assert_eq!(created.author.as_ref(), Some(&alice));
        assert_eq!(archived.author.as_ref(), Some(&bob));
        assert_eq!(task.author.as_ref(), Some(&alice));
    }

    /// The in-flight tile's creator set must NOT be windowed: a change created
    /// long before any bounded query window is still active today, and scoping
    /// it to a window makes the hero read "0 in flight" directly above a
    /// footnote counting that same change as active.
    #[test]
    fn me_created_change_ids_span_the_whole_log_and_scope_to_me() {
        let dir =
            std::env::temp_dir().join(format!("specforge-actlog-created-{}", std::process::id()));
        let path = dir.join("activity.json");
        let _ = std::fs::remove_file(&path);

        let log = ActivityLog::load(path);
        let me = author(Some("Me"), Some("me@x.com"));
        let other = author(Some("Them"), Some("them@x.com"));
        let created = |id: &str, days_ago: i64| {
            Achievement::new(
                AchievementKind::ChangeCreated,
                ts(days_ago),
                PathBuf::from("/ws"),
                Some(id.to_string()),
                1,
            )
        };

        // Far outside every window the Dashboard ever queries (371 days).
        log.record(
            created("c-900", 900)
                .with_author(Some(me.clone()))
                .as_backfilled(),
        );
        log.record(created("c-1", 1).with_author(Some(me.clone())));
        log.record(created("theirs", 1).with_author(Some(other)));
        // A non-creation event by me must not contribute an id.
        log.record(ev(AchievementKind::TaskCompleted, ts(0), 1).with_author(Some(me.clone())));

        let ids = log.me_created_change_ids(&config_with(vec![me]));
        assert!(
            ids.contains("c-900"),
            "a change created 900 days ago is still mine and still active: {ids:?}"
        );
        assert!(ids.contains("c-1"));
        assert_eq!(
            ids.len(),
            2,
            "another author's creation is excluded: {ids:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
