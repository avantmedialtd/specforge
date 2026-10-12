import { useCallback, useEffect, useState } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import { getCommitGraph, onGraphChanged } from "../api"
import { createRequestSequence, readLatest } from "../refreshSignals"
import type { CommitGraph } from "../types"
import { useCoalescedRefetch } from "./useCoalescedRefetch"
import { useLatest } from "./useLatest"

export interface UseCommitGraphResult {
    graph: CommitGraph | null
    loading: boolean
    error: string | null
}

/// Fetches the commit graph for `repoId` (null → no fetch, empty rail) and
/// refetches whenever the backend reports that repository's refs moved. The
/// `limit` is the window size; growing it (a "load more" click) re-runs the
/// fetch with the larger cap.
///
/// Every read, the first one and each live refresh, goes through one
/// coalesced scheduler: a burst of ref-change notices during a read costs
/// exactly one read after it. Only the newest read's reply is applied, and
/// none once the repository or window it was for is no longer shown, so a
/// slow older reply never replaces a newer graph. The loading state is only
/// for a rail with no graph yet: a refresh keeps the current graph on screen
/// (`commit-graph`: *Overlapping Graph Refreshes Settle on the Newest*).
export function useCommitGraph(
    repoId: string | null,
    limit: number,
): UseCommitGraphResult {
    const [graph, setGraph] = useState<CommitGraph | null>(null)
    const [loading, setLoading] = useState(false)
    const [error, setError] = useState<string | null>(null)

    // The scheduler below is made once, so its load reads what it fetches,
    // and whether a graph is held, as of the latest render.
    const shown = useLatest({ repoId, limit, graph })
    const [sequence] = useState(createRequestSequence)

    const load = useCallback(async () => {
        const { repoId: id, limit: size, graph: held } = shown.current
        if (!id) return
        if (held === null) setLoading(true)
        await readLatest(
            sequence,
            () => getCommitGraph(id, size),
            (next) => {
                setGraph(next)
                setError(null)
                setLoading(false)
            },
            (err) => {
                setError(String(err))
                setGraph(null)
                setLoading(false)
            },
        )
    }, [shown, sequence])
    const schedule = useCoalescedRefetch(load)

    useEffect(() => {
        if (!repoId) {
            sequence.cancel()
            setGraph(null)
            setError(null)
            setLoading(false)
            return
        }

        let active = true
        let unsub: UnlistenFn | undefined

        // Set here, in the commit that shows the repository, rather than when
        // the scheduled read starts a task later: a rail with no graph must
        // show its loading state from its first frame, never "No commits".
        if (shown.current.graph === null) setLoading(true)
        schedule()

        // Live refresh: re-fetch when THIS repo's refs change. The backend
        // already debounces ref events, and the scheduler coalesces what
        // still arrives in a burst.
        void onGraphChanged((payload) => {
            if (payload.repoId === repoId) schedule()
        }).then((u) => {
            if (active) unsub = u
            else u()
        })

        return () => {
            active = false
            // A read still in flight was for this repository and window.
            sequence.cancel()
            unsub?.()
        }
    }, [repoId, limit, schedule, sequence, shown])

    return { graph, loading, error }
}
