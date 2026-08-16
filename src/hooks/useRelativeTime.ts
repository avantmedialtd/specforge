import { useEffect, useRef, useState } from "react"
import { formatRelativeTime, nextTickDelayMs } from "../relativeTime"

/// How long ago `unixSeconds` was, kept current on its own.
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
/// Returns the formatted string rather than a timestamp, so the one pairing of
/// "tick" with "format" lives here. A caller that needs the text twice — the
/// identity header renders it and repeats it in a tooltip — gets one value that
/// cannot disagree with itself across a tick.
export function useRelativeTime(unixSeconds: number): string {
    const [now, setNow] = useState(() => Date.now())
    // The subject this hook is currently anchored to. `useState`'s initializer
    // already anchored the first render, so re-anchoring on mount would only
    // force a second render pass with a `now` a few milliseconds different —
    // never `Object.is`-equal, so React cannot bail out of it. On a sidebar of
    // hundreds of rows that is hundreds of wasted renders on mount and again on
    // every watcher batch.
    const anchoredTo = useRef(unixSeconds)

    useEffect(() => {
        let cancelled = false
        let timer: number | undefined
        const anchor = Date.now()
        // Re-anchor only when the subject actually changed: the hook may have
        // been sitting on a week-long timeout when it was swapped, and dating
        // the new subject from a `now` that old would be wrong.
        if (anchoredTo.current !== unixSeconds) {
            anchoredTo.current = unixSeconds
            setNow(anchor)
        }
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

    return formatRelativeTime(unixSeconds, now)
}
