//! Append-only activity log: a private record of observed achievements.
//!
//! The log lives in the application's data directory, never inside any
//! workspace `openspec/` tree — so SpecForge stays a read-only *viewer* of
//! workspaces while keeping its own diary of what it observed. The Dashboard
//! derives its today, streak, heatmap, and milestone views from this log.
//!
//! Events are append-only: once recorded they are never rewritten or removed,
//! even if the underlying workspace later loses tasks or changes
//! (`activity-log`: *Incremental, Ordered, Crash-Safe Log Persistence*).
//!
//! - **Format.** `activity.jsonl`, one compact JSON [`Achievement`] per line,
//!   each terminated by `\n`. Recording a batch appends exactly that batch's
//!   lines in one write, so persisting costs O(batch), never O(log). The write
//!   happens under the same lock as the in-memory push, so the file lists
//!   events in the order this instance recorded them.
//! - **Crash safety.** An interrupted append leaves an unterminated final
//!   fragment, which the next load drops (truncating it away) while keeping
//!   every complete line. Any other unparseable content is moved aside to an
//!   `activity.jsonl.corrupt-<n>` sibling first, and every line that parses
//!   is kept. A legacy whole-file `activity.json` array is migrated once with
//!   [`write_atomic`] and kept as `activity.json.migrated` (or the first free
//!   `activity.json.migrated-<n>`), never deleted or overwritten.
//! - **The persistence lease** (`activity-log`: *Single Persisting Instance*).
//!   The desktop app, `specforge-serve`, the TUI and worktree dev slots share
//!   one config directory. Only the instance holding an exclusive advisory
//!   lock on `activity.lock` writes the log; the others record in memory for
//!   their own session and retry the lock on each batch. One that acquires it
//!   later persists only what it records from then on, since the previous
//!   holder, watching the same registry, already persisted the rest; it first
//!   loads the file as a holder and merges in the facts it could otherwise
//!   derive and append a second time (lifecycle facts and backfilled events). The OS
//!   releases the lock when its process exits, cleanly or not. The lock is on
//!   a separate file so that replacing `activity.jsonl` atomically never swaps
//!   the locked inode away.

