import { describe, expect, test } from "bun:test"
import {
    formatLastChanged,
    LAST_CHANGED_WIDEST,
    nextTickDelayMs,
} from "./lastChanged"

const SECOND = 1_000
const MINUTE = 60 * SECOND
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

/// A fixed modification time, with "now" derived from it, so every case below
/// states the elapsed interval it is testing rather than a pair of timestamps.
/// Both helpers keep the arithmetic in integers — no division — so nothing here
/// turns on floating-point round-tripping.
const AT_SECS = 1_700_000_000
const label = (elapsed: number) =>
    formatLastChanged(AT_SECS, AT_SECS * SECOND + elapsed)
const tick = (elapsed: number) =>
    nextTickDelayMs(AT_SECS, AT_SECS * SECOND + elapsed)

describe("formatLastChanged", () => {
    test("reads as the present moment below a minute", () => {
        expect(label(0)).toBe("just now")
        expect(label(MINUTE - 1)).toBe("just now")
    })

    test("crosses into minutes, hours, days, months and years", () => {
        expect(label(MINUTE)).toBe("1 min ago")
        expect(label(HOUR - 1)).toBe("59 min ago")
        expect(label(HOUR)).toBe("1 hr ago")
        expect(label(DAY - 1)).toBe("23 hr ago")
        expect(label(DAY)).toBe("1 day ago")
        expect(label(30 * DAY - 1)).toBe("29 days ago")
        expect(label(30 * DAY)).toBe("1 mo ago")
        expect(label(365 * DAY - 1)).toBe("12 mo ago")
        expect(label(365 * DAY)).toBe("1 yr ago")
    })

    test("says one day, not one days", () => {
        expect(label(DAY)).toBe("1 day ago")
        expect(label(2 * DAY)).toBe("2 days ago")
    })

    test("a modification time in the future reads as the present moment", () => {
        // Clock skew, a restored archive, a network filesystem. `in 4 minutes`
        // would read as a bug in the application rather than a fact about the
        // file (`spec-browser`: *…* — "A modification time in the future is not
        // shown as future").
        expect(label(-4 * MINUTE)).toBe("just now")
        expect(label(-400 * DAY)).toBe("just now")
    })
})

describe("nextTickDelayMs", () => {
    test("is always positive, so a tick can never busy-loop", () => {
        for (const elapsed of [
            -DAY,
            0,
            MINUTE,
            HOUR,
            DAY,
            30 * DAY,
            365 * DAY,
            4000 * DAY,
        ]) {
            expect(tick(elapsed)).toBeGreaterThan(0)
        }
    })

    test("lands exactly on the instant the label changes", () => {
        // The strongest statement of the contract: at the scheduled moment the
        // text is different, and one millisecond earlier it is not. A tick that
        // fired late would leave a stale label on screen; one that fired early
        // would wake the pane to rerender the same words.
        const sweep = [
            0,
            30 * SECOND,
            MINUTE,
            90 * SECOND,
            59 * MINUTE,
            HOUR,
            HOUR + 30 * MINUTE,
            23 * HOUR,
            DAY,
            5 * DAY,
            29 * DAY,
            30 * DAY,
            100 * DAY,
            364 * DAY,
            365 * DAY,
            800 * DAY,
        ]
        for (const elapsed of sweep) {
            const delay = tick(elapsed)
            expect(label(elapsed + delay)).not.toBe(label(elapsed))
            expect(label(elapsed + delay - 1)).toBe(label(elapsed))
        }
    })

    test("does not miss the months-to-years boundary", () => {
        // A year is 365 days and a month here is 30, so the next month-multiple
        // after day 364 is day 390 — 25 days after the label should already have
        // become "1 yr ago". The range bound is what catches this.
        expect(tick(364 * DAY)).toBe(DAY)
        expect(label(364 * DAY)).toBe("12 mo ago")
        expect(label(364 * DAY + DAY)).toBe("1 yr ago")
    })

    test("is never finer than the unit on screen", () => {
        // A pane parked on an old artifact must not wake every few seconds.
        expect(tick(12 * DAY + 6 * HOUR)).toBe(18 * HOUR)
        expect(tick(HOUR + 20 * MINUTE)).toBe(40 * MINUTE)
        expect(tick(90 * DAY)).toBe(30 * DAY)
    })

    test("a future modification time still schedules a real tick", () => {
        expect(tick(-4 * MINUTE)).toBe(MINUTE)
    })
})

describe("LAST_CHANGED_WIDEST", () => {
    test("no label the formatter can produce is wider", () => {
        // Swept densely across every range, including both sides of each
        // boundary. The identity row is monospace, so character count IS the
        // rendered width — which is what lets the header reserve a box the
        // label can never outgrow.
        const widest = LAST_CHANGED_WIDEST.length
        const probes: number[] = [-DAY, 0]
        for (let m = 1; m < 60; m++) probes.push(m * MINUTE, m * MINUTE - 1)
        for (let h = 1; h < 24; h++) probes.push(h * HOUR, h * HOUR - 1)
        for (let d = 1; d < 30; d++) probes.push(d * DAY, d * DAY - 1)
        for (let mo = 1; mo <= 12; mo++) probes.push(mo * 30 * DAY)
        for (let y = 1; y <= 40; y++) probes.push(y * 365 * DAY)

        for (const elapsed of probes) {
            expect(label(elapsed).length).toBeLessThanOrEqual(widest)
        }
    })

    test("is a label the formatter actually produces", () => {
        // A declared maximum nothing can emit would be a reserved box wider
        // than anything that ever fills it.
        expect(label(29 * DAY)).toBe(LAST_CHANGED_WIDEST)
    })

    test("matches the reserved width in App.css", () => {
        // `.identity-changed { min-width: 11ch }` is stated in characters
        // because the row is monospace. Rewording a label must fail here rather
        // than silently shrink the box and let the change name start moving.
        expect(LAST_CHANGED_WIDEST.length).toBe(11)
    })
})
