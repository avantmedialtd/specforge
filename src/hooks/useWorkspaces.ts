import { useCallback, useEffect, useState } from "react"
import {
    getWorkspaceViews,
    listWorkspaces,
    onCacheUpdated,
    onChangeAdded,
    onChangeArchived,
    onInstanceAdded,
    onInstanceRemoved,
    onLogicalChangeAdded,
    onLogicalChangeArchived,
    onWorkspacePresentationUpdated,
    onWorkspaceRemoved,
} from "../api"
import type { RegisteredWorkspace, WorkspaceView } from "../types"

export interface UseWorkspacesResult {
    /** User-registered workspaces for the Settings UI. Discovered worktrees
     * do not appear here — Settings shows only the manageable set. */
    workspaces: RegisteredWorkspace[]
    /** Tree-pane source of truth. One entry per top-level repo group or
     * non-git workspace. Includes auto-discovered worktrees aggregated under
     * their parent repo. */
    views: WorkspaceView[]
    /** Force a full refresh; useful after register/unregister. */
    refresh: () => Promise<void>
}

export function useWorkspaces(): UseWorkspacesResult {
    const [workspaces, setWorkspaces] = useState<RegisteredWorkspace[]>([])
    const [views, setViews] = useState<WorkspaceView[]>([])

    const refreshViews = useCallback(async () => {
        try {
            const next = await getWorkspaceViews()
            setViews(next)
        } catch (err) {
            console.warn("failed to refresh workspace views:", err)
        }
    }, [])

    const refresh = useCallback(async () => {
        const [list, next] = await Promise.all([
            listWorkspaces(),
            getWorkspaceViews(),
        ])
        setWorkspaces(list)
        setViews(next)
    }, [])

    useEffect(() => {
        let mounted = true
        let cleanup: (() => void) | undefined

        ;(async () => {
            await refresh()
            if (!mounted) return

            // Subscribe to every cache event the backend emits. Any of them
            // implies the aggregated view may have changed, so we refetch.
            // The backend already debounces, so this isn't a hot loop.
            const unsubs = await Promise.all([
                onCacheUpdated(() => refreshViews()),
                onChangeAdded(() => refreshViews()),
                onChangeArchived(() => refreshViews()),
                onWorkspaceRemoved(() => refreshViews()),
                onLogicalChangeAdded(() => refreshViews()),
                onLogicalChangeArchived(() => refreshViews()),
                onInstanceAdded(() => refreshViews()),
                onInstanceRemoved(() => refreshViews()),
                // Presentation changes (rename / recolour) need both the
                // Settings workspace list AND the tree views to refresh, so
                // re-run the full refresh rather than just refreshViews.
                onWorkspacePresentationUpdated(() => {
                    void refresh()
                }),
            ])

            if (!mounted) {
                unsubs.forEach((u) => u())
                return
            }
            cleanup = () => unsubs.forEach((u) => u())
        })()

        return () => {
            mounted = false
            cleanup?.()
        }
    }, [refresh, refreshViews])

    return { workspaces, views, refresh }
}
