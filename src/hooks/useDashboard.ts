import { useEffect, useState } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import {
    getDashboard,
    onCacheUpdated,
    onChangeAdded,
    onChangeArchived,
    onGraphChanged,
} from "../api"
import type { DashboardData, DashboardScope } from "../types"

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
/// The backend already debounces these events, so this isn't a hot loop.
export function useDashboard(scope: DashboardScope = "me"): UseDashboardResult {
    const [data, setData] = useState<DashboardData | null>(null)
    const [loading, setLoading] = useState(false)
    const [error, setError] = useState<string | null>(null)

    useEffect(() => {
        let cancelled = false
        const unsubs: UnlistenFn[] = []

        const load = async () => {
            setLoading(true)
            try {
                const next = await getDashboard(scope)
                if (!cancelled) {
                    setData(next)
                    setError(null)
                }
            } catch (err) {
                if (!cancelled) setError(String(err))
            } finally {
                if (!cancelled) setLoading(false)
            }
        }

        void load()
        ;(async () => {
            const subs = await Promise.all([
                onCacheUpdated(() => load()),
                onChangeAdded(() => load()),
                onChangeArchived(() => load()),
                onGraphChanged(() => load()),
            ])
            if (cancelled) subs.forEach((u) => u())
            else unsubs.push(...subs)
        })()

        return () => {
            cancelled = true
            unsubs.forEach((u) => u())
        }
        // Refetch when the scope changes so Me/Everyone swap recomputes.
    }, [scope])

    return { data, loading, error }
}
