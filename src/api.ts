import { invoke as tauriInvoke } from "@tauri-apps/api/core"
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event"
import type {
    ArchivedChangeSummary,
    ArtifactRead,
    ArtifactReadKind,
    ArtifactStatus,
    Author,
    CacheUpdatedPayload,
    DocumentChangedPayload,
    ChangeAddedPayload,
    ChangeArchivedPayload,
    ChangeData,
    ChatGptQuotaState,
    ClaudeQuotaState,
    CommitFile,
    CommitGraph,
    DashboardData,
    GraphChangedPayload,
    IdentityInfo,
    InstancePayload,
    LogicalChangePayload,
    PaletteColor,
    Person,
    RegisteredWorkspace,
    WebServerConfig,
    WorkspaceGarden,
    WorkspaceRemovedPayload,
    WorkspaceView,
} from "./types"
import {
    EVENT_CACHE_UPDATED,
    EVENT_DOCUMENT_CHANGED,
    EVENT_CHANGE_ADDED,
    EVENT_CHANGE_ARCHIVED,
    EVENT_GRAPH_CHANGED,
    EVENT_INSTANCE_ADDED,
    EVENT_INSTANCE_REMOVED,
    EVENT_LOGICAL_CHANGE_ADDED,
    EVENT_LOGICAL_CHANGE_ARCHIVED,
    EVENT_QUOTA_UPDATED,
    EVENT_TOGGLE_COMMIT_RAIL,
    EVENT_TOGGLE_SIDEBAR,
    EVENT_WORKSPACE_PRESENTATION_UPDATED,
    EVENT_WORKSPACE_REMOVED,
} from "./types"
import { CLIENT_ID, subscribeToEventStream } from "./eventStream"
import { shortHash } from "./routing/slug"

// Re-exported for call sites that import the artifact-kind union from the
// API surface (the canonical definition lives in ./types).
export type { ArtifactRead, ArtifactReadKind } from "./types"

// -------------------------------------------------------------------------
// Transport — host detection
// -------------------------------------------------------------------------
//
// The same bundle runs in two hosts. Inside the Tauri desktop shell it uses
// in-process `invoke`/`listen`; served over HTTP by `specforge-web` it uses
// `fetch` + `EventSource`. Everything below `invokeLogged`/`listenLogged` is
// transport-agnostic, so the entire command surface is shared.

/// True when running inside the native Tauri shell. Tauri v2 injects
/// `__TAURI_INTERNALS__`; the legacy `__TAURI__` global is checked too.
export function isTauri(): boolean {
    return (
        typeof window !== "undefined" &&
        ("__TAURI_INTERNALS__" in window || "__TAURI__" in window)
    )
}

/// True when running as a browser tab served by the local web server.
export function isWeb(): boolean {
    return !isTauri()
}

// The web command transport: POST { command, args } to the server's invoke
// endpoint, mirroring Tauri's `invoke(command, args)` shape. A non-2xx response
// carries a `{ error }` envelope which becomes a thrown Error, matching how a
// rejected Tauri command surfaces to callers.
async function webInvoke<T>(
    command: string,
    args?: Record<string, unknown>,
): Promise<T> {
    const res = await fetch("/api/invoke", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ command, args: args ?? {} }),
    })
    if (!res.ok) {
        let message = `${command} failed (${res.status})`
        try {
            const body = await res.json()
            if (body && typeof body.error === "string") message = body.error
        } catch {
            // Non-JSON error body — keep the status-based message.
        }
        throw new Error(message)
    }
    // 200: the body is the raw result JSON (`null` for unit-returning commands).
    return (await res.json()) as T
}

// The web event stream. Its lifecycle — including recovery after the document
// has been suspended — lives in ./eventStream; this only adapts SSE frames into
// typed payloads.

