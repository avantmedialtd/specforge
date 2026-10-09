import { describe, expect, test } from "bun:test"
import { commitDecision } from "./settingsFields"
import {
    errorsByIndex,
    parsePattern,
    refusalText,
    withAddedPattern,
    withoutPattern,
    withPattern,
} from "./skipPatterns"
import type { PatternError } from "./types"

const LIST = ["**/tests/**", "*.md", "**/tests/**"]

function refused(index: number, pattern: string, reason: string): PatternError {
    return { index, pattern, reason }
}

describe("a pattern field's commit", () => {
    // `settings-view`: *Settings Persist by One Rule*, as a free-text field.
    test("trims, and an emptied field reads as no pattern", () => {
        expect(parsePattern("  docs/** ")).toEqual({ ok: true, value: "docs/**" })
        expect(parsePattern("   ")).toEqual({ ok: true, value: "" })
    })

    test("writes a changed pattern, removes an emptied one, and leaves the rest", () => {
        expect(commitDecision(" *.md ", "*.md", parsePattern)).toEqual({ kind: "unchanged" })
        expect(commitDecision("docs/**", "*.md", parsePattern)).toEqual({
            kind: "write",
            value: "docs/**",
        })
        expect(commitDecision("", "*.md", parsePattern)).toEqual({ kind: "write", value: "" })
        // The empty add field commits nothing.
        expect(commitDecision("  ", "", parsePattern)).toEqual({ kind: "unchanged" })
    })
})

describe("editing the list", () => {
    test("a committed pattern replaces its own place only", () => {
        expect(withPattern(LIST, 1, "docs/**")).toEqual(["**/tests/**", "docs/**", "**/tests/**"])
        expect(withPattern(LIST, 2, "*.rs")).toEqual(["**/tests/**", "*.md", "*.rs"])
        expect(LIST).toEqual(["**/tests/**", "*.md", "**/tests/**"])
    })

    // *An empty list skips nothing*: removing every pattern leaves the empty
    // list to store.
    test("an emptied field, and Remove, take out their own pattern and no duplicate of it", () => {
        expect(withPattern(LIST, 0, "")).toEqual(["*.md", "**/tests/**"])
        expect(withoutPattern(LIST, 2)).toEqual(["**/tests/**", "*.md"])
        expect(withoutPattern(["*.md"], 0)).toEqual([])
    })

    test("the add field appends", () => {
        expect(withAddedPattern(LIST, "docs/**")).toEqual([...LIST, "docs/**"])
        expect(withAddedPattern([], "docs/**")).toEqual(["docs/**"])
    })
})

describe("refusalText", () => {
    const BAD_REGEX = refused(3, "/(unclosed/", "the regular expression does not compile: unclosed group")

    // *A pattern that does not compile is refused on its field*.
    test("a field's own refused pattern says why", () => {
        expect(refusalText([BAD_REGEX], 3)).toBe(
            "the regular expression does not compile: unclosed group",
        )
    })

    // *A hand-edited pattern that does not compile is ignored*: another
    // field's change cannot be stored beside it, and says which and why.
    test("another refused pattern is named beside the field's own reason", () => {
        const handEdited = refused(0, "/(x/", "the regular expression does not compile: unclosed group")
        expect(refusalText([handEdited], 3)).toBe(
            "/(x/: the regular expression does not compile: unclosed group",
        )
        expect(refusalText([handEdited, BAD_REGEX], 3)).toBe(
            "the regular expression does not compile: unclosed group; /(x/: the regular expression does not compile: unclosed group",
        )
        // A removal sends no pattern of its own.
        expect(refusalText([handEdited], null)).toBe(
            "/(x/: the regular expression does not compile: unclosed group",
        )
    })
})

describe("errorsByIndex", () => {
    test("each stored error lands on its own place", () => {
        const errors = errorsByIndex([
            refused(0, "/(x/", "the regular expression does not compile: unclosed group"),
            refused(2, "/build/**", "a glob cannot start with a slash"),
        ])
        expect(errors.get(0)).toBe("the regular expression does not compile: unclosed group")
        expect(errors.get(1)).toBeUndefined()
        expect(errors.get(2)).toBe("a glob cannot start with a slash")
        expect(errorsByIndex([]).size).toBe(0)
    })
})
