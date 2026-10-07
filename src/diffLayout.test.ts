import { describe, expect, test } from "bun:test"
import {
    DIFF_LAYOUT_STORAGE_KEY,
    NARROW_FALLBACK_TEXT,
    SPLIT_ENTER_CH,
    SPLIT_LEAVE_CH,
    fallbackHolds,
    foldedHunk,
    hunkFirstLine,
    hunkFoldControl,
    hunkRange,
    layoutInEffect,
    lineIdentity,
    lineNumberDigits,
    lineNumberLabel,
    modelCopyText,
    navigatorFoldsForSplit,
    nextNamedSide,
    oneColumnSide,
    placeKey,
    pruneShown,
    readStoredLayout,
    splitRowIdentity,
    splitRows,
    storeLayout,
    topmostVisible,
    widthInCh,
    withShown,
    type CodePosition,
    type CopyRequest,
    type DiffLayout,
    type DiffLayoutStore,
    type SplitRow,
} from "./diffLayout"
import { escapeLine, sourceOffset } from "./hiddenChars"
import type { Hunk, Line } from "./types"

/// A hunk from git's own spelling of its lines, a marker (" ", "-" or "+")
/// then the text, numbered from the starts as the parser numbers them.
function hunk(oldStart: number, newStart: number, spec: readonly string[]): Hunk {
    let oldNo = oldStart
    let newNo = newStart
    const lines = spec.map((entry): Line => {
        const text = entry.slice(1)
        if (entry[0] === "-") return { kind: "removed", oldNo: oldNo++, newNo: null, text }
        if (entry[0] === "+") return { kind: "added", oldNo: null, newNo: newNo++, text }
        return { kind: "context", oldNo: oldNo++, newNo: newNo++, text }
    })
    return {
        oldStart,
        oldLines: oldNo - oldStart,
        newStart,
        newLines: newNo - newStart,
        section: null,
        lines,
    }
}

/// Rows as the texts each half shows, a filler as null, for readable pairs.
function texts(h: Hunk): [string | null, string | null][] {
    const text = (index: number | null) => (index === null ? null : h.lines[index].text)
    return splitRows(h).map((row) => [text(row.old), text(row.new)])
}

