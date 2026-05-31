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
pub mod parser;
pub mod presentation;
pub mod registry;
pub mod repo_monitor;
pub mod repo_view;
pub mod self_write;
pub mod types;
pub mod watcher;

pub use activity_log::{
    build_backfill, day_axis, diff_achievements, today_str, Achievement, AchievementKind,
    AchievementTotals, ActivityLog,
};
pub use cache::WorkspaceCache;
pub use dashboard::{
    compute_dashboard, compute_progress, ActivityBucket, DashboardData, HeatmapCell,
    LifecycleMetrics, Milestone, ProgressData, RecentEntry, RepoBreakdown, StreakInfo,
    SummaryMetrics, TodayProgress,
};
pub use git::{
    change_lifecycle, commit_activity, commit_diff, commit_files, commit_log, current_branch,
    default_branch, git_common_dir, task_completion_history, worktree_list, ChangeLifecycle,
    CommitFile, CommitRef, RawCommit, RefKind, RepoId, WorktreeInfo,
};
pub use graph::{layout as layout_commit_graph, CommitGraph, EdgeSegment, LaidOutCommit};
pub use parser::{
    list_active_changes, list_archived_changes, parse_all_archived, parse_all_changes,
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
pub use self_write::SelfWriteTracker;
pub use types::{
    ArtifactStatus, ChangeData, PaletteColor, RegisteredWorkspace, Section, Task, WorkspaceFolder,
};
pub use watcher::{CacheEvent, WatcherError, WatcherManager};
