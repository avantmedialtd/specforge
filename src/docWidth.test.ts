import { describe, expect, test } from "bun:test"
import {
    DEFAULT_DOCUMENT_WIDTH,
    DOC_WIDTHS,
    DOC_WIDTH_LABELS,
    DOC_WIDTH_ORDER,
    DOC_WIDTH_STORAGE_KEY,
    docWidthTokens,
    normalizeDocumentWidth,
    readMirroredDocumentWidth,
    writeMirroredDocumentWidth,
    type DocWidthStore,
} from "./docWidth"
import type { DocumentWidth } from "./types"

/// An upper bound for `1ch` of the prose font at `--text-lg`.
///
/// MEASURED, not assumed: Inter Variable's zero advance at 16px is 10.09px,
/// read off the rendered document. An earlier version of this constant used
/// 8.4px from the stylesheet's own arithmetic, which was wrong — that figure
/// is the AVERAGE prose character (~7.6px), not the digit advance the `ch`
/// unit is defined by. 10.5 leaves headroom above the real value without
/// becoming a font-version tripwire.
const MAX_CH_PX = 10.5

const px = (value: string) => Number.parseFloat(value)
const ch = (value: string) => Number.parseFloat(value)

const BOUNDED: DocumentWidth[] = ["compact", "default", "wide"]

describe("the ladder", () => {
    test("each rung carries the pair the spec tabulates", () => {
        expect(DOC_WIDTHS.compact).toEqual({ column: "720px", measure: "50ch" })
        expect(DOC_WIDTHS.default).toEqual({ column: "880px", measure: "74ch" })
        expect(DOC_WIDTHS.wide).toEqual({ column: "1040px", measure: "86ch" })
        expect(DOC_WIDTHS.full).toEqual({ column: "none", measure: "96ch" })
    })

    test("the default rung is the pre-existing rendering", () => {
        // The two literals this change removed from App.css. If a future edit
        // retunes the default rung it must be a deliberate visual-identity
        // change, not a side effect of adding a rung.
        expect(DOC_WIDTHS[DEFAULT_DOCUMENT_WIDTH].column).toBe("880px")
        expect(DOC_WIDTHS[DEFAULT_DOCUMENT_WIDTH].measure).toBe("74ch")
    })

    test("prose is bounded at every rung, including the widest", () => {
        for (const rung of DOC_WIDTH_ORDER) {
            const { measure } = DOC_WIDTHS[rung]
            expect(measure.endsWith("ch")).toBe(true)
            expect(Number.isFinite(ch(measure))).toBe(true)
            expect(ch(measure)).toBeGreaterThan(0)
        }
    })

    test("only the widest rung lets objects take the surface", () => {
        expect(DOC_WIDTHS.full.column).toBe("none")
        for (const rung of BOUNDED) {
            expect(DOC_WIDTHS[rung].column.endsWith("px")).toBe(true)
        }
    })

    test("the bounded rungs are monotonic in both tiers", () => {
        for (let i = 1; i < BOUNDED.length; i++) {
            const prev = DOC_WIDTHS[BOUNDED[i - 1]!]
            const next = DOC_WIDTHS[BOUNDED[i]!]
            expect(px(next.column)).toBeGreaterThan(px(prev.column))
            expect(ch(next.measure)).toBeGreaterThan(ch(prev.measure))
        }
    })

    /// Measured off the rendered document, in Inter Variable at `--text-lg`:
    /// the average prose character, against the digit-zero advance the `ch`
    /// unit is defined by. The gap between them is why the ladder's rungs
    /// cannot be read as character counts directly.
    const PROSE_CHAR_PX = 7.56
    const CH_PX = 10.09
    const charsAt = (measure: string) => (ch(measure) * CH_PX) / PROSE_CHAR_PX

    test("the narrow rung reaches a comfortable measure", () => {
        // The entire reason `compact` is 50ch rather than stepping evenly with
        // the column. A reader picks it because the text feels wide; at 62ch
        // it delivered ~83 characters, which is still outside the range
        // conventionally called comfortable, and the rung did not do its job.
        expect(charsAt(DOC_WIDTHS.compact.measure)).toBeLessThanOrEqual(75)
    })

    test("the widest rung still refuses a runaway line", () => {
        // `full` unbinds the objects, not the text.
        expect(charsAt(DOC_WIDTHS.full.measure)).toBeLessThanOrEqual(130)
    })

    test("the default rung is left where it was, wide line and all", () => {
        // ~97 characters — above the comfortable range, and deliberately not
        // touched: it is what every existing install already renders.
        expect(Math.round(charsAt(DOC_WIDTHS.default.measure))).toBeGreaterThan(90)
    })

    test("prose never exceeds the object column at a bounded rung", () => {
        // The invariant `visual-identity` requires of every rung. `full` is
        // exempt only because an unbounded column trivially satisfies it.
        for (const rung of BOUNDED) {
            const { column, measure } = DOC_WIDTHS[rung]
            expect(ch(measure) * MAX_CH_PX).toBeLessThanOrEqual(px(column))
        }
    })

    test("every rung has an order entry and a label", () => {
        const rungs = Object.keys(DOC_WIDTHS) as DocumentWidth[]
        expect([...DOC_WIDTH_ORDER].sort()).toEqual([...rungs].sort())
        for (const rung of rungs) {
            expect(DOC_WIDTH_LABELS[rung]).toBeTruthy()
        }
    })
})