function webListen<T>(
    event: string,
    handler: (payload: T) => void,
): Promise<UnlistenFn> {
    const listener = ((e: MessageEvent) => {
        let payload: T
        try {
            payload = e.data ? (JSON.parse(e.data) as T) : (undefined as T)
        } catch {
            payload = undefined as T
        }
        handler(payload)
    }) as EventListener
    const unlisten: UnlistenFn = subscribeToEventStream(event, listener)
    return Promise.resolve(unlisten)
}

// Wraps the active transport so every command logs its name, args, and
// result/error in dev. `import.meta.env.DEV` is constant-folded out of
// production builds so the logging has zero runtime cost there.
async function invokeLogged<T>(
    command: string,
    args?: Record<string, unknown>,
): Promise<T> {
    if (import.meta.env.DEV) {
        console.log(`[api] → ${command}`, args ?? {})
    }
    try {
        const result = isTauri()
            ? await tauriInvoke<T>(command, args)
            : await webInvoke<T>(command, args)
        if (import.meta.env.DEV) {
            console.log(`[api] ← ${command}`, result)
        }
        return result
    } catch (err) {
        if (import.meta.env.DEV) {
            console.warn(`[api] ✗ ${command}`, err)
        }
        throw err
    }
}

function listenLogged<T>(
    event: string,
    handler: (payload: T) => void,
): Promise<UnlistenFn> {
    const wrapped = (payload: T) => {
        if (import.meta.env.DEV) {
            console.log(`[event] ${event}`, payload)
        }
        handler(payload)
    }
    if (isTauri()) {
        return tauriListen<T>(event, (e) => wrapped(e.payload))
    }
    return webListen<T>(event, wrapped)
}

// -------------------------------------------------------------------------
// Commands
// -------------------------------------------------------------------------

export async function registerWorkspace(path: string): Promise<RegisteredWorkspace> {
    return invokeLogged<RegisteredWorkspace>("register_workspace", { path })
}

export async function unregisterWorkspace(path: string): Promise<boolean> {
    return invokeLogged<boolean>("unregister_workspace", { path })
}

export async function listWorkspaces(): Promise<RegisteredWorkspace[]> {
    return invokeLogged<RegisteredWorkspace[]>("list_workspaces")
}

export async function getChanges(workspace: string): Promise<ChangeData[]> {
    return invokeLogged<ChangeData[]>("get_changes", { workspace })
}

/// Lists one workspace's archived changes for the Archive view — a lightweight
/// `{ id, date, title }` per archive directory, newest-first. Read on demand
/// when the Archive view opens or its selected workspace changes.
export async function listArchived(
    workspace: string,
): Promise<ArchivedChangeSummary[]> {
    return invokeLogged<ArchivedChangeSummary[]>("list_archived", { workspace })
}

/// Reports which artifacts an archived change has on disk, so the Archive view
/// can offer per-artifact navigation. `dirName` is the archive directory name
/// (`<YYYY-MM-DD>-<id>`).
export async function archivedArtifactStatus(
    workspace: string,
    dirName: string,
): Promise<ArtifactStatus> {
    return invokeLogged<ArtifactStatus>("archived_artifact_status", {
        workspace,
        dirName,
    })
}

export async function getWorkspaceViews(): Promise<WorkspaceView[]> {
    return invokeLogged<WorkspaceView[]>("get_workspace_views")
}

export async function getActiveCount(): Promise<number> {
    return invokeLogged<number>("get_active_count")
}

/// Aggregate the global Dashboard payload across every registered workspace.
/// The progress layer is always resolved to the canonical developer over all
/// available history — there is no audience (Me/Everyone) selector.
export async function getDashboard(): Promise<DashboardData> {
    return invokeLogged<DashboardData>("get_dashboard")
}

/// The commit garden: one stylized plant per top-level entry, grown from today's
/// commits.
export async function getCommitGarden(): Promise<WorkspaceGarden[]> {
    return invokeLogged<WorkspaceGarden[]>("get_commit_garden")
}