describe("splitRows", () => {
    test("a git change block pairs by position, the third removed line facing a filler", () => {
        const h = hunk(10, 10, [" before", "-r1", "-r2", "-r3", "+a1", "+a2", " after"])
        const rows = splitRows(h)
        expect(rows).toEqual([
            { old: 0, new: 0 },
            { old: 1, new: 4 },
            { old: 2, new: 5 },
            { old: 3, new: null },
            { old: 6, new: 6 },
        ])
        // Each context line fills one row on both sides, with both numbers.
        const numbers = (row: SplitRow) => [
            row.old === null ? null : h.lines[row.old].oldNo,
            row.new === null ? null : h.lines[row.new].newNo,
        ]
        expect(numbers(rows[0])).toEqual([10, 10])
        expect(numbers(rows[4])).toEqual([14, 13])
    })

    test("interleaved provider text pairs each added line with the open slot", () => {
        expect(texts(hunk(1, 1, ["-a", "+b", "-c", "+d"]))).toEqual([
            ["a", "b"],
            ["c", "d"],
        ])
    })

    test("an added line never pairs with a later removed line", () => {
        expect(texts(hunk(1, 1, ["+a", "-b"]))).toEqual([
            [null, "a"],
            ["b", null],
        ])
    })

    test("the open slot is the earliest unpaired left-only row", () => {
        expect(texts(hunk(1, 1, ["-a", "-b", "+c", "-d", "+e", "+f"]))).toEqual([
            ["a", "c"],
            ["b", "e"],
            ["d", "f"],
        ])
        expect(texts(hunk(1, 1, ["-a", "+b", "+c", "-d", "+e"]))).toEqual([
            ["a", "b"],
            [null, "c"],
            ["d", "e"],
        ])
    })

    test("a context line ends the block, so nothing pairs across it", () => {
        expect(texts(hunk(1, 1, ["-a", " b", "+c"]))).toEqual([
            ["a", null],
            ["b", "b"],
            [null, "c"],
        ])
    })

    test("three removed lines and one blank added line: the filler cell's rows", () => {
        const h = hunk(5, 5, ["-one", "-two", "-three", "+"])
        expect(splitRows(h)).toEqual([
            { old: 0, new: 3 },
            { old: 1, new: null },
            { old: 2, new: null },
        ])
        // The blank added line faces the first removed line with its number.
        expect(h.lines[3]).toEqual({ kind: "added", oldNo: null, newNo: 5, text: "" })
    })

    test("k removed then m added lines take max(k, m) rows, paired by position", () => {
        for (let k = 0; k <= 5; k += 1) {
            for (let m = 0; m <= 5; m += 1) {
                const spec = [...Array(k).fill("-r"), ...Array(m).fill("+a")]
                const rows = splitRows(hunk(1, 1, spec))
                expect(rows).toHaveLength(Math.max(k, m))
                rows.forEach((row, i) => {
                    expect(row).toEqual({
                        old: i < k ? i : null,
                        new: i < m ? k + i : null,
                    })
                })
            }
        }
    })

    test("over every short hunk, each line shows once per side, in order, never paired backwards", () => {
        const markers = [" ", "-", "+"]
        let specs: string[][] = [[]]
        for (let length = 1; length <= 6; length += 1) {
            specs = specs.flatMap((spec) =>
                spec.length === length - 1 ? markers.map((marker) => [...spec, `${marker}x`]) : [],
            )
            for (const spec of specs) {
                const h = hunk(1, 1, spec)
                const rows = splitRows(h)
                const onSide = (kind: Line["kind"]) =>
                    h.lines.flatMap((line, i) =>
                        line.kind === "context" || line.kind === kind ? [i] : [],
                    )
                // Each half lists exactly its side's lines, in the model's order.
                expect(rows.flatMap((row) => (row.old === null ? [] : [row.old]))).toEqual(
                    onSide("removed"),
                )
                expect(rows.flatMap((row) => (row.new === null ? [] : [row.new]))).toEqual(
                    onSide("added"),
                )
                for (const row of rows) {
                    expect(row.old !== null || row.new !== null).toBe(true)
                    if (row.old === null || row.new === null || row.old === row.new) continue
                    // A pair is a removed line and a LATER added line of the
                    // same block, with no context line between them.
                    expect(h.lines[row.old].kind).toBe("removed")
                    expect(h.lines[row.new].kind).toBe("added")
                    expect(row.old).toBeLessThan(row.new)
                    const between = h.lines.slice(row.old, row.new)
                    expect(between.some((line) => line.kind === "context")).toBe(false)
                }
            }
        }
    })

    test("rows are memoised per hunk", () => {
        const h = hunk(1, 1, [" a", "-b", "+c"])
        expect(splitRows(h)).toBe(splitRows(h))
        expect(splitRows(hunk(1, 1, [" a", "-b", "+c"]))).not.toBe(splitRows(h))
    })
})

describe("oneColumnSide", () => {
    test("an added file renders in one column of the new side", () => {
        const twenty = Array.from({ length: 20 }, (_, i) => `+line ${i + 1}`)
        expect(oneColumnSide([hunk(0, 1, twenty)])).toBe("new")
    })

    test("a deleted file renders in one column of the old side", () => {
        expect(oneColumnSide([hunk(1, 0, ["-a", "-b", "-c"])])).toBe("old")
    })

    test("a file emptied or filled from empty takes that side's column", () => {
        expect(oneColumnSide([hunk(1, 0, ["-only line"])])).toBe("old")
        expect(oneColumnSide([hunk(0, 1, ["+first", "+second"])])).toBe("new")
        // Across hunks too, while every line stays on one side.
        expect(oneColumnSide([hunk(0, 1, ["+a"]), hunk(0, 9, ["+b"])])).toBe("new")
    })

    test("a file whose only hunk adds lines between context lines keeps both columns", () => {
        expect(oneColumnSide([hunk(3, 3, [" a", "+b", "+c", " d"])])).toBeNull()
    })

    test("a file that adds and removes with no context keeps both columns", () => {
        expect(oneColumnSide([hunk(1, 1, ["-a", "+b"])])).toBeNull()
        expect(oneColumnSide([hunk(1, 0, ["-a"]), hunk(0, 1, ["+b"])])).toBeNull()
    })

    test("a single context line anywhere keeps both columns", () => {
        expect(oneColumnSide([hunk(0, 1, ["+a"]), hunk(5, 6, [" b"])])).toBeNull()
    })

    test("a file with no lines lays out no rows, and keeps the default", () => {
        expect(oneColumnSide([])).toBeNull()
        expect(oneColumnSide([hunk(1, 1, [])])).toBeNull()
    })
})

