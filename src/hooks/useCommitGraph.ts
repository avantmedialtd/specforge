import { useEffect, useState } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import { getCommitGraph, onGraphChanged } from "../api"
import type { CommitGraph } from "../types"

export interface UseCommitGraphResult {
    graph: CommitGraph | null
    loading: boolean
    error: string | null
}

/// Fetches the commit graph for `repoId` (null → no fetch, empty rail) and
/// refetches whenever the backend reports that repository's refs moved. The
/// `limit` is the window size; growing it (a "load more" click) re-runs the
/// fetch with the larger cap.
export function useCommitGraph(
    repoId: string | null,
    limit: number,
): UseCommitGraphResult {
    const [graph, setGraph] = useState<CommitGraph | null>(null)
    const [loading, setLoading] = useState(false)
    const [error, setError] = useState<string | null>(null)

    useEffect(() => {
        if (!repoId) {
            setGraph(null)
            setError(null)
            setLoading(false)
            return
        }

        let cancelled = false
        let unsub: UnlistenFn | undefined

        const load = async () => {
            setLoading(true)
            try {
                const next = await getCommitGraph(repoId, limit)
                if (!cancelled) {
                    setGraph(next)
                    setError(null)
                }
            } catch (err) {
                if (!cancelled) {
                    setError(String(err))
                    setGraph(null)
                }
            } finally {
                if (!cancelled) setLoading(false)
            }
        }

        void load()

        // Live refresh: re-fetch when THIS repo's refs change. The backend
        // already debounces ref events, so this isn't a hot loop.
        void onGraphChanged((payload) => {
            if (payload.repoId === repoId) void load()
        }).then((u) => {
            if (cancelled) u()
            else unsub = u
        })

        return () => {
            cancelled = true
            unsub?.()
        }
    }, [repoId, limit])

    return { graph, loading, error }
}
