//! Seasons: a monthly battle-pass layer over the activity log.
//!
//! Pure and Tauri-free so it is unit-testable from `cargo test`. Everything here
//! is a **derivation** over data the app already has — the append-only
//! [`Achievement`](crate::activity_log::Achievement) log plus git commit counts —
//! except the treatment *locker*, which the Tauri shell persists. No new event
//! kind is recorded for seasons; standings are recomputed, like the dashboard's
//! other views.
//!
//! Two tracks (see the `seasons` capability): the **seasonal** track (score →
//! band/tier ladder, objectives, treatments) resets every calendar month; the
//! **career** track ([`career_tier`]) is derived from lifetime totals and only
//! rises *through play* — it is monotonic in lifetime ships against a fixed set
//! of [`CAREER_THRESHOLDS`], so organic progression never demotes. A deliberate
//! retune of those thresholds is a rebalance and recomputes the tier (which may
//! then change, including downward) — outside the never-demote guarantee. The
//! streak is a career line and is handled by the dashboard, not reset here.
//!
//! Determinism is the backbone: the season **name**, **objective** rotation, and
//! **treatment** descriptors are pure functions of the integer season index, so
//! the same month always reproduces — which is what lets backfilled history
//! reconstruct past seasons identically.

use crate::activity_log::{local_day, Achievement, AchievementKind, AchievementTotals};
use chrono::{Datelike, Local, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// Bumped if the treatment generator or band math changes; baked into every
/// treatment id so a previously unlocked treatment keeps its original rendering.
pub const GENERATOR_VERSION: u32 = 1;

/// Number of (non-overflow) tiers in a season's battle-pass track.
pub const TIER_COUNT: u32 = 30;
/// Tiers per named band (`TIER_COUNT / BANDS.len()`).
const TIERS_PER_BAND: u32 = 5;
const BANDS: [&str; 6] = ["Bronze", "Silver", "Gold", "Platinum", "Diamond", "Master"];
const ROMAN: [&str; 5] = ["I", "II", "III", "IV", "V"];

/// How many objectives a season presents.
pub const OBJECTIVES_PER_SEASON: usize = 3;
/// Bonus season score granted per completed objective.
pub const OBJECTIVE_BONUS: u32 = 40;

// Score weights. Ships lead; the within-change increments and commits trail.
const ACTIVE_DAY_POINTS: u32 = 10;
const COMMIT_POINTS: u32 = 2;

// Adaptive-pacing knobs (calibration; tune against real cadence later).
const COMPLETION_FACTOR: f64 = 0.85;
const FLOOR_TOTAL: u32 = 300;
// Raised from 6_000: a heavy dogfooding month (~940 pts/day observed) blew past
// a 6k cap in ~6 days, capping per_tier at 200 and defeating the adaptive
// scaling. At 30k the adaptive target governs for all but the most extreme
// cadence, so the climb tracks real output instead of slamming the ceiling.
const CEIL_TOTAL: u32 = 30_000;

/// Per-kind season-score weight (per unit of magnitude).
pub fn weight_for(kind: AchievementKind) -> u32 {
    match kind {
        AchievementKind::ChangeArchived => 50, // the headline act
        AchievementKind::ArtifactReached => 15,
        AchievementKind::TaskCompleted => 8,
        AchievementKind::ChangeCreated => 5, // low, to deter gaming via empty changes
    }
}

/// A stable integer index for a calendar month: `year*12 + (month-1)`.
pub fn season_index_for(year: i32, month: u32) -> i64 {
    year as i64 * 12 + (month as i64 - 1)
}

/// The `(year, month)` a season index denotes (inverse of [`season_index_for`]).
pub fn season_year_month(index: i64) -> (i32, u32) {
    (index.div_euclid(12) as i32, index.rem_euclid(12) as u32 + 1)
}

/// The season index for the present local month.
pub fn current_season_index() -> i64 {
    let now = Local::now();
    season_index_for(now.year(), now.month())
}

/// First-instant-of-month local unix timestamp.
fn local_month_start_ts(year: i32, month: u32) -> i64 {
    let date = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let dt = date.and_hms_opt(0, 0, 0).unwrap();
    Local
        .from_local_datetime(&dt)
        .earliest()
        .map(|d| d.timestamp())
        .unwrap_or(0)
}

/// Number of calendar days in a month.
fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let next = NaiveDate::from_ymd_opt(ny, nm, 1).unwrap();
    (next - first).num_days() as u32
}

