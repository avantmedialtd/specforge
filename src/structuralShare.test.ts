import { describe, expect, test } from "bun:test"
import { shareStructure } from "./structuralShare"

// `spec-browser`: *Unchanged Workspace Views Keep Their Rows*; design D3.

function views() {
    return [
        {
            kind: "repo",
            repoId: "a",
            active: [
                { name: "one", instances: [{ path: "/a", tasks: 3, done: 1 }] },
                { name: "two", instances: [{ path: "/a", tasks: 2, done: 2 }] },
            ],
        },
        { kind: "flat", workspace: { uri: "/b" }, changes: [{ changeId: "x" }] },
    ]
}

describe("shareStructure", () => {
    test("a fully equal value returns the previous one by identity", () => {
        const previous = views()
        expect(shareStructure(previous, views())).toBe(previous)
    })

    test("one changed leaf replaces only the path to it", () => {
        const previous = views()
        const next = views()
        next[0]!.active![1]!.instances[0]!.done = 1
        const shared = shareStructure(previous, next)

        expect(shared).not.toBe(previous)
        expect(shared).toEqual(next)
        // The path to the leaf is new…
        expect(shared[0]).not.toBe(previous[0])
        expect(shared[0]!.active).not.toBe(previous[0]!.active)
        expect(shared[0]!.active![1]).not.toBe(previous[0]!.active![1])
        // …and every sibling keeps its identity.
        expect(shared[1]).toBe(previous[1])
        expect(shared[0]!.active![0]).toBe(previous[0]!.active![0])
    })

    test("an added key is a change", () => {
        const previous: Record<string, unknown> = { a: 1 }
        const shared = shareStructure(previous, { a: 1, b: 2 })
        expect(shared).not.toBe(previous)
        expect(shared).toEqual({ a: 1, b: 2 })
    })

    test("a removed key is a change", () => {
        const previous: Record<string, unknown> = { a: 1, b: 2 }
        const shared = shareStructure(previous, { a: 1 })
        expect(shared).not.toBe(previous)
        expect(Object.keys(shared)).toEqual(["a"])
    })

    test("undefined differs from a missing key", () => {
        const previous: Record<string, unknown> = { a: 1, b: undefined }
        expect(shareStructure(previous, { a: 1 })).not.toBe(previous)
        const missing: Record<string, unknown> = { a: 1 }
        const shared = shareStructure(missing, { a: 1, b: undefined })
        expect(shared).not.toBe(missing)
        expect("b" in shared).toBe(true)
        // Same key count, different keys.
        const other: Record<string, unknown> = { a: undefined }
        expect(shareStructure(other, { b: undefined })).not.toBe(other)
    })

    test("reordered items are a change, matched by position", () => {
        const first = { id: 1 }
        const second = { id: 2 }
        const previous = [first, second]
        const next = [{ id: 2 }, { id: 1 }]
        const shared = shareStructure(previous, next)
        expect(shared).not.toBe(previous)
        expect(shared).toEqual([{ id: 2 }, { id: 1 }])
        // Positions, not items, are compared: a moved item is not recognised,
        // so neither position keeps the previous object.
        expect(shared[0]).not.toBe(first)
        expect(shared[0]).not.toBe(second)
        expect(shared[1]).not.toBe(first)
        expect(shared[1]).not.toBe(second)
    })

    test("an inserted item changes the positions after it, and keeps those before", () => {
        const first = { id: 1 }
        const previous = [first, { id: 2 }, { id: 3 }]
        const next = [{ id: 1 }, { id: 9 }, { id: 2 }, { id: 3 }]
        const shared = shareStructure(previous, next)
        expect(shared[0]).toBe(first)
        // Every position after the insertion holds a new object.
        expect(shared[1]).not.toBe(previous[1])
        expect(shared[2]).not.toBe(previous[2])
        expect(shared[3]).toBe(next[3])
        expect(shared).toEqual(next)
    })

    test("a change after equal keys keeps those keys' values", () => {
        const kept = { deep: [1, 2] }
        const previous = { a: kept, b: 1, c: { x: 1 } }
        const shared = shareStructure(previous, { a: { deep: [1, 2] }, b: 2, c: { x: 1 } })
        expect(shared).not.toBe(previous)
        expect(shared.a).toBe(kept)
        expect(shared.b).toBe(2)
        expect(shared.c).toBe(previous.c)
        expect(Object.keys(shared)).toEqual(["a", "b", "c"])
    })

    test("a length change is a change, and the common prefix keeps its identity", () => {
        const first = { id: 1 }
        const previous = [first, { id: 2 }]
        const shorter = shareStructure(previous, [{ id: 1 }])
        expect(shorter).not.toBe(previous)
        expect(shorter).toHaveLength(1)
        expect(shorter[0]).toBe(first)

        const longer = shareStructure(previous, [{ id: 1 }, { id: 2 }, { id: 3 }])
        expect(longer).not.toBe(previous)
        expect(longer).toHaveLength(3)
        expect(longer[0]).toBe(first)
        expect(longer[1]).toBe(previous[1])
    })

    test("primitives compare by Object.is", () => {
        const nan = { n: Number.NaN }
        expect(shareStructure(nan, { n: Number.NaN })).toBe(nan)
        const zero = { n: 0 }
        expect(shareStructure(zero, { n: -0 })).not.toBe(zero)
        const text = { s: "a" }
        expect(shareStructure(text, { s: "b" })).toEqual({ s: "b" })
        // A type change at a leaf is a change.
        const mixed: Record<string, unknown> = { v: 1 }
        expect(shareStructure(mixed, { v: "1" })).not.toBe(mixed)
        const nested: Record<string, unknown> = { v: [1] }
        expect(shareStructure(nested, { v: { 0: 1 } })).toEqual({ v: { 0: 1 } })
    })

    test("null and primitives at the root", () => {
        expect(shareStructure<unknown>(null, null)).toBeNull()
        const value = { a: 1 }
        expect(shareStructure<unknown>(null, value)).toBe(value)
        expect(shareStructure<unknown>(value, null)).toBeNull()
        expect(shareStructure(3, 3)).toBe(3)
        expect(shareStructure("a", "b")).toBe("b")
        expect(Object.is(shareStructure(Number.NaN, Number.NaN), Number.NaN)).toBe(true)
        expect(Object.is(shareStructure(0, -0), -0)).toBe(true)
    })

    test("anything but plain objects and arrays is kept only when it is the same value", () => {
        const date = new Date(0)
        const previous = { at: date }
        const shared = shareStructure(previous, { at: new Date(0) })
        expect(shared).not.toBe(previous)
        expect(shareStructure(previous, { at: date })).toBe(previous)
    })
})
