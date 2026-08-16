import { describe, expect, test } from "bun:test"
import {
    effectiveTrigger,
    INITIAL,
    reduce,
    type DetailState,
} from "./refreshPolicy"

/// Unix seconds, fixed rather than derived from the clock: these tests assert
/// exact equality on the value the header will render, so nothing here should
/// depend on when the suite runs.
const AT = 1_700_000_000
const LATER = AT + 60

const READY: DetailState = {
    content: "# Tasks",
    modifiedAt: AT,
    error: null,
    loading: false,
}
const ERRORED: DetailState = {
    content: null,
    modifiedAt: null,
    error: "boom",
    loading: false,
}
const LOADING: DetailState = {
    content: "# Tasks",
    modifiedAt: AT,
    error: null,
    loading: true,
}

describe("select", () => {
    test("raises loading and clears any error", () => {
        expect(reduce(ERRORED, { kind: "select" })).toEqual({
            content: null,
            modifiedAt: null,
            error: null,
            loading: true,
        })
    })

    test("keeps the outgoing artifact on screen while the next loads", () => {
        expect(reduce(READY, { kind: "select" }).content).toBe("# Tasks")
    })

    test("drops the outgoing artifact's time even though it keeps its content", () => {
        // The asymmetry is deliberate. The document is retained because it is
        // still what the reader sees; the time is not, because the header's
        // name and branch chip come from the render target and have ALREADY
        // moved to the incoming artifact. Retaining it would date the new
        // artifact with the old one's write — and stepping proposal → tasks
        // inside one change, that is the sibling's write time the spec
        // explicitly forbids reporting as this artifact's.
        const next = reduce(READY, { kind: "select" })
        expect(next.content).toBe("# Tasks")
        expect(next.modifiedAt).toBeNull()
    })

    test("resolving lands the content and drops loading", () => {
        const next = reduce(LOADING, {
            kind: "resolved",
            trigger: "select",
            content: "# Fresh",
            modifiedAt: LATER,
        })
        expect(next).toEqual({
            content: "# Fresh",
            modifiedAt: LATER,
            error: null,
            loading: false,
        })
    })

    test("a user-initiated read is unaffected by the equality guard", () => {
        // The guard is scoped to `watch`. A user who re-selects the artifact
        // they are already reading gets a fresh state object even when nothing
        // whatsoever changed, because their read has a loading flag to clear.
        const next = reduce(LOADING, {
            kind: "resolved",
            trigger: "select",
            content: LOADING.content as string,
            modifiedAt: AT,
        })
        expect(next).not.toBe(LOADING)
        expect(next.loading).toBe(false)
    })

    test("failing surfaces the error and clears the content", () => {
        const next = reduce(LOADING, {
            kind: "failed",
            trigger: "select",
            error: "no such file",
        })
        expect(next).toEqual({
            content: null,
            modifiedAt: null,
            error: "no such file",
            loading: false,
        })
    })
})

describe("watch", () => {
    test("starting a read is not observable", () => {
        expect(reduce(READY, { kind: "watch" })).toBe(READY)
    })

    test("unchanged bytes AND unchanged time return the identical state object", () => {
        const next = reduce(READY, {
            kind: "resolved",
            trigger: "watch",
            content: "# Tasks",
            modifiedAt: AT,
        })
        expect(next).toBe(READY)
    })

    test("unchanged bytes with a newer time update the header, not the document", () => {
        // A rewrite with identical content — a branch switch, an idempotent
        // write, a formatter. The state object must be NEW so the header
        // re-renders with the newer time, while `content` stays referentially
        // equal so `memo(MarkdownView)` skips the whole markdown pipeline.
        const next = reduce(READY, {
            kind: "resolved",
            trigger: "watch",
            content: READY.content as string,
            modifiedAt: LATER,
        })
        expect(next).not.toBe(READY)
        expect(next.content).toBe(READY.content)
        expect(next.modifiedAt).toBe(LATER)
        expect(next.loading).toBe(false)
    })

    test("changed bytes replace the content without raising loading", () => {
        const next = reduce(READY, {
            kind: "resolved",
            trigger: "watch",
            content: "# Tasks\n- [x] done",
            modifiedAt: LATER,
        })
        expect(next).toEqual({
            content: "# Tasks\n- [x] done",
            modifiedAt: LATER,
            error: null,
            loading: false,
        })
    })

    test("an absent modification time is carried as null, not fabricated", () => {
        const next = reduce(READY, {
            kind: "resolved",
            trigger: "watch",
            content: "# Changed",
            modifiedAt: null,
        })
        expect(next.modifiedAt).toBeNull()
    })

    test("a read that only loses its time is still observable", () => {
        // Same bytes, but the filesystem stopped reporting a time. The header
        // must drop its label rather than keep showing the last one it saw.
        const next = reduce(READY, {
            kind: "resolved",
            trigger: "watch",
            content: READY.content as string,
            modifiedAt: null,
        })
        expect(next).not.toBe(READY)
        expect(next.modifiedAt).toBeNull()
    })

    test("a failed read leaves the reader's content and its time in place", () => {
        const next = reduce(READY, {
            kind: "failed",
            trigger: "watch",
            error: "archived out from under us",
        })
        expect(next).toBe(READY)
        expect(next.modifiedAt).toBe(AT)
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
            modifiedAt: AT,
        })
        expect(next).toEqual({
            content: "# Back",
            modifiedAt: AT,
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
                modifiedAt: LATER,
            }),
        ).toEqual({
            content: "# Racing",
            modifiedAt: LATER,
            error: null,
            loading: false,
        })
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

    test("clears a lingering time even when no content is held", () => {
        // Defends an invariant rather than exercising a reachable path: no
        // event sequence produces `content: null` alongside a non-null time,
        // because every branch that nulls the content nulls the time with it.
        // The guard is here so that a future branch which forgets to would be
        // caught by `cleared` returning INITIAL rather than silently handing
        // back a state still carrying a dead timestamp.
        const stray: DetailState = {
            content: null,
            modifiedAt: AT,
            error: null,
            loading: false,
        }
        expect(reduce(stray, { kind: "cleared" })).toEqual(INITIAL)
    })
})
