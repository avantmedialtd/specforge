import { invoke } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import type {
    ArchivedChangeSummary,
    ArtifactReadKind,
    ArtifactStatus,
    Author,
    CacheUpdatedPayload,
    ChangeAddedPayload,
    ChangeArchivedPayload,
    ChangeData,
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
    EVENT_WORKSPACE_PRESENTATION_UPDATED,
    EVENT_WORKSPACE_REMOVED,
} from "./types"

// Re-exported for call sites that import the artifact-kind union from the
// API surface (the canonical definition lives in ./types).
export type { ArtifactReadKind } from "./types"

// Wraps invoke so every Tauri command logs its name, args, and result/error
// in dev. `import.meta.env.DEV` is constant-folded out of production builds
// so the logging has zero runtime cost there.
async function invokeLogged<T>(
    command: string,
    args?: Record<string, unknown>,
): Promise<T> {
    if (import.meta.env.DEV) {
        console.log(`[api] → ${command}`, args ?? {})
    }
    try {
        const result = await invoke<T>(command, args)
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
    return listen<T>(event, (e) => {
        if (import.meta.env.DEV) {
            console.log(`[event] ${event}`, e.payload)
        }
        handler(e.payload)
    })
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

export async function getNotificationsEnabled(): Promise<boolean> {
    return invokeLogged<boolean>("get_notifications_enabled")
}

export async function setNotificationsEnabled(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_notifications_enabled", { enabled })
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
