import { describe, expect, test } from "bun:test"
import {
    formatRelativeTime,
    nextTickDelayMs,
    RELATIVE_TIME_WIDEST,
} from "./relativeTime"

const SECOND = 1_000
const MINUTE = 60 * SECOND
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR
const WEEK = 7 * DAY
const MONTH = 30 * DAY

/// A fixed subject time, with "now" derived from it, so every case states the
/// elapsed interval it is testing rather than a pair of timestamps. Both helpers
/// keep the arithmetic in integers, so nothing here turns on floating-point
/// round-tripping.
const AT_SECS = 1_700_000_000
const label = (elapsed: number) =>
    formatRelativeTime(AT_SECS, AT_SECS * SECOND + elapsed)
const tick = (elapsed: number) =>
    nextTickDelayMs(AT_SECS, AT_SECS * SECOND + elapsed)

describe("formatRelativeTime", () => {
    test("reads as the present moment below a minute", () => {
        expect(label(0)).toBe("just now")
        expect(label(MINUTE - 1)).toBe("just now")
    })

    test("crosses each tier boundary", () => {
        expect(label(MINUTE)).toBe("1m ago")
        expect(label(HOUR - 1)).toBe("59m ago")
        expect(label(HOUR)).toBe("1h ago")
        expect(label(DAY - 1)).toBe("23h ago")
        expect(label(DAY)).toBe("1d ago")
        expect(label(WEEK - 1)).toBe("6d ago")
        expect(label(WEEK)).toBe("1w ago")
        expect(label(MONTH - 1)).toBe("4w ago")
        expect(label(MONTH)).toBe("1mo ago")
        expect(label(12 * MONTH)).toBe("12mo ago")
    })

    test("a time in the future reads as the present moment", () => {
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
        for (const elapsed of [-DAY, 0, MINUTE, HOUR, DAY, WEEK, MONTH, 4000 * DAY]) {
            expect(tick(elapsed)).toBeGreaterThan(0)
        }
    })

    test("lands exactly on the instant the label changes", () => {
        // The strongest statement of the contract: at the scheduled moment the
        // text is different, and one millisecond earlier it is not. A tick that
        // fired late would leave a stale label on screen; one that fired early
        // would wake the surface to render the same words again.
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
            3 * DAY,
            6 * DAY,
            WEEK,
            2 * WEEK,
            29 * DAY,
            MONTH,
            3 * MONTH,
            40 * MONTH,
        ]
        for (const elapsed of sweep) {
            const delay = tick(elapsed)
            expect(label(elapsed + delay)).not.toBe(label(elapsed))
            expect(label(elapsed + delay - 1)).toBe(label(elapsed))
        }
    })

    test("does not miss the weeks-to-months boundary", () => {
        // A 30-day month is not a whole number of 7-day weeks, so the next week
        // multiple after day 29 is day 35 — five days after the label should
        // already have become "1mo ago". The tier bound is what catches this.
        expect(tick(29 * DAY)).toBe(DAY)
        expect(label(29 * DAY)).toBe("4w ago")
        expect(label(30 * DAY)).toBe("1mo ago")
    })

    test("is never finer than the unit on screen", () => {
        // A surface parked on an old row must not wake every few seconds.
        expect(tick(HOUR + 20 * MINUTE)).toBe(40 * MINUTE)
        expect(tick(3 * DAY + 6 * HOUR)).toBe(18 * HOUR)
        expect(tick(2 * WEEK + DAY)).toBe(WEEK - DAY)
        expect(tick(3 * MONTH + 5 * DAY)).toBe(MONTH - 5 * DAY)
    })

    test("a future time still schedules a real tick", () => {
        expect(tick(-4 * MINUTE)).toBe(MINUTE)
    })
})

describe("RELATIVE_TIME_WIDEST", () => {
    test("no label the formatter can produce is wider, up to 83 years", () => {
        // Swept densely across every tier, including both sides of each
        // boundary. The surfaces that render this are monospace, so character
        // count IS the rendered width — which is what lets the identity header
        // reserve a box its label can never outgrow.
        const widest = RELATIVE_TIME_WIDEST.length
        const probes: number[] = [-DAY, 0]
        for (let m = 1; m < 60; m++) probes.push(m * MINUTE, m * MINUTE - 1)
        for (let h = 1; h < 24; h++) probes.push(h * HOUR, h * HOUR - 1)
        for (let d = 1; d < 7; d++) probes.push(d * DAY, d * DAY - 1)
        for (let w = 1; w < 5; w++) probes.push(w * WEEK, w * WEEK - 1)
        for (let mo = 1; mo <= 999; mo++) probes.push(mo * MONTH)

        for (const elapsed of probes) {
            expect(label(elapsed).length).toBeLessThanOrEqual(widest)
        }
    })

    test("is a label the formatter actually produces", () => {
        // A declared maximum nothing can emit would be a reserved box wider than
        // anything that ever fills it.
        expect(label(999 * MONTH)).toBe(RELATIVE_TIME_WIDEST)
    })

    test("matches the reserved width the identity header applies", () => {
        // `DetailPane` sets `min-width: ${RELATIVE_TIME_WIDEST.length}ch`, so
        // this pins the box to the vocabulary. Rewording a label fails here
        // rather than silently letting the change name start moving on a tick.
        expect(RELATIVE_TIME_WIDEST.length).toBe(9)
    })
})
