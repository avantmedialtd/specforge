import { describe, expect, test } from "bun:test"
import { anchorThreads } from "./reviewAnchors"
import type { DiffFile, Hunk, Line, PullRequestComment, ReviewThread } from "./types"

// `pull-request-viewer`: *Changed Files in the Pull-Request View*; design D11.
// A file's current single-line threads go on their line; everything the diff
// cannot render there keeps the file's preamble.

function comment(id: string, overrides: Partial<PullRequestComment> = {}): PullRequestComment {
    return {
        id,
        author: "ada",
        body: `Comment ${id}`,
        postedAtUnix: 1_000,
        url: null,
        minimizedReason: null,
        deleted: false,
        ...overrides,
    }
}

function thread(overrides: Partial<ReviewThread> = {}): ReviewThread {
    const id = overrides.id ?? "t1"
    return {
        id,
        path: "src/api.ts",
        side: "new",
        line: 22,
        startSide: null,
        startLine: null,
        originalLine: null,
        originalStartLine: null,
        resolved: false,
        outdated: false,
        replyable: true,
        comments: [comment(`${id}-c1`)],
        ...overrides,
    }
}

function line(kind: Line["kind"], oldNo: number | null, newNo: number | null): Line {
    return { kind, oldNo, newNo, text: `${kind} line` }
}

/// A modified file: old 20/21 face new 20/22. The context line carries both
/// identities, the removed line only its old one, the added line only its new
/// one.
function hunk(): Hunk {
    return {
        oldStart: 20,
        oldLines: 2,
        newStart: 20,
        newLines: 2,
        section: null,
        lines: [
            line("context", 20, 20),
            line("removed", 21, null),
            line("added", null, 22),
        ],
    }
}

/// A second hunk far below the first, so a fold can separate the two.
function laterHunk(): Hunk {
    return {
        oldStart: 100,
        oldLines: 1,
        newStart: 100,
        newLines: 1,
        section: null,
        lines: [line("context", 100, 100)],
    }
}

function file(overrides: Partial<DiffFile> = {}): DiffFile {
    return {
        oldPath: "src/api.ts",
        newPath: "src/api.ts",
        oldMode: "100644",
        newMode: "100644",
        status: { kind: "modified" },
        additions: 1,
        deletions: 1,
        content: { kind: "hunks", hunks: [hunk()] },
        ...overrides,
    }
}

