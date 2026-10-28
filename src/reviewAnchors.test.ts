import { describe, expect, test } from "bun:test"
import {
    anchorDraftItems,
    anchorInlineDrafts,
    anchorThreads,
    fileKeysByPath,
    renderedLines,
} from "./reviewAnchors"
import type {
    DiffFile,
    Hunk,
    Line,
    PlacedInlineItem,
    PlacedReplyItem,
    PullRequestComment,
    ReviewThread,
    UnplacedInlineItem,
    UnplacedReplyItem,
} from "./types"

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


// ---- Draft comments and replies ------------------------------------------

function placedInline(
    id: string,
    path: string,
    side: "old" | "new",
    line: number,
): PlacedInlineItem {
    return { kind: "placedInline", id, body: `Draft ${id}`, path, side, line }
}

function unplacedInline(
    id: string,
    path: string,
    side: "old" | "new",
    line: number,
): UnplacedInlineItem {
    return {
        kind: "unplacedInline",
        id,
        body: `Former ${id}`,
        path,
        side,
        line,
        basis: { head: "old-head", base: "old-base" },
    }
}

function placedReply(id: string, target: string): PlacedReplyItem {
    return { kind: "placedReply", id, body: `Reply ${id}`, target }
}

function unplacedReply(id: string, target: string): UnplacedReplyItem {
    return { kind: "unplacedReply", id, body: `Former reply ${id}`, target }
}

describe("renderedLines", () => {
    test("a context line belongs to both sides; a removed and an added line to one each", () => {
        const rendered = renderedLines(file())
        expect([...rendered.old].sort((a, b) => a - b)).toEqual([20, 21])
        expect([...rendered.new].sort((a, b) => a - b)).toEqual([20, 22])
    })

    test("a folded hunk contributes none of its lines", () => {
        const twoHunks = file({ content: { kind: "hunks", hunks: [hunk(), laterHunk()] } })
        const rendered = renderedLines(twoHunks, new Set([0]))
        expect([...rendered.old]).toEqual([100])
        expect([...rendered.new]).toEqual([100])
    })

    test("a file with no hunks renders no line", () => {
        expect(renderedLines(file({ content: { kind: "withheld" } }))).toEqual({
            old: new Set(),
            new: new Set(),
        })
    })
})

describe("fileKeysByPath", () => {
    test("a renamed file answers to both its new and old path under its new-path identity", () => {
        const renamed = file({ oldPath: "src/old.ts", newPath: "src/new.ts" })
        const keys = fileKeysByPath([renamed])
        expect(keys.get("src/new.ts")).toBe("src/new.ts")
        expect(keys.get("src/old.ts")).toBe("src/new.ts")
    })

    test("a deleted file answers to its old path under that identity", () => {
        const deleted = file({
            oldPath: "src/gone.ts",
            newPath: null,
            status: { kind: "deleted" },
        })
        expect(fileKeysByPath([deleted]).get("src/gone.ts")).toBe("src/gone.ts")
    })

    test("a path one file was renamed to wins over another file's former path", () => {
        const renamed = file({ oldPath: "src/old.ts", newPath: "src/shared.ts" })
        const deleted = file({
            oldPath: "src/shared.ts",
            newPath: null,
            status: { kind: "deleted" },
        })
        expect(fileKeysByPath([renamed, deleted]).get("src/shared.ts")).toBe("src/shared.ts")
    })
})

describe("anchorInlineDrafts", () => {
    test("a file with no draft comments gets no slot and no preamble", () => {
        expect(anchorInlineDrafts([], file())).toEqual({ slots: [], preamble: [] })
    })

    test("a placed new-side comment sits on its rendered line", () => {
        const draft = placedInline("d1", "src/api.ts", "new", 22)
        const anchors = anchorInlineDrafts([draft], file())
        expect(anchors.preamble).toEqual([])
        expect(anchors.slots).toHaveLength(1)
        expect(anchors.slots[0]).toEqual({ side: "new", line: 22, items: [draft] })
    })

    test("a removed line's comment takes the old side", () => {
        const draft = placedInline("d1", "src/api.ts", "old", 21)
        const anchors = anchorInlineDrafts([draft], file())
        expect(anchors.slots.map((slot) => [slot.side, slot.line])).toEqual([["old", 21]])
    })

    test("a context line carries old and new comments in independent slots", () => {
        const old = placedInline("old", "src/api.ts", "old", 20)
        const next = placedInline("new", "src/api.ts", "new", 20)
        const anchors = anchorInlineDrafts([old, next], file())
        expect(anchors.slots.map((slot) => [slot.side, slot.line, slot.items])).toEqual([
            ["old", 20, [old]],
            ["new", 20, [next]],
        ])
    })

    test("comments on one line share one slot in draft order", () => {
        const first = placedInline("first", "src/api.ts", "new", 22)
        const second = placedInline("second", "src/api.ts", "new", 22)
        const anchors = anchorInlineDrafts([first, second], file())
        expect(anchors.slots).toHaveLength(1)
        expect(anchors.slots[0]!.items).toEqual([first, second])
    })

    test("a comment on an absent line keeps the preamble", () => {
        const absent = placedInline("absent", "src/api.ts", "new", 99)
        const anchors = anchorInlineDrafts([absent], file())
        expect(anchors.slots).toEqual([])
        expect(anchors.preamble).toEqual([absent])
    })

    test("a folded hunk's comment keeps the preamble while another hunk's stays on its line", () => {
        const twoHunks = file({ content: { kind: "hunks", hunks: [hunk(), laterHunk()] } })
        const inFirst = placedInline("first", "src/api.ts", "new", 22)
        const inSecond = placedInline("second", "src/api.ts", "new", 100)
        const anchors = anchorInlineDrafts([inFirst, inSecond], twoHunks, new Set([0]))
        expect(anchors.preamble).toEqual([inFirst])
        expect(anchors.slots.map((slot) => [slot.line, slot.items])).toEqual([[100, [inSecond]]])
    })

    test("a withheld file renders no line, so every comment keeps the preamble", () => {
        const withheld = file({ content: { kind: "withheld" } })
        const draft = placedInline("d1", "src/api.ts", "new", 22)
        expect(anchorInlineDrafts([draft], withheld)).toEqual({
            slots: [],
            preamble: [draft],
        })
    })

    test("a comment naming another file keeps the preamble", () => {
        const other = placedInline("d1", "src/other.ts", "new", 22)
        expect(anchorInlineDrafts([other], file())).toEqual({
            slots: [],
            preamble: [other],
        })
    })

    test("a deleted file places its old-side comment", () => {
        const deleted = file({
            newPath: null,
            status: { kind: "deleted" },
            content: { kind: "hunks", hunks: [hunk()] },
        })
        const draft = placedInline("d1", "src/api.ts", "old", 21)
        expect(anchorInlineDrafts([draft], deleted).slots).toEqual([
            { side: "old", line: 21, items: [draft] },
        ])
    })
})

