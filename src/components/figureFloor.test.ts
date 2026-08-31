import { describe, expect, test } from "bun:test"
import { FALLBACK_LABEL_PX, MIN_LABEL_PX, floorWidth } from "./figureFloor"

describe("floorWidth", () => {
    test("holds a wide diagram at the width its labels stay legible", () => {
        // The audit's ten-node flowchart: 2579.5px natural, 15px labels.
        // 10/15 of natural is 1719.67 -> 1720 after the ceil.
        expect(floorWidth(2579.5, 15)).toBe(1720)
    })

    test("the floor renders labels at no less than the minimum", () => {
        const natural = 2579.5
        const label = 15
        const floor = floorWidth(natural, label) as number
        // Rendered label size scales with the diagram: label * (floor/natural).
        expect(label * (floor / natural)).toBeGreaterThanOrEqual(MIN_LABEL_PX)
    })

    test("a larger authored label permits a smaller floor", () => {
        // A sequence diagram labels at 16px, so it can shrink further than a
        // flowchart of the same width before hitting the same floor.
        expect(floorWidth(780, 16)).toBe(488)
        expect(floorWidth(780, 15)).toBe(520)
    })

    test("never enlarges a diagram past its natural width", () => {
        // Labels already at or below the floor: scaling cannot rescue them,
        // so the floor is the natural width, not something larger.
        expect(floorWidth(400, MIN_LABEL_PX)).toBe(400)
        expect(floorWidth(400, 6)).toBe(400)
    })

    test("never exceeds a FRACTIONAL natural width", () => {
        // Mermaid viewBox widths are routinely fractional, and state diagrams
        // hard-code 10px label text — so this pair is reachable, not academic.
        // Rounding after the clamp would return 2580 here and outrank
        // mermaid's own `max-width: 2579.5px`.
        expect(floorWidth(2579.5, MIN_LABEL_PX)).toBe(2579.5)
        expect(floorWidth(880.4, 8)).toBe(880.4)
        expect(floorWidth(100.5, 15)).toBeLessThanOrEqual(100.5)
    })

    test("rounds the floor up, never a fraction short of the guarantee", () => {
        // 100 * 10/15 = 66.67 -> 67, not 66.
        expect(floorWidth(100, 15)).toBe(67)
    })

    test("returns null when the natural width is not a usable measurement", () => {
        expect(floorWidth(0, 15)).toBeNull()
        expect(floorWidth(-10, 15)).toBeNull()
        expect(floorWidth(Number.NaN, 15)).toBeNull()
        expect(floorWidth(Number.POSITIVE_INFINITY, 15)).toBeNull()
    })

    test("falls back to the engine's own label size when the label is unmeasurable", () => {
        const expected = floorWidth(900, FALLBACK_LABEL_PX)
        expect(floorWidth(900, Number.NaN)).toBe(expected)
        expect(floorWidth(900, 0)).toBe(expected)
        expect(floorWidth(900, -5)).toBe(expected)
    })

    test("honours an explicit minimum label size", () => {
        expect(floorWidth(1000, 20, 10)).toBe(500)
        expect(floorWidth(1000, 20, 20)).toBe(1000)
    })
})
