import { describe, expect, test } from "bun:test"
import type { Extents, Point, ZoomState } from "./figureZoom"
import {
    MAX_SCALE,
    actualSizeState,
    clampScale,
    fitScale,
    fitState,
    panBy,
    pinchFactor,
    zoomAt,
} from "./figureZoom"

// ---- Fixtures ----------------------------------------------------------

/// The lightbox's content area throughout.
const VIEWPORT: Extents = { width: 800, height: 600 }

/// Content overflowing the viewport on both axes, so offsets have travel on
/// both and the clamps are not silently doing the work in anchor tests.
const CONTENT: Extents = { width: 2000, height: 1500 }

const NO_PADDING = 0

function state(scale: number, left: number, top: number): ZoomState {
    return { scale, left, top }
}

/// The figure coordinate currently sitting under `pointer`, per
/// x = (offset + pointer) / scale. Anchored zoom must leave this unchanged.
function anchoredAt(offset: number, pointer: number, scale: number): number {
    return (offset + pointer) / scale
}

// ---- fitScale ----------------------------------------------------------

describe("fitScale", () => {
    test("is constrained by width when width is the tighter axis", () => {
        // 800/2000 = 0.4 against 600/600 = 1.0
        expect(fitScale(VIEWPORT, { width: 2000, height: 600 }, NO_PADDING)).toBeCloseTo(0.4)
    })

    test("is constrained by height when height is the tighter axis", () => {
        // 600/3000 = 0.2 against 800/800 = 1.0
        expect(fitScale(VIEWPORT, { width: 800, height: 3000 }, NO_PADDING)).toBeCloseTo(0.2)
    })

    test("subtracts padding from both sides of each axis", () => {
        // (800 - 40)/2000 = 0.38 against (600 - 40)/600 = 0.933…
        expect(fitScale(VIEWPORT, { width: 2000, height: 600 }, 20)).toBeCloseTo(0.38)
    })

    test("exceeds 1 for content smaller than the viewport", () => {
        expect(fitScale(VIEWPORT, { width: 400, height: 300 }, NO_PADDING)).toBeCloseTo(2)
    })

    test("falls back to actual size for zero-area content", () => {
        expect(fitScale(VIEWPORT, { width: 0, height: 500 }, NO_PADDING)).toBe(1)
        expect(fitScale(VIEWPORT, { width: 500, height: 0 }, NO_PADDING)).toBe(1)
    })

    test("falls back to actual size when padding exceeds the viewport", () => {
        expect(fitScale(VIEWPORT, CONTENT, 500)).toBe(1)
    })
})

// ---- clampScale --------------------------------------------------------

describe("clampScale", () => {
    test("holds at the fit floor when fit is below actual size", () => {
        expect(clampScale(0.1, 0.4)).toBeCloseTo(0.4)
    })

    test("holds at the ceiling", () => {
        expect(clampScale(1000, 0.4)).toBe(MAX_SCALE)
    })

    test("passes a scale that is already inside the range", () => {
        expect(clampScale(0.5, 0.4)).toBeCloseTo(0.5)
    })

    test("floors at actual size, not at fit, when fit exceeds 1", () => {
        // A figure smaller than the viewport has fit > 1; clamping up to it
        // would forbid the actual-size control the requirement mandates.
        expect(clampScale(0.5, 2)).toBe(1)
        expect(clampScale(1.5, 2)).toBeCloseTo(1.5)
    })

    test("collapses a non-finite scale to the floor", () => {
        expect(clampScale(Number.NaN, 0.4)).toBeCloseTo(0.4)
        expect(clampScale(Number.POSITIVE_INFINITY, 0.4)).toBeCloseTo(0.4)
    })
})

// ---- zoomAt ------------------------------------------------------------