/// The developer-identity configuration plus detected candidate identities.
export async function getIdentity(): Promise<IdentityInfo> {
    return invokeLogged<IdentityInfo>("get_identity")
}

/// Set the canonical display name (pass null to clear it).
export async function setDisplayName(name: string | null): Promise<void> {
    return invokeLogged<void>("set_display_name", { name })
}

/// Replace the set of alias identities that resolve to "me".
export async function setIdentityAliases(aliases: Author[]): Promise<void> {
    return invokeLogged<void>("set_identity_aliases", { aliases })
}

/// Replace the whole contributor roster (named people other than "me").
export async function setPeople(people: Person[]): Promise<void> {
    return invokeLogged<void>("set_people", { people })
}

/// The distinct non-"me" authors observed across registered repositories within
/// the Dashboard window — the candidate pool for naming and merging on the roster.
export async function observedAuthors(): Promise<Author[]> {
    return invokeLogged<Author[]>("observed_authors")
}

export async function readArtifact(
    workspace: string,
    changeId: string,
    artifactKind: ArtifactReadKind,
    capability?: string,
): Promise<ArtifactRead> {
    return invokeLogged<ArtifactRead>("read_artifact", {
        workspace,
        changeId,
        artifactKind,
        capability,
    })
}

/// The markdown files under a workspace browse root (a repo's main worktree
/// or a flat workspace folder) — gitignore-aware for a git repository, a
/// bounded walk otherwise. Sorted, forward-slash relative paths.
export async function listMarkdownFiles(root: string): Promise<string[]> {
    return invokeLogged<string[]>("list_markdown_files", { root })
}

/// Read one markdown file from a workspace browse root. Unlike
/// `readArtifact`, not confined to `openspec/changes/`.
export async function readWorkspaceFile(
    root: string,
    relPath: string,
): Promise<string> {
    return invokeLogged<string>("read_workspace_file", { root, relPath })
}

/// Opens a link clicked in rendered artifact markdown via the OS default
/// handler — an external URL in the system browser, or a validated,
/// allow-listed workspace file. `root` is the authorized root the rendering
/// surface already holds (a registered workspace, or a file-browser's browse
/// root); `basePath` is the root-relative path of the markdown file being
/// viewed, which relative hrefs in `href` resolve against. Desktop-only: not
/// present on the web dispatch surface (see `isWeb()` call sites), so this
/// must never be invoked there. Rejects — never navigates or throws
/// synchronously — when the link is refused, dangling, or inert at the
/// service layer; callers surface that as a quiet failure.
export async function openArtifactLink(
    root: string,
    basePath: string,
    href: string,
): Promise<void> {
    return invokeLogged<void>("open_artifact_link", { root, basePath, href })
}

/// Build the commit-graph for a repository (identified by its git common dir
/// `repoId`), reading up to `limit` commits across all refs.
export async function getCommitGraph(
    repoId: string,
    limit: number,
): Promise<CommitGraph> {
    return invokeLogged<CommitGraph>("get_commit_graph", { repoId, limit })
}

/// The files a commit changed, with per-file added/removed counts.
export async function getCommitDetail(
    repoId: string,
    sha: string,
): Promise<CommitFile[]> {
    return invokeLogged<CommitFile[]>("get_commit_detail", { repoId, sha })
}

/// The raw unified diff for one file of a commit.
export async function getCommitDiff(
    repoId: string,
    sha: string,
    path: string,
): Promise<string> {
    return invokeLogged<string>("get_commit_diff", { repoId, sha, path })
}

export async function getLaunchOnLogin(): Promise<boolean> {
    return invokeLogged<boolean>("get_launch_on_login")
}

export async function setLaunchOnLogin(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_launch_on_login", { enabled })
}

/// The latest opt-in Claude usage-quota snapshot (`status: "disabled"` when off).
export async function getClaudeQuota(): Promise<ClaudeQuotaState> {
    return invokeLogged<ClaudeQuotaState>("get_claude_quota")
}

