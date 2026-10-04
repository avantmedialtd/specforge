import { describe, expect, test } from "bun:test"
import {
    drawTokens,
    hunkTokens,
    languageFor,
    unifiedSide,
    type HunkTokens,
    type Token,
    type TokenLine,
} from "./diffHighlight"
import { escapeLine, escapeText } from "./hiddenChars"
import type { Hunk, Line } from "./types"

/// A hunk from git's own spelling of its lines, a marker (" ", "-" or "+")
/// then the text, numbered from line 1 on both sides.
function hunk(spec: readonly string[]): Hunk {
    let oldNo = 1
    let newNo = 1
    const lines = spec.map((entry): Line => {
        const text = entry.slice(1)
        if (entry[0] === "-") return { kind: "removed", oldNo: oldNo++, newNo: null, text }
        if (entry[0] === "+") return { kind: "added", oldNo: null, newNo: newNo++, text }
        return { kind: "context", oldNo: oldNo++, newNo: newNo++, text }
    })
    return {
        oldStart: 1,
        oldLines: oldNo - 1,
        newStart: 1,
        newLines: newNo - 1,
        section: null,
        lines,
    }
}

const hasScope = (token: Token, scope: string) =>
    token.scopes.some((s) => s.split(" ").includes(scope))
const allIn = (line: TokenLine | null, scope: string) =>
    line !== null && line.length > 0 && line.every((token) => hasScope(token, scope))
const anyIn = (line: TokenLine | null, scope: string) =>
    line !== null && line.some((token) => hasScope(token, scope))

/// Every side's tokens re-spell their lines exactly, and the side a line is
/// not on holds null.
function expectWhole(h: Hunk, tokens: HunkTokens) {
    h.lines.forEach((line, i) => {
        const onOld = line.kind !== "added"
        const onNew = line.kind !== "removed"
        expect(tokens.old[i] === null).toBe(!onOld)
        expect(tokens.new[i] === null).toBe(!onNew)
        for (const side of [tokens.old[i], tokens.new[i]]) {
            if (side) expect(side.map((token) => token.text).join("")).toBe(line.text)
        }
    })
}

describe("languageFor", () => {
    test("a file's extension names its language", () => {
        expect(languageFor("src/diffLayout.ts")).toBe("ts")
        expect(languageFor("crates/openspec-core/src/diff.rs")).toBe("rs")
        expect(languageFor("README.MD")).toBe("MD")
    })

    test("an extensionless file is resolved by its whole name", () => {
        expect(languageFor("Makefile")).not.toBeNull()
        expect(languageFor("build.d/Makefile")).not.toBeNull()
    })

    test("a name no grammar knows renders plain", () => {
        expect(languageFor("notes.qqq")).toBeNull()
        expect(languageFor("Dockerfile")).toBeNull()
        expect(languageFor(".gitignore")).toBeNull()
        expect(languageFor("trailing.")).toBeNull()
        expect(languageFor("")).toBeNull()
    })
})

describe("unifiedSide", () => {
    test("unified draws a removed line from the old side, the rest from the new side", () => {
        expect(unifiedSide("removed")).toBe("old")
        expect(unifiedSide("added")).toBe("new")
        expect(unifiedSide("context")).toBe("new")
    })
})

describe("hunkTokens", () => {
    test("a line inside a block comment is highlighted as a comment, in both layouts", () => {
        const h = hunk([
            " /*",
            "  * still the comment",
            "- * removed inside it",
            "  */",
            " let x = 1;",
        ])
        const tokens = hunkTokens(h, languageFor("src/a.ts"))
        expectWhole(h, tokens)
        // Side by side, its left cell draws the old side's tokens; unified
        // draws a removed line from the old side too.
        expect(allIn(tokens.old[2], "hljs-comment")).toBe(true)
        expect(allIn(tokens[unifiedSide("removed")][2], "hljs-comment")).toBe(true)
        // Tokenised on its own, the same text is not a comment.
        const alone = hunkTokens(hunk(["- * removed inside it"]), languageFor("src/a.ts"))
        expect(anyIn(alone.old[0], "hljs-comment")).toBe(false)
    })

    test("unified context lines read as the code now reads", () => {
        const h = hunk(["+/*", " x = 1;", "+*/"])
        const tokens = hunkTokens(h, languageFor("src/a.ts"))
        expectWhole(h, tokens)
        // Side by side, the context line's left cell is code and its right
        // cell is inside the new comment.
        expect(anyIn(tokens.old[1], "hljs-comment")).toBe(false)
        expect(anyIn(tokens.old[1], "hljs-number")).toBe(true)
        expect(allIn(tokens.new[1], "hljs-comment")).toBe(true)
        // Unified draws the context line from the new side: a comment.
        expect(allIn(tokens[unifiedSide("context")][1], "hljs-comment")).toBe(true)
    })

    test("an extensionless file is highlighted by its name", () => {
        const h = hunk(["+all: build", "+\t$(CC) -o app main.c"])
        const tokens = hunkTokens(h, languageFor("Makefile"))
        expectWhole(h, tokens)
        expect(anyIn(tokens.new[0], "hljs-section")).toBe(true)
        expect(anyIn(tokens.new[1], "hljs-variable")).toBe(true)
    })

    test("an unknown language renders plain", () => {
        const h = hunk([" alpha", "-beta", "+gamma", "+"])
        const tokens = hunkTokens(h, languageFor("notes.qqq"))
        expectWhole(h, tokens)
        expect(tokens.old).toEqual([
            [{ text: "alpha", scopes: [] }],
            [{ text: "beta", scopes: [] }],
            null,
            null,
        ])
        expect(tokens.new).toEqual([
            [{ text: "alpha", scopes: [] }],
            null,
            [{ text: "gamma", scopes: [] }],
            [],
        ])
    })

    test("a span crossing a newline continues on the next line with its classes", () => {
        const h = hunk([" const s = `first", "+second`;"])
        const tokens = hunkTokens(h, "ts")
        expectWhole(h, tokens)
        expect(tokens.new[1]?.[0]).toEqual({ text: "second`", scopes: ["hljs-string"] })
    })

    test("nested spans keep every enclosing scope, outermost first", () => {
        const h = hunk([" const s = `a${b}c`;"])
        const tokens = hunkTokens(h, "ts")
        const substitution = tokens.new[0]?.find((token) => token.text === "${b}")
        expect(substitution?.scopes).toEqual(["hljs-string", "hljs-subst"])
    })

    test("a hunk with no lines on one side tokenises nothing for it", () => {
        const h = hunk(["+a", "+b"])
        const tokens = hunkTokens(h, "ts")
        expect(tokens.old).toEqual([null, null])
        expectWhole(h, tokens)
    })

    test("a second request for a hunk returns the memoised lines", () => {
        const h = hunk([" let a = 1;", "-let b = 2;", "+let b = 3;"])
        const first = hunkTokens(h, "ts")
        expect(hunkTokens(h, "ts")).toBe(first)
        // An equal hunk is another object, so another entry.
        expect(hunkTokens(hunk([" let a = 1;", "-let b = 2;", "+let b = 3;"]), "ts")).not.toBe(
            first,
        )
    })

    test("a request in another language is not answered from the memo", () => {
        const h = hunk([" let a = 1;"])
        expect(hunkTokens(h, null).new[0]).toEqual([{ text: "let a = 1;", scopes: [] }])
        expect(anyIn(hunkTokens(h, "ts").new[0], "hljs-keyword")).toBe(true)
    })
})

