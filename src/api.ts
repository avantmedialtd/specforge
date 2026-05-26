import { invoke } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import type {
    CacheUpdatedPayload,
    ChangeAddedPayload,
    ChangeArchivedPayload,
    ChangeData,
    InstancePayload,
    LogicalChangePayload,
    PaletteColor,
    RegisteredWorkspace,
    WorkspaceRemovedPayload,
    WorkspaceView,
} from "./types"
import {
    EVENT_CACHE_UPDATED,
    EVENT_CHANGE_ADDED,
    EVENT_CHANGE_ARCHIVED,
    EVENT_INSTANCE_ADDED,
    EVENT_INSTANCE_REMOVED,
    EVENT_LOGICAL_CHANGE_ADDED,
    EVENT_LOGICAL_CHANGE_ARCHIVED,
    EVENT_WORKSPACE_PRESENTATION_UPDATED,
    EVENT_WORKSPACE_REMOVED,
} from "./types"

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

export async function getWorkspaceViews(): Promise<WorkspaceView[]> {
    return invokeLogged<WorkspaceView[]>("get_workspace_views")
}

export async function getActiveCount(): Promise<number> {
    return invokeLogged<number>("get_active_count")
}

export type ArtifactReadKind = "proposal" | "design" | "tasks" | "spec"

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

export async function getLaunchOnLogin(): Promise<boolean> {
    return invokeLogged<boolean>("get_launch_on_login")
}

export async function setLaunchOnLogin(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_launch_on_login", { enabled })
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