use crate::config_file::{move_aside, preserve_corrupt_config, read_if_present, write_atomic};
use crate::identity::{Author, IdentityConfig};
use crate::types::{ArtifactStatus, ChangeData};
use chrono::{Duration as ChronoDuration, Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// The persisted log, in the config directory.
pub const LOG_FILE: &str = "activity.jsonl";
/// The file whose advisory lock is the persistence lease.
pub const LOCK_FILE: &str = "activity.lock";
/// The whole-file JSON array the log was persisted as before.
pub const LEGACY_FILE: &str = "activity.json";

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

/// Append-only store of [`Achievement`]s, persisted as JSON Lines under the
/// persistence lease. See the module doc.
pub struct ActivityLog {
    state: Mutex<LogState>,
}

/// Everything behind the log's one lock: the events, and the file they are
/// appended to. One lock for both is what keeps the file's order equal to the
/// order of the in-memory `Vec`.
struct LogState {
    events: Vec<Achievement>,
    store: Store,
}

/// The persisted side of the log.
struct Store {
    /// The config directory holding `activity.jsonl`, `activity.lock` and any
    /// legacy `activity.json`.
    dir: PathBuf,
    /// `activity.lock`, opened once; `None` when it could not be opened, in
    /// which case this instance never persists.
    lock: Option<File>,
    /// Whether this instance holds the lease on `lock`.
    held: bool,
    /// False when loading refused to write: the file could not be read, or
    /// was damaged and could not be moved aside, or a migration could not be
    /// written. Nothing is appended for the rest of the session.
    writable: bool,
    /// The append handle, opened on the first append.
    log: Option<File>,
    /// Re-check the file's tail before the next append, after an append that
    /// failed, possibly part-way.
    check_tail: bool,
    /// Whether a lock failure other than "held elsewhere" was reported, so it
    /// is reported once rather than on every batch.
    lock_error_reported: bool,
    #[cfg(test)]
    appends: usize,
}

impl ActivityLog {
    /// Open the log in `config_dir`, taking the persistence lease when no
    /// other instance holds it.
    ///
    /// The holder loads `activity.jsonl`, trimming a torn tail and moving a
    /// damaged file aside, or migrates a legacy `activity.json`; see the
    /// module doc. Any other instance only parses, keeping every complete
    /// line that parses (from `activity.jsonl`, else from the legacy array),
    /// and writes, truncates and moves nothing.
    pub fn open(config_dir: &Path) -> Self {
        let _ = fs::create_dir_all(config_dir);
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(config_dir.join(LOCK_FILE))
            .ok();
        let mut store = Store {
            dir: config_dir.to_path_buf(),
            lock,
            held: false,
            writable: true,
            log: None,
            check_tail: false,
            lock_error_reported: false,
            #[cfg(test)]
            appends: 0,
        };
        let events = if store.try_lease() {
            let (events, writable) = load_as_holder(config_dir);
            store.held = true;
            store.writable = writable;
            events
        } else {
            load_as_reader(config_dir)
        };
        Self {
            state: Mutex::new(LogState { events, store }),
        }
    }

    fn state(&self) -> MutexGuard<'_, LogState> {
        self.state.lock().unwrap()
    }

    /// Append one achievement and persist it.
    pub fn record(&self, event: Achievement) {
        self.record_all(vec![event]);
    }

    /// Append many achievements and persist them with one write. An empty
    /// batch writes nothing.
    pub fn record_all(&self, new_events: Vec<Achievement>) {
        self.state().append(new_events);
    }

    /// True when no achievement has ever been recorded.
    pub fn is_empty(&self) -> bool {
        self.state().events.is_empty()
    }

    /// A clone of every recorded achievement, in record order.
    pub fn snapshot(&self) -> Vec<Achievement> {
        self.state().events.clone()
    }

    /// Achievements at or after `cutoff_ts` (unix seconds).
    pub fn query_since(&self, cutoff_ts: i64) -> Vec<Achievement> {
        self.state()
            .events
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
        self.state()
            .events
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
    ///
    /// The lease is taken first if it has come free, so the facts are judged
    /// against what the previous holder persisted, and the missing events are
    /// computed and appended under one hold of the lock, so two concurrent
    /// reconciliations cannot both append the same missing event.
    pub fn reconcile_lifecycle(
        &self,
        workspace: &Path,
        lifecycles: &[crate::git::ChangeLifecycle],
    ) -> usize {
        let mut state = self.state();
        state.acquire_if_free();
        let missing = missing_lifecycle_events(&state.events, workspace, lifecycles);
        let n = missing.len();
        state.append(missing);
        n
    }

    /// How many appends this instance has written to the persisted log, which
    /// the file's contents alone cannot show.
    #[cfg(test)]
    pub(crate) fn persisted_appends(&self) -> usize {
        self.state().store.appends
    }
}

impl LogState {
    /// Push `batch` into memory and, under the lease, append it to the file,
    /// all under the caller's hold of the lock. An empty batch does nothing.
    ///
    /// A backfilled event identical to one already held is dropped: a
    /// backfilled event is a deterministic reconstruction from git, so an
    /// identical one is the same fact derived twice — by this instance and by
    /// a previous lease holder whose log it merged on acquiring the lease.
    fn append(&mut self, batch: Vec<Achievement>) {
        if batch.is_empty() {
            return;
        }
        self.acquire_if_free();
        let batch: Vec<Achievement> = batch
            .into_iter()
            .filter(|e| !e.backfilled || !self.events.contains(e))
            .collect();
        if batch.is_empty() {
            return;
        }
        let lines = to_lines(&batch);
        self.events.extend(batch);
        self.store.persist(&lines);
    }

    /// Take the lease if this instance does not hold it and it has come free.
    /// The new holder loads the file as a holder does at open — trimming a
    /// torn tail, moving damage aside, migrating a legacy file — and merges
    /// into memory the facts it may be asked to derive again
    /// ([`merge_previous_holder`]), so it never appends a second copy of what
    /// the previous holder persisted.
    fn acquire_if_free(&mut self) {
        if self.store.held || !self.store.try_lease() {
            return;
        }
        let (on_disk, writable) = load_as_holder(&self.store.dir);
        self.store.held = true;
        self.store.writable = writable;
        self.store.log = None;
        self.store.check_tail = false;
        merge_previous_holder(&mut self.events, on_disk);
    }
}

/// Merge into `events` what a previous lease holder persisted that this
/// instance could derive again and append twice. Two kinds of event are
/// derived from the log's own contents rather than observed:
///
/// - Lifecycle facts (`ChangeCreated`, `ChangeArchived`), which
///   [`ActivityLog::reconcile_lifecycle`] appends when the log lacks one by
///   `(kind, change_id)`. They are merged under that same key.
/// - Backfilled events, which are deterministic reconstructions from git.
///   They are merged when no identical event is held.
///
/// Live task and artifact events are not merged: this instance observed its
/// own copies of whatever both instances saw, and merging the holder's too
/// would count each twice in this instance's views.
fn merge_previous_holder(events: &mut Vec<Achievement>, on_disk: Vec<Achievement>) {
    let mut facts: HashSet<(AchievementKind, String)> =
        events.iter().filter_map(lifecycle_key).collect();
    for event in on_disk {
        let merge = match lifecycle_key(&event) {
            Some(key) => facts.insert(key),
            None => event.backfilled && !events.contains(&event),
        };
        if merge {
            events.push(event);
        }
    }
}

/// A lifecycle fact's dedup key, `(kind, change_id)`; `None` for any other
/// event.
fn lifecycle_key(event: &Achievement) -> Option<(AchievementKind, String)> {
    match event.kind {
        AchievementKind::ChangeCreated | AchievementKind::ChangeArchived => {
            Some((event.kind, event.change_id.clone()?))
        }
        AchievementKind::TaskCompleted | AchievementKind::ArtifactReached => None,
    }
}

impl Store {
    fn path(&self) -> PathBuf {
        self.dir.join(LOG_FILE)
    }

    /// Try to take the lease. A failure other than "another instance holds
    /// it" is reported once, and this instance then simply does not persist.
    fn try_lease(&mut self) -> bool {
        let Some(lock) = &self.lock else {
            return false;
        };
        match lock.try_lock() {
            Ok(()) => true,
            Err(fs::TryLockError::WouldBlock) => false,
            Err(fs::TryLockError::Error(err)) => {
                if !self.lock_error_reported {
                    self.lock_error_reported = true;
                    eprintln!(
                        "specforge: could not lock {} ({err}); this instance keeps its activity in memory",
                        self.dir.join(LOCK_FILE).display()
                    );
                }
                false
            }
        }
    }

    /// Append `lines` when this instance holds the lease and may write. A
    /// failed append flags the tail for a re-check.
    fn persist(&mut self, lines: &[u8]) {
        if !self.held || !self.writable {
            return;
        }
        match self.write(lines) {
            Ok(()) => {
                #[cfg(test)]
                {
                    self.appends += 1;
                }
            }
            Err(err) => {
                eprintln!(
                    "specforge: could not append to {} ({err}); the events stay in memory",
                    self.path().display()
                );
                self.check_tail = true;
                self.log = None;
            }
        }
    }

    fn write(&mut self, lines: &[u8]) -> io::Result<()> {
        let path = self.path();
        if self.check_tail {
            repair_tail(&path)?;
            self.check_tail = false;
        }
        let log = match &mut self.log {
            Some(log) => log,
            None => self
                .log
                .insert(OpenOptions::new().create(true).append(true).open(&path)?),
        };
        log.write_all(lines)
    }
}

/// `events` as JSON Lines: one compact object per line, each ending in `\n`.
/// An event that cannot be serialised (a workspace path that is not UTF-8) is
/// left out, whole, rather than written as half a line.
fn to_lines(events: &[Achievement]) -> Vec<u8> {
    let mut out = Vec::new();
    for event in events {
        if let Ok(mut line) = serde_json::to_vec(event) {
            line.push(b'\n');
            out.extend_from_slice(&line);
        }
    }
    out
}

/// What [`parse_lines`] found besides the events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// Every line parsed, and the file ends with `\n` (or is empty).
    Clean,
    /// Every complete line parsed, at least one did, and an unterminated
    /// fragment follows them: an interrupted append. `keep` is the length of
    /// the complete lines.
    TornTail { keep: u64 },
    /// Anything else: a complete line that does not parse (the final one
    /// included), or a file with no complete parseable line before its
    /// fragment.
    Damaged,
}

