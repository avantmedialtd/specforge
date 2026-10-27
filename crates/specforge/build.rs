//! Build-time Tauri setup (`pull-request-actions`: *Application Commands Are
//! Granted Per Window Kind*; design D1).
//!
//! Tauri 2 constrains plugin invocations through capabilities but not
//! `#[tauri::command]` handlers. Declaring every handler in the application
//! manifest autogenerates an `allow-<command>` / `deny-<command>` permission
//! pair for each, so the capability files can grant application commands per
//! window kind instead of leaving every handler reachable from every window.
//!
//! `APP_COMMANDS` must name exactly the handlers registered by
//! `tauri::generate_handler!` in `src/lib.rs`: the source-level test in
//! `src/pull_request_window.rs` fails if the two lists drift or either repeats
//! an entry.

/// Every application command, in the registration order of `src/lib.rs`.
///
/// The first 83 predate `pull-request-actions`; the eight review-action
/// handlers follow the pull-request reads and precede the detached-window
/// operations. Names are the handler functions' snake-case identifiers, which
/// is what Tauri's generated permissions slugify.
const APP_COMMANDS: &[&str] = &[
    "register_workspace",
    "unregister_workspace",
    "list_workspaces",
    "get_changes",
    "archived_artifact_status",
    "list_archived_rows",
    "get_workspace_views",
    "get_active_count",
    "get_dashboard",
    "get_commit_garden",
    "read_artifact",
    "list_markdown_files",
    "list_workspace_file_rows",
    "read_workspace_file",
    "watch_document",
    "unwatch_document",
    "open_reader_window",
    "set_reader_window_size",
    "open_artifact_link",
    "get_commit_graph",
    "get_commit_detail",
    "get_commit_diff",
    "get_commit_file_image",
    "get_launch_on_login",
    "set_launch_on_login",
    "get_notifications_enabled",
    "set_notifications_enabled",
    "get_document_width",
    "set_document_width",
    "get_commit_history_enabled",
    "set_commit_history_enabled",
    "get_collapsed_tree_node_ids",
    "set_collapsed_tree_node_ids",
    "get_expanded_tree_node_ids",
    "set_expanded_tree_node_ids",
    "get_favorite_change_ids",
    "update_favorite_change_ids",
    "set_workspace_presentation",
    "set_workspace_disabled",
    "get_identity",
    "get_identity_config",
    "set_display_name",
    "set_identity_aliases",
    "get_claude_quota",
    "get_claude_quota_enabled",
    "set_claude_quota_enabled",
    "get_chatgpt_quota",
    "get_chatgpt_quota_enabled",
    "set_chatgpt_quota_enabled",
    "get_bitbucket_config",
    "set_bitbucket_enabled",
    "set_bitbucket_credentials",
    "set_bitbucket_panel_position",
    "get_bitbucket_pull_requests",
    "get_github_config",
    "set_github_enabled",
    "set_github_token",
    "set_github_panel_position",
    "get_github_pull_requests",
    "get_pull_request_links",
    "open_pull_request",
    "get_pull_request_detail",
    "get_pull_request_file",
    "get_pull_request_file_image",
    "get_review_progress",
    "set_file_viewed",
    "set_hunk_viewed",
    "set_file_included",
    "get_review_skip_patterns",
    "set_review_skip_patterns",
    "open_pull_request_link",
    "get_review_action_state",
    "create_review_draft",
    "edit_review_draft",
    "recover_review_draft",
    "start_review_draft_over",
    "submit_review",
    "resume_review_operation",
    "abandon_review_operation",
    "open_pull_request_window",
    "open_image_window",
    "set_pull_request_window_size",
    "get_wsl_poll_interval_secs",
    "set_wsl_poll_interval_secs",
    "get_web_config",
    "set_web_enabled",
    "set_web_port",
    "set_web_tailscale_enabled",
    "set_web_tailscale_name",
    "set_web_tailscale_allowed_logins",
    "resolve_tailscale_name",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(APP_COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
