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

use openspec_app::events::{EVENT_DOCUMENT_WIDTH_CHANGED, EVENT_WORKSPACE_PRESENTATION_UPDATED};
use openspec_app::{AppService, DocumentWidth};
use openspec_core::{Author, PaletteColor, Person};
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
        "list_archived" => {
            let a: WorkspaceArg = parse(args)?;
            to_val(svc.list_archived(&PathBuf::from(a.workspace))?)?
        }
        "archived_artifact_status" => {
            let a: ArchivedArg = parse(args)?;
            to_val(svc.archived_artifact_status(&PathBuf::from(a.workspace), &a.dir_name)?)?
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
                svc.commit_diff(PathBuf::from(a.repo_id), a.sha, a.path)
                    .await?,
            )?
        }

        // ---- Identity ---------------------------------------------------
        "get_identity" => to_val(svc.identity_info()?)?,
        "observed_authors" => to_val(svc.observed_authors())?,
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
        "set_people" => {
            let a: PeopleArg = parse(args)?;
            svc.settings
                .set_people(a.people)
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitDiffArg {
    repo_id: String,
    sha: String,
    path: String,
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
struct PeopleArg {
    people: Vec<Person>,
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