describe("zoomAt", () => {
    const START = state(1, 400, 300)

    test.each<[string, Point]>([
        ["the viewport's left edge", { x: 0, y: 0 }],
        ["the viewport's centre", { x: 400, y: 300 }],
        ["the viewport's right edge", { x: 800, y: 600 }],
    ])("holds the figure point under a pointer at %s", (_label, pointer) => {
        const before = {
            x: anchoredAt(START.left, pointer.x, START.scale),
            y: anchoredAt(START.top, pointer.y, START.scale),
        }

        const next = zoomAt(START, 2, pointer, VIEWPORT, CONTENT, NO_PADDING)

        expect(next.scale).toBeCloseTo(2)
        expect(anchoredAt(next.left, pointer.x, next.scale)).toBeCloseTo(before.x)
        expect(anchoredAt(next.top, pointer.y, next.scale)).toBeCloseTo(before.y)
    })

    test("composed with its inverse returns to the original state", () => {
        const pointer = { x: 400, y: 300 }
        const zoomed = zoomAt(START, 2, pointer, VIEWPORT, CONTENT, NO_PADDING)
        const back = zoomAt(zoomed, 0.5, pointer, VIEWPORT, CONTENT, NO_PADDING)

        expect(back.scale).toBeCloseTo(START.scale)
        expect(back.left).toBeCloseTo(START.left)
        expect(back.top).toBeCloseTo(START.top)
    })

    test("bounds the scale rather than compounding past the ceiling", () => {
        let next = state(1, 0, 0)
        for (let i = 0; i < 20; i++) {
            next = zoomAt(next, 2, { x: 0, y: 0 }, VIEWPORT, CONTENT, NO_PADDING)
        }
        expect(next.scale).toBe(MAX_SCALE)
    })

    test("bounds the scale rather than reducing past fit", () => {
        let next = state(1, 0, 0)
        for (let i = 0; i < 20; i++) {
            next = zoomAt(next, 0.5, { x: 0, y: 0 }, VIEWPORT, CONTENT, NO_PADDING)
        }
        expect(next.scale).toBeCloseTo(fitScale(VIEWPORT, CONTENT, NO_PADDING))
    })

    test("holds offsets inside their range when zooming out", () => {
        const wide = state(4, 7200, 5400)
        const next = zoomAt(wide, 0.25, { x: 0, y: 0 }, VIEWPORT, CONTENT, NO_PADDING)

        expect(next.left).toBeGreaterThanOrEqual(0)
        expect(next.top).toBeGreaterThanOrEqual(0)
        expect(next.left).toBeLessThanOrEqual(CONTENT.width * next.scale - VIEWPORT.width)
        expect(next.top).toBeLessThanOrEqual(CONTENT.height * next.scale - VIEWPORT.height)
    })

    test("re-fits from the origin when the current scale carries no anchor", () => {
        const next = zoomAt(state(0, 100, 100), 2, { x: 10, y: 10 }, VIEWPORT, CONTENT, NO_PADDING)

        expect(Number.isFinite(next.scale)).toBe(true)
        expect(next.left).toBe(0)
        expect(next.top).toBe(0)
    })
})

// ---- panBy -------------------------------------------------------------

describe("panBy", () => {
    // At scale 1: 2000 - 800 = 1200 of travel left, 1500 - 600 = 900 up.
    const MAX_LEFT = 1200
    const MAX_TOP = 900

    test("moves the figure against the pointer", () => {
        const next = panBy(state(1, 400, 300), { x: 100, y: 50 }, VIEWPORT, CONTENT)
        expect(next.left).toBe(300)
        expect(next.top).toBe(250)
    })

    test("clamps at the left limit", () => {
        expect(panBy(state(1, 10, 300), { x: 100, y: 0 }, VIEWPORT, CONTENT).left).toBe(0)
    })

    test("clamps at the right limit", () => {
        expect(panBy(state(1, 1150, 300), { x: -100, y: 0 }, VIEWPORT, CONTENT).left).toBe(MAX_LEFT)
    })

    test("clamps at the top limit", () => {
        expect(panBy(state(1, 400, 10), { x: 0, y: 50 }, VIEWPORT, CONTENT).top).toBe(0)
    })

    test("clamps at the bottom limit", () => {
        expect(panBy(state(1, 400, 850), { x: 0, y: -100 }, VIEWPORT, CONTENT).top).toBe(MAX_TOP)
    })

    test("pins both offsets to zero when the figure fits the viewport", () => {
        const next = panBy(state(1, 0, 0), { x: -500, y: -500 }, VIEWPORT, {
            width: 400,
            height: 300,
        })
        expect(next.left).toBe(0)
        expect(next.top).toBe(0)
    })

    test("preserves the scale", () => {
        expect(panBy(state(2.5, 0, 0), { x: 10, y: 10 }, VIEWPORT, CONTENT).scale).toBe(2.5)
    })
})

