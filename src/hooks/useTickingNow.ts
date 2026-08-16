import { useEffect, useState } from "react"
import { nextTickDelayMs } from "../relativeTime"

/// A `Date.now()` that advances exactly when a relative label derived from
/// `unixSeconds` would change.
///
/// Every surface showing a relative time needs this, and for the same reason:
/// the label is stale the instant it renders, and nothing has to happen for it
/// to go wrong — the reader simply sits there. The sidebar's memoized rows stop
/// re-rendering by design; the detail pane's equality guard exists specifically
/// to prevent repaints. Both would otherwise freeze a label for the life of a
/// quiet session.
///
/// A chain of one-shot timeouts rather than a fixed interval, because the right
/// cadence depends on the unit currently displayed. `nextTickDelayMs` lands on
/// the instant the text changes, so a three-week-old row wakes once a week
/// instead of once a minute, and a fresh one is never a minute out of date.
///
/// Re-anchors whenever `unixSeconds` changes: the hook may have been sitting on
/// a week-long timeout when its subject was swapped, and computing the new label
/// from a `now` that old would date it wrongly.
export function useTickingNow(unixSeconds: number): number {
    const [now, setNow] = useState(() => Date.now())

    useEffect(() => {
        let cancelled = false
        let timer: number | undefined
        const anchor = Date.now()
        setNow(anchor)
        const schedule = (at: number) => {
            timer = window.setTimeout(() => {
                if (cancelled) return
                const tickedAt = Date.now()
                setNow(tickedAt)
                schedule(tickedAt)
            }, nextTickDelayMs(unixSeconds, at))
        }
        schedule(anchor)
        // Cleared on unmount and whenever the subject changes, so nothing
        // advances a label describing something no longer on screen
        // (`spec-browser`: *Change Identity Header in the Detail Pane* — "The
        // label stops when the artifact it described is gone").
        return () => {
            cancelled = true
            window.clearTimeout(timer)
        }
    }, [unixSeconds])

    return now
}