/// The half-open window `[month start, next month start)` for a season, as local
/// unix timestamps.
pub fn season_window(index: i64) -> (i64, i64) {
    let (y, m) = season_year_month(index);
    let start = local_month_start_ts(y, m);
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    let end = local_month_start_ts(ny, nm);
    (start, end)
}

// --- deterministic mixing (splitmix64) -------------------------------------

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn seed2(a: i64, b: i64) -> u64 {
    splitmix64((a as u64).wrapping_mul(0x0100_0000_01B3) ^ splitmix64(b as u64))
}

const ADJ: [&str; 16] = [
    "Quiet", "Amber", "Crimson", "Verdant", "Azure", "Golden", "Silent", "Restless", "Northern",
    "Hidden", "Distant", "Endless", "Frosted", "Ember", "Lucid", "Wandering",
];
const NOUN: [&str; 16] = [
    "Tide", "Forge", "Ember", "Summit", "Drift", "Harbor", "Meridian", "Thicket", "Lantern",
    "Current", "Cinder", "Vale", "Aurora", "Bastion", "Reverie", "Expanse",
];

/// A deterministic two-word season name from the season index (no per-season
/// authoring). Stable: the same index always yields the same name.
pub fn season_name(index: i64) -> String {
    let h = splitmix64(index as u64 ^ 0xA5A5_5A5A_1234_ABCD);
    let adj = ADJ[(h % ADJ.len() as u64) as usize];
    let noun = NOUN[((h / ADJ.len() as u64) % NOUN.len() as u64) as usize];
    format!("{adj} {noun}")
}

/// Identifying header for a season.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonInfo {
    pub index: i64,
    pub name: String,
    pub year: i32,
    pub month: u32,
    pub start_ts: i64,
    pub end_ts: i64,
}

/// Build the [`SeasonInfo`] for a season index.
pub fn season_info(index: i64) -> SeasonInfo {
    let (year, month) = season_year_month(index);
    let (start_ts, end_ts) = season_window(index);
    SeasonInfo {
        index,
        name: season_name(index),
        year,
        month,
        start_ts,
        end_ts,
    }
}

// --- score -----------------------------------------------------------------

/// The local day-ordinal (days from CE) a timestamp falls on, for streak/gap math.
fn day_ordinal(ts: i64) -> i32 {
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.date_naive().num_days_from_ce())
        .unwrap_or(0)
}

/// Derived per-season aggregates over a window's events — the basis for both
/// the score's active-day credit and every objective's progress.
#[derive(Debug, Clone, Default)]
pub struct SeasonStats {
    pub ships: u32,
    pub tasks: u32,
    pub artifacts: u32,
    pub creates: u32,
    pub active_days: BTreeSet<i32>,
    pub per_day_tasks: BTreeMap<i32, u32>,
    created_ids: HashSet<String>,
    archived_ids: HashSet<String>,
}

impl SeasonStats {
    /// Aggregate the (already Me-scoped, already in-window) `events`.
    pub fn from_events(events: &[Achievement]) -> Self {
        let mut s = SeasonStats::default();
        for e in events {
            let ord = day_ordinal(e.timestamp);
            s.active_days.insert(ord);
            match e.kind {
                AchievementKind::ChangeArchived => {
                    s.ships += e.magnitude;
                    if let Some(id) = &e.change_id {
                        s.archived_ids.insert(id.clone());
                    }
                }
                AchievementKind::ArtifactReached => s.artifacts += e.magnitude,
                AchievementKind::TaskCompleted => {
                    s.tasks += e.magnitude;
                    *s.per_day_tasks.entry(ord).or_insert(0) += e.magnitude;
                }
                AchievementKind::ChangeCreated => {
                    s.creates += e.magnitude;
                    if let Some(id) = &e.change_id {
                        s.created_ids.insert(id.clone());
                    }
                }
            }
        }
        s
    }

    /// Changes both created and archived within the window (full lifecycle).
    pub fn finishes(&self) -> u32 {
        self.created_ids.intersection(&self.archived_ids).count() as u32
    }

    /// The longest run of consecutive active local days within the window.
    pub fn longest_streak(&self) -> u32 {
        let mut best = 0u32;
        let mut run = 0u32;
        let mut prev: Option<i32> = None;
        for &d in &self.active_days {
            run = match prev {
                Some(p) if d == p + 1 => run + 1,
                _ => 1,
            };
            best = best.max(run);
            prev = Some(d);
        }
        best
    }

    /// The largest single-day task count (for the "burst" objective).
    pub fn max_daily_tasks(&self) -> u32 {
        self.per_day_tasks.values().copied().max().unwrap_or(0)
    }