// ---- pinchFactor -------------------------------------------------------

describe("pinchFactor", () => {
    test("is the ratio of the contacts' separations", () => {
        const previous: [Point, Point] = [
            { x: 0, y: 0 },
            { x: 0, y: 100 },
        ]
        const current: [Point, Point] = [
            { x: 0, y: 0 },
            { x: 0, y: 200 },
        ]
        expect(pinchFactor(previous, current)).toBeCloseTo(2)
        expect(pinchFactor(current, previous)).toBeCloseTo(0.5)
    })

    test("measures diagonally, not per axis", () => {
        const previous: [Point, Point] = [
            { x: 0, y: 0 },
            { x: 3, y: 4 },
        ]
        const current: [Point, Point] = [
            { x: 0, y: 0 },
            { x: 6, y: 8 },
        ]
        expect(pinchFactor(previous, current)).toBeCloseTo(2)
    })

    test("is neutral when the contacts started coincident", () => {
        const coincident: [Point, Point] = [
            { x: 5, y: 5 },
            { x: 5, y: 5 },
        ]
        const apart: [Point, Point] = [
            { x: 0, y: 0 },
            { x: 0, y: 100 },
        ]
        expect(pinchFactor(coincident, apart)).toBe(1)
    })

    test("is neutral when the contacts became coincident", () => {
        const apart: [Point, Point] = [
            { x: 0, y: 0 },
            { x: 0, y: 100 },
        ]
        const coincident: [Point, Point] = [
            { x: 5, y: 5 },
            { x: 5, y: 5 },
        ]
        expect(pinchFactor(apart, coincident)).toBe(1)
    })

    test("is neutral for a non-finite contact", () => {
        const apart: [Point, Point] = [
            { x: 0, y: 0 },
            { x: 0, y: 100 },
        ]
        const broken: [Point, Point] = [
            { x: Number.NaN, y: 0 },
            { x: 0, y: 100 },
        ]
        expect(pinchFactor(apart, broken)).toBe(1)
        expect(pinchFactor(broken, apart)).toBe(1)
    })

    test("never yields a non-finite scale through clampScale", () => {
        const coincident: [Point, Point] = [
            { x: 1, y: 1 },
            { x: 1, y: 1 },
        ]
        const factor = pinchFactor(coincident, coincident)
        expect(Number.isFinite(clampScale(0.4 * factor, 0.4))).toBe(true)
    })
})

// ---- fitState / actualSizeState ----------------------------------------

describe("fitState", () => {
    test("opens at the fit scale with no offset", () => {
        expect(fitState(VIEWPORT, CONTENT, NO_PADDING)).toEqual({
            scale: fitScale(VIEWPORT, CONTENT, NO_PADDING),
            left: 0,
            top: 0,
        })
    })
})

describe("actualSizeState", () => {
    test("reaches actual size from fit", () => {
        const next = actualSizeState(
            fitState(VIEWPORT, CONTENT, NO_PADDING),
            VIEWPORT,
            CONTENT,
            NO_PADDING,
        )
        expect(next.scale).toBeCloseTo(1)
    })

    test("keeps the viewport's centre on the same figure point", () => {
        const start = fitState(VIEWPORT, CONTENT, NO_PADDING)
        const centre = { x: VIEWPORT.width / 2, y: VIEWPORT.height / 2 }
        const before = anchoredAt(start.left, centre.x, start.scale)

        const next = actualSizeState(start, VIEWPORT, CONTENT, NO_PADDING)

        expect(anchoredAt(next.left, centre.x, next.scale)).toBeCloseTo(before)
    })

    test("is defined for a degenerate starting scale", () => {
        const next = actualSizeState(state(0, 0, 0), VIEWPORT, CONTENT, NO_PADDING)
        expect(Number.isFinite(next.scale)).toBe(true)
        expect(next.scale).toBeGreaterThan(0)
    })
})
