//! The `/api/invoke` command table.
//!
//! One arm per command, mirroring the Tauri `#[command]` handlers in
//! `specforge/commands.rs` — but both sides are thin: they deserialize args and
//! call the shared `AppService` / `SettingsStore` / `WatcherManager` surface,
//! where the real logic lives. New commands need a new arm here and a new arm in
//! the Tauri crate, never new routes.
//!
//! Argument keys are camelCase, matching what the frontend sends (Tauri maps
//! snake_case Rust params to camelCase on the JS side; the web transport sends
//! the same shape, so one `api.ts` serves both hosts).

use std::path::PathBuf;
use std::time::Duration;

use openspec_app::events::{
    PanelMovedPayload, PullRequestProvider, EVENT_COMMIT_HISTORY_ENABLED_CHANGED,
    EVENT_DOCUMENT_WIDTH_CHANGED, EVENT_PULL_REQUEST_PANEL_MOVED,
    EVENT_WORKSPACE_PRESENTATION_UPDATED,
};
use openspec_app::{AppService, DocumentWidth, PanelPosition, PullRequestReference};
use openspec_core::{ArchiveScope, Author, FileScope, PaletteColor};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::broadcast;

/// Dispatch one command by name. Returns the JSON-serialised result, or an error
/// message the HTTP layer turns into a `{ "error": ... }` envelope. Unknown
/// commands are rejected (never silently ignored).
pub async fn dispatch(
    svc: &AppService,
    extra_tx: &broadcast::Sender<(String, Value)>,
    command: &str,
    args: Value,
) -> Result<Value, String> {
    let value = match command {
        // ---- Workspaces -------------------------------------------------
        "register_workspace" => {
            let a: PathArg = parse(args)?;
            to_val(svc.add_workspace(PathBuf::from(a.path)).await?)?
        }
        "unregister_workspace" => {
            let a: PathArg = parse(args)?;
            to_val(svc.remove_workspace(PathBuf::from(a.path)).await?)?
        }
        "list_workspaces" => to_val(svc.list_workspaces()?)?,
        "get_changes" => {
            let a: WorkspaceArg = parse(args)?;
            to_val(svc.changes_for(&PathBuf::from(a.workspace)))?
        }
        "get_workspace_views" => to_val(svc.workspace_views())?,
        "get_active_count" => to_val(svc.active_count())?,
        "set_workspace_presentation" => {
            let a: PresentationArg = parse(args)?;
            svc.set_workspace_presentation(
                PathBuf::from(a.uri),
                a.repo_id.map(PathBuf::from),
                a.display_name,
                a.color,
            )?;
            // Not a CacheEvent — emit on the app-event channel so the SSE stream
            // delivers `workspace-presentation-updated` to refetch the tree.
            let _ = extra_tx.send((
                EVENT_WORKSPACE_PRESENTATION_UPDATED.to_string(),
                Value::Null,
            ));
            Value::Null
        }
        "set_workspace_disabled" => {
            let a: DisabledArg = parse(args)?;
            svc.set_workspace_disabled(
                PathBuf::from(a.uri),
                a.repo_id.map(PathBuf::from),
                a.disabled,
            )
            .await?;
            let _ = extra_tx.send((
                EVENT_WORKSPACE_PRESENTATION_UPDATED.to_string(),
                Value::Null,
            ));
            Value::Null
        }

        // ---- Archive ----------------------------------------------------
        "archived_artifact_status" => {
            let a: ArchivedArg = parse(args)?;
            to_val(svc.archived_artifact_status(&PathBuf::from(a.workspace), &a.dir_name)?)?
        }
        "list_archived_rows" => {
            let a: ArchiveScopeArg = parse(args)?;
            to_val(svc.list_archived_rows(a.scope).await?)?
        }

        // ---- Artifacts --------------------------------------------------
        "read_artifact" => {
            let a: ReadArtifactArg = parse(args)?;
            to_val(
                svc.read_artifact(
                    &PathBuf::from(a.workspace),
                    &a.change_id,
                    &a.artifact_kind,
                    a.capability.as_deref(),
                )
                .await?,
            )?
        }
        "list_markdown_files" => {
            let a: RootArg = parse(args)?;
            to_val(svc.list_markdown_files(PathBuf::from(a.root)).await?)?
        }
        "list_workspace_file_rows" => {
            let a: FileScopeArg = parse(args)?;
            to_val(svc.list_workspace_file_rows(a.scope).await?)?
        }
        "read_workspace_file" => {
            let a: ReadWorkspaceFileArg = parse(args)?;
            to_val(
                svc.read_workspace_file(PathBuf::from(a.root), a.rel_path)
                    .await?,
            )?
        }
        // The browser has no window label to own a registration, so the
        // frontend mints a per-page client id and sends it here; the SSE
        // stream carries the same id and releases everything it owns when the
        // connection drops (see `sse.rs`). That is what makes a closed — or
        // killed — tab unable to strand a watch.
        "watch_document" => {
            let a: DocumentWatchArg = parse(args)?;
            to_val(
                svc.watch_document(&a.client_id, PathBuf::from(a.root), a.rel_path)
                    .await?,
            )?
        }
        "unwatch_document" => {
            let a: DocumentWatchArg = parse(args)?;
            svc.unwatch_document(&a.client_id, PathBuf::from(a.root), a.rel_path)
                .await;
            Value::Null
        }

        // ---- Desktop-only: opening artifact links -----------------------
        // Deliberately not mirrored (see the `web-ui` capability's *Link
        // Handling in the Browser Skin* requirement): the open operation acts
        // on the *serving host's* filesystem/OS, and a browser request must
        // never be able to make the server machine launch an application or
        // open a file. `MarkdownView` never invokes this command on the web
        // transport (`isWeb()` branches to a non-navigating affordance
        // instead), so reaching this arm at all means either a stale/crafted
        // client request — reject it the same clear way `launch_on_login`
        // does, rather than silently no-op or fall through as a generic
        // "unknown command".
        "open_artifact_link" => {
            return Err(
                "opening links is a desktop-only capability and is not available in the web UI"
                    .to_string(),
            )
        }
        // `open_pull_request` goes further and has NO arm at all, so it falls
        // through to `unknown command` below (`bitbucket-pull-requests`:
        // *Opening a Pull Request*, design D8): the transport simply has no
        // operation that opens a URL on the serving host. In the browser skin a
        // row links to its pull request's `/pr/...` address instead, and the
        // provider's page is the view header's opener-isolated link, an
        // `<a target="_blank" rel="noopener noreferrer">`.
        // `open_pull_request_is_an_unknown_command_on_the_web_transport` pins it.
        //
        // The pull-request viewer's three desktop-only commands have none
        // either, each pinned the same way by a test named for it
        // (`pull-request-viewer`: *Pull-Request Window*, *Desktop Link Opener*,
        // *Pull-Request Window Geometry*):
        // - `open_pull_request_link` would open a link on the serving host; the
        //   browser skin opens pull-request links as opener-isolated tabs;
        // - `open_pull_request_window` would open a window on the serving host;
        //   the browser skin's pull-request window is a tab it opens itself;
        // - `set_pull_request_window_size` sizes a native window, which this
        //   transport never opens.

        // ---- Dashboard / garden -----------------------------------------
        "get_dashboard" => to_val(svc.dashboard().await?)?,
        "get_commit_garden" => to_val(svc.commit_garden().await?)?,

        // ---- Commit graph -----------------------------------------------
        "get_commit_graph" => {
            let a: CommitGraphArg = parse(args)?;
            to_val(svc.commit_graph(PathBuf::from(a.repo_id), a.limit).await?)?
        }
        "get_commit_detail" => {
            let a: CommitDetailArg = parse(args)?;
            to_val(svc.commit_detail(PathBuf::from(a.repo_id), a.sha).await?)?
        }
        "get_commit_diff" => {
            let a: CommitDiffArg = parse(args)?;
            to_val(
                svc.commit_diff(PathBuf::from(a.repo_id), a.sha, a.path, a.old_path)
                    .await?,
            )?
        }
        // An image file's versions take the arguments a file's load does.
        "get_commit_file_image" => {
            let a: CommitDiffArg = parse(args)?;
            to_val(
                svc.commit_file_image(PathBuf::from(a.repo_id), a.sha, a.path, a.old_path)
                    .await?,
            )?
        }

        // ---- Identity ---------------------------------------------------
        "get_identity" => to_val(svc.identity_info()?)?,
        "set_display_name" => {
            let a: NameArg = parse(args)?;
            svc.settings
                .set_display_name(a.name)
                .map_err(|e| e.to_string())?;
            Value::Null
        }
        "set_identity_aliases" => {
            let a: AliasesArg = parse(args)?;
            svc.settings
                .set_identity_aliases(a.aliases)
                .map_err(|e| e.to_string())?;
            Value::Null
        }
        // ---- Settings: quota / notifications ----------------------------
        "get_claude_quota" => to_val(svc.claude_quota())?,
        "get_claude_quota_enabled" => to_val(svc.settings.claude_quota_enabled())?,
        "set_claude_quota_enabled" => {
            let a: EnabledArg = parse(args)?;
            svc.settings
                .set_claude_quota_enabled(a.enabled)
                .map_err(|e| e.to_string())?;
            Value::Null
        }
        "get_chatgpt_quota" => to_val(svc.chatgpt_quota())?,
        "get_chatgpt_quota_enabled" => to_val(svc.settings.chatgpt_quota_enabled())?,
        "set_chatgpt_quota_enabled" => {
            let a: EnabledArg = parse(args)?;
            svc.settings
                .set_chatgpt_quota_enabled(a.enabled)
                .map_err(|e| e.to_string())?;
            Value::Null
        }
        // ---- Settings: BitBucket pull-request panel ----------------------
        // The getter serves `BitbucketConfigView`, which never carries the
        // token: on a Tailscale or non-loopback bind this is reachable by
        // anyone who can reach the page.
        "get_bitbucket_config" => to_val(svc.settings.bitbucket_config_view())?,
        // The flag and the credential go through the service, the one path
        // every transport's write takes (`pull-request-viewer`: *Provider
        // Enabled Flags Stay Current*): disabling or saving drops the
        // provider's cached details, enabling inside a deadline publishes
        // `unavailable` at once, and a flag write raises
        // `pull-request-provider-changed` on the service's broadcast, which
        // reaches every tab and every desktop window. Nothing is sent on
        // `extra_tx`, which would reach this transport's tabs alone.
        "set_bitbucket_enabled" => {
            let a: EnabledArg = parse(args)?;
            svc.set_bitbucket_enabled(a.enabled)
                .map_err(|e| e.to_string())?;
            Value::Null
        }
        "set_bitbucket_credentials" => {
            let a: BitbucketCredentialsArg = parse(args)?;
            svc.set_bitbucket_credentials(a.username, a.api_token)
                .map_err(|e| e.to_string())?;
            Value::Null
        }
        "set_bitbucket_panel_position" => {
            let a: PanelPositionArg = parse(args)?;
            svc.settings
                .set_bitbucket_panel_position(a.position)
                .map_err(|e| e.to_string())?;
            // Not a CacheEvent — emit on the app-event channel so the SSE
            // stream delivers `pull-request-panel-moved` to every connected
            // surface, as `set_document_width` does for the reading width.
            let payload = PanelMovedPayload {
                provider: PullRequestProvider::Bitbucket,
                position: a.position,
            };
            let _ = extra_tx.send((
                EVENT_PULL_REQUEST_PANEL_MOVED.to_string(),
                serde_json::to_value(payload).map_err(|e| e.to_string())?,
            ));
            Value::Null
        }
        // Renamed from the provider-less `get_my_pull_requests`, which now
        // answers `unknown command` (`bitbucket-pull-requests`: *The
        // Snapshot Is Announced on the Cache Stream*).
        "get_bitbucket_pull_requests" => to_val(svc.bitbucket_pull_requests())?,

        // ---- Settings: GitHub pull-request panel -------------------------
        // The getter serves `GithubConfigView`, which never carries the
        // token, for the same reason as the BitBucket getter above.
        "get_github_config" => to_val(svc.settings.github_config_view())?,
        // Through the service, as BitBucket's twins are above.
        "set_github_enabled" => {
            let a: EnabledArg = parse(args)?;
            svc.set_github_enabled(a.enabled)
                .map_err(|e| e.to_string())?;
            Value::Null
        }
        "set_github_token" => {
            let a: GithubTokenArg = parse(args)?;
            svc.set_github_token(a.token).map_err(|e| e.to_string())?;
            Value::Null
        }
        "set_github_panel_position" => {
            let a: PanelPositionArg = parse(args)?;
            svc.settings
                .set_github_panel_position(a.position)
                .map_err(|e| e.to_string())?;
            let payload = PanelMovedPayload {
                provider: PullRequestProvider::Github,
                position: a.position,
            };
            let _ = extra_tx.send((
                EVENT_PULL_REQUEST_PANEL_MOVED.to_string(),
                serde_json::to_value(payload).map_err(|e| e.to_string())?,
            ));
            Value::Null
        }
        "get_github_pull_requests" => to_val(svc.github_pull_requests())?,
        // A pure read of in-memory state (plus, at most, one local
        // `git remote -v` per warm repository): no host-side effect, so
        // unlike `open_pull_request` it is served here too.
        "get_pull_request_links" => to_val(svc.pull_request_links().await)?,

        // ---- Pull-request viewer -----------------------------------------
        // Served here too. A read answers only for a pull request the host's
        // account already lists, through its row, and only while its provider
        // is enabled (`pull-request-viewer`: *Detail Reads Are Scoped to the
        // Snapshot*); review progress is the reader's local state, never a
        // write to either host (*Review Progress*). Each stored mark raises
        // `review-progress-changed` on the service's broadcast, never here.
        "get_pull_request_detail" => {
            let a: PullRequestDetailArg = parse(args)?;
            to_val(
                svc.pull_request_detail(a.reference, a.manual, a.cached_only)
                    .await,
            )?
        }
        "get_pull_request_file" => {
            let a: PullRequestFileArg = parse(args)?;
            to_val(
                svc.pull_request_file(&a.reference, &a.path, &a.head, &a.base)
                    .await,
            )?
        }
        "get_pull_request_file_image" => {
            let a: PullRequestFileArg = parse(args)?;
            to_val(
                svc.pull_request_file_image(&a.reference, &a.path, &a.head, &a.base)
                    .await,
            )?
        }
        // Both read `review-progress.json` afresh, and a mark syncs its write,
        // so they run on the blocking pool rather than on the runtime.
        "get_review_progress" => {
            let a: ReferenceArg = parse(args)?;
            let svc = svc.clone();
            to_val(
                tokio::task::spawn_blocking(move || svc.review_progress(&a.reference))
                    .await
                    .map_err(|e| e.to_string())??,
            )?
        }
        "set_file_viewed" => {
            let a: FileViewedArg = parse(args)?;
            let svc = svc.clone();
            tokio::task::spawn_blocking(move || {
                svc.set_file_viewed(&a.reference, &a.path, a.viewed, &a.head, &a.base)
            })
            .await
            .map_err(|e| e.to_string())??;
            Value::Null
        }
        "set_hunk_viewed" => {
            let a: HunkViewedArg = parse(args)?;
            let svc = svc.clone();
            tokio::task::spawn_blocking(move || {
                svc.set_hunk_viewed(&a.reference, &a.path, a.hunk, a.viewed, &a.head, &a.base)
            })
            .await
            .map_err(|e| e.to_string())??;
            Value::Null
        }

        // ---- Settings: reading width -------------------------------------
        "get_document_width" => to_val(svc.settings.document_width())?,
        "set_document_width" => {
            let a: DocumentWidthArg = parse(args)?;
            svc.settings
                .set_document_width(a.width)
                .map_err(|e| e.to_string())?;
            // Not a CacheEvent — emit on the app-event channel so the SSE
            // stream delivers `document-width-changed` to every connected
            // surface, including a reader tab already open at the old width.
            let _ = extra_tx.send((
                EVENT_DOCUMENT_WIDTH_CHANGED.to_string(),
                serde_json::to_value(a.width).map_err(|e| e.to_string())?,
            ));
            Value::Null
        }

        // ---- Settings: commit history ------------------------------------
        "get_commit_history_enabled" => to_val(svc.settings.commit_history_enabled())?,
        "set_commit_history_enabled" => {
            let a: EnabledArg = parse(args)?;
            svc.settings
                .set_commit_history_enabled(a.enabled)
                .map_err(|e| e.to_string())?;
            // Not a CacheEvent — emit on the app-event channel so the SSE
            // stream tells every connected surface to add or drop its graph.
            let _ = extra_tx.send((
                EVENT_COMMIT_HISTORY_ENABLED_CHANGED.to_string(),
                Value::Bool(a.enabled),
            ));
            Value::Null
        }

        "get_notifications_enabled" => to_val(svc.settings.snapshot().notifications_enabled)?,
        "set_notifications_enabled" => {
            let a: EnabledArg = parse(args)?;
            svc.settings
                .set_notifications_enabled(a.enabled)
                .map_err(|e| e.to_string())?;
            Value::Null
        }

        // ---- Settings: tree node state ----------------------------------
        "get_collapsed_tree_node_ids" => to_val(svc.settings.snapshot().collapsed_tree_node_ids)?,
        "set_collapsed_tree_node_ids" => {
            let a: IdsArg = parse(args)?;
            svc.settings
                .set_collapsed_tree_node_ids(a.ids)
                .map_err(|e| e.to_string())?;
            Value::Null
        }
        "get_expanded_tree_node_ids" => to_val(svc.settings.snapshot().expanded_tree_node_ids)?,
        "set_expanded_tree_node_ids" => {
            let a: IdsArg = parse(args)?;
            svc.settings
                .set_expanded_tree_node_ids(a.ids)
                .map_err(|e| e.to_string())?;
            Value::Null
        }
        "get_favorite_change_ids" => to_val(svc.settings.snapshot().favorite_change_ids)?,
        "update_favorite_change_ids" => {
            let a: FavoriteDeltaArg = parse(args)?;
            to_val(
                svc.settings
                    .update_favorite_change_ids(a.add, a.remove)
                    .map_err(|e| e.to_string())?,
            )?
        }

        // ---- Settings: WSL poll (Windows-only; null elsewhere) ----------
        "get_wsl_poll_interval_secs" => {
            #[cfg(target_os = "windows")]
            let v: Option<u64> = Some(svc.settings.wsl_poll_interval_secs());
            #[cfg(not(target_os = "windows"))]
            let v: Option<u64> = None;
            to_val(v)?
        }
        "set_wsl_poll_interval_secs" => {
            let a: SecsArg = parse(args)?;
            svc.settings
                .set_wsl_poll_interval_secs(a.secs)
                .map_err(|e| e.to_string())?;
            svc.watcher.set_poll_interval(Duration::from_secs(a.secs));
            Value::Null
        }

        // ---- Desktop-only: launch-on-login ------------------------------
        // Managed by the OS via the autostart plugin, which the headless server
        // has no access to. The web Settings view hides the control; if called
        // anyway, fail clearly rather than lie about success.
        "get_launch_on_login" | "set_launch_on_login" => {
            return Err(
                "launch-on-login is managed by the desktop app and is not available in the web UI"
                    .to_string(),
            )
        }

        other => return Err(format!("unknown command: {other}")),
    };
    Ok(value)
}