describe("drawTokens", () => {
    test("a line without escapes draws each token as one piece at its offset", () => {
        const line: Line = { kind: "context", oldNo: 1, newNo: 1, text: "let x = 1;" }
        const tokens = hunkTokens(hunk([` ${line.text}`]), "ts").new[0] ?? []
        const drawn = drawTokens(tokens, escapeLine(line).segments)
        expect(drawn.map((token) => token.scopes)).toEqual(tokens.map((token) => token.scopes))
        let offset = 0
        drawn.forEach((token, k) => {
            expect(token.pieces).toEqual([{ kind: "text", text: tokens[k].text, offset }])
            offset += tokens[k].text.length
        })
    })

    test("an escape is drawn inside the token that holds it, its neighbours keeping their offsets", () => {
        const text = "s = 'a​b';"
        const tokens: Token[] = [
            { text: "s = ", scopes: [] },
            { text: "'a​b'", scopes: ["hljs-string"] },
            { text: ";", scopes: [] },
        ]
        const drawn = drawTokens(tokens, escapeText(text).segments)
        expect(drawn[1]).toEqual({
            scopes: ["hljs-string"],
            pieces: [
                { kind: "text", text: "'a", offset: 4 },
                { kind: "escape", text: "​", offset: 6, label: "U+200B", warns: true },
                { kind: "text", text: "b'", offset: 7 },
            ],
        })
        expect(drawn[2]).toEqual({ scopes: [], pieces: [{ kind: "text", text: ";", offset: 9 }] })
    })

    test("an escape split by a token boundary is drawn whole where it starts", () => {
        // U+E0068 is two UTF-16 units; a token boundary between them must not
        // cut the escape.
        const tag = "\u{E0068}"
        const text = `a${tag}b`
        const tokens: Token[] = [
            { text: `a${tag[0]}`, scopes: ["x"] },
            { text: `${tag[1]}b`, scopes: ["y"] },
        ]
        const drawn = drawTokens(tokens, escapeText(text).segments)
        expect(drawn).toEqual([
            {
                scopes: ["x"],
                pieces: [
                    { kind: "text", text: "a", offset: 0 },
                    { kind: "escape", text: tag, offset: 1, label: "U+E0068", warns: true },
                ],
            },
            { scopes: ["y"], pieces: [{ kind: "text", text: "b", offset: 3 }] },
        ])
    })

    test("a token an escape covers entirely is dropped", () => {
        const tag = "\u{E0068}"
        const tokens: Token[] = [
            { text: tag[0], scopes: ["x"] },
            { text: tag[1], scopes: ["y"] },
        ]
        expect(drawTokens(tokens, escapeText(tag).segments)).toEqual([
            {
                scopes: ["x"],
                pieces: [{ kind: "escape", text: tag, offset: 0, label: "U+E0068", warns: true }],
            },
        ])
    })

    test("pieces spell the line again, whatever the tokens and escapes", () => {
        const line: Line = {
            kind: "added",
            oldNo: null,
            newNo: 4,
            text: "const greeting = `hi \u{1F468}‍\u{1F469} ‮ok`; // done",
        }
        const tokens = hunkTokens(hunk([`+${line.text}`]), "ts").new[0] ?? []
        const drawn = drawTokens(tokens, escapeLine(line).segments)
        const pieces = drawn.flatMap((token) => token.pieces)
        expect(pieces.map((piece) => piece.text).join("")).toBe(line.text)
        expect(pieces.filter((piece) => piece.kind === "escape")).toHaveLength(2)
    })

    test("an empty line draws nothing", () => {
        expect(drawTokens([], [])).toEqual([])
    })
})
