import { useCallback, useEffect, useRef, useState } from "react"
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
import { createSignal, type Subscribe } from "../refreshSignals"
import { shareStructure } from "../structuralShare"
import type { RegisteredWorkspace, WorkspaceView } from "../types"
import { useCoalescedRefetch } from "./useCoalescedRefetch"

export interface UseWorkspacesResult {
    /** User-registered workspaces for the Settings UI. Discovered worktrees
     * do not appear here — Settings shows only the manageable set. */
    workspaces: RegisteredWorkspace[]
    /** Tree-pane source of truth. One entry per top-level repo group or
     * non-git workspace. Includes auto-discovered worktrees aggregated under
     * their parent repo. */
    views: WorkspaceView[]
    /** True until the first fetch (register or otherwise) has completed —
     * distinguishes "no workspaces registered" from "haven't asked yet",
     * which cold-load address resolution needs to know whether to render a
     * pending state rather than resolving against an empty `views` (`view-
     * routing`: *Cold-Load Address Resolution*). Never flips back to true. */
    loading: boolean
    /** Force a full refresh; useful after register/unregister. */
    refresh: () => Promise<void>
    /** Subscribes a listener called after every successful views read,
     * whether or not the read changed anything, and returns its unsubscribe.
     * Stable for the hook's life. Structural sharing keeps `views`' identity
     * across an equal read, so a consumer that must act on every read — the
     * links re-read (`pull-request-worktree-links`: *The Pull-Request Links
     * Snapshot*) — listens here rather than keying on `views`. */
    onViewsRead: Subscribe
}

export function useWorkspaces(): UseWorkspacesResult {
    const [workspaces, setWorkspaces] = useState<RegisteredWorkspace[]>([])
    const [views, setViews] = useState<WorkspaceView[]>([])
    const [loading, setLoading] = useState(true)

    // Last request wins. Both reads are async commands, so two overlapping
    // requests can answer out of order; a response is applied only if no
    // newer request for the same state was issued after it.
    const viewsRequest = useRef(0)
    const listRequest = useRef(0)

    // Fired after every successful views read; see `onViewsRead`.
    const [viewsRead] = useState(createSignal)

    // Every reply goes through `shareStructure`, so a read that changes
    // nothing keeps the held array (and `setState` bails out), and a read
    // that changes one repository keeps every other repository's objects —
    // what lets the tree's memoized rows and `App`'s views-keyed memos skip
    // (`spec-browser`: *Unchanged Workspace Views Keep Their Rows*).
    const refreshViews = useCallback(async () => {
        const seq = ++viewsRequest.current
        try {
            const next = await getWorkspaceViews()
            if (seq === viewsRequest.current) setViews((prev) => shareStructure(prev, next))
        } catch (err) {
            console.warn("failed to refresh workspace views:", err)
            return
        }
        viewsRead.fire()
    }, [viewsRead])

    const refresh = useCallback(async () => {
        const viewsSeq = ++viewsRequest.current
        const listSeq = ++listRequest.current
        const [list, next] = await Promise.all([
            listWorkspaces(),
            getWorkspaceViews(),
        ])
        if (listSeq === listRequest.current) setWorkspaces((prev) => shareStructure(prev, list))
        if (viewsSeq === viewsRequest.current) setViews((prev) => shareStructure(prev, next))
        viewsRead.fire()
    }, [viewsRead])

    // Every listener below funnels through one of these two coalesced
    // schedulers rather than calling refreshViews/refresh directly. The
    // backend debounces *filesystem events into one batch*, but a single
    // batch still emits several distinct CacheEvents (e.g. an archival fires
    // ChangeArchived + Updated + a derived logical-change event), each
    // subscribed below — without coalescing, one user action would trigger
    // several redundant getWorkspaceViews() round trips.
    const scheduleViewsRefresh = useCoalescedRefetch(refreshViews)
    const scheduleFullRefresh = useCoalescedRefetch(refresh)

    useEffect(() => {
        let mounted = true
        let cleanup: (() => void) | undefined

        void (async () => {
            try {
                await refresh()
            } finally {
                // Always flips, even on a rejected first fetch — the tree
                // (and address resolution) must become interactive rather
                // than stay pending forever, mirroring how the tree's own
                // hydration effect degrades on an unreadable read.
                if (mounted) setLoading(false)
            }
            if (!mounted) return

            // Subscribe to every cache event the backend emits. Any of them
            // implies the aggregated view may have changed, so we refetch —
            // coalesced via scheduleViewsRefresh so a multi-event batch
            // produces one getWorkspaceViews() call, not one per event.
            const unsubs = await Promise.all([
                onCacheUpdated(() => scheduleViewsRefresh()),
                onChangeAdded(() => scheduleViewsRefresh()),
                onChangeArchived(() => scheduleViewsRefresh()),
                onWorkspaceRemoved(() => scheduleViewsRefresh()),
                onLogicalChangeAdded(() => scheduleViewsRefresh()),
                onLogicalChangeArchived(() => scheduleViewsRefresh()),
                onInstanceAdded(() => scheduleViewsRefresh()),
                onInstanceRemoved(() => scheduleViewsRefresh()),
                // Presentation changes (rename / recolour) need both the
                // Settings workspace list AND the tree views to refresh, so
                // this one schedules the full refresh() instead of
                // refreshViews() — coalesced on its own scheduler so it
                // never gets skipped because a views-only refresh happened
                // to be in flight at the same moment.
                onWorkspacePresentationUpdated(() => scheduleFullRefresh()),
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
    }, [refresh, scheduleViewsRefresh, scheduleFullRefresh])

    return { workspaces, views, loading, refresh, onViewsRead: viewsRead.subscribe }
}
