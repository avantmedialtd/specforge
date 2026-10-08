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

pub mod bitbucket;
pub(crate) mod bitbucket_detail;
pub mod chatgpt_quota;
pub mod config;
pub mod events;
pub mod github;
pub(crate) mod github_detail;
pub mod pull_request_cache;
pub mod pull_request_detail;
pub mod pull_request_limits;
pub mod pull_request_links;
pub(crate) mod pull_request_read;
pub mod pull_requests;
pub mod quota;
pub mod review_progress;
pub mod service;
pub mod settings;
pub(crate) mod usage_http;
pub mod window_title;

pub use bitbucket::{BitbucketLimits, BitbucketPullRequestsHandle, BitbucketPullRequestsState};
pub use chatgpt_quota::{ChatGptQuotaHandle, ChatGptQuotaState, ChatGptQuotaWindow};
pub use config::{config_dir, APP_IDENTIFIER};
pub use events::{
    document_envelope, event_envelope, notice_envelope, PanelMovedPayload, PullRequestProvider,
    PullRequestProviderChangedPayload, ServiceNotice, EVENT_DOCUMENT_CHANGED,
    EVENT_PULL_REQUEST_PROVIDER_CHANGED, EVENT_REVIEW_PROGRESS_CHANGED,
};
pub use github::{GithubLimits, GithubPullRequestsHandle, GithubPullRequestsState};
pub use pull_request_detail::{
    ConversationEntry, DiffSide, FileReadFailure, ImageSide, ImageVersions, PullRequestCheck,
    PullRequestCheckState, PullRequestComment, PullRequestDetail, PullRequestDetailOutcome,
    PullRequestFileOutcome, PullRequestImageOutcome, PullRequestKey, PullRequestReference,
    ReviewState, ReviewThread,
};
pub use pull_request_links::{
    LinkedPullRequest, LinkedWorktree, PullRequestLinks, PullRequestRole, PullRequestWorktrees,
    WorktreePullRequests,
};
pub use pull_requests::{ChecksState, PullRequestSummary, PullRequestsStatus, ReviewSummary};
pub use quota::{ClaudeQuotaState, QuotaHandle, QuotaStatus, QuotaWindow, ScopedQuotaWindow};
pub use review_progress::{FileReviewProgress, FileReviewState, ReviewProgress};
pub use service::{
    row_key_for_workspace, AppService, ArtifactRead, IdentityInfo, LinkResolution,
    DASHBOARD_HEATMAP_WINDOW_DAYS,
};
pub use settings::{
    AppSettings, BitbucketConfig, BitbucketConfigView, DocumentWidth, GithubConfig,
    GithubConfigView, PanelPosition, PullRequestWindowGeometry, ReaderWindowGeometry,
    SettingsStore, TailscaleConfig, WebServerConfig, PULL_REQUEST_WINDOW_MIN_HEIGHT,
    PULL_REQUEST_WINDOW_MIN_WIDTH,
};
pub use window_title::{sanitize_window_title, WINDOW_TITLE_CAP};
