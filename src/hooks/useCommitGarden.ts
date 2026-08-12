import { useCallback, useEffect, useRef, useState } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import {
    getCommitGarden,
    onCacheUpdated,
    onChangeArchived,
    onGraphChanged,
} from "../api"
import { useCoalescedRefetch } from "./useCoalescedRefetch"
import type { WorkspaceGarden } from "../types"

/// Fetches the commit garden and refreshes it independently of the rest of the
/// Dashboard. The garden is *today*-scoped and deciduous, so on top of the
/// backend's graph/cache events it re-derives on a local-midnight tick and on
/// window focus — a window left open or backgrounded across midnight resets to
/// the new day without user action, and without recomputing the whole dashboard.
/// Unconditional: no setting gates the garden.
///
/// Every trigger routes through `useCoalescedRefetch` rather than calling
/// `load()` directly, for the same reason `useDashboard` does: one backend
/// batch (an archival, say) emits several distinct CacheEvents, each subscribed
/// below, and each `getCommitGarden()` runs a bounded `git log` per registered
/// repository. Called directly, one archival would start three concurrent round
/// trips — and if the first-issued response resolved last, its `setPlants`
/// would overwrite the fresher result, leaving "Today's commits" showing the
/// pre-archive graph until the next event.
export function useCommitGarden(): WorkspaceGarden[] {
    const [plants, setPlants] = useState<WorkspaceGarden[]>([])
    // `load` and `scheduleLoad` must live at the hook's top level, not inside
    // the effect — `useCoalescedRefetch` calls hooks internally, and hooks can
    // only be called during render. `cancelledRef` therefore replaces what
    // would otherwise be an effect-local `cancelled` closure variable.
    const cancelledRef = useRef(false)

    const load = useCallback(async () => {
        try {
            const next = await getCommitGarden()
            if (!cancelledRef.current) setPlants(next)
        } catch {
            // Keep the prior plants on a transient fetch error rather than
            // flashing an empty garden.
        }
    }, [])
    const scheduleLoad = useCoalescedRefetch(load)

    useEffect(() => {
        // Reset for every mount — StrictMode's dev-only mount/unmount/remount
        // cycle reuses the same ref instance, so without this a remount would
        // inherit `true` from the previous cleanup and never apply a result.
        cancelledRef.current = false
        const unsubs: UnlistenFn[] = []

        scheduleLoad()
        ;(async () => {
            const subs = await Promise.all([
                onGraphChanged(() => scheduleLoad()),
                onCacheUpdated(() => scheduleLoad()),
                onChangeArchived(() => scheduleLoad()),
            ])
            if (cancelledRef.current) subs.forEach((u) => u())
            else unsubs.push(...subs)
        })()

        // Re-derive when the day rolls over: a one-shot timer to just past the
        // next local midnight that reloads and reschedules itself.
        let midnight: ReturnType<typeof setTimeout>
        const scheduleMidnight = () => {
            const now = new Date()
            const next = new Date(
                now.getFullYear(),
                now.getMonth(),
                now.getDate() + 1,
                0,
                0,
                5,
            )
            midnight = setTimeout(() => {
                scheduleLoad()
                scheduleMidnight()
            }, next.getTime() - now.getTime())
        }
        scheduleMidnight()

        // A window resumed after being backgrounded past midnight self-corrects
        // on focus even if the timer was throttled.
        const onFocus = () => scheduleLoad()
        window.addEventListener("focus", onFocus)

        return () => {
            cancelledRef.current = true
            unsubs.forEach((u) => u())
            clearTimeout(midnight)
            window.removeEventListener("focus", onFocus)
        }
    }, [load, scheduleLoad])

    return plants
}
