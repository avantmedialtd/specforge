import { useCallback, useEffect, useState } from "react"
import {
    getChanges,
    listWorkspaces,
    onCacheUpdated,
    onChangeAdded,
    onChangeArchived,
} from "../api"
import type { ChangeData, RegisteredWorkspace } from "../types"

export interface UseWorkspacesResult {
    workspaces: RegisteredWorkspace[]
    changesByWorkspace: Map<string, ChangeData[]>
    /** Force a full refresh of every workspace; useful after register/unregister. */
    refresh: () => Promise<void>
}

export function useWorkspaces(): UseWorkspacesResult {
    const [workspaces, setWorkspaces] = useState<RegisteredWorkspace[]>([])
    const [changesByWorkspace, setChangesByWorkspace] = useState<
        Map<string, ChangeData[]>
    >(new Map())

    const refreshWorkspace = useCallback(async (workspaceUri: string) => {
        try {
            const changes = await getChanges(workspaceUri)
            setChangesByWorkspace((prev) => {
                const next = new Map(prev)
                next.set(workspaceUri, changes)
                return next
            })
        } catch (err) {
            // If a workspace is unregistered while an event is in flight, the
            // command will reject; swallow.
            console.warn(`failed to refresh ${workspaceUri}:`, err)
        }
    }, [])

    const refresh = useCallback(async () => {
        const list = await listWorkspaces()
        setWorkspaces(list)
        const next = new Map<string, ChangeData[]>()
        for (const ws of list) {
            if (!ws.isMissing) {
                try {
                    next.set(ws.uri, await getChanges(ws.uri))
                } catch (err) {
                    console.warn(`failed to load ${ws.uri}:`, err)
                    next.set(ws.uri, [])
                }
            } else {
                next.set(ws.uri, [])
            }
        }
        setChangesByWorkspace(next)
    }, [])

    useEffect(() => {
        let mounted = true
        let cleanup: (() => void) | undefined

        ;(async () => {
            await refresh()
            if (!mounted) return

            const unsubs = await Promise.all([
                onCacheUpdated((payload) => refreshWorkspace(payload.workspace)),
                onChangeAdded((payload) => refreshWorkspace(payload.workspace)),
                onChangeArchived((payload) => refreshWorkspace(payload.workspace)),
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
    }, [refresh, refreshWorkspace])

    return { workspaces, changesByWorkspace, refresh }
}
