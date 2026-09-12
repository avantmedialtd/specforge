import { describe, expect, test } from "bun:test"
import {
    headingId,
    outlineEntries,
    sectionForHeading,
    sectionProgress,
    type HeadingEntry,
} from "./outline"
import type { Section, Task } from "./types"

function task(text: string, completed: boolean): Task {
    return { text, completed, indent: 0, lineNumber: 1 }
}

function section(title: string, completed: boolean[]): Section {
    return { title, tasks: completed.map((done, i) => task(`t${i}`, done)) }
}

function heading(level: number, text: string, line = 1): HeadingEntry {
    return { level, text, line, id: text.toLowerCase() }
}

// ---- headingId -----------------------------------------------------------

describe("headingId", () => {
    test("lower-cases and hyphenates whitespace", () => {
        expect(headingId("What Changes", new Map())).toBe("what-changes")
    })

    test("collapses a run of whitespace to a single hyphen", () => {
        expect(headingId("Goals   /   Non-Goals", new Map())).toBe("goals-non-goals")
    })

    test("drops punctuation but keeps hyphens", () => {
        expect(headingId("What Changes?", new Map())).toBe("what-changes")
        expect(headingId("Risks / Trade-offs", new Map())).toBe("risks-trade-offs")
        // A run of dropped punctuation collapses with the whitespace around
        // it, so no empty segment survives as a doubled hyphen.
        expect(headingId("`code`, (parens) & co.", new Map())).toBe("code-parens-co")
    })

    test("later duplicates take a numeric suffix, in document order", () => {
        const seen = new Map<string, number>()
        expect(headingId("Scenario", seen)).toBe("scenario")
        expect(headingId("Scenario", seen)).toBe("scenario-1")
        expect(headingId("Scenario", seen)).toBe("scenario-2")
    })

    test("duplicates are matched after normalisation, not on the raw text", () => {
        const seen = new Map<string, number>()
        expect(headingId("What Changes?", seen)).toBe("what-changes")
        expect(headingId("what changes", seen)).toBe("what-changes-1")
    })

    test("a heading written entirely in punctuation has no identifier, and does not mint one for the next", () => {
        const seen = new Map<string, number>()
        expect(headingId("???", seen)).toBe("")
        expect(headingId("***", seen)).toBe("")
        // A real heading after them is unaffected.
        expect(headingId("Context", seen)).toBe("context")
    })

    test("a heading that already ends in a digit keeps it — the digit is text, not a dedupe suffix", () => {
        const seen = new Map<string, number>()
        expect(headingId("Step 1", seen)).toBe("step-1")
        expect(headingId("Step 1", seen)).toBe("step-1-1")
    })

    test("a fresh map means a fresh document: ids restart", () => {
        expect(headingId("Scenario", new Map())).toBe("scenario")
        expect(headingId("Scenario", new Map())).toBe("scenario")
    })
})

// ---- outlineEntries ------------------------------------------------------

describe("outlineEntries", () => {
    test("lists level two and three only, in document order", () => {
        const headings = [
            heading(1, "Title"),
            heading(2, "Context"),
            heading(3, "Detail"),
            heading(4, "Scenario: something"),
            heading(2, "Decisions"),
        ]
        expect(outlineEntries(headings).map((h) => h.text)).toEqual([
            "Context",
            "Detail",
            "Decisions",
        ])
    })

    test("a document with no level-two or -three heading yields nothing", () => {
        expect(outlineEntries([heading(1, "Title"), heading(4, "Scenario")])).toEqual([])
    })
})

// ---- sectionForHeading / sectionProgress ---------------------------------

describe("sectionForHeading", () => {
    const sections = [
        section("1. Selection model", [true, false, false]),
        section("2. **The change header**", [true, true]),
    ]

    test("an exact heading match finds its section", () => {
        expect(sectionForHeading("1. Selection model", sections)?.title).toBe(
            "1. Selection model",
        )
    })

    test("a heading matching no parsed section finds nothing", () => {
        expect(sectionForHeading("Verification", sections)).toBeUndefined()
    })

    test("a section title carrying inline markdown matches the rendered heading text", () => {
        expect(sectionForHeading("2. The change header", sections)?.tasks).toHaveLength(2)
    })

    test("surrounding whitespace on the heading does not defeat the match", () => {
        expect(sectionForHeading("  1. Selection model  ", sections)).toBeDefined()
    })
})

describe("sectionProgress", () => {
    test("counts completed against total", () => {
        expect(sectionProgress(section("s", [true, false, false]))).toEqual({
            completed: 1,
            total: 3,
        })
    })

    test("a complete section reports every task done, which is what shows the glyph", () => {
        const progress = sectionProgress(section("s", [true, true]))
        expect(progress).toEqual({ completed: 2, total: 2 })
        expect(progress.completed >= progress.total).toBe(true)
    })

    test("a section with no tasks has nothing to report", () => {
        expect(sectionProgress(section("s", []))).toEqual({ completed: 0, total: 0 })
    })
})