/// Parse a JSON Lines log: every complete line that parses, in order, and the
/// file's [`Shape`]. An unterminated final fragment is never an event, and a
/// blank line is skipped as no content.
fn parse_lines(bytes: &[u8]) -> (Vec<Achievement>, Shape) {
    let complete_len = bytes.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    let (complete, fragment) = bytes.split_at(complete_len);
    let mut events = Vec::new();
    let mut damaged = false;
    for line in complete.split(|&b| b == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match serde_json::from_slice(line) {
            Ok(event) => events.push(event),
            Err(_) => damaged = true,
        }
    }
    let shape = match (damaged, fragment.is_empty(), events.is_empty()) {
        (true, _, _) | (false, false, true) => Shape::Damaged,
        (false, true, _) => Shape::Clean,
        (false, false, false) => Shape::TornTail {
            keep: complete_len as u64,
        },
    };
    (events, shape)
}

/// The holder's load: `activity.jsonl` when it exists, else a migration of
/// the legacy file. Returns the events and whether appending is allowed.
fn load_as_holder(config_dir: &Path) -> (Vec<Achievement>, bool) {
    let path = config_dir.join(LOG_FILE);
    let legacy = config_dir.join(LEGACY_FILE);
    match read_if_present(&path) {
        Ok(Some(bytes)) => {
            let (events, shape) = parse_lines(&bytes);
            let writable = match shape {
                Shape::Clean => true,
                Shape::TornTail { keep } => trim(&path, keep),
                Shape::Damaged => replace_damaged(&path, &events),
            };
            // An `activity.json` beside the log is a leftover: a migration
            // that stopped before its rename, or an older build still writing
            // the old format. The log wins; the leftover is kept, never
            // loaded, overwritten or deleted.
            keep_legacy(&legacy);
            (events, writable)
        }
        Ok(None) => migrate_legacy(&path, &legacy),
        Err(err) => {
            eprintln!(
                "specforge: could not read {} ({err}); starting with no activity history and leaving the file untouched",
                path.display()
            );
            (Vec::new(), false)
        }
    }
}

/// Any other instance's load: parse, and change nothing on disk.
///
/// When neither file is there it reads `activity.jsonl` once more: the holder
/// may have migrated the legacy file between the two reads, writing the log
/// and then moving `activity.json` away.
fn load_as_reader(config_dir: &Path) -> Vec<Achievement> {
    let log = config_dir.join(LOG_FILE);
    let from_log = |bytes: Vec<u8>| parse_lines(&bytes).0;
    match read_if_present(&log) {
        Ok(Some(bytes)) => from_log(bytes),
        Ok(None) => match read_if_present(&config_dir.join(LEGACY_FILE)) {
            Ok(Some(bytes)) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Ok(None) => read_if_present(&log)
                .ok()
                .flatten()
                .map(from_log)
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        },
        Err(_) => Vec::new(),
    }
}

/// Truncate the log at `path` to its first `keep` bytes, dropping a torn tail.
/// Returns whether that worked; appending after a fragment that is still there
/// would glue the next event onto it.
fn trim(path: &Path, keep: u64) -> bool {
    match OpenOptions::new()
        .write(true)
        .open(path)
        .and_then(|f| f.set_len(keep))
    {
        Ok(()) => {
            eprintln!(
                "specforge: dropped an incomplete final entry from {}",
                path.display()
            );
            true
        }
        Err(err) => {
            eprintln!(
                "specforge: could not drop an incomplete final entry from {} ({err}); not appending to it this session",
                path.display()
            );
            false
        }
    }
}

