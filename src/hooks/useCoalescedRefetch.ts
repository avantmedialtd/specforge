import { useCallback, useEffect, useRef } from "react"

/**
 * Wraps an async refetch function so any number of calls to the returned
 * `schedule()` collapse into a single refetch, at two levels:
 *
 * - Within one leading-edge window: `schedule()` is idempotent, guarded by
 *   a "scheduled" flag — the first call starts a zero-delay timer that runs
 *   `fn` once; further calls before that timer fires are no-ops.
 * - Across an in-flight call: if `schedule()` is called again while a
 *   previous invocation of `fn` is still running, no second concurrent call
 *   starts. Instead a "run again when done" flag is set, and exactly one
 *   follow-up call happens after the in-flight one settles.
 *
 * Why `setTimeout(fn, 0)` and not `queueMicrotask`/`requestAnimationFrame`:
 * Tauri (2.11.2, verified) delivers each event listener callback via its own
 * `evaluateJavaScript` injection with its own microtask checkpoint — event
 * #1's `queueMicrotask` callback runs to completion *before* event #2's
 * script even starts, so a microtask leading edge does not actually coalesce
 * same-batch events (a burst of N events would still trigger up to N
 * refetches). A macrotask does: every event from one backend-debounced batch
 * lands within the same JS task-queue window well under a task boundary, so
 * `setTimeout(fn, 0)` reliably catches the whole batch. `requestAnimationFrame`
 * would stall the refetch entirely while the window is hidden — this app's
 * window is frequently closed to the tray, so a refetch must not depend on a
 * frame ever being scheduled.
 */
export function useCoalescedRefetch(fn: () => Promise<void>): () => void {
    const fnRef = useRef(fn)
    // Not a render-phase side effect concern in practice for a plain ref
    // write, but keeping it in an effect (rather than the render body) is
    // the more conventionally-correct place for a "hold the latest callback
    // for use in an event handler" ref update.
    useEffect(() => {
        fnRef.current = fn
    })

    const scheduled = useRef(false)
    const inFlight = useRef(false)
    const runAgain = useRef(false)

    const runOnce = useCallback(() => {
        inFlight.current = true
        void fnRef.current().finally(() => {
            inFlight.current = false
            if (runAgain.current) {
                runAgain.current = false
                runOnce()
            }
        })
    }, [])

    const schedule = useCallback(() => {
        if (inFlight.current) {
            runAgain.current = true
            return
        }
        if (scheduled.current) return
        scheduled.current = true
        setTimeout(() => {
            scheduled.current = false
            runOnce()
        }, 0)
    }, [runOnce])

    return schedule
}
