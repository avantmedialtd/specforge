import { describe, expect, test } from "bun:test"
import {
    IMAGE_MAX_SCALE,
    openingState,
    pinchWheelFactor,
    sharpPixels,
    unionExtents,
    zoomKeyOf,
} from "./imageZoom"

const MAC = "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_6) AppleWebKit/605.1.15"
const WINDOWS = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"

describe("pinchWheelFactor", () => {
    test("a pinch outward zooms in and inward zooms out, in proportion", () => {
        expect(pinchWheelFactor(-10, 0)).toBeCloseTo(Math.exp(0.1))
        expect(pinchWheelFactor(10, 0)).toBeCloseTo(Math.exp(-0.1))
        expect(pinchWheelFactor(-10, 0) * pinchWheelFactor(10, 0)).toBeCloseTo(1)
        expect(pinchWheelFactor(0, 0)).toBe(1)
    })

    test("lines and pages count as pixels, and one event moves at most 50", () => {
        expect(pinchWheelFactor(-1, 1)).toBeCloseTo(Math.exp(0.16))
        expect(pinchWheelFactor(-1, 2)).toBeCloseTo(Math.exp(0.5))
        expect(pinchWheelFactor(-1000, 0)).toBeCloseTo(Math.exp(0.5))
        expect(pinchWheelFactor(1000, 0)).toBeCloseTo(Math.exp(-0.5))
        expect(pinchWheelFactor(Number.NaN, 0)).toBe(1)
    })
})

describe("zoomKeyOf", () => {
    const press = (key: string, modifiers: Partial<Record<"metaKey" | "ctrlKey" | "altKey", boolean>>) => ({
        key,
        metaKey: false,
        ctrlKey: false,
        altKey: false,
        ...modifiers,
    })

    test("Command on macOS, as Preview binds the keys", () => {
        expect(zoomKeyOf(press("=", { metaKey: true }), MAC)).toBe("in")
        expect(zoomKeyOf(press("+", { metaKey: true }), MAC)).toBe("in")
        expect(zoomKeyOf(press("-", { metaKey: true }), MAC)).toBe("out")
        expect(zoomKeyOf(press("_", { metaKey: true }), MAC)).toBe("out")
        expect(zoomKeyOf(press("0", { metaKey: true }), MAC)).toBe("actual")
        expect(zoomKeyOf(press("9", { metaKey: true }), MAC)).toBe("fit")
        expect(zoomKeyOf(press("=", { ctrlKey: true }), MAC)).toBeNull()
        expect(zoomKeyOf(press("=", { metaKey: true, altKey: true }), MAC)).toBeNull()
    })

    test("Control elsewhere, and no zoom key without it", () => {
        expect(zoomKeyOf(press("=", { ctrlKey: true }), WINDOWS)).toBe("in")
        expect(zoomKeyOf(press("0", { ctrlKey: true }), WINDOWS)).toBe("actual")
        expect(zoomKeyOf(press("=", { metaKey: true }), WINDOWS)).toBeNull()
        expect(zoomKeyOf(press("=", {}), MAC)).toBeNull()
        expect(zoomKeyOf(press("w", { metaKey: true }), MAC)).toBeNull()
    })
})

describe("unionExtents", () => {
    test("holds every version, aligned at the top-left", () => {
        expect(
            unionExtents([
                { width: 393, height: 13584 },
                { width: 400, height: 13537 },
            ]),
        ).toEqual({ width: 400, height: 13584 })
        expect(unionExtents([{ width: 16, height: 16 }])).toEqual({ width: 16, height: 16 })
        expect(unionExtents([])).toEqual({ width: 0, height: 0 })
    })
})

describe("openingState", () => {
    const viewport = { width: 600, height: 800 }

    test("fits a large version wholly inside the frame", () => {
        const opening = openingState(viewport, { width: 393, height: 13584 }, 24)
        expect(opening.scale).toBeCloseTo((800 - 48) / 13584)
        expect([opening.left, opening.top]).toEqual([0, 0])
    })

    test("holds a tiny icon at the ceiling rather than past it", () => {
        expect(IMAGE_MAX_SCALE).toBe(32)
        expect(openingState(viewport, { width: 16, height: 16 }, 24).scale).toBe(32)
        expect(openingState(viewport, { width: 64, height: 64 }, 24).scale).toBeCloseTo(552 / 64)
    })
})

describe("sharpPixels", () => {
    test("only above actual size", () => {
        expect(sharpPixels(1)).toBe(false)
        expect(sharpPixels(0.5)).toBe(false)
        expect(sharpPixels(1.01)).toBe(true)
        expect(sharpPixels(32)).toBe(true)
    })
})