    /// Whether the developer returned after a gap of at least `gap` idle days —
    /// an active day following another active day more than `gap` days earlier.
    pub fn came_back_after(&self, gap: i32) -> bool {
        let mut prev: Option<i32> = None;
        for &d in &self.active_days {
            if let Some(p) = prev {
                if d - p > gap {
                    return true;
                }
            }
            prev = Some(d);
        }
        false
    }
}

/// The base season score from a window's Me-scoped events and Me-authored commit
/// count — a weighted sum plus active-day credit. Objective bonuses are added on
/// top by [`compute_season`]. Monotonic non-decreasing as activity accrues.
pub fn season_score(events: &[Achievement], commit_count: u32) -> u32 {
    let stats = SeasonStats::from_events(events);
    score_from(&stats, events, commit_count)
}

fn score_from(stats: &SeasonStats, events: &[Achievement], commit_count: u32) -> u32 {
    let mut total = 0u32;
    for e in events {
        total += weight_for(e.kind).saturating_mul(e.magnitude);
    }
    total += stats.active_days.len() as u32 * ACTIVE_DAY_POINTS;
    total += commit_count.saturating_mul(COMMIT_POINTS);
    total
}

// --- adaptive pacing + tier ladder -----------------------------------------

/// The developer's recent per-day output, used to scale the pass. Mirrors the
/// recent-daily-averages the dashboard already computes for the Today comparison.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonBaseline {
    pub ships_per_day: f64,
    pub tasks_per_day: f64,
    pub commits_per_day: f64,
}

/// The total score that completes the pass, scaled to the developer's baseline
/// over the month and clamped between a floor and ceiling so it is never trivial
/// nor impossible.
pub fn target_total(baseline: &SeasonBaseline, days: u32) -> u32 {
    let per_day = baseline.ships_per_day * weight_for(AchievementKind::ChangeArchived) as f64
        + baseline.tasks_per_day * weight_for(AchievementKind::TaskCompleted) as f64
        + baseline.commits_per_day * COMMIT_POINTS as f64
        + ACTIVE_DAY_POINTS as f64;
    let raw = per_day * days as f64 * COMPLETION_FACTOR;
    (raw.round() as i64).clamp(FLOOR_TOTAL as i64, CEIL_TOTAL as i64) as u32
}

/// Where a score sits on the battle-pass ladder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BandTier {
    pub score: u32,
    /// Tiers crossed (0 = unranked, capped-display handled by `label`).
    pub tier: u32,
    pub band: String,
    /// e.g. "Gold II", or "Unranked" at zero, or "Master ∞+N" in overflow.
    pub label: String,
    pub gap_to_next: u32,
    pub next_threshold: u32,
    pub per_tier: u32,
    pub overflow: bool,
}

/// Map a `score` to its tier/band given the season's completion `total`.
pub fn tier_for(score: u32, total: u32) -> BandTier {
    let per_tier = (total / TIER_COUNT).max(1);
    let crossed = score / per_tier;
    let next_threshold = (crossed + 1).saturating_mul(per_tier);
    let gap_to_next = next_threshold.saturating_sub(score);
    let overflow = crossed >= TIER_COUNT;
    let label = if crossed == 0 {
        "Unranked".to_string()
    } else if overflow {
        format!("Master ∞+{}", crossed - TIER_COUNT)
    } else {
        let band_idx = ((crossed - 1) / TIERS_PER_BAND) as usize;
        let within = ((crossed - 1) % TIERS_PER_BAND) as usize;
        format!("{} {}", BANDS[band_idx.min(BANDS.len() - 1)], ROMAN[within])
    };
    let band = if overflow {
        BANDS[BANDS.len() - 1].to_string()
    } else if crossed == 0 {
        "Unranked".to_string()
    } else {
        BANDS[(((crossed - 1) / TIERS_PER_BAND) as usize).min(BANDS.len() - 1)].to_string()
    };
    BandTier {
        score,
        tier: crossed,
        band,
        label,
        gap_to_next,
        next_threshold,
        per_tier,
        overflow,
    }
}

// --- objectives ------------------------------------------------------------

/// Reusable objective archetypes — all derivable from existing achievement data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Archetype {
    ShipVolume,
    TaskVolume,
    ActiveDays,
    StreakLength,
    DailyBurst,
    Finish,
    Comeback,
}