export async function getClaudeQuotaEnabled(): Promise<boolean> {
    return invokeLogged<boolean>("get_claude_quota_enabled")
}

export async function setClaudeQuotaEnabled(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_claude_quota_enabled", { enabled })
}

/// The latest opt-in ChatGPT usage-quota snapshot (`status: "disabled"` when
/// off). A twin of `getClaudeQuota`.
export async function getChatGptQuota(): Promise<ChatGptQuotaState> {
    return invokeLogged<ChatGptQuotaState>("get_chatgpt_quota")
}

export async function getChatGptQuotaEnabled(): Promise<boolean> {
    return invokeLogged<boolean>("get_chatgpt_quota_enabled")
}

export async function setChatGptQuotaEnabled(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_chatgpt_quota_enabled", { enabled })
}

export async function getNotificationsEnabled(): Promise<boolean> {
    return invokeLogged<boolean>("get_notifications_enabled")
}

export async function setNotificationsEnabled(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_notifications_enabled", { enabled })
}

/// The WSL polling-watcher interval in seconds, or `null` on platforms where
/// WSL workspaces can't occur (macOS, Linux). `null` means "hide the control".
export async function getWslPollIntervalSecs(): Promise<number | null> {
    return invokeLogged<number | null>("get_wsl_poll_interval_secs")
}

export async function setWslPollIntervalSecs(secs: number): Promise<void> {
    return invokeLogged<void>("set_wsl_poll_interval_secs", { secs })
}

export async function getCollapsedTreeNodeIds(): Promise<string[]> {
    return invokeLogged<string[]>("get_collapsed_tree_node_ids")
}

export async function setCollapsedTreeNodeIds(ids: string[]): Promise<void> {
    return invokeLogged<void>("set_collapsed_tree_node_ids", { ids })
}

export async function getExpandedTreeNodeIds(): Promise<string[]> {
    return invokeLogged<string[]>("get_expanded_tree_node_ids")
}

export async function setExpandedTreeNodeIds(ids: string[]): Promise<void> {
    return invokeLogged<void>("set_expanded_tree_node_ids", { ids })
}

export async function getFavoriteChangeIds(): Promise<string[]> {
    return invokeLogged<string[]>("get_favorite_change_ids")
}

/// Apply a favorites delta (ids to star / unstar) and get back the merged
/// list. A delta, not a whole-list write, so one client's toggle can never
/// erase favorites another client persisted since this one hydrated.
export async function updateFavoriteChangeIds(
    add: string[],
    remove: string[],
): Promise<string[]> {
    return invokeLogged<string[]>("update_favorite_change_ids", {
        add,
        remove,
    })
}

/// Best-effort favorites flush for page dismissal. Over the web transport a
/// plain fetch can be killed with the page, so use sendBeacon (built for
/// exactly this); in the native shell fall back to a fire-and-forget invoke.
export function updateFavoriteChangeIdsOnPageHide(
    add: string[],
    remove: string[],
): void {
    if (isWeb() && typeof navigator.sendBeacon === "function") {
        navigator.sendBeacon(
            "/api/invoke",
            new Blob(
                [
                    JSON.stringify({
                        command: "update_favorite_change_ids",
                        args: { add, remove },
                    }),
                ],
                { type: "application/json" },
            ),
        )
        return
    }
    void updateFavoriteChangeIds(add, remove)
}

/// Persists the display-name and tint-colour overrides for a top-level row.
/// Pass `repoId` to address a repository group's shared presentation key, or
/// leave it `null` to address a flat workspace's own key.
export async function setWorkspacePresentation(
    uri: string,
    repoId: string | null,
    displayName: string | null,
    color: PaletteColor | null,
): Promise<void> {
    return invokeLogged<void>("set_workspace_presentation", {
        uri,
        repoId,
        displayName,
        color,
    })
}

