import { describe, expect, test } from "bun:test"
import {
    canHighlight, comparePixels, comparisonSize, IMAGE_DIFF_LIMIT_TEXT,
    type ImageSize,
} from "./imageDiffPixels"

function buffer(width: number, height: number, color: number[]) {
    return new Uint8ClampedArray(Array.from({ length: width * height }, () => color).flat())
}

describe("highlighted image pixels", () => {
    const size = { width: 3, height: 3 }

    test("identical pixels become muted grayscale with no changes", () => {
        const pixels = buffer(3, 3, [0, 120, 0, 255])
        const result = comparePixels({ before: size, after: size, oldPixels: pixels, newPixels: pixels })
        expect(result.changed).toBe(0)
        expect(result.pixels[0]).toBe(result.pixels[1])
        expect(result.pixels[1]).toBe(result.pixels[2])
        expect(result.pixels[0]).toBeGreaterThan(230)
        expect(result.pixels[3]).toBe(255)
    })

    test("changed pixels are red and counted exactly once", () => {
        const result = comparePixels({
            before: size, after: size,
            oldPixels: buffer(3, 3, [255, 255, 255, 255]),
            newPixels: buffer(3, 3, [0, 0, 0, 255]),
        })
        expect(result.changed).toBe(9)
        expect([...result.pixels.slice(0, 4)]).toEqual([255, 0, 0, 255])
        expect(result.width).toBe(3)
        expect(result.height).toBe(3)
    })

    test("a visible green-to-teal asset color change is highlighted", () => {
        expect(comparePixels({
            before: size, after: size,
            oldPixels: buffer(3, 3, [33, 112, 73, 255]),
            newPixels: buffer(3, 3, [14, 116, 144, 255]),
        }).changed).toBe(9)
    })

    test("invisible RGB changes do not count, visible alpha changes do", () => {
        const transparent = buffer(3, 3, [255, 255, 255, 0])
        expect(comparePixels({
            before: size, after: size,
            oldPixels: transparent, newPixels: buffer(3, 3, [0, 0, 0, 0]),
        }).changed).toBe(0)
        expect(comparePixels({
            before: size, after: size,
            oldPixels: transparent, newPixels: buffer(3, 3, [0, 0, 0, 255]),
        }).changed).toBe(9)
    })

    test("resizing transparent images highlights their symmetric difference, not the empty corner", () => {
        const before = { width: 2, height: 1 }
        const after = { width: 1, height: 2 }
        const result = comparePixels({
            before, after,
            oldPixels: buffer(2, 2, [0, 0, 0, 0]),
            newPixels: buffer(2, 2, [0, 0, 0, 0]),
        })
        expect(result.changed).toBe(2)
        expect([...result.pixels.slice(4, 12)]).toEqual([255, 0, 0, 255, 255, 0, 0, 255])
        expect([...result.pixels.slice(12)]).toEqual([255, 255, 255, 255])
    })

    test("the processing ceiling applies to the union, before allocating pixels", () => {
        expect(canHighlight({ width: 2000, height: 2000 }, { width: 1, height: 1 })).toBe(true)
        expect(canHighlight({ width: 2001, height: 2000 }, { width: 1, height: 1 })).toBe(false)
        const before = { width: 40000, height: 1 }
        const after = { width: 1, height: 40000 }
        expect(comparisonSize(before, after)).toEqual({ width: 40000, height: 40000 })
        expect(() => comparePixels({
            before, after, oldPixels: new Uint8ClampedArray(), newPixels: new Uint8ClampedArray(),
        })).toThrow(IMAGE_DIFF_LIMIT_TEXT)
        for (const invalid of [
            { width: 0, height: 1 }, { width: -1, height: 1 },
            { width: NaN, height: 1 }, { width: 1.5, height: 1 },
        ] satisfies ImageSize[]) {
            expect(canHighlight(invalid, size)).toBe(false)
        }
    })

    test("mismatched buffers fail rather than showing a misleading summary", () => {
        expect(() => comparePixels({
            before: size, after: size,
            oldPixels: buffer(3, 3, [0, 0, 0, 0]), newPixels: new Uint8ClampedArray(),
        })).toThrow("Invalid pixel buffer dimensions")
    })
})