describe("hunk rows and gutters", () => {
    test("a hunk header spells out both ranges", () => {
        expect(hunkRange(hunk(10, 10, [" a", "-b", "+c", "+d", " e"]))).toBe("@@ -10,3 +10,4 @@")
        // An added file's empty old range is written as git writes it.
        expect(hunkRange(hunk(0, 1, ["+a", "+b", "+c"]))).toBe("@@ -0,0 +1,3 @@")
    })

    test("the gutter is as wide as the widest number on either side", () => {
        expect(lineNumberDigits([hunk(1, 1, [" a", "-b"])])).toBe(1)
        expect(lineNumberDigits([hunk(9, 9, [" a", "+b"])])).toBe(2)
        expect(lineNumberDigits([hunk(1, 1, [" a"]), hunk(998, 99, ["-a", "-b", "-c"])])).toBe(4)
        expect(lineNumberDigits([hunk(95, 999, [" a", "+b"])])).toBe(4)
        expect(lineNumberDigits([])).toBe(1)
    })

    test("a line-number cell is named by side, number and kind", () => {
        expect(lineNumberLabel("old", 12, "removed")).toBe("old line 12, removed")
        expect(lineNumberLabel("new", 14, "context")).toBe("new line 14, context")
    })
})

describe("line identity and switching", () => {
    test("a unified line carries its own side's number, and a context line both", () => {
        const h = hunk(12, 14, [" a", "-b", "+c"])
        expect(h.lines.map(lineIdentity)).toEqual([
            { old: 12, new: 14 },
            { old: 13, new: null },
            { old: null, new: 15 },
        ])
    })

    test("a side-by-side row carries its left line's old number and its right line's new one", () => {
        const h = hunk(10, 10, [" a", "-b", "-c", "-d", "+e", "+f", " g"])
        expect(splitRows(h).map((row) => splitRowIdentity(h, row))).toEqual([
            { old: 10, new: 10 },
            { old: 11, new: 11 },
            { old: 12, new: 12 },
            { old: 13, new: null },
            { old: 14, new: 13 },
        ])
        const added = hunk(1, 1, ["+a", "-b"])
        expect(splitRows(added).map((row) => splitRowIdentity(added, row))).toEqual([
            { old: null, new: 1 },
            { old: 1, new: null },
        ])
    })

    test("the place is kept by the old number, else the new", () => {
        expect(placeKey({ old: 40, new: 42 })).toEqual({ side: "old", line: 40 })
        expect(placeKey({ old: 40, new: null })).toEqual({ side: "old", line: 40 })
        expect(placeKey({ old: null, new: 41 })).toEqual({ side: "new", line: 41 })
        expect(placeKey({ old: null, new: null })).toBeNull()
    })

    test("a switch keeps the place: the row holding old line 40 is found in either layout", () => {
        // Removed old 40 is the topmost line in unified; side by side it
        // shares a row with added new 40, and that row carries old 40.
        const h = hunk(38, 38, [" a", " b", "-c", "+d"])
        const unifiedKey = placeKey(lineIdentity(h.lines[2]))
        const rows = splitRows(h).map((row) => splitRowIdentity(h, row))
        expect(rows.findIndex((row) => row.old === unifiedKey?.line)).toBe(2)
        // And back: the context row old 39, new 39 keeps its old number.
        expect(placeKey(rows[1])).toEqual({ side: "old", line: 39 })
        expect(lineIdentity(h.lines[1]).old).toBe(39)
    })

    test("the topmost visible row is the first whose bottom lies below the port's top", () => {
        // Rows 20 px tall, laid out from 100: bottoms at 120, 140, …
        const bottoms = [120, 140, 160, 180, 200]
        const at = (line: number) => topmostVisible(bottoms.length, (i) => bottoms[i], line)
        expect(at(0)).toBe(0)
        expect(at(119)).toBe(0)
        // A row whose bottom sits exactly on the line is no longer visible.
        expect(at(120)).toBe(1)
        expect(at(121)).toBe(1)
        expect(at(199)).toBe(4)
        expect(at(200)).toBe(5)
        expect(topmostVisible(0, () => 0, 0)).toBe(0)
    })

    test("finding the topmost row reads only a handful of boxes", () => {
        const read: number[] = []
        topmostVisible(
            21000,
            (i) => {
                read.push(i)
                return (i + 1) * 18
            },
            180000,
        )
        expect(read.length).toBeLessThanOrEqual(16)
    })
})