const ARCHETYPES: [Archetype; 7] = [
    Archetype::ShipVolume,
    Archetype::TaskVolume,
    Archetype::ActiveDays,
    Archetype::StreakLength,
    Archetype::DailyBurst,
    Archetype::Finish,
    Archetype::Comeback,
];

/// The archetypes a season presents — a contiguous block of [`OBJECTIVES_PER_SEASON`]
/// stepping by the same amount each season, so adjacent seasons never share one.
pub fn season_archetypes(index: i64) -> Vec<Archetype> {
    let n = ARCHETYPES.len() as i64;
    (0..OBJECTIVES_PER_SEASON as i64)
        .map(|j| {
            let k = (index * OBJECTIVES_PER_SEASON as i64 + j).rem_euclid(n) as usize;
            ARCHETYPES[k]
        })
        .collect()
}

/// One generated objective with its baseline-scaled target and derived progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonObjective {
    pub archetype: Archetype,
    pub title: String,
    pub target: u32,
    pub progress: u32,
    pub complete: bool,
}

fn scaled(v: f64, lo: u32, hi: u32) -> u32 {
    (v.round() as i64).clamp(lo as i64, hi as i64) as u32
}

/// The target for an archetype scaled to the developer's baseline over `days`.
fn objective_target(arch: Archetype, baseline: &SeasonBaseline, days: u32) -> u32 {
    let d = days as f64;
    match arch {
        Archetype::ShipVolume => scaled(baseline.ships_per_day * d * 0.8, 1, 999),
        Archetype::TaskVolume => scaled(baseline.tasks_per_day * d * 0.8, 3, 9999),
        Archetype::ActiveDays => scaled(d * 0.5, 3, days),
        Archetype::StreakLength => scaled(d * 0.25, 3, 14),
        Archetype::DailyBurst => scaled(baseline.tasks_per_day * 1.5, 2, 99),
        Archetype::Finish => scaled(baseline.ships_per_day * d * 0.3, 1, 99),
        Archetype::Comeback => 1,
    }
}

fn objective_progress(arch: Archetype, stats: &SeasonStats, commit_count: u32) -> u32 {
    let _ = commit_count;
    match arch {
        Archetype::ShipVolume => stats.ships,
        Archetype::TaskVolume => stats.tasks,
        Archetype::ActiveDays => stats.active_days.len() as u32,
        Archetype::StreakLength => stats.longest_streak(),
        Archetype::DailyBurst => stats.max_daily_tasks(),
        Archetype::Finish => stats.finishes(),
        Archetype::Comeback => u32::from(stats.came_back_after(3)),
    }
}

fn objective_title(arch: Archetype, target: u32) -> String {
    match arch {
        Archetype::ShipVolume => format!("Ship {target} changes"),
        Archetype::TaskVolume => format!("Complete {target} tasks"),
        Archetype::ActiveDays => format!("Be active on {target} days"),
        Archetype::StreakLength => format!("Hold a {target}-day streak"),
        Archetype::DailyBurst => format!("Complete {target} tasks in one day"),
        Archetype::Finish => format!("Finish {target} change(s) end-to-end"),
        Archetype::Comeback => "Make a comeback after a quiet stretch".to_string(),
    }
}

/// The objectives for a season, with progress derived from `stats`.
pub fn season_objectives(
    index: i64,
    baseline: &SeasonBaseline,
    days: u32,
    stats: &SeasonStats,
    commit_count: u32,
) -> Vec<SeasonObjective> {
    season_archetypes(index)
        .into_iter()
        .map(|arch| {
            let target = objective_target(arch, baseline, days);
            let progress = objective_progress(arch, stats, commit_count);
            SeasonObjective {
                archetype: arch,
                title: objective_title(arch, target),
                target,
                progress,
                complete: progress >= target,
            }
        })
        .collect()
}

// --- procedural treatments -------------------------------------------------

/// Rarity of a battle-pass treatment, rising with tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Rarity {
    Common,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    /// Ordering rank (higher = rarer), for comparisons and tests.
    pub fn rank(self) -> u8 {
        match self {
            Rarity::Common => 0,
            Rarity::Rare => 1,
            Rarity::Epic => 2,
            Rarity::Legendary => 3,
        }
    }
}

/// Rarity for a 1-based tier index; overflow tiers (beyond [`TIER_COUNT`]) are
/// always legendary.
pub fn rarity_for_tier(tier: u32) -> Rarity {
    if tier == 0 {
        return Rarity::Common;
    }
    if tier > TIER_COUNT {
        return Rarity::Legendary;
    }
    let f = tier as f64 / TIER_COUNT as f64;
    if f <= 0.5 {
        Rarity::Common
    } else if f <= 0.8 {
        Rarity::Rare
    } else {
        Rarity::Epic
    }
}

