import { describe, expect, test } from "bun:test"
import {
    effectiveTrigger,
    INITIAL,
    reduce,
    type DetailState,
} from "./refreshPolicy"

const READY: DetailState = { content: "# Tasks", error: null, loading: false }
const ERRORED: DetailState = { content: null, error: "boom", loading: false }
const LOADING: DetailState = { content: "# Tasks", error: null, loading: true }

describe("select", () => {
    test("raises loading and clears any error", () => {
        expect(reduce(ERRORED, { kind: "select" })).toEqual({
            content: null,
            error: null,
            loading: true,
        })
    })

    test("keeps the outgoing artifact on screen while the next loads", () => {
        expect(reduce(READY, { kind: "select" }).content).toBe("# Tasks")
    })

    test("resolving lands the content and drops loading", () => {
        const next = reduce(LOADING, {
            kind: "resolved",
            trigger: "select",
            content: "# Fresh",
        })
        expect(next).toEqual({
            content: "# Fresh",
            error: null,
            loading: false,
        })
    })

    test("failing surfaces the error and clears the content", () => {
        const next = reduce(LOADING, {
            kind: "failed",
            trigger: "select",
            error: "no such file",
        })
        expect(next).toEqual({
            content: null,
            error: "no such file",
            loading: false,
        })
    })
})

describe("watch", () => {
    test("starting a read is not observable", () => {
        expect(reduce(READY, { kind: "watch" })).toBe(READY)
    })

    test("unchanged bytes return the identical state object", () => {
        const next = reduce(READY, {
            kind: "resolved",
            trigger: "watch",
            content: "# Tasks",
        })
        expect(next).toBe(READY)
    })

    test("changed bytes replace the content without raising loading", () => {
        const next = reduce(READY, {
            kind: "resolved",
            trigger: "watch",
            content: "# Tasks\n- [x] done",
        })
        expect(next).toEqual({
            content: "# Tasks\n- [x] done",
            error: null,
            loading: false,
        })
    })

    test("a failed read leaves the reader's content in place", () => {
        const next = reduce(READY, {
            kind: "failed",
            trigger: "watch",
            error: "archived out from under us",
        })
        expect(next).toBe(READY)
    })

    test("a failed read does not resurrect a pane that is already errored", () => {
        expect(
            reduce(ERRORED, {
                kind: "failed",
                trigger: "watch",
                error: "still gone",
            }),
        ).toBe(ERRORED)
    })

    test("a successful read clears an error the user's own read left", () => {
        const next = reduce(ERRORED, {
            kind: "resolved",
            trigger: "watch",
            content: "# Back",
        })
        expect(next).toEqual({
            content: "# Back",
            error: null,
            loading: false,
        })
    })

    test("a fresher result is not dropped for an older outstanding read", () => {
        // The watcher read was issued last, so it carries the newer bytes.
        // Dropping it here would let the older select read install stale
        // content that the equality guard then pins in place.
        expect(
            reduce(LOADING, {
                kind: "resolved",
                trigger: "watch",
                content: "# Racing",
            }),
        ).toEqual({ content: "# Racing", error: null, loading: false })
    })
})

describe("effectiveTrigger", () => {
    test("a watch read that supersedes a user read inherits select", () => {
        expect(effectiveTrigger("watch", "select")).toBe("select")
    })

    test("a watch read with nothing outstanding stays watch", () => {
        expect(effectiveTrigger("watch", null)).toBe("watch")
        expect(effectiveTrigger("watch", "watch")).toBe("watch")
    })

    test("a user read is always select", () => {
        expect(effectiveTrigger("select", null)).toBe("select")
        expect(effectiveTrigger("select", "select")).toBe("select")
        expect(effectiveTrigger("select", "watch")).toBe("select")
    })
})

describe("cleared", () => {
    test("empties a pane that was showing something", () => {
        expect(reduce(READY, { kind: "cleared" })).toEqual(INITIAL)
    })

    test("is identity on an already-empty pane", () => {
        expect(reduce(INITIAL, { kind: "cleared" })).toBe(INITIAL)
    })
})