describe("the width decisions", () => {
    test("the thresholds are 104 and 96 ch", () => {
        expect(SPLIT_ENTER_CH).toBe(104)
        expect(SPLIT_LEAVE_CH).toBe(96)
    })

    test("side by side holds between the thresholds", () => {
        let inEffect = layoutInEffect("split", 110, "unified")
        expect(inEffect).toBe("split")
        inEffect = layoutInEffect("split", 100, inEffect)
        expect(inEffect).toBe("split")
        inEffect = layoutInEffect("split", 95, inEffect)
        expect(inEffect).toBe("unified")
        inEffect = layoutInEffect("split", 100, inEffect)
        expect(inEffect).toBe("unified")
        inEffect = layoutInEffect("split", 104, inEffect)
        expect(inEffect).toBe("split")
    })

    test("side by side comes into effect at exactly 104 ch, not just under it", () => {
        expect(layoutInEffect("split", 104, "unified")).toBe("split")
        expect(layoutInEffect("split", 103.99, "unified")).toBe("unified")
        expect(layoutInEffect("split", 96, "unified")).toBe("unified")
    })

    test("once in effect, side by side stays at exactly 96 ch and leaves just under it", () => {
        expect(layoutInEffect("split", 96, "split")).toBe("split")
        expect(layoutInEffect("split", 95.99, "split")).toBe("unified")
        expect(layoutInEffect("split", 104, "split")).toBe("split")
    })

    test("unified chosen is unified at any width", () => {
        for (const width of [0, 80, 95.99, 96, 103.99, 104, 200, 10_000, Infinity]) {
            for (const previous of ["unified", "split"] as const) {
                expect(layoutInEffect("unified", width, previous)).toBe("unified")
            }
        }
    })

    test("the fallback says why and keeps the choice", () => {
        const store = fakeStore("split")
        const chosen = readStoredLayout(store)
        expect(chosen).toBe("split")
        for (const previous of ["unified", "split"] as const) {
            const inEffect = layoutInEffect(chosen, 80, previous)
            expect(inEffect).toBe("unified")
            // "Side by side" stays checked, since the control shows `chosen`.
            expect(fallbackHolds(chosen, inEffect)).toBe(true)
        }
        expect(NARROW_FALLBACK_TEXT).toBe("Too narrow — showing unified")
        expect(store.data.get(DIFF_LAYOUT_STORAGE_KEY)).toBe("split")
    })

    test("no fallback while unified is chosen or side by side is in effect", () => {
        expect(fallbackHolds("unified", "unified")).toBe(false)
        expect(fallbackHolds("split", "split")).toBe(false)
    })

    test("a width converts to ch of the code font, and an unmeasured probe reads as too narrow", () => {
        expect(widthInCh(832, 8)).toBe(104)
        expect(widthInCh(1000, 10)).toBe(100)
        for (const ch of [0, -8, Number.NaN, Infinity]) {
            const width = widthInCh(1600, ch)
            expect(width).toBeNaN()
            expect(layoutInEffect("split", width, "split")).toBe("unified")
        }
    })

    test("zoom moves the threshold: the same column in larger ch falls back", () => {
        // 960px is 120 ch at 8px per ch; zoomed in, ch measures 10.67px.
        expect(layoutInEffect("split", widthInCh(960, 8), "unified")).toBe("split")
        expect(layoutInEffect("split", widthInCh(960, 960 / 90), "split")).toBe("unified")
    })
})