describe("anchorDraftItems", () => {
    const renamed = file({ oldPath: "src/old.ts", newPath: "src/new.ts" })
    const deleted = file({
        oldPath: "src/gone.ts",
        newPath: null,
        status: { kind: "deleted" },
    })

    test("a draft with no items gets empty groups", () => {
        expect(anchorDraftItems([], [renamed, deleted], [])).toEqual({
            inlineByFile: new Map(),
            unplacedInlineByFile: new Map(),
            inlineAfter: [],
            repliesByThread: new Map(),
            unplacedReplies: [],
            orphanReplies: [],
        })
    })

    test("a placed comment groups under the file identity it names", () => {
        const first = placedInline("first", "src/new.ts", "new", 22)
        const second = placedInline("second", "src/gone.ts", "old", 21)
        const anchors = anchorDraftItems([first, second], [renamed, deleted], [])
        expect(anchors.inlineByFile).toEqual(
            new Map([
                ["src/new.ts", [first]],
                ["src/gone.ts", [second]],
            ]),
        )
        expect(anchors.inlineAfter).toEqual([])
    })

    test("a placed comment on a file this detail does not list follows the files", () => {
        const gone = placedInline("gone", "src/elsewhere.ts", "new", 4)
        const anchors = anchorDraftItems([gone], [renamed, deleted], [])
        expect(anchors.inlineByFile.size).toBe(0)
        expect(anchors.inlineAfter).toEqual([gone])
    })

    test("an unplaced comment on a renamed file's former path groups at that file", () => {
        const former = unplacedInline("former", "src/old.ts", "new", 22)
        const anchors = anchorDraftItems([former], [renamed], [])
        expect(anchors.unplacedInlineByFile).toEqual(new Map([["src/new.ts", [former]]]))
        expect(anchors.inlineAfter).toEqual([])
    })

    test("an unplaced comment on a deleted file's path groups at the deleted file", () => {
        const former = unplacedInline("former", "src/gone.ts", "old", 21)
        const anchors = anchorDraftItems([former], [deleted], [])
        expect(anchors.unplacedInlineByFile).toEqual(new Map([["src/gone.ts", [former]]]))
    })

    test("an unplaced comment whose former path is unlisted follows the files, never a same-line file", () => {
        const former = unplacedInline("former", "src/elsewhere.ts", "new", 22)
        const anchors = anchorDraftItems([former], [renamed, deleted], [])
        expect(anchors.unplacedInlineByFile.size).toBe(0)
        expect(anchors.inlineAfter).toEqual([former])
    })

    test("a reply groups under its current target thread", () => {
        const target = thread({ id: "target" })
        const reply = placedReply("reply", "target")
        const anchors = anchorDraftItems([reply], [renamed], [target])
        expect(anchors.repliesByThread).toEqual(new Map([["target", [reply]]]))
        expect(anchors.orphanReplies).toEqual([])
    })

    test("a reply whose target is not in this detail follows the files rather than vanishing", () => {
        const orphan = placedReply("orphan", "gone-thread")
        const anchors = anchorDraftItems([orphan], [renamed], [])
        expect(anchors.repliesByThread.size).toBe(0)
        expect(anchors.orphanReplies).toEqual([orphan])
    })

    test("an unplaced reply keeps its former target and follows the files", () => {
        const former = unplacedReply("former", "gone-thread")
        const anchors = anchorDraftItems([former], [renamed], [])
        expect(anchors.unplacedReplies).toEqual([former])
        expect(anchors.orphanReplies).toEqual([])
    })

    test("groups keep the draft's item order", () => {
        const first = placedInline("first", "src/new.ts", "new", 22)
        const other = placedInline("other", "src/gone.ts", "old", 21)
        const second = placedInline("second", "src/new.ts", "new", 22)
        const anchors = anchorDraftItems([first, other, second], [renamed, deleted], [])
        expect(anchors.inlineByFile.get("src/new.ts")).toEqual([first, second])
        expect(anchors.inlineByFile.get("src/gone.ts")).toEqual([other])
    })
})
