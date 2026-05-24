//! Headless core for the OpenSpec Tray application.
//!
//! Owns the registered-workspace list and its persistence, filesystem
//! watching, OpenSpec parsing, the in-memory cache, and the data shapes
//! shared with the Tauri shell. UI concerns live in `openspec-tray`.

pub mod cache;
pub mod parser;
pub mod registry;
pub mod self_write;
pub mod types;
pub mod watcher;

pub use cache::WorkspaceCache;
pub use parser::{
    list_active_changes, parse_all_changes, parse_artifact_status, parse_change,
    parse_proposal_title, parse_tasks_md, ParsedTasks,
};
pub use registry::{RegistrationError, WorkspaceRegistry};
pub use self_write::SelfWriteTracker;
pub use types::{
    ArtifactStatus, ChangeData, RegisteredWorkspace, Section, Task, WorkspaceFolder,
};
pub use watcher::{CacheEvent, WatcherError, WatcherManager};