const EFFECTS: [&str; 8] = [
    "holo", "sheen", "ember", "frost", "prism", "aurora", "static", "gilded",
];

/// A deterministic, local description of a badge treatment. Carries no artwork —
/// the desktop app renders it (token-palette tint + a build-time asset for the
/// effect). `id` encodes the generator version so a later generator change does
/// not alter a previously unlocked treatment's rendering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreatmentDescriptor {
    pub id: String,
    pub season_index: i64,
    pub tier_index: u32,
    pub rarity: Rarity,
    /// Palette indices into the application's token palette.
    pub palette: Vec<u32>,
    pub effect: String,
    pub generator_version: u32,
}

/// Rebuild a treatment descriptor from its persisted id (`s{season}-t{tier}-g{gen}`).
/// Returns `None` for an unparseable id. Used to render an equipped treatment
/// whose original season may be in the past.
pub fn treatment_from_id(id: &str) -> Option<TreatmentDescriptor> {
    let rest = id.strip_prefix('s')?;
    let (season, rest) = rest.split_once("-t")?;
    let (tier, gen) = rest.split_once("-g")?;
    let season: i64 = season.parse().ok()?;
    let tier: u32 = tier.parse().ok()?;
    let _gen: u32 = gen.parse().ok()?;
    Some(treatment(season, tier))
}

/// The treatment unlocked at `(season_index, tier_index)` — deterministic.
pub fn treatment(season_index: i64, tier_index: u32) -> TreatmentDescriptor {
    let h = seed2(season_index, tier_index as i64);
    let effect = EFFECTS[(h % EFFECTS.len() as u64) as usize].to_string();
    let palette = vec![
        (h % 12) as u32,
        ((h >> 8) % 12) as u32,
        ((h >> 16) % 12) as u32,
    ];
    TreatmentDescriptor {
        id: format!("s{season_index}-t{tier_index}-g{GENERATOR_VERSION}"),
        season_index,
        tier_index,
        rarity: rarity_for_tier(tier_index),
        palette,
        effect,
        generator_version: GENERATOR_VERSION,
    }
}

// --- career tier (permanent) -----------------------------------------------

const CAREER_LABELS: [&str; 7] = [
    "Newcomer",
    "Initiate",
    "Builder",
    "Maker",
    "Shipwright",
    "Veteran",
    "Architect",
];
const CAREER_THRESHOLDS: [u32; 6] = [50, 250, 750, 1500, 3000, 5000];

/// The permanent career tier — distinct from the seasonal band and monotonic in
/// lifetime ships.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CareerTier {
    pub tier: u32,
    pub label: String,
    pub ships: u32,
    pub next_at: Option<u32>,
}

/// Derive the career tier from lifetime totals (ships drive it).
pub fn career_tier(totals: &AchievementTotals) -> CareerTier {
    let ships = totals.changes_archived;
    let tier = CAREER_THRESHOLDS.iter().filter(|&&t| ships >= t).count() as u32;
    let next_at = CAREER_THRESHOLDS.get(tier as usize).copied();
    CareerTier {
        tier,
        label: CAREER_LABELS[(tier as usize).min(CAREER_LABELS.len() - 1)].to_string(),
        ships,
        next_at,
    }
}

// --- soft-FOMO vault -------------------------------------------------------

/// The past seasons whose treatments are currently re-earnable (soft FOMO). A
/// deterministic, rotating subset of recent past seasons — so a treatment missed
/// in its original season can come back, while earned ones are never lost.
pub fn vault(current_index: i64) -> Vec<i64> {
    let offsets = [2 + current_index.rem_euclid(3), 4 + current_index.rem_euclid(2)];
    let mut out: Vec<i64> = offsets
        .iter()
        .map(|o| current_index - o)
        .filter(|&i| i >= 0 && i < current_index)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

// --- recap -----------------------------------------------------------------

/// An auto-minted summary of a finished season.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonRecap {
    pub season: SeasonInfo,
    pub shipped: u32,
    pub tasks_completed: u32,
    pub commits: u32,
    pub best_streak: u32,
    pub band: String,
    pub tier: u32,
    pub objectives_completed: u32,
    pub treatments_unlocked: u32,
}