describe("navigatorFoldsForSplit", () => {
    test("folds while side by side is chosen and the sections beside it would be under 104 ch", () => {
        expect(navigatorFoldsForSplit("split", 143.99, 40)).toBe(true)
        expect(navigatorFoldsForSplit("split", 144, 40)).toBe(false)
    })

    test("unified chosen keeps the navigator beside the sections", () => {
        // 120 ch less a 40 ch navigator leaves 80 ch: under 104, yet no fold.
        expect(navigatorFoldsForSplit("unified", 120, 40)).toBe(false)
    })

    test("widening never turns side by side off", () => {
        const navigator = 40
        let inEffect: DiffLayout = "unified"
        let folded = false
        for (let view = 110; view <= 220; view += 0.5) {
            folded = navigatorFoldsForSplit("split", view, navigator)
            const sections = folded ? view : view - navigator
            inEffect = layoutInEffect("split", sections, inEffect)
            expect(inEffect).toBe("split")
            // Folded, until the navigator fits beside sections of 104 ch.
            expect(folded).toBe(view - navigator < 104)
        }
        expect(folded).toBe(false)
    })
})

function fakeStore(initial?: string): DiffLayoutStore & { data: Map<string, string> } {
    const data = new Map<string, string>()
    if (initial !== undefined) data.set(DIFF_LAYOUT_STORAGE_KEY, initial)
    return {
        data,
        getItem: (key) => data.get(key) ?? null,
        setItem: (key, value) => {
            data.set(key, value)
        },
    }
}

const throwingStore: DiffLayoutStore = {
    getItem: () => {
        throw new Error("blocked")
    },
    setItem: () => {
        throw new Error("blocked")
    },
}

describe("the stored choice", () => {
    test("the choice lives under its own view-state key", () => {
        expect(DIFF_LAYOUT_STORAGE_KEY).toBe("specforge.diffLayout")
    })

    test("no stored value reads as unified, the default", () => {
        expect(readStoredLayout(fakeStore())).toBe("unified")
    })

    test("only exactly split reads as side by side", () => {
        expect(readStoredLayout(fakeStore("split"))).toBe("split")
        for (const value of ["Split", "split ", "side-by-side", " split", "SPLIT", "", "unified"]) {
            expect(readStoredLayout(fakeStore(value))).toBe("unified")
        }
    })

    test("a read that throws, or no store at all, reads as unified", () => {
        expect(readStoredLayout(throwingStore)).toBe("unified")
        expect(readStoredLayout(null)).toBe("unified")
    })

    test("a write that throws reports failure instead of throwing", () => {
        expect(storeLayout("split", throwingStore)).toBe(false)
        expect(storeLayout("split", null)).toBe(false)
    })

    test("a refused write leaves the stored choice for the next diff", () => {
        const store = fakeStore("unified")
        const refusing: DiffLayoutStore = {
            getItem: store.getItem,
            setItem: () => {
                throw new Error("quota")
            },
        }
        expect(storeLayout("split", refusing)).toBe(false)
        expect(readStoredLayout(store)).toBe("unified")
    })

    test("a choice round-trips under the key", () => {
        const store = fakeStore()
        expect(storeLayout("split", store)).toBe(true)
        expect(store.data.get(DIFF_LAYOUT_STORAGE_KEY)).toBe("split")
        expect(readStoredLayout(store)).toBe("split")
        expect(storeLayout("unified", store)).toBe(true)
        expect(readStoredLayout(store)).toBe("unified")
    })
})

