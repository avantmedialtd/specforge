/// Syntax highlighting for diff lines (`diff-view`: *Syntax Highlighting*;
/// design D4), with the highlighter and grammars fenced code uses: a `lowlight`
/// instance built from `common`, the set `rehype-highlight` registers by
/// default. `lowlight` is a direct dependency pinned to the version
/// `rehype-highlight` resolves, so both stay one copy of one highlighter.
///
/// Each hunk is highlighted twice, each side as one contiguous text: its old
/// side (context and removed lines) and its new side (context and added
/// lines). Tokenising lines one by one would break every construct that spans
/// lines, highlighting a removed line inside a block comment as code. Each
/// result is split back into lines, and the token lines are memoised per hunk,
/// so both layouts read the same ones and a switch tokenises nothing again.
///
/// Highlighting runs here rather than in the model because it is
/// presentation, which keeps the payload text-only. Pure, for the reason
/// `diffLayout.ts` gives in its header.

import type { ElementContent, Root, RootContent } from "hast"
import { common, createLowlight } from "lowlight"
import type { Side } from "./diffLayout"
import type { Segment } from "./hiddenChars"
import type { Hunk, LineKind } from "./types"

const lowlight = createLowlight(common)

/// One highlighted run within one line: its text, and the classes of the
/// spans that enclose it, outermost first, each one span's classes joined by
/// spaces. A renderer nests one span per scope, as `rehype-highlight` does, so
/// rules that select a token inside another match in a diff as they do in
/// fenced code. Plain text has no scopes.
export interface Token {
    text: string
    scopes: readonly string[]
}

/// A line's tokens, in order, their texts concatenating to the line's text.
/// An empty line has none.
export type TokenLine = readonly Token[]

/// A hunk's token lines for each side, indexed like the hunk's `lines`, so
/// `tokens[side][index]` reads a line's tokens on that side. The old side
/// holds a context or removed line's tokens and null for an added line; the
/// new side holds a context or added line's, and null for a removed line.
export interface HunkTokens {
    readonly old: readonly (TokenLine | null)[]
    readonly new: readonly (TokenLine | null)[]
}

/// The language a file is highlighted in, from its path's last name: its
/// extension when that names a language, else the whole name, as for
/// `Makefile`, else null, which renders plain. Pass the file's key, so a
/// renamed file takes the language of its new name.
export function languageFor(path: string): string | null {
    const name = path.slice(path.lastIndexOf("/") + 1)
    const dot = name.lastIndexOf(".")
    const extension = dot === -1 ? null : name.slice(dot + 1)
    if (extension && lowlight.registered(extension)) return extension
    return name && lowlight.registered(name) ? name : null
}

/// The side whose tokens a line draws in unified: a removed line draws the
/// old side's, and an added or context line the new side's, the code as it
/// now reads. Side by side, each column draws its own side's.
export function unifiedSide(kind: LineKind): Side {
    return kind === "removed" ? "old" : "new"
}

const tokensByHunk = new WeakMap<Hunk, { language: string | null; tokens: HunkTokens }>()

/// A hunk's token lines, computed once per hunk object, in `language` (from
/// `languageFor`). A language that is null, or that the highlighter fails on,
/// gives plain lines.
export function hunkTokens(hunk: Hunk, language: string | null): HunkTokens {
    const cached = tokensByHunk.get(hunk)
    if (cached && cached.language === language) return cached.tokens
    const tokens: HunkTokens = {
        old: highlightSide(hunk, "removed", language),
        new: highlightSide(hunk, "added", language),
    }
    tokensByHunk.set(hunk, { language, tokens })
    return tokens
}

/// One token as drawn: its scopes, and the parts of its line's escape
/// segments that fall inside it.
export interface DrawnToken {
    scopes: readonly string[]
    pieces: readonly Segment[]
}

/// A line's tokens with its escapes drawn inside them. The escapes are
/// decided from the whole line (`escapeLine` in `hiddenChars.ts`), so no
/// decision depends on where a token ends; this only places them. A text
/// segment is cut at token boundaries, and every piece keeps its source
/// offset for copying. An escape is never cut: it is drawn whole in the token
/// it starts in, even where a token boundary falls inside its surrogate pair,
/// and a token it covers entirely is dropped.
export function drawTokens(tokens: TokenLine, segments: readonly Segment[]): DrawnToken[] {
    const drawn: DrawnToken[] = []
    let next = 0
    let covered = 0
    let tokenEnd = 0
    for (const token of tokens) {
        tokenEnd += token.text.length
        const pieces: Segment[] = []
        while (covered < tokenEnd && next < segments.length) {
            const segment = segments[next]
            const segmentEnd = segment.offset + segment.text.length
            if (segment.kind === "escape") {
                pieces.push(segment)
                covered = segmentEnd
            } else {
                const until = Math.min(segmentEnd, tokenEnd)
                pieces.push({
                    kind: "text",
                    text: segment.text.slice(covered - segment.offset, until - segment.offset),
                    offset: covered,
                })
                covered = until
            }
            if (covered >= segmentEnd) next += 1
        }
        if (pieces.length > 0) drawn.push({ scopes: token.scopes, pieces })
    }
    return drawn
}

/// One side of a hunk, highlighted as one text and split back into its lines.
/// `changed` is the kind of line the side keeps besides its context lines.
function highlightSide(
    hunk: Hunk,
    changed: "removed" | "added",
    language: string | null,
): (TokenLine | null)[] {
    const side: (TokenLine | null)[] = hunk.lines.map(() => null)
    const indices: number[] = []
    hunk.lines.forEach((line, index) => {
        if (line.kind === "context" || line.kind === changed) indices.push(index)
    })
    if (indices.length === 0) return side
    const texts = indices.map((index) => hunk.lines[index].text)
    const lines = tokenise(texts, language)
    indices.forEach((lineIndex, k) => {
        side[lineIndex] = lines[k]
    })
    return side
}

/// Token lines for consecutive lines of one side, one per text.
function tokenise(texts: readonly string[], language: string | null): TokenLine[] {
    const plain = () => texts.map((text) => (text === "" ? [] : [{ text, scopes: [] }]))
    if (language === null) return plain()
    let root: Root
    try {
        root = lowlight.highlight(language, texts.join("\n"))
    } catch {
        return plain()
    }
    const lines: Token[][] = [[]]
    // A span that crosses a newline continues on the next line with its
    // classes, since every text node is split there under its whole scope.
    const walk = (nodes: readonly (RootContent | ElementContent)[], scopes: readonly string[]) => {
        for (const node of nodes) {
            if (node.type === "text") {
                node.value.split("\n").forEach((piece, k) => {
                    if (k > 0) lines.push([])
                    if (piece !== "") lines[lines.length - 1].push({ text: piece, scopes })
                })
            } else if (node.type === "element") {
                const classes = (node.properties.className ?? []).join(" ")
                walk(node.children, classes === "" ? scopes : [...scopes, classes])
            }
        }
    }
    walk(root.children, [])
    // Every character of the text is in the tree, so the newlines come back
    // one for one; the guard only keeps the count exact if that ever changed.
    return texts.map((_, k) => lines[k] ?? [])
}