/// Parks or un-parks a top-level row. Keyed exactly like
/// `setWorkspacePresentation` — pass `repoId` for a repository group, or `null`
/// for a flat workspace — but a separate command so toggling this cannot clobber
/// the row's display name or tint. A parked row leaves the tree pane, the tray
/// badge, and desktop notifications; it stays in this Settings listing and in
/// every Dashboard figure.
export async function setWorkspaceDisabled(
    uri: string,
    repoId: string | null,
    disabled: boolean,
): Promise<void> {
    return invokeLogged<void>("set_workspace_disabled", {
        uri,
        repoId,
        disabled,
    })
}

/// The embedded web-UI configuration, for the desktop-only "Web UI" settings
/// section. Not available in the web frontend (the section is hidden there).
export async function getWebConfig(): Promise<WebServerConfig> {
    return invokeLogged<WebServerConfig>("get_web_config")
}

/// Enable/disable the embedded web server. Persisted; applied on next launch.
export async function setWebEnabled(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_web_enabled", { enabled })
}

/// Set the embedded web server's loopback port. Persisted; applied on next launch.
export async function setWebPort(port: number): Promise<void> {
    return invokeLogged<void>("set_web_port", { port })
}

/// Enable/disable Tailscale Serve access (trusting the host's tailnet name in
/// the web guard). Persisted; applied when the server next builds its router.
export async function setWebTailscaleEnabled(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_web_tailscale_enabled", { enabled })
}

/// Set the manual Tailscale MagicDNS-name override (null/empty restores
/// auto-discovery).
export async function setWebTailscaleName(name: string | null): Promise<void> {
    return invokeLogged<void>("set_web_tailscale_name", { name })
}

/// Replace the Tailscale per-user login allow-list (empty = trust the whole
/// tailnet).
export async function setWebTailscaleAllowedLogins(
    logins: string[],
): Promise<void> {
    return invokeLogged<void>("set_web_tailscale_allowed_logins", { logins })
}

/// The tailnet name the web server would currently trust (manual override, else
/// discovered, else null) — shown read-only so a stale/missing name is visible.
export async function resolveTailscaleName(): Promise<string | null> {
    return invokeLogged<string | null>("resolve_tailscale_name")
}

// -------------------------------------------------------------------------
// Document watches and reader windows
// -------------------------------------------------------------------------

/// Who owns a document registration, as the two hosts each name it.
///
/// The desktop shell needs no identifier at all: the command runs in a window,
/// and `window.label()` on the Rust side is both the owner and the thing that
/// can go away. The browser has no such handle, so the page names itself.
function documentWatchArgs(root: string, relPath: string): Record<string, unknown> {
    return isTauri() ? { root, relPath } : { clientId: CLIENT_ID, root, relPath }
}

/// Register interest in one markdown document, so this surface is notified
/// when the file changes on disk. Reference-counted in the shared layer:
/// several surfaces may hold the same document, and each must release its own.
export async function watchDocument(root: string, relPath: string): Promise<void> {
    return invokeLogged<void>("watch_document", documentWatchArgs(root, relPath))
}

/// Release one registration taken by `watchDocument`. Safe to call for a
/// document that is not registered.
export async function unwatchDocument(root: string, relPath: string): Promise<void> {
    return invokeLogged<void>("unwatch_document", documentWatchArgs(root, relPath))
}

/// Open — or focus — a reader window for `addressPath` (an `encodeAddress`
/// result). `title` names the document in the window's titlebar.
///
/// Must be called **synchronously inside the click handler** in the browser
/// host: `window.open` is discarded by popup blockers when it is reached from
/// a promise continuation rather than from the user gesture. That is why this
/// is not `async` even though the desktop path invokes a command — the invoke
/// is fired and not awaited, so both hosts open from the gesture itself.
export function openReaderWindow(addressPath: string, title: string): void {
    if (isTauri()) {
        void invokeLogged<void>("open_reader_window", { addressPath, title }).catch((err) => {
            console.warn("failed to open reader window:", err)
        })
        return
    }
    // The window NAME is the deduplication: opening the same document again
    // targets the window that already shows it instead of making a second.
    const opened = window.open(
        `${addressPath}?reader=1`,
        `specforge-reader:${shortHash(addressPath)}`,
    )
    // A reused window is not raised by `open` alone; the page it already holds
    // has to ask. Blocked or cross-origin access simply leaves it where it is.
    try {
        opened?.focus()
    } catch {
        /* a blocked popup returns null, and a focus refusal is not an error */
    }
}