describe("nextNamedSide", () => {
    test("a click ends the named side", () => {
        let named = nextNamedSide("new", { type: "pointerDown", codeCell: "old" })
        expect(named).toBe("old")
        named = nextNamedSide(named, { type: "pointerUp", collapsed: true, insideView: true })
        expect(named).toBeNull()
    })

    test("a drag keeps the side its pointer-down named, carried across files", () => {
        // The pointer-down names its side; the collapsed caret it leaves is
        // not consulted until the pointer comes up.
        let named = nextNamedSide(null, { type: "pointerDown", codeCell: "new" })
        expect(named).toBe("new")
        named = nextNamedSide(named, { type: "pointerUp", collapsed: false, insideView: true })
        expect(named).toBe("new")
    })

    test("a pointer-down outside a code cell clears the side", () => {
        expect(nextNamedSide("old", { type: "pointerDown", codeCell: null })).toBeNull()
        expect(nextNamedSide(null, { type: "pointerDown", codeCell: null })).toBeNull()
    })

    test("a pointer-up leaving the selection outside the view clears the side", () => {
        expect(
            nextNamedSide("old", { type: "pointerUp", collapsed: false, insideView: false }),
        ).toBeNull()
    })

    test("a unified code cell neither names nor clears a side", () => {
        expect(nextNamedSide("old", { type: "pointerDown", codeCell: "unified" })).toBe("old")
        expect(nextNamedSide(null, { type: "pointerDown", codeCell: "unified" })).toBeNull()
    })
})

