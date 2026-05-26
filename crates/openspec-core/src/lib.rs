//! Headless core for the SpecForge application.
//!
//! Owns the registered-workspace list and its persistence, filesystem
//! watching, OpenSpec parsing, the in-memory cache, and the data shapes
//! shared with the Tauri shell. UI concerns live in `specforge`.

pub mod cache;
pub mod git;
pub mod parser;
pub mod presentation;
pub mod registry;
pub mod repo_monitor;
pub mod repo_view;
pub mod self_write;
pub mod types;
pub mod watcher;

pub use cache::WorkspaceCache;
pub use git::{
    current_branch, default_branch, git_common_dir, worktree_list, RepoId, WorktreeInfo,
};
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