/// Persist the size a reader window was resized to, so the next one adopts it.
/// Desktop-only: a browser window's size is the browser's business.
export async function setReaderWindowSize(width: number, height: number): Promise<void> {
    if (!isTauri()) return
    return invokeLogged<void>("set_reader_window_size", { width, height })
}

// -------------------------------------------------------------------------
// Events
// -------------------------------------------------------------------------

/// Fires when a document some surface registered changes on disk. Carries
/// identifiers only — the receiver re-reads through the guarded read.
export function onDocumentChanged(
    handler: (payload: DocumentChangedPayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<DocumentChangedPayload>(EVENT_DOCUMENT_CHANGED, handler)
}

export function onCacheUpdated(
    handler: (payload: CacheUpdatedPayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<CacheUpdatedPayload>(EVENT_CACHE_UPDATED, handler)
}

export function onChangeAdded(
    handler: (payload: ChangeAddedPayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<ChangeAddedPayload>(EVENT_CHANGE_ADDED, handler)
}

export function onChangeArchived(
    handler: (payload: ChangeArchivedPayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<ChangeArchivedPayload>(EVENT_CHANGE_ARCHIVED, handler)
}

export function onWorkspaceRemoved(
    handler: (payload: WorkspaceRemovedPayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<WorkspaceRemovedPayload>(EVENT_WORKSPACE_REMOVED, handler)
}

export function onLogicalChangeAdded(
    handler: (payload: LogicalChangePayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<LogicalChangePayload>(EVENT_LOGICAL_CHANGE_ADDED, handler)
}

export function onLogicalChangeArchived(
    handler: (payload: LogicalChangePayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<LogicalChangePayload>(EVENT_LOGICAL_CHANGE_ARCHIVED, handler)
}

export function onInstanceAdded(
    handler: (payload: InstancePayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<InstancePayload>(EVENT_INSTANCE_ADDED, handler)
}

export function onInstanceRemoved(
    handler: (payload: InstancePayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<InstancePayload>(EVENT_INSTANCE_REMOVED, handler)
}

export function onWorkspacePresentationUpdated(
    handler: () => void,
): Promise<UnlistenFn> {
    return listenLogged<unknown>(EVENT_WORKSPACE_PRESENTATION_UPDATED, () => handler())
}

export function onGraphChanged(
    handler: (payload: GraphChangedPayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<GraphChangedPayload>(EVENT_GRAPH_CHANGED, handler)
}

/// The quota snapshot was refreshed; the payload is empty, so callers re-read
/// via `getClaudeQuota`.
export function onQuotaUpdated(handler: () => void): Promise<UnlistenFn> {
    return listenLogged<unknown>(EVENT_QUOTA_UPDATED, () => handler())
}

/// The macOS View menu asked to toggle the sidebar. Desktop-only: only the
/// Tauri shell emits it (the web UI covers the same gesture with its own
/// keyboard binding), so subscribe only under `isTauri()`.
export function onToggleSidebar(handler: () => void): Promise<UnlistenFn> {
    return listenLogged<unknown>(EVENT_TOGGLE_SIDEBAR, () => handler())
}

/// The macOS View menu asked to toggle the commit rail — see `onToggleSidebar`.
export function onToggleCommitRail(handler: () => void): Promise<UnlistenFn> {
    return listenLogged<unknown>(EVENT_TOGGLE_COMMIT_RAIL, () => handler())
}
