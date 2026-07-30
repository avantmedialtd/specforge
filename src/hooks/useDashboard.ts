import { useCallback, useEffect, useRef, useState } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import {
    getDashboard,
    onCacheUpdated,
    onChangeAdded,
    onChangeArchived,
    onGraphChanged,
} from "../api"
import type { DashboardData } from "../types"
import { useCoalescedRefetch } from "./useCoalescedRefetch"

export interface UseDashboardResult {
    data: DashboardData | null
    loading: boolean
    error: string | null
}

/// Fetches the global Dashboard payload and refetches whenever the backend
/// reports cache or graph activity. The hook is only mounted while the
/// Dashboard is the active surface (App renders `DashboardView` solely then),
/// so its subscriptions are torn down the moment the user navigates away —
/// the "refresh while shown" contract falls out of the component lifecycle.
/// The backend debounces *filesystem events into one batch*, not into one
/// event per batch — a single batch (e.g. an archival) still emits several
/// distinct CacheEvents, each subscribed below, so `load()` is scheduled
/// through `useCoalescedRefetch` rather than called directly: it collapses
/// same-batch events into one `getDashboard()` call and de-dupes overlapping
/// batches so at most one follow-up runs after an in-flight load settles.
export function useDashboard(): UseDashboardResult {
    const [data, setData] = useState<DashboardData | null>(null)
    const [loading, setLoading] = useState(false)
    const [error, setError] = useState<string | null>(null)
    // `load` and `scheduleLoad` must live at the hook's top level (not inside
    // the effect below) — `useCoalescedRefetch` calls hooks internally, and
    // hooks can only be called during render, never from inside an effect
    // callback. `cancelledRef` replaces the old effect-local `cancelled`
    // closure variable so `load` can still guard its setState calls after
    // teardown despite no longer closing over the effect body directly.
    const cancelledRef = useRef(false)

    const load = useCallback(async () => {
        setLoading(true)
        try {
            const next = await getDashboard()
            if (!cancelledRef.current) {
                setData(next)
                setError(null)
            }
        } catch (err) {
            if (!cancelledRef.current) setError(String(err))
        } finally {
            if (!cancelledRef.current) setLoading(false)
        }
    }, [])
    const scheduleLoad = useCoalescedRefetch(load)

    useEffect(() => {
        // Reset for every mount — StrictMode's dev-only mount/unmount/remount
        // cycle reuses the same ref instance, so without this a remount would
        // inherit `true` from the previous cleanup and `load` would never
        // apply its result again.
        cancelledRef.current = false
        const unsubs: UnlistenFn[] = []

        // Routed through the same scheduler as every event listener below
        // (not a direct `void load()`) so the in-flight tracking actually
        // sees this call: a direct call the coalescer doesn't know about
        // would let an event arriving during its ~639ms round trip start a
        // second, truly concurrent `getDashboard()` — and if that second
        // (fresher) response resolves before the first, the first's
        // `setData` would overwrite it with stale data on its own resolve.
        scheduleLoad()
        ;(async () => {
            const subs = await Promise.all([
                onCacheUpdated(() => scheduleLoad()),
                onChangeAdded(() => scheduleLoad()),
                onChangeArchived(() => scheduleLoad()),
                onGraphChanged(() => scheduleLoad()),
            ])
            if (cancelledRef.current) subs.forEach((u) => u())
            else unsubs.push(...subs)
        })()

        return () => {
            cancelledRef.current = true
            unsubs.forEach((u) => u())
        }
    }, [load, scheduleLoad])

    return { data, loading, error }
}
