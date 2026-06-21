import { invoke as tauriInvoke } from "@tauri-apps/api/core"
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event"
import type {
    ArchivedChangeSummary,
    ArtifactReadKind,
    ArtifactStatus,
    Author,
    CacheUpdatedPayload,
    ChangeAddedPayload,
    ChangeArchivedPayload,
    ChangeData,
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
    TreatmentLocker,
    WebServerConfig,
    WorkspaceGarden,
    WorkspaceRemovedPayload,
    WorkspaceView,
} from "./types"
import {
    EVENT_CACHE_UPDATED,
    EVENT_CHANGE_ADDED,
    EVENT_CHANGE_ARCHIVED,
    EVENT_GRAPH_CHANGED,
    EVENT_INSTANCE_ADDED,
    EVENT_INSTANCE_REMOVED,
    EVENT_LOGICAL_CHANGE_ADDED,
    EVENT_LOGICAL_CHANGE_ARCHIVED,
    EVENT_QUOTA_UPDATED,
    EVENT_WORKSPACE_PRESENTATION_UPDATED,
    EVENT_WORKSPACE_REMOVED,
} from "./types"

// Re-exported for call sites that import the artifact-kind union from the
// API surface (the canonical definition lives in ./types).
export type { ArtifactReadKind } from "./types"

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

// One shared EventSource for the web event stream. The server names each SSE
// frame (`event:`) the same as the desktop's Tauri events, so handlers register
// by the identical name.
let sharedEventSource: EventSource | null = null
function webEventSource(): EventSource {
    if (!sharedEventSource) {
        sharedEventSource = new EventSource("/api/events")
    }
    return sharedEventSource
}

function webListen<T>(
    event: string,
    handler: (payload: T) => void,
): Promise<UnlistenFn> {
    const source = webEventSource()
    const listener = (e: MessageEvent) => {
        let payload: T
        try {
            payload = e.data ? (JSON.parse(e.data) as T) : (undefined as T)
        } catch {
            payload = undefined as T
        }
        handler(payload)
    }
    source.addEventListener(event, listener as EventListener)
    const unlisten: UnlistenFn = () =>
        source.removeEventListener(event, listener as EventListener)
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
/// The gamified layer is always resolved to the canonical developer over all
/// available history — there is no audience (Me/Everyone) or time-window
/// (This Season/All Time) selector.
export async function getDashboard(): Promise<DashboardData> {
    return invokeLogged<DashboardData>("get_dashboard")
}

/// The commit garden: one stylized plant per top-level entry, grown from today's
/// commits. Empty when gamification is disabled.
export async function getCommitGarden(): Promise<WorkspaceGarden[]> {
    return invokeLogged<WorkspaceGarden[]>("get_commit_garden")
}

/// Equip a treatment finish by its id (pass null to clear).
export async function setEquippedTreatment(
    treatmentId: string | null,
): Promise<void> {
    return invokeLogged<void>("set_equipped_treatment", { treatmentId })
}

/// The treatment wardrobe (all unlocked finishes + the equipped one) for Settings.
export async function getTreatmentLocker(): Promise<TreatmentLocker> {
    return invokeLogged<TreatmentLocker>("get_treatment_locker")
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
): Promise<string> {
    return invokeLogged<string>("read_artifact", {
        workspace,
        changeId,
        artifactKind,
        capability,
    })
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

export async function getGamificationEnabled(): Promise<boolean> {
    return invokeLogged<boolean>("get_gamification_enabled")
}

export async function setGamificationEnabled(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_gamification_enabled", { enabled })
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
// Events
// -------------------------------------------------------------------------

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