describe("modelCopyText", () => {
    const FILE = "src/numbers.ts"
    // Old lines 10–12 and new lines 10–12, with old 11 and new 11 shared.
    const changed = hunk(10, 10, [
        "-const a = 1;",
        "+const a = 2;",
        " const b = 3;",
        "-const c = 4;",
        "+const c = 5;",
    ])
    const at = (h: number, line: number, offset: number): CodePosition => ({
        file: FILE,
        hunk: h,
        line,
        offset,
    })
    const request = (overrides: Partial<CopyRequest>): CopyRequest => ({
        start: null,
        end: null,
        inEffect: "split",
        namedSide: null,
        hunksOf: (key) => (key === FILE ? [changed] : undefined),
        ...overrides,
    })

    test("copying one side yields that side's code, from mid old line 10 to mid old line 12", () => {
        const text = modelCopyText(
            request({ namedSide: "old", start: at(0, 0, 6), end: at(0, 3, 7) }),
        )
        expect(text).toBe("a = 1;\nconst b = 3;\nconst c")
        // No line number, marker or new-side text.
        expect(text).not.toContain("2")
        expect(text).not.toContain("5")
        expect(text).not.toMatch(/^[-+]/m)
    })

    test("the new side reads its own lines", () => {
        expect(
            modelCopyText(request({ namedSide: "new", start: at(0, 1, 6), end: at(0, 4, 13) })),
        ).toBe("a = 2;\nconst b = 3;\nconst c = 5;")
    })

    test("copying in unified yields the lines as shown", () => {
        const text = modelCopyText(
            request({ inEffect: "unified", start: at(0, 0, 0), end: at(0, 1, 13) }),
        )
        expect(text).toBe("const a = 1;\nconst a = 2;")
    })

    test("unified copies the lines as shown, whatever side was named before", () => {
        expect(
            modelCopyText(
                request({
                    inEffect: "unified",
                    namedSide: "old",
                    start: at(0, 0, 0),
                    end: at(0, 2, 5),
                }),
            ),
        ).toBe("const a = 1;\nconst a = 2;\nconst")
    })

    test("ends in either order give the same text", () => {
        const forward = modelCopyText(
            request({ namedSide: "old", start: at(0, 0, 6), end: at(0, 3, 7) }),
        )
        const backward = modelCopyText(
            request({ namedSide: "old", start: at(0, 3, 7), end: at(0, 0, 6) }),
        )
        expect(backward).toBe(forward)
    })

    test("a selection inside one line yields its part of that line", () => {
        expect(
            modelCopyText(request({ namedSide: "old", start: at(0, 2, 6), end: at(0, 2, 7) })),
        ).toBe("b")
    })

    test("a selection that crosses a hunk boundary yields the lines alone", () => {
        const first = hunk(1, 1, [" one", "-two", "+TWO"])
        const second = hunk(40, 40, [" forty", "-forty-one", "+FORTY-ONE"])
        const hunksOf = (key: string) => (key === FILE ? [first, second] : undefined)
        expect(
            modelCopyText(
                request({ hunksOf, namedSide: "old", start: at(0, 1, 0), end: at(1, 1, 5) }),
            ),
        ).toBe("two\nforty\nforty")
        expect(
            modelCopyText(
                request({ hunksOf, inEffect: "unified", start: at(0, 2, 0), end: at(1, 0, 5) }),
            ),
        ).toBe("TWO\nforty")
    })

    test("a selection without a named side copies in document order", () => {
        // Select-all from the keyboard: its ends lie outside any code cell.
        expect(modelCopyText(request({ namedSide: null }))).toBeNull()
    })

    test("side by side with both ends in code cells of one file, but no side named, takes document order", () => {
        expect(
            modelCopyText(request({ namedSide: null, start: at(0, 0, 0), end: at(0, 3, 4) })),
        ).toBeNull()
    })

    test("a selection reaching a header or a hunk row copies in document order", () => {
        expect(
            modelCopyText(request({ namedSide: "old", start: null, end: at(0, 0, 4) })),
        ).toBeNull()
        expect(
            modelCopyText(request({ inEffect: "unified", start: at(0, 0, 4), end: null })),
        ).toBeNull()
    })

    test("a selection spanning files copies in document order", () => {
        expect(
            modelCopyText(
                request({
                    inEffect: "unified",
                    start: at(0, 0, 0),
                    end: { file: "src/other.ts", hunk: 0, line: 0, offset: 1 },
                }),
            ),
        ).toBeNull()
    })

    test("an end on the other side's line, or nowhere in the model, copies in document order", () => {
        expect(
            modelCopyText(request({ namedSide: "old", start: at(0, 0, 0), end: at(0, 1, 3) })),
        ).toBeNull()
        expect(
            modelCopyText(request({ namedSide: "new", start: at(0, 0, 0), end: at(0, 1, 3) })),
        ).toBeNull()
        expect(
            modelCopyText(request({ inEffect: "unified", start: at(0, 0, 0), end: at(0, 9, 0) })),
        ).toBeNull()
        expect(
            modelCopyText(
                request({
                    inEffect: "unified",
                    hunksOf: () => undefined,
                    start: at(0, 0, 0),
                    end: at(0, 1, 0),
                }),
            ),
        ).toBeNull()
    })

    test("escaped characters copy as themselves", () => {
        const zeroWidth = hunk(1, 1, [" let zero = '​';"])
        const line = zeroWidth.lines[0]
        const segments = escapeLine(line).segments
        const escape = segments.find((segment) => segment.kind === "escape")
        if (escape?.kind !== "escape")
            throw new Error("expected the zero-width space to be escaped")
        // The selection ends inside the drawn escape, after its first glyph.
        const end = sourceOffset(escape, 3)
        const text = modelCopyText(
            request({
                inEffect: "unified",
                hunksOf: () => [zeroWidth],
                start: at(0, 0, 0),
                end: at(0, 0, end),
            }),
        )
        expect(text).toBe("let zero = '​")
        expect(text).not.toContain(escape.label)
    })
})

// ---- Folded hunks (`diff-view`: *Folded Hunks*) --------------------------

