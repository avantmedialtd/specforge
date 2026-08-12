import { useEffect, useState } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import {
    getCommitGarden,
    onCacheUpdated,
    onChangeArchived,
    onGraphChanged,
} from "../api"
import type { WorkspaceGarden } from "../types"

/// Fetches the commit garden and refreshes it independently of the rest of the
/// Dashboard. The garden is *today*-scoped and deciduous, so on top of the
/// backend's graph/cache events it re-derives on a local-midnight tick and on
/// window focus — a window left open or backgrounded across midnight resets to
/// the new day without user action, and without recomputing the whole dashboard.
/// Unconditional: no setting gates the garden.
export function useCommitGarden(): WorkspaceGarden[] {
    const [plants, setPlants] = useState<WorkspaceGarden[]>([])

    useEffect(() => {
        let cancelled = false
        const unsubs: UnlistenFn[] = []

        const load = async () => {
            try {
                const next = await getCommitGarden()
                if (!cancelled) setPlants(next)
            } catch {
                // Keep the prior plants on a transient fetch error rather than
                // flashing an empty garden.
            }
        }

        void load()
        ;(async () => {
            const subs = await Promise.all([
                onGraphChanged(() => load()),
                onCacheUpdated(() => load()),
                onChangeArchived(() => load()),
            ])
            if (cancelled) subs.forEach((u) => u())
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
                void load()
                scheduleMidnight()
            }, next.getTime() - now.getTime())
        }
        scheduleMidnight()

        // A window resumed after being backgrounded past midnight self-corrects
        // on focus even if the timer was throttled.
        const onFocus = () => void load()
        window.addEventListener("focus", onFocus)

        return () => {
            cancelled = true
            unsubs.forEach((u) => u())
            clearTimeout(midnight)
            window.removeEventListener("focus", onFocus)
        }
    }, [])

    return plants
}