/// Synthesise a recap for a finished season from its window's Me-scoped events.
pub fn season_recap(
    index: i64,
    events: &[Achievement],
    commit_count: u32,
    baseline: &SeasonBaseline,
) -> SeasonRecap {
    let info = season_info(index);
    let days = days_in_month(info.year, info.month);
    let stats = SeasonStats::from_events(events);
    let objectives = season_objectives(index, baseline, days, &stats, commit_count);
    let objectives_completed = objectives.iter().filter(|o| o.complete).count() as u32;
    let score =
        score_from(&stats, events, commit_count) + objectives_completed * OBJECTIVE_BONUS;
    let ladder = tier_for(score, target_total(baseline, days));
    SeasonRecap {
        season: info,
        shipped: stats.ships,
        tasks_completed: stats.tasks,
        commits: commit_count,
        best_streak: stats.longest_streak(),
        band: ladder.band,
        tier: ladder.tier,
        objectives_completed,
        treatments_unlocked: ladder.tier.min(TIER_COUNT) + ladder.tier.saturating_sub(TIER_COUNT),
    }
}

// --- top-level standing ----------------------------------------------------

/// The full live standing for the active season — what the dashboard renders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonStanding {
    pub season: SeasonInfo,
    pub score: u32,
    pub ladder: BandTier,
    pub objectives: Vec<SeasonObjective>,
    pub career: CareerTier,
    pub next_treatment: Option<TreatmentDescriptor>,
    pub days_in_month: u32,
}

/// Compute the active season's standing from its Me-scoped window `events`, the
/// Me-authored `commit_count`, the developer `baseline`, and lifetime `totals`.
pub fn compute_season(
    index: i64,
    events: &[Achievement],
    commit_count: u32,
    baseline: &SeasonBaseline,
    totals: &AchievementTotals,
) -> SeasonStanding {
    let info = season_info(index);
    let days = days_in_month(info.year, info.month);
    let stats = SeasonStats::from_events(events);
    let objectives = season_objectives(index, baseline, days, &stats, commit_count);
    let bonus = objectives.iter().filter(|o| o.complete).count() as u32 * OBJECTIVE_BONUS;
    let score = score_from(&stats, events, commit_count) + bonus;
    let ladder = tier_for(score, target_total(baseline, days));
    let next_treatment = Some(treatment(index, ladder.tier + 1));
    SeasonStanding {
        season: info,
        score,
        ladder,
        objectives,
        career: career_tier(totals),
        next_treatment,
        days_in_month: days,
    }
}

/// The treatments a score has unlocked this season: tier 1..=crossed.
pub fn unlocked_treatments(index: i64, ladder: &BandTier) -> Vec<TreatmentDescriptor> {
    (1..=ladder.tier).map(|t| treatment(index, t)).collect()
}

/// True when `ts` falls inside the season's window.
pub fn in_season(index: i64, ts: i64) -> bool {
    let (start, end) = season_window(index);
    ts >= start && ts < end
}