describe("normalizeDocumentWidth", () => {
    test("passes every known rung through unchanged", () => {
        for (const rung of DOC_WIDTH_ORDER) {
            expect(normalizeDocumentWidth(rung)).toBe(rung)
        }
    })

    test("folds anything unrecognised to the default rung", () => {
        // A settings file from a newer version, a hand edit, a failed read.
        for (const value of [
            "ultrawide",
            "",
            "Default",
            undefined,
            null,
            42,
            {},
            [],
            true,
        ]) {
            expect(normalizeDocumentWidth(value)).toBe(DEFAULT_DOCUMENT_WIDTH)
        }
    })

    test("does not admit inherited Object properties as rungs", () => {
        // `value in DOC_WIDTHS` walks the prototype chain, so a value like
        // "constructor" would pass a naive membership test and index to
        // something that is not a rung.
        expect(normalizeDocumentWidth("constructor")).toBe(DEFAULT_DOCUMENT_WIDTH)
        expect(normalizeDocumentWidth("toString")).toBe(DEFAULT_DOCUMENT_WIDTH)
        expect(normalizeDocumentWidth("__proto__")).toBe(DEFAULT_DOCUMENT_WIDTH)
    })
})

describe("docWidthTokens", () => {
    test("returns the rung's tokens", () => {
        expect(docWidthTokens("wide")).toEqual(DOC_WIDTHS.wide)
    })

    test("is total — an unrecognised value still yields the default tokens", () => {
        expect(docWidthTokens("ultrawide")).toEqual(DOC_WIDTHS.default)
        expect(docWidthTokens(undefined)).toEqual(DOC_WIDTHS.default)
    })
})

/// A store that records writes, so the mirror's round trip is observable.
function fakeStore(initial?: string): DocWidthStore & { value: string | null } {
    return {
        value: initial ?? null,
        getItem() {
            return this.value
        },
        setItem(_key, value) {
            this.value = value
        },
    }
}

const throwingStore: DocWidthStore = {
    getItem() {
        throw new Error("site data blocked")
    },
    setItem() {
        throw new Error("site data blocked")
    },
}

describe("the first-paint mirror", () => {
    test("round-trips a rung under the documented key", () => {
        const store = fakeStore()
        writeMirroredDocumentWidth("full", store)
        expect(store.value).toBe("full")
        expect(readMirroredDocumentWidth(store)).toBe("full")
    })

    test("writes under the key the bootstrap reads", () => {
        // Both helpers are used from different modules; a key typo in one
        // would cost a silent flash on every cold start and nothing else.
        const seen: string[] = []
        writeMirroredDocumentWidth("wide", {
            getItem: () => null,
            setItem: (key) => void seen.push(key),
        })
        expect(seen).toEqual([DOC_WIDTH_STORAGE_KEY])
    })

    test("an empty store reads as the default rung", () => {
        expect(readMirroredDocumentWidth(fakeStore())).toBe(DEFAULT_DOCUMENT_WIDTH)
    })

    test("a corrupted stored value reads as the default rung", () => {
        expect(readMirroredDocumentWidth(fakeStore("ultrawide"))).toBe(
            DEFAULT_DOCUMENT_WIDTH,
        )
    })

    test("an absent store is not an error", () => {
        // The non-browser case: no `localStorage` at all.
        expect(readMirroredDocumentWidth(null)).toBe(DEFAULT_DOCUMENT_WIDTH)
        expect(() => writeMirroredDocumentWidth("full", null)).not.toThrow()
    })

    test("a store that throws is not an error either", () => {
        // A private window, or a browser set to block site data. This runs on
        // the path that paints the first frame — throwing here would take the
        // application down to save a preference.
        expect(readMirroredDocumentWidth(throwingStore)).toBe(DEFAULT_DOCUMENT_WIDTH)
        expect(() => writeMirroredDocumentWidth("full", throwingStore)).not.toThrow()
    })
})
