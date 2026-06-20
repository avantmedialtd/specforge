//! Headless application service for SpecForge.
//!
//! Sits between the pure-primitive core (`openspec-core`) and the user-facing
//! frontends (the Tauri shell `specforge`, and the terminal frontend
//! `specforge-tui`). Owns the stateful "brain" that neither frontend should
//! duplicate: the file-backed settings store, the dashboard assembly,
//! first-launch backfill/seeding, the watcher lifecycle, and resolution of the
//! shared configuration directory.
//!
//! Nothing here depends on Tauri or on a terminal, so the orchestration stays
//! testable from `cargo test` and identical across both frontends.

pub mod config;
pub mod events;
pub mod quota;
pub mod service;
pub mod settings;

pub use config::{config_dir, APP_IDENTIFIER};
pub use events::event_envelope;
pub use quota::{ClaudeQuotaState, QuotaHandle, QuotaStatus, QuotaWindow};
pub use service::{
    AppService, IdentityInfo, TreatmentLocker, DASHBOARD_ACTIVITY_WINDOW_DAYS,
    DASHBOARD_HEATMAP_WINDOW_DAYS,
};
pub use settings::{AppSettings, SeasonState, SettingsStore, WebServerConfig};