describe("folded hunks", () => {
    const twelve = hunk(140, 143, [" a", "+b", ...Array.from({ length: 10 }, (_, n) => ` c${n}`)])

    // *The show control is announced with its state*.
    test("the control says how many lines, and is named by the first line", () => {
        expect(twelve.lines.length).toBe(12)
        expect(hunkFoldControl(twelve, false)).toEqual({
            text: "Show 12 lines",
            label: "Show 12 lines from new line 143",
        })
        expect(hunkFoldControl(twelve, true)).toEqual({
            text: "Hide",
            label: "Hide 12 lines from new line 143",
        })
        const one = hunk(7, 7, ["+x"])
        expect(hunkFoldControl(one, false).text).toBe("Show 1 line")
    })

    test("a hunk that only removes is named by its old line", () => {
        expect(hunkFirstLine(hunk(12, 11, ["-gone", "-also"]))).toBe("old line 12")
        expect(hunkFirstLine(hunk(12, 11, ["-gone", "+new"]))).toBe("old line 12")
        expect(hunkFirstLine(hunk(12, 11, [" kept", "-gone"]))).toBe("new line 11")
        expect(hunkFirstLine({ ...hunk(1, 5, []), lines: [] })).toBe("new line 5")
    })

    test("a hunk is folded while the host folds it and the reader has not shown it", () => {
        const host = new Set([1, 2])
        const shown = withShown(new Map(), "a.ts", 2, true)
        expect(foldedHunk(host, shown, "a.ts", 1)).toBe(true)
        expect(foldedHunk(host, shown, "a.ts", 2)).toBe(false)
        expect(foldedHunk(host, shown, "a.ts", 0)).toBe(false)
        // Shown per file.
        expect(foldedHunk(host, shown, "b.ts", 2)).toBe(true)
        expect(foldedHunk(undefined, shown, "a.ts", 1)).toBe(false)
    })

    test("showing and hiding keep one set per file, and drop an empty one", () => {
        const shown = withShown(withShown(new Map(), "a.ts", 1, true), "a.ts", 3, true)
        expect([...(shown.get("a.ts") ?? [])]).toEqual([1, 3])
        const hidden = withShown(withShown(shown, "a.ts", 1, false), "a.ts", 3, false)
        expect(hidden.has("a.ts")).toBe(false)
    })

    // *A hunk the host folds again folds*.
    test("a shown hunk the host stops folding is forgotten, and nothing else", () => {
        const shown = withShown(withShown(new Map(), "a.ts", 1, true), "b.ts", 0, true)
        const same = pruneShown(shown, () => new Set([0, 1]))
        expect(same).toBe(shown)
        const pruned = pruneShown(shown, (key) => (key === "a.ts" ? new Set([2]) : new Set([0])))
        expect(pruned.has("a.ts")).toBe(false)
        expect([...(pruned.get("b.ts") ?? [])]).toEqual([0])
        expect(pruneShown(shown, () => undefined).size).toBe(0)
    })
})

describe("modelCopyText across a folded hunk", () => {
    const FILE = "src/three.ts"
    const hunks = [
        hunk(1, 1, [" one", "-two", "+TWO"]),
        hunk(20, 20, [" hidden", "+folded"]),
        hunk(40, 41, ["-three", "+THREE", " four"]),
    ]
    const at = (h: number, line: number, offset: number): CodePosition => ({
        file: FILE,
        hunk: h,
        line,
        offset,
    })
    const request = (overrides: Partial<CopyRequest>): CopyRequest => ({
        start: at(0, 0, 0),
        end: at(2, 2, 4),
        inEffect: "unified",
        namedSide: null,
        hunksOf: (key) => (key === FILE ? hunks : undefined),
        ...overrides,
    })

    // *A folded hunk inside a selection is not copied*.
    test("the lines of a folded hunk between the ends are left out", () => {
        const folded = (key: string, h: number) => key === FILE && h === 1
        expect(modelCopyText(request({ folded }))).toBe("one\ntwo\nTWO\nthree\nTHREE\nfour")
        expect(modelCopyText(request({}))).toBe(
            "one\ntwo\nTWO\nhidden\nfolded\nthree\nTHREE\nfour",
        )
        // On a named side as well.
        expect(
            modelCopyText(request({ folded, inEffect: "split", namedSide: "new" })),
        ).toBe("one\nTWO\nTHREE\nfour")
    })
})