/// Move a damaged log aside, byte for byte, then write the `events` that did
/// parse to a fresh log. Returns whether appending is allowed: not when the
/// file could not be moved aside, nor when the fresh log could not be written.
fn replace_damaged(path: &Path, events: &[Achievement]) -> bool {
    let Some(backup) = preserve_corrupt_config(path) else {
        eprintln!(
            "specforge: {} holds entries that do not parse and could not be moved aside; not appending to it this session",
            path.display()
        );
        return false;
    };
    eprintln!(
        "specforge: {} holds entries that do not parse; preserved it as {} and kept the {} that do",
        path.display(),
        backup.display(),
        events.len()
    );
    write_whole(path, events)
}

/// Migrate the legacy whole-file array at `legacy` to the log at `path`: the
/// log is written atomically first, and only then is the legacy file kept
/// under its `.migrated` name, so a crash in between leaves the log complete
/// and the leftover is kept on the next load. A legacy file that does not
/// parse is moved aside and the log starts empty, so the backfill runs.
fn migrate_legacy(path: &Path, legacy: &Path) -> (Vec<Achievement>, bool) {
    let bytes = match read_if_present(legacy) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return (Vec::new(), true),
        Err(err) => {
            eprintln!(
                "specforge: could not read {} ({err}); starting with no activity history and not writing a new log this session",
                legacy.display()
            );
            return (Vec::new(), false);
        }
    };
    match serde_json::from_slice::<Vec<Achievement>>(&bytes) {
        Ok(events) => {
            let written = write_whole(path, &events);
            if written {
                keep_legacy(legacy);
            }
            (events, written)
        }
        Err(err) => {
            match preserve_corrupt_config(legacy) {
                Some(backup) => eprintln!(
                    "specforge: could not parse {} ({err}); preserved it as {} and starting with no activity history",
                    legacy.display(),
                    backup.display()
                ),
                None => eprintln!(
                    "specforge: could not parse {} ({err}) or move it aside; starting with no activity history",
                    legacy.display()
                ),
            }
            (Vec::new(), true)
        }
    }
}

/// Replace the log at `path` with `events`, atomically. Returns whether that
/// worked; when it did not, the events stay in memory only and nothing is
/// appended this session, since appends to a log missing them would read back
/// as the whole history.
fn write_whole(path: &Path, events: &[Achievement]) -> bool {
    let Err(err) = write_atomic(path, &to_lines(events)) else {
        return true;
    };
    eprintln!(
        "specforge: could not write {} ({err}); the events stay in memory and nothing is appended this session",
        path.display()
    );
    false
}

/// Keep a legacy `activity.json` under the first free name of
/// `activity.json.migrated`, `activity.json.migrated-1`, … so a copy kept
/// earlier is never replaced ([`move_aside`]). Returns the kept path, or
/// `None` when there was nothing to keep or it could not be moved.
fn keep_legacy(legacy: &Path) -> Option<PathBuf> {
    let kept = move_aside(legacy, |n| match n {
        0 => ".migrated".to_string(),
        n => format!(".migrated-{n}"),
    });
    if let Some(kept) = &kept {
        eprintln!("specforge: kept {} as {}", legacy.display(), kept.display());
    }
    kept
}

