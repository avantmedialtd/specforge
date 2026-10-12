import { describe, expect, test } from "bun:test"
import { createSlotCache, reuseMap, sameSignature } from "./slotCache"

// `diff-view`: *Host Slot Updates Re-render Only Their File*; design D5.

/// A cache with a build counter, so a test can tell a hit from a rebuild.
function counted() {
    const cache = createSlotCache<{ built: number }>()
    let builds = 0
    const get = (key: string, signature: readonly unknown[]) =>
        cache.get(key, signature, () => ({ built: ++builds }))
    return { cache, get, builds: () => builds }
}

describe("createSlotCache", () => {
    test("an equal signature returns the cached value without building", () => {
        const { get, builds } = counted()
        const shared = { path: "a.ts" }
        const first = get("a.ts", [shared, 1, "x", null, undefined])
        const second = get("a.ts", [shared, 1, "x", null, undefined])
        expect(second).toBe(first)
        expect(builds()).toBe(1)
    })

    test("a changed element rebuilds", () => {
        const { get, builds } = counted()
        const first = get("a.ts", [{ path: "a.ts" }, 1])
        // An equal-looking but new object is a change: identity is the input.
        const second = get("a.ts", [{ path: "a.ts" }, 1])
        expect(second).not.toBe(first)
        const third = get("a.ts", [second, 2])
        const fourth = get("a.ts", [second, 3])
        expect(fourth).not.toBe(third)
        expect(builds()).toBe(4)
    })

    test("a changed length rebuilds", () => {
        const { get, builds } = counted()
        const first = get("a.ts", [1, 2])
        const second = get("a.ts", [1, 2, 3])
        const third = get("a.ts", [1, 2])
        expect(second).not.toBe(first)
        expect(third).not.toBe(second)
        expect(builds()).toBe(3)
    })

    test("NaN is equal to itself, and -0 differs from 0", () => {
        const { get, builds } = counted()
        const first = get("a.ts", [Number.NaN])
        expect(get("a.ts", [Number.NaN])).toBe(first)
        get("a.ts", [0])
        get("a.ts", [-0])
        expect(builds()).toBe(3)
    })

    test("keys are independent", () => {
        const { get, builds } = counted()
        const a = get("a.ts", [1])
        const b = get("b.ts", [1])
        expect(b).not.toBe(a)
        // Rebuilding one leaves the other cached.
        const a2 = get("a.ts", [2])
        expect(a2).not.toBe(a)
        expect(get("b.ts", [1])).toBe(b)
        expect(builds()).toBe(3)
    })

    test("retain drops the keys of files that are gone", () => {
        const { cache, get, builds } = counted()
        const a = get("a.ts", [1])
        const b = get("b.ts", [1])
        cache.retain(["a.ts"])
        expect(get("a.ts", [1])).toBe(a)
        const b2 = get("b.ts", [1])
        expect(b2).not.toBe(b)
        expect(builds()).toBe(3)
        cache.retain([])
        expect(get("a.ts", [1])).not.toBe(a)
    })
})

describe("sameSignature", () => {
    test("compares length and each element by Object.is", () => {
        expect(sameSignature([], [])).toBe(true)
        expect(sameSignature([1, "a"], [1, "a"])).toBe(true)
        expect(sameSignature([1], [1, undefined])).toBe(false)
        expect(sameSignature([{}], [{}])).toBe(false)
    })
})

describe("reuseMap", () => {
    test("keeps the previous map when every entry is the same", () => {
        const slots = { folded: new Set<number>() }
        const previous = new Map([
            ["a", slots],
            ["b", undefined],
        ])
        const next = new Map([
            ["a", slots],
            ["b", undefined],
        ])
        expect(reuseMap(previous, next)).toBe(previous)
    })

    test("takes the new map when an entry, a key, the order or the size differs", () => {
        const slots = { folded: new Set<number>() }
        const previous = new Map([
            ["a", slots],
            ["b", slots],
        ])
        const changed = new Map([
            ["a", slots],
            ["b", { folded: new Set<number>() }],
        ])
        expect(reuseMap(previous, changed)).toBe(changed)
        const renamed = new Map([
            ["a", slots],
            ["c", slots],
        ])
        expect(reuseMap(previous, renamed)).toBe(renamed)
        const reordered = new Map([
            ["b", slots],
            ["a", slots],
        ])
        expect(reuseMap(previous, reordered)).toBe(reordered)
        const shorter = new Map([["a", slots]])
        expect(reuseMap(previous, shorter)).toBe(shorter)
        expect(reuseMap(null, shorter)).toBe(shorter)
    })
})