/// The local day a timestamp belongs to (re-export convenience).
pub fn season_day(ts: i64) -> String {
    local_day(ts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ev(kind: AchievementKind, ts: i64, mag: u32, change: Option<&str>) -> Achievement {
        Achievement::new(kind, ts, PathBuf::from("/ws"), change.map(str::to_string), mag)
    }

    fn day_ts(ord_offset: i64) -> i64 {
        // A timestamp `ord_offset` days from a fixed local noon anchor.
        let base = Local
            .with_ymd_and_hms(2026, 6, 15, 12, 0, 0)
            .single()
            .unwrap()
            .timestamp();
        base + ord_offset * 86_400
    }

    #[test]
    fn season_index_round_trips() {
        for (y, m) in [(2026, 1), (2026, 6), (2026, 12), (1999, 7)] {
            let idx = season_index_for(y, m);
            assert_eq!(season_year_month(idx), (y, m));
        }
        // Adjacent months are adjacent indices.
        assert_eq!(
            season_index_for(2026, 12) + 1,
            season_index_for(2027, 1)
        );
    }

    #[test]
    fn season_window_spans_one_month_and_is_ordered() {
        let idx = season_index_for(2026, 6);
        let (start, end) = season_window(idx);
        assert!(start < end);
        // 30 days in June.
        assert_eq!((end - start) / 86_400, 30);
    }

    #[test]
    fn season_name_is_deterministic_and_stable() {
        let a = season_name(24_311);
        let b = season_name(24_311);
        assert_eq!(a, b);
        assert!(a.contains(' '));
        // Different indices generally differ.
        assert_ne!(season_name(24_311), season_name(24_312));
    }

    #[test]
    fn score_weights_and_is_monotonic() {
        let mut events = vec![ev(AchievementKind::ChangeArchived, day_ts(0), 1, Some("a"))];
        let s1 = season_score(&events, 0);
        // ship(50) + 1 active day(10) = 60.
        assert_eq!(s1, 60);
        events.push(ev(AchievementKind::TaskCompleted, day_ts(0), 3, Some("a")));
        let s2 = season_score(&events, 0);
        // adding 3 tasks (24) cannot lower the score.
        assert!(s2 >= s1);
        assert_eq!(s2, 60 + 24);
        // commits add too.
        assert_eq!(season_score(&events, 5), s2 + 10);
    }

    #[test]
    fn tier_ladder_gap_band_and_overflow() {
        // total 3000 → per_tier 100.
        let t = tier_for(0, 3000);
        assert_eq!(t.tier, 0);
        assert_eq!(t.label, "Unranked");
        assert_eq!(t.gap_to_next, 100);

        let t = tier_for(250, 3000);
        assert_eq!(t.tier, 2); // 2 full tiers crossed
        assert_eq!(t.band, "Bronze");
        assert_eq!(t.label, "Bronze II");
        assert_eq!(t.gap_to_next, 50); // 300 - 250

        // Past the final tier → overflow, band Master.
        let t = tier_for(100 * 31, 3000);
        assert!(t.overflow);
        assert_eq!(t.band, "Master");
    }

    #[test]
    fn pacing_scales_and_clamps() {
        let low = SeasonBaseline {
            ships_per_day: 0.0,
            tasks_per_day: 0.0,
            commits_per_day: 0.0,
        };
        // A near-zero baseline is floored, not trivial.
        assert_eq!(target_total(&low, 30), FLOOR_TOTAL.max(30 * ACTIVE_DAY_POINTS).min(CEIL_TOTAL).max(FLOOR_TOTAL));
        assert!(target_total(&low, 30) >= FLOOR_TOTAL);

        let high = SeasonBaseline {
            ships_per_day: 50.0,
            tasks_per_day: 200.0,
            commits_per_day: 200.0,
        };
        // A huge baseline is capped at the ceiling.
        assert_eq!(target_total(&high, 30), CEIL_TOTAL);

        // A bigger baseline never yields a smaller target (monotone).
        let mid = SeasonBaseline {
            ships_per_day: 1.0,
            tasks_per_day: 4.0,
            commits_per_day: 3.0,
        };
        assert!(target_total(&mid, 30) >= target_total(&low, 30));
    }

    #[test]
    fn objectives_rotate_without_consecutive_repeats() {
        for index in 0..50i64 {
            let a: HashSet<_> = season_archetypes(index).into_iter().collect();
            let b: HashSet<_> = season_archetypes(index + 1).into_iter().collect();
            assert_eq!(a.len(), OBJECTIVES_PER_SEASON, "no dup within a season");
            assert!(
                a.is_disjoint(&b),
                "consecutive seasons share no archetype (idx {index})"
            );
        }
    }

    #[test]
    fn objective_targets_scale_with_baseline_and_progress_derives() {
        let small = SeasonBaseline {
            ships_per_day: 0.1,
            tasks_per_day: 1.0,
            commits_per_day: 0.0,
        };
        let big = SeasonBaseline {
            ships_per_day: 3.0,
            tasks_per_day: 30.0,
            commits_per_day: 0.0,
        };
        assert!(
            objective_target(Archetype::TaskVolume, &big, 30)
                > objective_target(Archetype::TaskVolume, &small, 30)
        );

        // Progress for a couple of archetypes from a known stat set.
        let events = vec![
            ev(AchievementKind::ChangeArchived, day_ts(0), 1, Some("a")),
            ev(AchievementKind::TaskCompleted, day_ts(0), 4, Some("a")),
            ev(AchievementKind::TaskCompleted, day_ts(1), 2, Some("a")),
        ];
        let stats = SeasonStats::from_events(&events);
        assert_eq!(objective_progress(Archetype::ShipVolume, &stats, 0), 1);
        assert_eq!(objective_progress(Archetype::TaskVolume, &stats, 0), 6);
        assert_eq!(objective_progress(Archetype::ActiveDays, &stats, 0), 2);
        assert_eq!(objective_progress(Archetype::DailyBurst, &stats, 0), 4);
    }

    #[test]
    fn finish_and_comeback_and_streak_derive() {
        let events = vec![
            ev(AchievementKind::ChangeCreated, day_ts(0), 1, Some("a")),
            ev(AchievementKind::ChangeArchived, day_ts(1), 1, Some("a")), // full lifecycle
            ev(AchievementKind::ChangeCreated, day_ts(2), 1, Some("b")), // created only
            // gap, then return on day 10 → comeback
            ev(AchievementKind::TaskCompleted, day_ts(10), 1, Some("b")),
        ];
        let stats = SeasonStats::from_events(&events);
        assert_eq!(stats.finishes(), 1, "only 'a' is created+archived");
        assert!(stats.came_back_after(3), "10 - 2 > 3");
        // active days {0,1,2,10}: longest run is 0,1,2 = 3.
        assert_eq!(stats.longest_streak(), 3);
    }

    #[test]
    fn treatment_is_deterministic_with_versioned_id_and_rising_rarity() {
        let a = treatment(24_311, 5);
        let b = treatment(24_311, 5);
        assert_eq!(a, b);
        assert!(a.id.ends_with(&format!("-g{GENERATOR_VERSION}")));
        assert_eq!(a.id, "s24311-t5-g1");
        // Rarity does not fall as tier rises; the top tier beats the bottom.
        assert!(rarity_for_tier(TIER_COUNT).rank() > rarity_for_tier(1).rank());
        assert!(rarity_for_tier(TIER_COUNT + 1).rank() >= rarity_for_tier(TIER_COUNT).rank());
        assert_eq!(rarity_for_tier(TIER_COUNT + 1), Rarity::Legendary);
    }

    #[test]
    fn career_tier_is_monotonic_in_ships() {
        let t = |ships| {
            career_tier(&AchievementTotals {
                changes_archived: ships,
                ..Default::default()
            })
            .tier
        };
        assert_eq!(t(0), 0); // Newcomer
        assert_eq!(t(49), 0); // still below the first threshold
        assert_eq!(t(50), 1); // Initiate
        assert_eq!(t(5000), 6); // Architect (top)
        assert!(t(3000) >= t(1500));
        // Never decreases as ships rise.
        let mut prev = 0;
        for s in (0..6000u32).step_by(50) {
            let cur = t(s);
            assert!(cur >= prev);
            prev = cur;
        }
    }

    #[test]
    fn vault_returns_past_indices_only() {
        let v = vault(100);
        assert!(v.iter().all(|&i| i >= 0 && i < 100));
        // Early seasons have a small or empty vault, never negative.
        assert!(vault(0).is_empty());
    }

    #[test]
    fn compute_season_adds_objective_bonus_and_unlocks_tiers() {
        let baseline = SeasonBaseline {
            ships_per_day: 0.2,
            tasks_per_day: 2.0,
            commits_per_day: 1.0,
        };
        let totals = AchievementTotals {
            changes_archived: 20,
            ..Default::default()
        };
        let idx = season_index_for(2026, 6);
        let (start, _end) = season_window(idx);
        let events = vec![
            ev(AchievementKind::ChangeArchived, start + 86_400, 1, Some("a")),
            ev(AchievementKind::TaskCompleted, start + 86_400, 5, Some("a")),
        ];
        let standing = compute_season(idx, &events, 3, &baseline, &totals);
        assert_eq!(standing.season.index, idx);
        assert_eq!(standing.objectives.len(), OBJECTIVES_PER_SEASON);
        assert_eq!(standing.career.ships, 20);
        // next_treatment is for tier+1 and is deterministic.
        assert_eq!(
            standing.next_treatment.unwrap().tier_index,
            standing.ladder.tier + 1
        );
        // Unlocked treatments cover 1..=tier.
        let unlocked = unlocked_treatments(idx, &standing.ladder);
        assert_eq!(unlocked.len() as u32, standing.ladder.tier);
    }

    #[test]
    fn recap_summarises_a_finished_season() {
        let baseline = SeasonBaseline {
            ships_per_day: 0.3,
            tasks_per_day: 3.0,
            commits_per_day: 1.0,
        };
        let idx = season_index_for(2026, 5);
        let (start, _end) = season_window(idx);
        let events = vec![
            ev(AchievementKind::ChangeArchived, start + 86_400, 2, Some("a")),
            ev(AchievementKind::TaskCompleted, start + 86_400, 4, Some("a")),
        ];
        let recap = season_recap(idx, &events, 7, &baseline);
        assert_eq!(recap.season.index, idx);
        assert_eq!(recap.shipped, 2);
        assert_eq!(recap.tasks_completed, 4);
        assert_eq!(recap.commits, 7);
    }
}