/// Serialize a command result to JSON.
fn to_val<T: serde::Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| format!("failed to serialize result: {e}"))
}

/// Deserialize a command's arguments object into its typed shape.
fn parse<T: serde::de::DeserializeOwned>(args: Value) -> Result<T, String> {
    serde_json::from_value(args).map_err(|e| format!("invalid arguments: {e}"))
}

// ---------------------------------------------------------------------------
// Argument shapes. camelCase to match the frontend's `invoke(cmd, args)` keys.
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathArg {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceArg {
    workspace: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchivedArg {
    workspace: String,
    dir_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveScopeArg {
    scope: ArchiveScope,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileScopeArg {
    scope: FileScope,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadArtifactArg {
    workspace: String,
    change_id: String,
    artifact_kind: String,
    #[serde(default)]
    capability: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RootArg {
    root: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadWorkspaceFileArg {
    root: String,
    rel_path: String,
}

/// Arguments for the document-watch commands. `clientId` identifies the page
/// holding the registration — see the `watch_document` arm.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocumentWatchArg {
    client_id: String,
    root: String,
    rel_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitGraphArg {
    repo_id: String,
    limit: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitDetailArg {
    repo_id: String,
    sha: String,
}

/// `oldPath` is sent for a renamed file only; `api.ts` leaves it out
/// otherwise, which reads as none.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitDiffArg {
    repo_id: String,
    sha: String,
    path: String,
    #[serde(default)]
    old_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnabledArg {
    enabled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocumentWidthArg {
    width: DocumentWidth,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BitbucketCredentialsArg {
    username: String,
    api_token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GithubTokenArg {
    token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PanelPositionArg {
    position: PanelPosition,
}

/// The pull request a viewer command names: `{ provider, owner, repo, number }`,
/// whose own fields `PullRequestReference` renames to the camelCase
/// `src/api.ts` sends.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceArg {
    reference: PullRequestReference,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestDetailArg {
    reference: PullRequestReference,
    manual: bool,
    cached_only: bool,
}

/// `path` is the file's key (`newPath ?? oldPath`), and `head` and `base` the
/// commits of the detail the view rendered.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestFileArg {
    reference: PullRequestReference,
    path: String,
    head: String,
    base: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileViewedArg {
    reference: PullRequestReference,
    path: String,
    viewed: bool,
    head: String,
    base: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HunkViewedArg {
    reference: PullRequestReference,
    path: String,
    hunk: usize,
    viewed: bool,
    head: String,
    base: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NameArg {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AliasesArg {
    aliases: Vec<Author>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SecsArg {
    secs: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdsArg {
    ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FavoriteDeltaArg {
    add: Vec<String>,
    remove: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PresentationArg {
    uri: String,
    #[serde(default)]
    repo_id: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    color: Option<PaletteColor>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DisabledArg {
    uri: String,
    #[serde(default)]
    repo_id: Option<String>,
    disabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use openspec_app::{
        notice_envelope, PullRequestWindowGeometry, EVENT_PULL_REQUEST_PROVIDER_CHANGED,
    };
    use serde_json::json;

    /// `set_document_width` must announce itself on the app-event channel.
    ///
    /// This asserts the emit in the arm above, which nothing else does. The
    /// round-trip test in `tests/server.rs` drives the router without
    /// subscribing, so `broadcast::Sender::send` there returns `Err` with zero
    /// receivers and the arm's `let _ = ...` discards it — the emit could be
    /// deleted outright and that test would still pass. The `sse.rs` test is
    /// no help either: it publishes on `extra_tx` by hand, exercising the
    /// stream rather than the producer.
    ///
    /// The name is compared against the constant, not a string literal, so a
    /// rename that missed one transport fails here instead of silently
    /// splitting the two hosts' event vocabularies.
    #[tokio::test]
    async fn set_document_width_emits_the_change_event() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, mut rx) = broadcast::channel(8);

        dispatch(&svc, &tx, "set_document_width", json!({ "width": "full" }))
            .await
            .expect("set_document_width should succeed");

        let (name, payload) = rx.try_recv().expect("an event must have been emitted");
        assert_eq!(name, EVENT_DOCUMENT_WIDTH_CHANGED);
        assert_eq!(
            payload,
            Value::String("full".into()),
            "the payload carries the new rung, so a listener re-stamps without a round trip"
        );
    }

    /// The Commit history switch must reach a connected browser skin, not only
    /// the desktop webview: an open tab adds or drops its graph from this event
    /// (`commit-graph`: *Commit History Can Be Turned Off*). Compared against
    /// the constant for the reason the reading-width test above gives, and with
    /// the frontend's literal argument JSON, since a missing or misnamed arm
    /// fails only at runtime in the browser.
    #[tokio::test]
    async fn set_commit_history_enabled_emits_the_change_event() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, mut rx) = broadcast::channel(8);

        dispatch(
            &svc,
            &tx,
            "set_commit_history_enabled",
            json!({ "enabled": false }),
        )
        .await
        .expect("set_commit_history_enabled should succeed");

        let (name, payload) = rx.try_recv().expect("an event must have been emitted");
        assert_eq!(name, EVENT_COMMIT_HISTORY_ENABLED_CHANGED);
        assert_eq!(payload, Value::Bool(false), "the payload is the new value");
        assert!(
            !svc.settings.commit_history_enabled(),
            "and it was persisted"
        );

        let read = dispatch(&svc, &tx, "get_commit_history_enabled", json!({}))
            .await
            .expect("get_commit_history_enabled should succeed");
        assert_eq!(
            read,
            Value::Bool(false),
            "the getter is routed and reads it back"
        );
        assert!(rx.try_recv().is_err(), "and a read announces nothing");
    }

    /// The file browser's union listing must be reachable through THIS
    /// transport, with the literal argument JSON `src/api.ts` sends.
    ///
    /// Two runtime-only failures meet here and nothing else can see either.
    /// A missing arm compiles, passes `tsc` and `cargo test`, and fails only in
    /// the browser with `unknown command`. And `FileScope` is an enum with
    /// struct variants, where `rename_all` alone leaves the inner field
    /// snake_case — `ArchiveScope` shipped exactly that way and every
    /// repository-scoped listing failed with `missing field repo_id`, with the
    /// whole suite green.
    ///
    /// The assertion is the *authorization* refusal, which is reached only
    /// after the command has been routed and its arguments deserialized: an
    /// unrouted command or an unparsed `repoId` produces a different message.
    #[tokio::test]
    async fn list_workspace_file_rows_is_routed_and_parses_the_frontends_json() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, _rx) = broadcast::channel(8);

        let err = dispatch(
            &svc,
            &tx,
            "list_workspace_file_rows",
            json!({ "scope": { "kind": "repo", "repoId": "/nope/.git" } }),
        )
        .await
        .expect_err("an unregistered repository is refused");
        assert_eq!(err, "unregistered repository");

        let err = dispatch(
            &svc,
            &tx,
            "list_workspace_file_rows",
            json!({ "scope": { "kind": "flat", "workspace": "/nope" } }),
        )
        .await
        .expect_err("an unregistered workspace is refused");
        assert_eq!(err, "unregistered workspace");
    }

    /// Both commit reads must be reachable through THIS transport with the
    /// literal argument JSON `src/api.ts` sends: `get_commit_detail`, and
    /// `get_commit_diff` with `oldPath`, as a renamed file's "Load diff" sends
    /// it, and without, as every other file's does.
    ///
    /// The assertion is the registration refusal, which is reached only once
    /// the arm has routed the command and parsed its arguments, past the
    /// object-id check the valid sha clears: an unrouted command, or arguments
    /// that do not parse, fail with a different message.
    #[tokio::test]
    async fn commit_reads_are_routed_and_parse_the_frontends_json() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, _rx) = broadcast::channel(8);
        let sha = "0123456789abcdef0123456789abcdef01234567";

        for (command, args) in [
            (
                "get_commit_detail",
                json!({ "repoId": "/nope/.git", "sha": sha }),
            ),
            (
                "get_commit_diff",
                json!({
                    "repoId": "/nope/.git",
                    "sha": sha,
                    "path": "src/new.ts",
                    "oldPath": "src/old.ts",
                }),
            ),
            (
                "get_commit_diff",
                json!({ "repoId": "/nope/.git", "sha": sha, "path": "src/new.ts" }),
            ),
            (
                "get_commit_file_image",
                json!({
                    "repoId": "/nope/.git",
                    "sha": sha,
                    "path": "icons/new.png",
                    "oldPath": "icons/old.png",
                }),
            ),
            (
                "get_commit_file_image",
                json!({ "repoId": "/nope/.git", "sha": sha, "path": "icons/app.png" }),
            ),
        ] {
            let err = dispatch(&svc, &tx, command, args.clone())
                .await
                .expect_err("an unregistered repository is refused");
            assert_eq!(err, "unregistered repository", "{command} {args}");
        }
    }

    /// `set_bitbucket_panel_position` must announce itself on the app-event
    /// channel, with the new slot — the twin of
    /// `set_document_width_emits_the_change_event`, and for the same reason:
    /// nothing else observes the producer, so the emit could be deleted with
    /// every other test still green.
    #[tokio::test]
    async fn set_bitbucket_panel_position_emits_the_move_event() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, mut rx) = broadcast::channel(8);

        dispatch(
            &svc,
            &tx,
            "set_bitbucket_panel_position",
            json!({ "position": "right-top" }),
        )
        .await
        .expect("set_bitbucket_panel_position should succeed");

        let (name, payload) = rx.try_recv().expect("an event must have been emitted");
        assert_eq!(name, EVENT_PULL_REQUEST_PANEL_MOVED);
        assert_eq!(
            payload,
            json!({ "provider": "bitbucket", "position": "right-top" }),
            "the payload carries the new slot, so a listener re-seats without a round trip"
        );
        assert_eq!(
            svc.settings.bitbucket_panel_position(),
            PanelPosition::RightTop,
            "and the slot was persisted before it was announced"
        );
    }

    /// The GitHub twin: the move names its provider, so a listener re-seats
    /// the GitHub panel and leaves the BitBucket one where it is.
    #[tokio::test]
    async fn set_github_panel_position_emits_the_move_event_with_its_provider() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, mut rx) = broadcast::channel(8);

        dispatch(
            &svc,
            &tx,
            "set_github_panel_position",
            json!({ "position": "right-bottom" }),
        )
        .await
        .expect("set_github_panel_position should succeed");

        let (name, payload) = rx.try_recv().expect("an event must have been emitted");
        assert_eq!(name, EVENT_PULL_REQUEST_PANEL_MOVED);
        assert_eq!(
            payload,
            json!({ "provider": "github", "position": "right-bottom" })
        );
        assert_eq!(
            svc.settings.github_panel_position(),
            PanelPosition::RightBottom
        );
        assert_eq!(
            svc.settings.bitbucket_panel_position(),
            PanelPosition::LeftBottom,
            "the BitBucket slot is untouched"
        );
    }

    /// The provider-less getter is retired: a script still calling it gets
    /// the exact unknown-command error rather than silently BitBucket data
    /// (`bitbucket-pull-requests`: *The Snapshot Is Announced on the Cache
    /// Stream*).
    #[tokio::test]
    async fn get_my_pull_requests_is_an_unknown_command() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, _rx) = broadcast::channel(8);

        let err = dispatch(&svc, &tx, "get_my_pull_requests", json!({}))
            .await
            .expect_err("the old name is gone");
        assert_eq!(err, "unknown command: get_my_pull_requests");
        let renamed = dispatch(&svc, &tx, "get_bitbucket_pull_requests", json!({}))
            .await
            .expect("the renamed getter answers");
        assert_eq!(renamed["status"], "disabled");
    }

    /// The GitHub token is write-only over this transport too, and the
    /// snapshot getter serves the two-list shape.
    #[tokio::test]
    async fn the_github_token_is_write_only_over_the_web_transport() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, _rx) = broadcast::channel(8);

        dispatch(
            &svc,
            &tx,
            "set_github_token",
            json!({ "token": "ghp_s3cret" }),
        )
        .await
        .expect("set_github_token should succeed");
        dispatch(&svc, &tx, "set_github_enabled", json!({ "enabled": true }))
            .await
            .expect("set_github_enabled should succeed");
        let view = dispatch(&svc, &tx, "get_github_config", json!({}))
            .await
            .expect("get_github_config should succeed");
        assert_eq!(view["tokenSet"], true);
        assert_eq!(view["enabled"], true);
        assert!(view.get("token").is_none(), "{view}");
        assert!(!view.to_string().contains("ghp_s3cret"), "{view}");

        let snapshot = dispatch(&svc, &tx, "get_github_pull_requests", json!({}))
            .await
            .expect("get_github_pull_requests should succeed");
        assert_eq!(snapshot["status"], "disabled");
        assert!(snapshot["authored"].is_array());
        assert!(snapshot["reviewRequested"].is_array());
    }

    /// The links snapshot is served over the web transport and has the
    /// two-sided shape the frontend mirrors; with nothing enabled it is
    /// empty and announces nothing.
    #[tokio::test]
    async fn get_pull_request_links_returns_the_snapshot_shape() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, mut rx) = broadcast::channel(8);

        let links = dispatch(&svc, &tx, "get_pull_request_links", json!({}))
            .await
            .expect("get_pull_request_links should succeed");

        assert_eq!(links, json!({ "worktrees": [], "pullRequests": [] }));
        assert!(rx.try_recv().is_err(), "a read is not a change");
    }

    /// The web transport must have no operation that opens a URL on the
    /// serving host (`web-ui`: *Link Handling in the Browser Skin*). Asserted
    /// as the exact unknown-command error, so a later "helpful" arm — even one
    /// that refused — would fail here and have to argue its case.
    #[tokio::test]
    async fn open_pull_request_is_an_unknown_command_on_the_web_transport() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, mut rx) = broadcast::channel(8);

        let err = dispatch(
            &svc,
            &tx,
            "open_pull_request",
            json!({ "url": "https://bitbucket.org/acme/app/pull-requests/7" }),
        )
        .await
        .expect_err("the web transport cannot open a pull request");

        assert_eq!(err, "unknown command: open_pull_request");
        assert!(rx.try_recv().is_err(), "and nothing was announced");
    }

    /// The pull request `src/api.ts` names, as the literal JSON it sends.
    fn reference_json(provider: &str) -> Value {
        json!({ "provider": provider, "owner": "acme", "repo": "api", "number": 42 })
    }

    const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";
    const BASE: &str = "89abcdef0123456789abcdef0123456789abcdef";

    fn now_unix() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// `get_pull_request_detail` takes the camelCase arguments `src/api.ts`
    /// sends, `cachedOnly` included, and for a pull request in no snapshot
    /// answers before any request: refused while GitHub is off, then not
    /// listed, or not cached for a cache-only call. It spends nothing of
    /// GitHub's detail budget and announces nothing on either channel
    /// (`pull-request-viewer`: *Detail Reads Are Scoped to the Snapshot*).
    #[tokio::test]
    async fn get_pull_request_detail_outside_the_snapshot_sends_nothing() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, mut rx) = broadcast::channel(8);
        let ask = |cached_only: bool| {
            json!({
                "reference": reference_json("github"),
                "manual": false,
                "cachedOnly": cached_only,
            })
        };

        let refused = dispatch(&svc, &tx, "get_pull_request_detail", ask(false))
            .await
            .expect("every answer is an outcome");
        assert_eq!(refused, json!({ "kind": "refused" }), "GitHub is off");

        svc.set_github_enabled(true).unwrap();
        let mut notices = svc.subscribe_notices();
        let not_listed = dispatch(&svc, &tx, "get_pull_request_detail", ask(false))
            .await
            .expect("every answer is an outcome");
        assert_eq!(not_listed, json!({ "kind": "notListed" }));
        let not_cached = dispatch(&svc, &tx, "get_pull_request_detail", ask(true))
            .await
            .expect("every answer is an outcome");
        assert_eq!(
            not_cached,
            json!({ "kind": "notCached" }),
            "`cachedOnly` was read"
        );

        assert_eq!(svc.github_limits.spent(now_unix()), 0, "nothing was sent");
        assert_eq!(svc.github_limits.in_flight(), 0);
        assert!(rx.try_recv().is_err(), "a read announces nothing");
        assert!(notices.try_recv().is_err());
    }

    /// `get_pull_request_file` takes its four arguments as `src/api.ts` sends
    /// them, and with nothing cached answers `changed`, an answer reached
    /// only once they parse, so the view reads the pull request again.
    #[tokio::test]
    async fn get_pull_request_file_answers_changed_with_nothing_cached() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, _rx) = broadcast::channel(8);
        svc.set_bitbucket_enabled(true).unwrap();

        let err = dispatch(
            &svc,
            &tx,
            "get_pull_request_file",
            json!({
                "reference": reference_json("bitbucket"),
                "path": "src/lib.rs",
                "head": HEAD,
                "base": BASE,
            }),
        )
        .await
        .expect("an outcome, not an error");
        assert_eq!(err, json!({ "kind": "changed" }));
    }

    /// `pull-request-viewer`: *The browser skin reads images*:
    /// `get_pull_request_file_image` takes its four arguments as `src/api.ts`
    /// sends them, and answers as the desktop application would: `changed`
    /// with nothing cached, and `failed` with `refused` while its provider is
    /// disabled.
    #[tokio::test]
    async fn get_pull_request_file_image_is_served_on_the_web_transport() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, _rx) = broadcast::channel(8);
        svc.set_bitbucket_enabled(true).unwrap();
        let ask = |provider: &str| {
            json!({
                "reference": reference_json(provider),
                "path": "icons/app.png",
                "head": HEAD,
                "base": BASE,
            })
        };

        let changed = dispatch(&svc, &tx, "get_pull_request_file_image", ask("bitbucket"))
            .await
            .expect("an outcome, not an error");
        assert_eq!(changed, json!({ "kind": "changed" }));
        let refused = dispatch(&svc, &tx, "get_pull_request_file_image", ask("github"))
            .await
            .expect("an outcome, not an error");
        assert_eq!(
            refused,
            json!({ "kind": "failed", "reason": "refused", "untilUnix": null })
        );
    }

    /// A mark needs a cached detail to key the file from: without one,
    /// `set_file_viewed` — routed with the arguments `src/api.ts` sends — is
    /// refused, `review-progress.json` is never created, and nothing is
    /// announced (`pull-request-viewer`: *Review Progress*).
    #[tokio::test]
    async fn set_file_viewed_without_a_cached_detail_is_refused_and_stores_nothing() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, mut rx) = broadcast::channel(8);
        svc.set_github_enabled(true).unwrap();
        let mut notices = svc.subscribe_notices();

        let err = dispatch(
            &svc,
            &tx,
            "set_file_viewed",
            json!({
                "reference": reference_json("github"),
                "path": "src/lib.rs",
                "viewed": true,
                "head": HEAD,
                "base": BASE,
            }),
        )
        .await
        .expect_err("nothing is cached to key the file from");
        assert_eq!(err, "no detail of this pull request is cached");
        assert!(
            !cfg.path().join("review-progress.json").exists(),
            "nothing was stored"
        );
        assert!(notices.try_recv().is_err(), "nothing was announced");
        assert!(rx.try_recv().is_err());
    }

    /// `set_hunk_viewed` is routed with the arguments `src/api.ts` sends, the
    /// hunk's index among them, and is refused as a file mark is without a
    /// cached detail, storing and announcing nothing (`pull-request-viewer`:
    /// *The browser skin marks hunks*).
    #[tokio::test]
    async fn set_hunk_viewed_without_a_cached_detail_is_refused_and_stores_nothing() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, mut rx) = broadcast::channel(8);
        svc.set_github_enabled(true).unwrap();
        let mut notices = svc.subscribe_notices();

        let err = dispatch(
            &svc,
            &tx,
            "set_hunk_viewed",
            json!({
                "reference": reference_json("github"),
                "path": "src/lib.rs",
                "hunk": 0,
                "viewed": true,
                "head": HEAD,
                "base": BASE,
            }),
        )
        .await
        .expect_err("nothing is cached to key the hunk from");
        assert_eq!(err, "no detail of this pull request is cached");
        assert!(
            !cfg.path().join("review-progress.json").exists(),
            "nothing was stored"
        );
        assert!(notices.try_recv().is_err(), "nothing was announced");
        assert!(rx.try_recv().is_err());
    }

    /// Progress is answered only while its provider is enabled, as a detail
    /// is: `get_review_progress` refuses without content while GitHub is off,
    /// and once it is on, with nothing cached, has no files to answer for.
    #[tokio::test]
    async fn get_review_progress_refuses_while_the_provider_is_disabled() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, _rx) = broadcast::channel(8);
        let args = json!({ "reference": reference_json("github") });

        let err = dispatch(&svc, &tx, "get_review_progress", args.clone())
            .await
            .expect_err("GitHub is off");
        assert_eq!(err, "the pull request's provider is disabled");

        svc.set_github_enabled(true).unwrap();
        let err = dispatch(&svc, &tx, "get_review_progress", args)
            .await
            .expect_err("nothing is cached");
        assert_eq!(err, "no detail of this pull request is cached");
    }

    /// The web transport's answer to one of the viewer's desktop-only
    /// commands: the exact unknown-command error, with nothing announced on
    /// either channel. Returns the service, so a caller can check that
    /// nothing changed.
    async fn assert_unknown_on_the_web_transport(
        command: &str,
        args: Value,
    ) -> (AppService, tempfile::TempDir) {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, mut rx) = broadcast::channel(8);
        let mut notices = svc.subscribe_notices();

        let err = dispatch(&svc, &tx, command, args)
            .await
            .expect_err("a desktop-only command is not served here");
        assert_eq!(err, format!("unknown command: {command}"));
        assert!(rx.try_recv().is_err(), "and nothing was announced");
        assert!(notices.try_recv().is_err());
        (svc, cfg)
    }

    /// No window opens on the serving host (`pull-request-viewer`:
    /// *Pull-Request Window*): the browser skin opens the pull request's tab
    /// itself.
    #[tokio::test]
    async fn open_pull_request_window_is_an_unknown_command_on_the_web_transport() {
        assert_unknown_on_the_web_transport(
            "open_pull_request_window",
            json!({ "addressPath": "/pr/github/acme/api/42", "title": "#42 — acme/api" }),
        )
        .await;
    }

    /// No window opens on the serving host for a zoom either (`diff-view`:
    /// *Image Comparison*, Zooming): the browser skin opens the file's zoom
    /// tab itself.
    #[tokio::test]
    async fn open_image_window_is_an_unknown_command_on_the_web_transport() {
        assert_unknown_on_the_web_transport(
            "open_image_window",
            json!({ "addressPath": "{}", "title": "icons/app.png — zoom" }),
        )
        .await;
    }

    /// No link opens on the serving host (`pull-request-viewer`: *Desktop Link
    /// Opener*): the browser skin opens pull-request links as opener-isolated
    /// tabs.
    #[tokio::test]
    async fn open_pull_request_link_is_an_unknown_command_on_the_web_transport() {
        assert_unknown_on_the_web_transport(
            "open_pull_request_link",
            json!({
                "reference": reference_json("github"),
                "href": "https://github.com/acme/api/pull/42",
            }),
        )
        .await;
    }

    /// The pull-request window's size is the desktop's alone
    /// (`pull-request-viewer`: *Pull-Request Window Geometry*).
    #[tokio::test]
    async fn set_pull_request_window_size_is_an_unknown_command_on_the_web_transport() {
        let (svc, _cfg) = assert_unknown_on_the_web_transport(
            "set_pull_request_window_size",
            json!({ "width": 1440, "height": 900 }),
        )
        .await;
        assert_eq!(
            svc.settings.pull_request_window(),
            PullRequestWindowGeometry::default(),
            "the remembered size is untouched"
        );
    }

    /// A provider toggle from the browser goes through the service, which
    /// raises `pull-request-provider-changed` on its own broadcast — reaching
    /// every tab and every desktop window — and nothing goes on this
    /// transport's own channel, which would reach its own tabs alone
    /// (`pull-request-viewer`: *Provider Enabled Flags Stay Current*).
    #[tokio::test]
    async fn a_provider_toggle_raises_its_notice_on_the_service_broadcast() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, mut rx) = broadcast::channel(8);
        let mut notices = svc.subscribe_notices();

        dispatch(&svc, &tx, "set_github_enabled", json!({ "enabled": true }))
            .await
            .expect("set_github_enabled should succeed");
        let (name, payload) = notice_envelope(&notices.try_recv().expect("a notice was raised"));
        assert_eq!(name, EVENT_PULL_REQUEST_PROVIDER_CHANGED);
        assert_eq!(payload, json!({ "provider": "github", "enabled": true }));
        assert!(notices.try_recv().is_err(), "exactly one");
        assert!(svc.settings.github_enabled(), "and the flag was stored");

        dispatch(
            &svc,
            &tx,
            "set_bitbucket_enabled",
            json!({ "enabled": false }),
        )
        .await
        .expect("set_bitbucket_enabled should succeed");
        let (name, payload) = notice_envelope(&notices.try_recv().expect("a notice was raised"));
        assert_eq!(name, EVENT_PULL_REQUEST_PROVIDER_CHANGED);
        assert_eq!(
            payload,
            json!({ "provider": "bitbucket", "enabled": false })
        );

        assert!(
            rx.try_recv().is_err(),
            "nothing on the transport's own app-event channel"
        );
    }

    /// The credential is write-only over this transport too: it goes in with
    /// the camelCase `apiToken` key `src/api.ts` sends, and the getter reports
    /// only that it is set.
    #[tokio::test]
    async fn bitbucket_credentials_are_write_only_over_the_web_transport() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, _rx) = broadcast::channel(8);

        dispatch(
            &svc,
            &tx,
            "set_bitbucket_credentials",
            json!({ "username": "ada", "apiToken": "s3cret-token" }),
        )
        .await
        .expect("set_bitbucket_credentials should succeed");
        let view = dispatch(&svc, &tx, "get_bitbucket_config", json!({}))
            .await
            .expect("get_bitbucket_config should succeed");

        assert_eq!(view["tokenSet"], true);
        assert_eq!(view["username"], "ada");
        assert!(view.get("apiToken").is_none(), "{view}");
        assert!(!view.to_string().contains("s3cret-token"), "{view}");
    }

    /// The getter must not announce anything — a read that emitted would make
    /// every surface re-stamp on every poll.
    #[tokio::test]
    async fn get_document_width_emits_nothing() {
        let cfg = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, mut rx) = broadcast::channel(8);

        dispatch(&svc, &tx, "get_document_width", json!({}))
            .await
            .expect("get_document_width should succeed");

        assert!(rx.try_recv().is_err(), "a read is not a change");
    }
}
