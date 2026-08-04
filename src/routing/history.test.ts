import { describe, expect, test } from "bun:test"
import { createAddressSnapshot, createMemoryHistory } from "./history"

describe("createMemoryHistory", () => {
    test("starts at the given initial path (default '/')", () => {
        expect(createMemoryHistory().current()).toBe("/")
        expect(createMemoryHistory("/settings").current()).toBe("/settings")
    })

    test("push navigates and becomes current", () => {
        const h = createMemoryHistory()
        h.push("/a")
        expect(h.current()).toBe("/a")
    })

    test("push notifies subscribers", () => {
        const h = createMemoryHistory()
        let calls = 0
        h.subscribe(() => {
            calls++
        })
        h.push("/a")
        expect(calls).toBe(1)
    })

    test("push to the current path is a no-op (no notification)", () => {
        const h = createMemoryHistory("/a")
        let calls = 0
        h.subscribe(() => {
            calls++
        })
        h.push("/a")
        expect(calls).toBe(0)
        expect(h.current()).toBe("/a")
    })

    test("replace swaps the current entry without adding one", () => {
        const h = createMemoryHistory()
        h.push("/a")
        h.replace("/b")
        expect(h.current()).toBe("/b")
        h.back()
        // Only the original "/" entry remains before "/b" — "/a" was
        // overwritten by the replace, never left as a separate entry.
        expect(h.current()).toBe("/")
    })

    test("replace notifies subscribers", () => {
        const h = createMemoryHistory()
        let calls = 0
        h.subscribe(() => calls++)
        h.replace("/a")
        expect(calls).toBe(1)
    })

    test("back/forward walk the entry list", () => {
        const h = createMemoryHistory()
        h.push("/a")
        h.push("/b")
        expect(h.current()).toBe("/b")
        h.back()
        expect(h.current()).toBe("/a")
        h.back()
        expect(h.current()).toBe("/")
        h.forward()
        expect(h.current()).toBe("/a")
        h.forward()
        expect(h.current()).toBe("/b")
    })

    test("back is a no-op at the start of history", () => {
        const h = createMemoryHistory()
        let calls = 0
        h.subscribe(() => calls++)
        h.back()
        expect(h.current()).toBe("/")
        expect(calls).toBe(0)
    })

    test("forward is a no-op at the end of history", () => {
        const h = createMemoryHistory()
        h.push("/a")
        let calls = 0
        h.subscribe(() => calls++)
        h.forward()
        expect(h.current()).toBe("/a")
        expect(calls).toBe(0)
    })

    test("back/forward notify subscribers", () => {
        const h = createMemoryHistory()
        h.push("/a")
        let calls = 0
        h.subscribe(() => calls++)
        h.back()
        expect(calls).toBe(1)
        h.forward()
        expect(calls).toBe(2)
    })

    test("push after back truncates the forward (redo) branch", () => {
        const h = createMemoryHistory()
        h.push("/a")
        h.push("/b")
        h.back() // now at "/a", with "/b" as a forward entry
        h.push("/c") // discards "/b"
        expect(h.current()).toBe("/c")
        h.forward() // nothing to redo
        expect(h.current()).toBe("/c")
        h.back()
        expect(h.current()).toBe("/a")
        h.back()
        expect(h.current()).toBe("/")
    })

    test("unsubscribe stops further notifications", () => {
        const h = createMemoryHistory()
        let calls = 0
        const unsubscribe = h.subscribe(() => calls++)
        h.push("/a")
        expect(calls).toBe(1)
        unsubscribe()
        h.push("/b")
        expect(calls).toBe(1)
    })

    test("multiple subscribers are all notified", () => {
        const h = createMemoryHistory()
        let a = 0
        let b = 0
        h.subscribe(() => a++)
        h.subscribe(() => b++)
        h.push("/x")
        expect(a).toBe(1)
        expect(b).toBe(1)
    })
})

// `useSyncExternalStore` compares consecutive `getSnapshot()` calls with
// `Object.is`; a getter that allocates a fresh object every call (a plain
// `() => decodeAddress(history.current())`, with no cache) looks like a
// perpetually-changing store and infinite-loops React (error #185). These
// tests assert the exact invariant React enforces at the hook boundary —
// no DOM, no React, no new dependency — so a regression here fails a `bun
// test` run rather than only surfacing as a blank-screen render bug.
describe("createAddressSnapshot", () => {
    test("calling the getter twice with no intervening history change returns an Object.is-equal value", () => {
        const h = createMemoryHistory("/settings")
        const getSnapshot = createAddressSnapshot(h)
        const first = getSnapshot()
        const second = getSnapshot()
        expect(Object.is(first, second)).toBe(true)
    })

    test("stays Object.is-equal across many repeated calls, not just two", () => {
        const h = createMemoryHistory("/w/foo")
        const getSnapshot = createAddressSnapshot(h)
        const snapshots = [getSnapshot(), getSnapshot(), getSnapshot()]
        expect(Object.is(snapshots[0], snapshots[1])).toBe(true)
        expect(Object.is(snapshots[1], snapshots[2])).toBe(true)
    })

    test("decodes the initial path correctly", () => {
        const h = createMemoryHistory("/w/foo")
        const getSnapshot = createAddressSnapshot(h)
        expect(getSnapshot()).toEqual({
            kind: "files",
            scope: { kind: "workspace", workspace: "foo" },
        })
    })

    test("returns a different (Object.is-unequal) value after push", () => {
        const h = createMemoryHistory()
        const getSnapshot = createAddressSnapshot(h)
        const before = getSnapshot()
        h.push("/settings")
        const after = getSnapshot()
        expect(Object.is(before, after)).toBe(false)
        expect(after).toEqual({ kind: "settings" })
    })

    test("returns a different value after replace", () => {
        const h = createMemoryHistory()
        const getSnapshot = createAddressSnapshot(h)
        const before = getSnapshot()
        h.replace("/settings")
        const after = getSnapshot()
        expect(Object.is(before, after)).toBe(false)
        expect(after).toEqual({ kind: "settings" })
    })

    test("returns a different value after back", () => {
        const h = createMemoryHistory()
        h.push("/settings")
        const getSnapshot = createAddressSnapshot(h)
        const before = getSnapshot()
        h.back()
        const after = getSnapshot()
        expect(Object.is(before, after)).toBe(false)
        expect(after).toEqual({ kind: "home" })
    })

    test("returns a different value after forward", () => {
        const h = createMemoryHistory()
        h.push("/settings")
        h.back()
        const getSnapshot = createAddressSnapshot(h)
        const before = getSnapshot()
        h.forward()
        const after = getSnapshot()
        expect(Object.is(before, after)).toBe(false)
        expect(after).toEqual({ kind: "settings" })
    })

    test("navigating away and back to the same path yields a stable value again", () => {
        const h = createMemoryHistory()
        const getSnapshot = createAddressSnapshot(h)
        const home1 = getSnapshot()
        expect(home1).toEqual({ kind: "home" })
        h.push("/settings")
        getSnapshot()
        h.back()
        const home2 = getSnapshot()
        // Not required to be the SAME reference as `home1` (the cache only
        // promises stability while unchanged, not identity across a round
        // trip) — but must decode correctly and be internally stable again.
        expect(home2).toEqual({ kind: "home" })
        expect(Object.is(home2, getSnapshot())).toBe(true)
    })
})