describe("anchorThreads", () => {
    test("a file with no threads gets no slot and no preamble", () => {
        expect(anchorThreads([], file())).toEqual({ slots: [], preamble: [] })
    })

    test("a current single-line thread on a rendered line is placed there", () => {
        const current = thread()
        const anchors = anchorThreads([current], file())
        expect(anchors.preamble).toEqual([])
        expect(anchors.slots).toHaveLength(1)
        expect(anchors.slots[0]!.side).toBe("new")
        expect(anchors.slots[0]!.line).toBe(22)
        expect(anchors.slots[0]!.threads).toEqual([current])
    })

    test("a removed line's thread takes the old side", () => {
        const removed = thread({ side: "old", line: 21 })
        const anchors = anchorThreads([removed], file())
        expect(anchors.preamble).toEqual([])
        expect(anchors.slots.map((slot) => [slot.side, slot.line])).toEqual([["old", 21]])
    })

    test("a context line carries both sides as independent slots", () => {
        const old = thread({ id: "old", side: "old", line: 20 })
        const next = thread({ id: "new", side: "new", line: 20 })
        const anchors = anchorThreads([old, next], file())
        expect(anchors.preamble).toEqual([])
        expect(anchors.slots.map((slot) => [slot.side, slot.line, slot.threads])).toEqual([
            ["old", 20, [old]],
            ["new", 20, [next]],
        ])
    })

    test("threads on one line share one slot, in the detail's order", () => {
        const first = thread({ id: "first" })
        const second = thread({ id: "second" })
        const anchors = anchorThreads([first, second], file())
        expect(anchors.preamble).toEqual([])
        expect(anchors.slots).toHaveLength(1)
        expect(anchors.slots[0]!.threads).toEqual([first, second])
    })

    test("a ranged thread keeps the preamble", () => {
        const ranged = thread({ startSide: "new", startLine: 20 })
        const anchors = anchorThreads([ranged], file())
        expect(anchors.slots).toEqual([])
        expect(anchors.preamble).toEqual([ranged])
    })

    test("a start that only repeats the line and side is one line", () => {
        const one = thread({ startSide: "new", startLine: 22 })
        const anchors = anchorThreads([one], file())
        expect(anchors.preamble).toEqual([])
        expect(anchors.slots.map((slot) => [slot.side, slot.line])).toEqual([["new", 22]])
    })

    test("a start on another side is a range", () => {
        const ranged = thread({ side: "new", line: 22, startSide: "old", startLine: 20 })
        const anchors = anchorThreads([ranged], file())
        expect(anchors.slots).toEqual([])
        expect(anchors.preamble).toEqual([ranged])
    })

    test("an outdated thread keeps the preamble even on a rendered line", () => {
        const outdated = thread({ outdated: true, originalLine: 22 })
        const anchors = anchorThreads([outdated], file())
        expect(anchors.slots).toEqual([])
        expect(anchors.preamble).toEqual([outdated])
    })

    test("a thread with no line keeps the preamble", () => {
        const whole = thread({ line: null })
        const anchors = anchorThreads([whole], file())
        expect(anchors.slots).toEqual([])
        expect(anchors.preamble).toEqual([whole])
    })

    test("a line the file does not render keeps the preamble", () => {
        const absent = thread({ line: 99 })
        const anchors = anchorThreads([absent], file())
        expect(anchors.slots).toEqual([])
        expect(anchors.preamble).toEqual([absent])
    })

    test("a withheld file renders no line, so every thread keeps the preamble", () => {
        const withheld = file({ content: { kind: "withheld" } })
        const onLine = thread()
        const anchors = anchorThreads([onLine], withheld)
        expect(anchors.slots).toEqual([])
        expect(anchors.preamble).toEqual([onLine])
    })

    test("a folded hunk's lines take no thread, and another hunk's still do", () => {
        const twoHunks = file({ content: { kind: "hunks", hunks: [hunk(), laterHunk()] } })
        const inFirst = thread({ id: "first", line: 22 })
        const inSecond = thread({ id: "second", line: 100 })
        const anchors = anchorThreads([inFirst, inSecond], twoHunks, new Set([0]))
        expect(anchors.preamble).toEqual([inFirst])
        expect(anchors.slots.map((slot) => [slot.line, slot.threads])).toEqual([[100, [inSecond]]])
    })

    test("the placed and unplaced keep the detail's order among themselves", () => {
        const first = thread({ id: "a", line: 22 })
        const ranged = thread({ id: "b", startLine: 20 })
        const second = thread({ id: "c", side: "old", line: 21 })
        const third = thread({ id: "d", line: 22 })
        const anchors = anchorThreads([first, ranged, second, third], file())
        expect(anchors.preamble.map((each) => each.id)).toEqual(["b"])
        expect(anchors.slots.map((slot) => slot.threads.map((each) => each.id))).toEqual([
            ["a", "d"],
            ["c"],
        ])
    })

    test("a deleted file places its old-side threads", () => {
        const deleted = file({
            newPath: null,
            status: { kind: "deleted" },
            content: { kind: "hunks", hunks: [hunk()] },
        })
        const onOld = thread({ side: "old", line: 21 })
        const anchors = anchorThreads([onOld], deleted)
        expect(anchors.preamble).toEqual([])
        expect(anchors.slots.map((slot) => [slot.side, slot.line])).toEqual([["old", 21]])
    })

    test("a renamed file keeps its two sides apart", () => {
        const renamed = file({ oldPath: "src/old.ts", newPath: "src/api.ts" })
        const onOld = thread({ id: "old", side: "old", line: 21 })
        const onNew = thread({ id: "new", side: "new", line: 22 })
        const anchors = anchorThreads([onOld, onNew], renamed)
        expect(anchors.preamble).toEqual([])
        expect(anchors.slots.map((slot) => [slot.side, slot.line, slot.threads])).toEqual([
            ["old", 21, [onOld]],
            ["new", 22, [onNew]],
        ])
    })
})
