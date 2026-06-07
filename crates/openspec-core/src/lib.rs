//! Headless core for the SpecForge application.
//!
//! Owns the registered-workspace list and its persistence, filesystem
//! watching, OpenSpec parsing, the in-memory cache, and the data shapes
//! shared with the Tauri shell. UI concerns live in `specforge`.

pub mod activity_log;
pub mod cache;
pub mod dashboard;
pub mod git;
pub mod graph;
pub mod identity;
pub mod parser;
pub mod presentation;
pub mod registry;
pub mod repo_monitor;
pub mod repo_view;
pub mod seasons;
pub mod self_write;
pub mod types;
pub mod watcher;

pub use activity_log::{
    build_backfill, day_axis, diff_achievements, event_is_me, today_str, Achievement,
    AchievementKind, AchievementTotals, ActivityLog,
};
pub use cache::WorkspaceCache;
pub use dashboard::{
    compute_dashboard, compute_leaderboard, compute_progress, ActivityBucket, DashboardData,
    HeatmapCell, LeaderboardEntry, LifecycleMetrics, ProgressData, RecentEntry, RepoBreakdown,
    StreakInfo, SummaryMetrics, TodayProgress,
};
pub use git::{
    change_lifecycle, commit_activity, commit_activity_with_authors, commit_diff, commit_files,
    commit_log, current_branch, default_branch, git_common_dir, git_identity,
    task_completion_history, worktree_list, ChangeLifecycle, CommitFile, CommitRef, RawCommit,
    RefKind, RepoId, WorktreeInfo,
};
pub use graph::{layout as layout_commit_graph, CommitGraph, EdgeSegment, LaidOutCommit};
pub use identity::{detect_candidate_identities, is_me, normalized_key, Author, IdentityConfig};
pub use parser::{
    archive_dir_date, archive_dir_logical_id, list_active_changes, list_archived_changes,
    list_archived_stubs, list_archived_summaries, parse_all_archived, parse_all_changes,
    parse_artifact_status, parse_change, parse_proposal_title, parse_tasks_md, ParsedTasks,
};
pub use presentation::{
    PresentationEntry, PresentationError, PresentationKey, WorkspacePresentationStore,
};
pub use registry::{RegistrationError, RegistryEntry, WorkspaceOrigin, WorkspaceRegistry};
pub use repo_view::{
    aggregate, compute_views, diff_views, ChangeInstance, DivergenceLabel, LogicalChange,
    RepoSnapshot, RepoView, WorkspaceView, WorktreeSnapshot,
};
pub use seasons::{
    career_tier, compute_season, current_season_index, in_season, season_index_for, season_info,
    season_name, season_objectives, season_recap, season_window, treatment, treatment_from_id,
    unlocked_treatments, vault, Archetype, BandTier, CareerTier, Rarity, SeasonBaseline,
    SeasonInfo, SeasonObjective, SeasonRecap, SeasonStanding, SeasonStats, TreatmentDescriptor,
};
pub use self_write::SelfWriteTracker;
pub use types::{
    ArchivedChangeSummary, ArtifactStatus, ChangeData, PaletteColor, RegisteredWorkspace, Section,
    Task, WorkspaceFolder,
};
pub use watcher::{CacheEvent, WatcherError, WatcherManager};