/// After a failed append, make sure the next line starts on a line of its
/// own. A torn tail after complete lines that parse is truncated away, as on
/// load. Any other unterminated tail is terminated with `\n` instead, so its
/// bytes survive as a damaged line for the next holder's load to preserve.
fn repair_tail(path: &Path) -> io::Result<()> {
    let Some(bytes) = read_if_present(path)? else {
        return Ok(());
    };
    if bytes.last().is_none_or(|&b| b == b'\n') {
        return Ok(());
    }
    match parse_lines(&bytes).1 {
        Shape::TornTail { keep } => OpenOptions::new().write(true).open(path)?.set_len(keep),
        _ => OpenOptions::new().append(true).open(path)?.write_all(b"\n"),
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
                }
                _ => {}
            }
        }
    }

    // A `ChangeArchived` already in the log may name its change by the DATED
    // archive directory: that is what a backfill recorded before
    // `change_lifecycle` was corrected to yield the bare logical id, while the
    // live watcher has always recorded the bare one. Without treating the two
    // spellings as one, a log written by an older build gains a second
    // archival for every historical change on the next reconcile, inflating
    // the shipped haul.
    //
    // But the alias is drawn from EVIDENCE, never applied unconditionally. A
    // change may legitimately be *named* `2026-06-05-add-thing` — the
    // date-headed shape `parser::archive_dir_logical_id` deliberately strips
    // only once, and which `tests/parser.rs` pins — and blindly aliasing every
    // persisted id to its stripped form would let that change's archival
    // permanently suppress the archival of a genuinely different change called
    // `add-thing`, under-counting the haul with no error.
    //
    // So a dated persisted id is read as a legacy record for its stripped form
    // only when git's own lifecycle set says that stripped form is a real
    // change AND the dated spelling is not itself one. When both exist they
    // are two distinct changes and no alias is drawn.
    let known: std::collections::HashSet<&str> = lifecycles
        .iter()
        .map(|lc| lc.change_name.as_str())
        .collect();
    let aliases: Vec<&str> = existing
        .iter()
        .filter(|e| matches!(e.kind, AchievementKind::ChangeArchived))
        .filter_map(|e| e.change_id.as_deref())
        .filter_map(|id| {
            let bare = crate::parser::archive_dir_logical_id(id);
            (bare != id && known.contains(bare) && !known.contains(id)).then_some(bare)
        })
        .collect();
    have_archived.extend(aliases);

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
        let dir = tempfile::tempdir().unwrap();

        let log = ActivityLog::open(dir.path());
        assert!(log.is_empty());
        log.record(ev(AchievementKind::TaskCompleted, ts(0), 3));
        log.record(ev(AchievementKind::ChangeArchived, ts(1), 1));
        drop(log);

        // Reload from disk and confirm both events survive.
        let reloaded = ActivityLog::open(dir.path());
        let snap = reloaded.snapshot();
        assert_eq!(snap.len(), 2);
        assert_eq!(total_for(&reloaded, AchievementKind::TaskCompleted), 3);
        assert_eq!(total_for(&reloaded, AchievementKind::ChangeArchived), 1);
    }

    #[test]
    fn query_window_is_bounded() {
        let dir = tempfile::tempdir().unwrap();

        let log = ActivityLog::open(dir.path());
        log.record(ev(AchievementKind::TaskCompleted, ts(0), 1)); // today
        log.record(ev(AchievementKind::TaskCompleted, ts(2), 1)); // 2 days ago
        log.record(ev(AchievementKind::TaskCompleted, ts(40), 1)); // far outside

        // A 7-day window keeps today + 2-days-ago, drops the 40-day-old event.
        let within = log.query_window(7);
        assert_eq!(within.len(), 2);
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

    /// A change may legitimately be NAMED `2026-06-05-add-thing` — the date
    /// strip is deliberately applied exactly once, so such an id survives it.
    /// Aliasing every persisted id to its stripped form unconditionally would
    /// let that change's archival permanently suppress the archival of a
    /// genuinely different change called `add-thing`, under-counting the haul
    /// with no error and no test failure. The alias must be drawn from git's
    /// own lifecycle set: when BOTH spellings name real changes they are two
    /// changes, and no alias is drawn.
    #[test]
    fn a_date_headed_change_id_does_not_suppress_a_different_change() {
        // The log covers only the date-headed change.
        let existing = vec![Achievement::new(
            AchievementKind::ChangeArchived,
            999,
            PathBuf::from("/ws"),
            Some("2026-06-05-add-thing".into()),
            1,
        )
        .as_backfilled()];

        // Git says BOTH are real, distinct changes.
        let lifecycles = vec![
            lc("2026-06-05-add-thing", None, Some(200)),
            lc("add-thing", None, Some(300)),
        ];
        let out = missing_lifecycle_events(&existing, Path::new("/ws"), &lifecycles);

        // The date-headed one is covered; the genuinely different `add-thing`
        // is still appended rather than being swallowed by a phantom alias.
        assert_eq!(out.len(), 1, "expected exactly the uncovered change");
        assert_eq!(out[0].change_id.as_deref(), Some("add-thing"));
        assert_eq!(out[0].kind, AchievementKind::ChangeArchived);
    }

    #[test]
    fn reconcile_lifecycle_appends_then_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();

        let log = ActivityLog::open(dir.path());
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
        assert_eq!(
            lines(dir.path()).len(),
            4,
            "every reconciled event persisted"
        );
        assert_eq!(log.persisted_appends(), 2, "one append per non-empty pass");
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
            ..Default::default()
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
        let dir = tempfile::tempdir().unwrap();

        let log = ActivityLog::open(dir.path());
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
    }

    // ---- persistence: JSON Lines, the lease, load, migration ----

    fn line_of(event: &Achievement) -> String {
        format!("{}\n", serde_json::to_string(event).unwrap())
    }

    fn task(magnitude: u32) -> Achievement {
        ev(
            AchievementKind::TaskCompleted,
            1_000 + i64::from(magnitude),
            magnitude,
        )
    }

    fn log_bytes(dir: &Path) -> Vec<u8> {
        fs::read(dir.join(LOG_FILE)).unwrap()
    }

    fn lines(dir: &Path) -> Vec<Achievement> {
        String::from_utf8(log_bytes(dir))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// Take the lease the way another instance would, so the log opened next
    /// is not its holder.
    fn hold_lease(dir: &Path) -> File {
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join(LOCK_FILE))
            .unwrap();
        lock.try_lock().unwrap();
        lock
    }

    #[test]
    fn a_batch_appends_its_lines_and_leaves_every_earlier_byte() {
        let dir = tempfile::tempdir().unwrap();
        let earlier: String = (1..=3).map(|n| line_of(&task(n))).collect();
        fs::write(dir.path().join(LOG_FILE), &earlier).unwrap();

        let log = ActivityLog::open(dir.path());
        log.record_all(vec![task(4), task(5)]);

        let bytes = log_bytes(dir.path());
        assert!(bytes.starts_with(earlier.as_bytes()));
        assert_eq!(
            String::from_utf8(bytes[earlier.len()..].to_vec()).unwrap(),
            line_of(&task(4)) + &line_of(&task(5))
        );
        assert_eq!(log.persisted_appends(), 1);
        assert_eq!(log.snapshot().len(), 5);
    }

    #[test]
    fn an_empty_batch_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let log = ActivityLog::open(dir.path());
        log.record_all(Vec::new());
        assert!(!dir.path().join(LOG_FILE).exists());
        assert_eq!(log.persisted_appends(), 0);
        assert_eq!(log.reconcile_lifecycle(Path::new("/ws"), &[]), 0);
        assert!(!dir.path().join(LOG_FILE).exists());
    }

    #[test]
    fn events_from_several_threads_reload_in_record_order() {
        let dir = tempfile::tempdir().unwrap();
        let log = std::sync::Arc::new(ActivityLog::open(dir.path()));
        let threads: Vec<_> = (0..8u32)
            .map(|t| {
                let log = log.clone();
                std::thread::spawn(move || {
                    for n in 0..25 {
                        log.record_all(vec![task(t * 100 + n), task(t * 100 + n + 50)]);
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let recorded = log.snapshot();
        assert_eq!(recorded.len(), 400);
        drop(log);

        assert_eq!(ActivityLog::open(dir.path()).snapshot(), recorded);
    }

    #[test]
    fn a_torn_final_entry_is_dropped_and_truncated_away() {
        let dir = tempfile::tempdir().unwrap();
        let good = line_of(&task(1)) + &line_of(&task(2));
        let torn = &line_of(&task(3))[..20];
        fs::write(dir.path().join(LOG_FILE), good.clone() + torn).unwrap();

        let log = ActivityLog::open(dir.path());

        assert_eq!(log.snapshot(), [task(1), task(2)]);
        assert!(!log.is_empty(), "the backfill must not re-run");
        assert_eq!(log_bytes(dir.path()), good.as_bytes());
        assert_eq!(entries(dir.path()), [LOG_FILE, LOCK_FILE]);
        log.record(task(4));
        assert_eq!(lines(dir.path()), [task(1), task(2), task(4)]);
    }

    #[test]
    fn a_terminated_unparseable_final_line_is_damage() {
        let dir = tempfile::tempdir().unwrap();
        let original = line_of(&task(1)) + "{\"kind\":\"taskCompleted\"\n";
        fs::write(dir.path().join(LOG_FILE), &original).unwrap();

        let log = ActivityLog::open(dir.path());

        assert_eq!(log.snapshot(), [task(1)]);
        assert_eq!(
            fs::read_to_string(dir.path().join("activity.jsonl.corrupt-0")).unwrap(),
            original
        );
        assert_eq!(lines(dir.path()), [task(1)]);
    }

    #[test]
    fn a_bad_middle_line_is_preserved_and_every_good_line_kept() {
        let dir = tempfile::tempdir().unwrap();
        let original = line_of(&task(1)) + "not json\n\n" + &line_of(&task(2));
        fs::write(dir.path().join(LOG_FILE), &original).unwrap();

        let log = ActivityLog::open(dir.path());
        log.record(task(3));

        assert_eq!(
            fs::read_to_string(dir.path().join("activity.jsonl.corrupt-0")).unwrap(),
            original
        );
        assert_eq!(lines(dir.path()), [task(1), task(2), task(3)]);
        drop(log);
        // The fresh file is clean: reopening moves nothing else aside.
        ActivityLog::open(dir.path());
        assert_eq!(
            entries(dir.path()),
            [LOG_FILE, "activity.jsonl.corrupt-0", LOCK_FILE]
        );
    }

    #[test]
    fn a_file_with_no_complete_good_line_is_moved_aside_whole() {
        for original in ["garbage\n", "{\"kind\":\"task", "\n\n{\"kind\""] {
            let dir = tempfile::tempdir().unwrap();
            fs::write(dir.path().join(LOG_FILE), original).unwrap();

            let log = ActivityLog::open(dir.path());

            assert!(log.is_empty(), "{original:?}");
            assert_eq!(
                fs::read_to_string(dir.path().join("activity.jsonl.corrupt-0")).unwrap(),
                original
            );
            assert_eq!(log_bytes(dir.path()), b"", "{original:?}");
        }
    }

    #[test]
    fn a_reader_parses_a_damaged_or_torn_log_and_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let _lease = hold_lease(dir.path());
        let original = line_of(&task(1)) + "bad\n" + &line_of(&task(2)) + "{\"kind";
        fs::write(dir.path().join(LOG_FILE), &original).unwrap();

        let log = ActivityLog::open(dir.path());
        log.record(task(3));

        assert_eq!(log.snapshot(), [task(1), task(2), task(3)]);
        assert_eq!(log_bytes(dir.path()), original.as_bytes());
        assert_eq!(entries(dir.path()), [LOG_FILE, LOCK_FILE]);
        assert_eq!(log.persisted_appends(), 0);
    }

    #[test]
    fn a_reader_falls_back_to_the_legacy_array() {
        let dir = tempfile::tempdir().unwrap();
        let _lease = hold_lease(dir.path());
        let legacy = serde_json::to_vec(&[task(1), task(2)]).unwrap();
        fs::write(dir.path().join(LEGACY_FILE), &legacy).unwrap();

        let log = ActivityLog::open(dir.path());

        assert_eq!(log.snapshot(), [task(1), task(2)]);
        assert_eq!(fs::read(dir.path().join(LEGACY_FILE)).unwrap(), legacy);
        assert_eq!(entries(dir.path()), [LEGACY_FILE, LOCK_FILE]);
    }

    #[test]
    fn a_legacy_array_migrates_whole_and_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let events = vec![
            task(1)
                .as_backfilled()
                .with_author(Some(author(Some("Me"), Some("me@x.com")))),
            Achievement::new(
                AchievementKind::ChangeArchived,
                5,
                PathBuf::from("/ws"),
                Some("foo".into()),
                1,
            ),
        ];
        let legacy = serde_json::to_vec_pretty(&events).unwrap();
        fs::write(dir.path().join(LEGACY_FILE), &legacy).unwrap();

        let log = ActivityLog::open(dir.path());
        log.record(task(9));

        assert_eq!(log.snapshot()[..2], events[..]);
        assert_eq!(
            fs::read(dir.path().join("activity.json.migrated")).unwrap(),
            legacy
        );
        assert!(!dir.path().join(LEGACY_FILE).exists());
        assert_eq!(
            lines(dir.path()),
            [events[0].clone(), events[1].clone(), task(9)]
        );
    }

    #[test]
    fn a_leftover_legacy_file_never_replaces_a_kept_copy() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(LOG_FILE), line_of(&task(1))).unwrap();
        fs::write(dir.path().join("activity.json.migrated"), "first copy").unwrap();
        let again = serde_json::to_vec(&[task(7)]).unwrap();
        fs::write(dir.path().join(LEGACY_FILE), &again).unwrap();

        let log = ActivityLog::open(dir.path());

        assert_eq!(log.snapshot(), [task(1)], "the log wins");
        assert_eq!(
            fs::read_to_string(dir.path().join("activity.json.migrated")).unwrap(),
            "first copy"
        );
        assert_eq!(
            fs::read(dir.path().join("activity.json.migrated-1")).unwrap(),
            again
        );
        assert!(!dir.path().join(LEGACY_FILE).exists());
        assert_eq!(keep_legacy(&dir.path().join(LEGACY_FILE)), None);
    }

    #[test]
    fn an_unparseable_legacy_file_is_moved_aside_and_the_log_starts_empty() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(LEGACY_FILE), "[{\"kind\":").unwrap();

        let log = ActivityLog::open(dir.path());
        assert!(log.is_empty());
        assert_eq!(
            fs::read_to_string(dir.path().join("activity.json.corrupt-0")).unwrap(),
            "[{\"kind\":"
        );
        log.record(task(1));
        assert_eq!(lines(dir.path()), [task(1)]);
    }

    #[test]
    fn only_the_lease_holder_appends_and_the_lease_passes_on() {
        let dir = tempfile::tempdir().unwrap();
        let first = ActivityLog::open(dir.path());
        let second = ActivityLog::open(dir.path());

        second.record(task(1));
        assert!(!dir.path().join(LOG_FILE).exists(), "a reader never writes");
        assert_eq!(second.query_since(0), [task(1)], "but serves its own view");

        first.record(task(2));
        assert_eq!(lines(dir.path()), [task(2)]);

        drop(first);
        second.record(task(3));
        assert_eq!(
            lines(dir.path()),
            [task(2), task(3)],
            "only what it records after acquiring"
        );
        assert_eq!(second.persisted_appends(), 1);
    }

    #[test]
    fn a_new_holder_trims_a_torn_tail_before_appending() {
        let dir = tempfile::tempdir().unwrap();
        let lease = hold_lease(dir.path());
        let log = ActivityLog::open(dir.path());
        fs::write(
            dir.path().join(LOG_FILE),
            line_of(&task(1)) + &line_of(&task(2))[..15],
        )
        .unwrap();
        drop(lease);

        log.record(task(3));

        assert_eq!(lines(dir.path()), [task(1), task(3)]);
    }

    #[test]
    fn a_new_holder_moves_a_lone_fragment_aside_as_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let lease = hold_lease(dir.path());
        let log = ActivityLog::open(dir.path());
        fs::write(dir.path().join(LOG_FILE), "{\"kind").unwrap();
        drop(lease);

        log.record(task(3));

        assert_eq!(
            fs::read_to_string(dir.path().join("activity.jsonl.corrupt-0")).unwrap(),
            "{\"kind"
        );
        assert_eq!(lines(dir.path()), [task(3)]);
    }

    /// A lifecycle fact the previous holder persisted is not appended again by
    /// the instance that takes the lease over, although it never saw it.
    #[test]
    fn a_new_holder_does_not_re_append_the_previous_holders_lifecycle_facts() {
        let dir = tempfile::tempdir().unwrap();
        let first = ActivityLog::open(dir.path());
        let second = ActivityLog::open(dir.path());
        let lifecycles = [lc("x", Some(1), Some(2))];

        assert_eq!(first.reconcile_lifecycle(Path::new("/ws"), &lifecycles), 2);
        drop(first);
        assert_eq!(second.reconcile_lifecycle(Path::new("/ws"), &lifecycles), 0);

        assert_eq!(lines(dir.path()).len(), 2);
        let kinds: Vec<AchievementKind> = second.snapshot().iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            [
                AchievementKind::ChangeCreated,
                AchievementKind::ChangeArchived
            ],
            "the merged facts reach the new holder's views"
        );
    }

    /// A live lifecycle fact the previous holder persisted covers the same
    /// change's backfilled fact, as `(kind, change_id)` does in memory.
    #[test]
    fn a_previous_holders_live_lifecycle_fact_is_not_reconciled_again() {
        let dir = tempfile::tempdir().unwrap();
        let first = ActivityLog::open(dir.path());
        let second = ActivityLog::open(dir.path());
        first.record(Achievement::new(
            AchievementKind::ChangeArchived,
            50,
            PathBuf::from("/ws"),
            Some("x".into()),
            1,
        ));
        drop(first);

        let reconciled = second.reconcile_lifecycle(Path::new("/ws"), &[lc("x", None, Some(2))]);

        assert_eq!(reconciled, 0);
        assert_eq!(lines(dir.path()).len(), 1);
    }

    /// A backfill the previous holder persisted is merged, an identical
    /// backfilled event is not appended twice, and live events are neither
    /// merged nor deduplicated.
    #[test]
    fn a_new_holder_merges_backfill_and_leaves_live_events_alone() {
        let dir = tempfile::tempdir().unwrap();
        let first = ActivityLog::open(dir.path());
        let second = ActivityLog::open(dir.path());
        let filled = |n| task(n).as_backfilled();

        second.record_all(vec![filled(1), task(7)]);
        first.record_all(vec![filled(1), filled(2), task(7), task(8)]);
        drop(first);
        second.record_all(vec![filled(2), filled(3), task(7)]);

        assert_eq!(
            lines(dir.path()),
            [filled(1), filled(2), task(7), task(8), filled(3), task(7)]
        );
        assert_eq!(
            second.snapshot(),
            [filled(1), task(7), filled(2), filled(3), task(7)]
        );
    }

    #[test]
    fn a_new_holder_migrates_a_legacy_file_before_appending() {
        let dir = tempfile::tempdir().unwrap();
        let lease = hold_lease(dir.path());
        let legacy = serde_json::to_vec(&[task(1)]).unwrap();
        fs::write(dir.path().join(LEGACY_FILE), &legacy).unwrap();
        let log = ActivityLog::open(dir.path());
        drop(lease);

        log.record(task(2));

        assert_eq!(lines(dir.path()), [task(1), task(2)]);
        assert_eq!(
            fs::read(dir.path().join("activity.json.migrated")).unwrap(),
            legacy
        );
    }

    #[test]
    fn a_failed_append_re_checks_the_tail_before_the_next() {
        let dir = tempfile::tempdir().unwrap();
        let log = ActivityLog::open(dir.path());
        let path = dir.path().join(LOG_FILE);
        fs::create_dir(&path).unwrap();

        log.record(task(1));
        assert_eq!(log.persisted_appends(), 0, "the append failed");

        fs::remove_dir(&path).unwrap();
        fs::write(&path, line_of(&task(1)) + "{\"ki").unwrap();
        log.record(task(2));

        assert_eq!(lines(dir.path()), [task(1), task(2)]);
        assert_eq!(log.persisted_appends(), 1);
    }

    #[test]
    fn an_unreadable_log_is_left_alone_and_never_appended_to() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(LOG_FILE);
        fs::create_dir(&path).unwrap();

        let log = ActivityLog::open(dir.path());
        fs::remove_dir(&path).unwrap();
        log.record(task(1));

        assert!(!path.exists());
        assert_eq!(log.snapshot(), [task(1)]);
    }

    #[test]
    fn concurrent_reconciliations_append_each_missing_event_once() {
        let dir = tempfile::tempdir().unwrap();
        let log = std::sync::Arc::new(ActivityLog::open(dir.path()));
        let lifecycles: Vec<_> = (0..20)
            .map(|n| lc(&format!("c{n}"), Some(n), Some(n + 100)))
            .collect();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
        let counts: Vec<usize> = (0..4)
            .map(|_| {
                let (log, lifecycles, barrier) = (log.clone(), lifecycles.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    log.reconcile_lifecycle(Path::new("/ws"), &lifecycles)
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect();

        assert_eq!(counts.iter().sum::<usize>(), 40);
        assert_eq!(lines(dir.path()).len(), 40);
    }

    #[test]
    fn the_window_includes_an_event_at_its_first_instant() {
        let dir = tempfile::tempdir().unwrap();
        let log = ActivityLog::open(dir.path());
        let start = window_start_ts(7);
        log.record(ev(AchievementKind::TaskCompleted, start, 1));
        log.record(ev(AchievementKind::TaskCompleted, start - 1, 1));
        assert_eq!(log.query_window(7).len(), 1);
    }

    #[test]
    fn parsing_reports_each_shape() {
        let one = line_of(&task(1));
        assert_eq!(parse_lines(b""), (vec![], Shape::Clean));
        assert_eq!(parse_lines(b"\n \n"), (vec![], Shape::Clean));
        assert_eq!(parse_lines(one.as_bytes()), (vec![task(1)], Shape::Clean));
        assert_eq!(
            parse_lines((one.clone() + "{").as_bytes()),
            (
                vec![task(1)],
                Shape::TornTail {
                    keep: one.len() as u64
                }
            )
        );
        assert_eq!(parse_lines(b"{"), (vec![], Shape::Damaged));
        assert_eq!(
            parse_lines((one.clone() + "x\n").as_bytes()),
            (vec![task(1)], Shape::Damaged)
        );
        assert_eq!(
            parse_lines(("x\n".to_string() + &one + "{").as_bytes()),
            (vec![task(1)], Shape::Damaged)
        );
    }

    /// A workspace path that is not UTF-8 cannot be written as JSON; such an
    /// event stays in memory and its neighbours are still written whole.
    #[cfg(unix)]
    #[test]
    fn an_unserialisable_event_is_left_out_whole() {
        use std::os::unix::ffi::OsStrExt;
        let mut odd = task(2);
        odd.workspace = PathBuf::from(std::ffi::OsStr::from_bytes(b"/ws\xff"));
        let out = String::from_utf8(to_lines(&[task(1), odd, task(3)])).unwrap();
        assert_eq!(out, line_of(&task(1)) + &line_of(&task(3)));
    }
}
