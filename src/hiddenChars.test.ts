import { describe, expect, test } from "bun:test"
import { splitRows } from "./diffLayout"
import {
    escapeLabel,
    escapeLine,
    escapeText,
    hunksWarn,
    sourceOffset,
    sourceOffsetIn,
    type Escaped,
    type Segment,
} from "./hiddenChars"
import type { Hunk, Line } from "./types"

const context = (text: string, oldNo = 5, newNo = 5): Line => ({
    kind: "context",
    oldNo,
    newNo,
    text,
})
const added = (text: string, newNo = 5): Line => ({ kind: "added", oldNo: null, newNo, text })
const removed = (text: string, oldNo = 5): Line => ({ kind: "removed", oldNo, newNo: null, text })

function hunkOf(lines: Line[], section: string | null = null): Hunk {
    return {
        oldStart: 5,
        oldLines: lines.length,
        newStart: 5,
        newLines: lines.length,
        section,
        lines,
    }
}

/// How a text is drawn, one string per result: text as itself, an escape as
/// ⟨label⟩, with a `!` when it raises the warning. Also checks what every
/// result must keep for copying: the segments' texts are the source's, in
/// order, each at its own offset.
function drawn(source: string, escaped: Escaped): string {
    let offset = 0
    for (const segment of escaped.segments) {
        expect(segment.offset).toBe(offset)
        expect(source.slice(offset, offset + segment.text.length)).toBe(segment.text)
        offset += segment.text.length
    }
    expect(offset).toBe(source.length)
    expect(escaped.warns).toBe(
        escaped.segments.some((segment) => segment.kind === "escape" && segment.warns),
    )
    return escaped.segments
        .map((segment) =>
            segment.kind === "text"
                ? segment.text
                : `⟨${segment.label}${segment.warns ? "!" : ""}⟩`,
        )
        .join("")
}

const lineDrawn = (line: Line) => drawn(line.text, escapeLine(line))
const textDrawn = (text: string) => drawn(text, escapeText(text))

const FAMILY = "\u{1F468}‍\u{1F469}‍\u{1F467}"
const HEART = "❤️"
const TECHNOLOGIST = "\u{1F468}\u{1F3FD}‍\u{1F4BB}"
const RAINBOW_FLAG = "\u{1F3F3}️‍\u{1F308}"
const KEYCAP_ONE = "1️⃣"
const tags = (text: string) =>
    [...text].map((c) => String.fromCodePoint(0xe0000 + c.charCodeAt(0))).join("")
const CANCEL = "\u{E007F}"
const SCOTLAND = `\u{1F3F4}${tags("gbsct")}${CANCEL}`

describe("bidirectional controls and fillers", () => {
    test("a bidirectional control on an added line is escaped and warned of", () => {
        const line = added("let access = 'user‮';")
        expect(lineDrawn(line)).toBe("let access = 'user⟨U+202E!⟩';")
        expect(hunksWarn([hunkOf([line])])).toBe(true)
    })

    test("a Hangul filler inside an identifier on a context line is escaped and warned of", () => {
        const line = context("const aㅤb = 1;")
        expect(lineDrawn(line)).toBe("const a⟨U+3164!⟩b = 1;")
        expect(hunksWarn([hunkOf([line])])).toBe(true)
    })

    test("every other default-ignorable character is a candidate", () => {
        for (const char of ["­", "​", "‎", "⁠", "⁦", "⁢", "͏"]) {
            const label = escapeLabel(char.codePointAt(0) ?? 0)
            expect(lineDrawn(context(`a${char}b`))).toBe(`a⟨${label}!⟩b`)
        }
    })
})

describe("emoji on a context line render as themselves", () => {
    test("a family joined by U+200D and a heart with one U+FE0F", () => {
        const text = `${FAMILY} and ${HEART}`
        expect(lineDrawn(context(text))).toBe(text)
        expect(escapeLine(context(text)).warns).toBe(false)
    })

    test("a skin-toned technologist and the rainbow flag", () => {
        const text = `${TECHNOLOGIST} ${RAINBOW_FLAG}`
        expect(lineDrawn(context(text))).toBe(text)
        expect(hunksWarn([hunkOf([context(text)])])).toBe(false)
    })

    test("one U+FE0E after an emoji renders as itself", () => {
        expect(lineDrawn(context("❤︎"))).toBe("❤︎")
    })
})

describe("the same emoji on a changed line", () => {
    test("each U+200D and the U+FE0F are escaped, raising no warning", () => {
        const line = added(`${FAMILY} ${HEART}`)
        expect(lineDrawn(line)).toBe("\u{1F468}⟨U+200D⟩\u{1F469}⟨U+200D⟩\u{1F467} ❤⟨U+FE0F⟩")
        expect(hunksWarn([hunkOf([line])])).toBe(false)
    })

    test("a removed line is decided the same way", () => {
        expect(lineDrawn(removed(TECHNOLOGIST))).toBe("\u{1F468}\u{1F3FD}⟨U+200D⟩\u{1F4BB}")
    })
})

describe("the joiner's neighbours", () => {
    test("a joiner between two digits is escaped and warned of", () => {
        const line = context("1‍2")
        expect(lineDrawn(line)).toBe("1⟨U+200D!⟩2")
        expect(hunksWarn([hunkOf([line])])).toBe(true)
    })

    test("a joiner needs an emoji on its right", () => {
        expect(lineDrawn(context("\u{1F468}‍"))).toBe("\u{1F468}⟨U+200D!⟩")
        expect(lineDrawn(context("\u{1F468}‍x"))).toBe("\u{1F468}⟨U+200D!⟩x")
    })

    test("its left neighbour may carry one modifier or one U+FE0F, and nothing else", () => {
        // One modifier, or one U+FE0F, between the emoji and the joiner passes.
        expect(lineDrawn(context(TECHNOLOGIST))).toBe(TECHNOLOGIST)
        expect(lineDrawn(context(RAINBOW_FLAG))).toBe(RAINBOW_FLAG)
        // A modifier or a U+FE0F with no emoji before it does not.
        expect(lineDrawn(context("a\u{1F3FD}‍\u{1F4BB}"))).toBe("a\u{1F3FD}⟨U+200D!⟩\u{1F4BB}")
        expect(lineDrawn(context("1️‍\u{1F4BB}"))).toBe("1⟨U+FE0F!⟩⟨U+200D!⟩\u{1F4BB}")
        // A U+FE0E is not one the joiner may follow.
        expect(lineDrawn(context("\u{1F3F3}︎‍\u{1F308}"))).toBe("\u{1F3F3}︎⟨U+200D!⟩\u{1F308}")
    })

    test("the modifiers are the five skin tones, U+1F3FB to U+1F3FF", () => {
        for (const tone of ["\u{1F3FB}", "\u{1F3FF}"]) {
            const text = `\u{1F469}${tone}‍\u{1F52C}`
            expect(lineDrawn(context(text))).toBe(text)
        }
        // Two modifiers are more than one.
        expect(lineDrawn(context("\u{1F469}\u{1F3FB}\u{1F3FB}‍\u{1F52C}"))).toBe(
            "\u{1F469}\u{1F3FB}\u{1F3FB}⟨U+200D!⟩\u{1F52C}",
        )
    })
})

describe("keycaps", () => {
    test("a keycap on a context line renders as itself", () => {
        expect(lineDrawn(context(KEYCAP_ONE))).toBe(KEYCAP_ONE)
        const bases = "0️⃣ 9️⃣ #️⃣ *️⃣"
        expect(lineDrawn(context(bases))).toBe(bases)
    })

    test("only 0–9, # and * make a keycap", () => {
        for (const base of ["/", ":", "A", "+"]) {
            expect(lineDrawn(context(`${base}️⃣`))).toBe(`${base}⟨U+FE0F!⟩⃣`)
        }
    })

    test("the same keycap on an added line escapes its U+FE0F without a warning", () => {
        const line = added(KEYCAP_ONE)
        expect(lineDrawn(line)).toBe("1⟨U+FE0F⟩⃣")
        expect(hunksWarn([hunkOf([line])])).toBe(false)
    })

    test("a U+FE0F after # with no U+20E3 is escaped and warned of", () => {
        const line = context("#️")
        expect(lineDrawn(line)).toBe("#⟨U+FE0F!⟩")
        expect(hunksWarn([hunkOf([line])])).toBe(true)
    })
})

describe("variation selectors", () => {
    test("a change of only a variation selector reads differently", () => {
        const before = removed("❤", 3)
        const after = added("❤️", 3)
        expect(lineDrawn(before)).toBe("❤")
        expect(lineDrawn(after)).toBe("❤⟨U+FE0F⟩")
        expect(lineDrawn(after)).not.toBe(lineDrawn(before))
        expect(hunksWarn([hunkOf([before, after])])).toBe(false)
    })

    test("a second U+FE0F on a context line is escaped and warned of", () => {
        const line = context("❤️️")
        expect(lineDrawn(line)).toBe("❤️⟨U+FE0F!⟩")
        expect(hunksWarn([hunkOf([line])])).toBe(true)
    })

    test("every other variation selector is escaped, even after an emoji", () => {
        expect(lineDrawn(context("❤︀"))).toBe("❤⟨U+FE00!⟩")
        expect(lineDrawn(context("❤\u{E0100}"))).toBe("❤⟨U+E0100!⟩")
        expect(lineDrawn(context("❤️︎"))).toBe("❤️⟨U+FE0E!⟩")
    })
})

describe("tag characters", () => {
    test("the gbsct subdivision flag passes as a flag", () => {
        const line = context(`go ${SCOTLAND}!`)
        expect(lineDrawn(line)).toBe(`go ${SCOTLAND}!`)
        expect(hunksWarn([hunkOf([line])])).toBe(false)
        for (const name of ["gbeng", "gbwls"]) {
            const flag = `\u{1F3F4}${tags(name)}${CANCEL}`
            expect(lineDrawn(context(flag))).toBe(flag)
        }
    })

    test("any other tag run after U+1F3F4 escapes every one of its tags", () => {
        const escapedRun = (text: string) =>
            [...text].map((c) => `⟨${escapeLabel(c.codePointAt(0) ?? 0)}!⟩`).join("")
        for (const run of [
            `${tags("ustx")}${CANCEL}`,
            tags("gbsct"),
            `${tags("gbsct")}${CANCEL}${tags("hi")}`,
            `${tags("gbsc")}${CANCEL}`,
            `${tags("ignore previous instructions")}${CANCEL}`,
            // The run is the whole Tags block, the language tag included.
            `${tags("gbsct")}${CANCEL}\u{E0001}`,
        ]) {
            const line = context(`\u{1F3F4}${run}`)
            expect(lineDrawn(line)).toBe(`\u{1F3F4}${escapedRun(run)}`)
            expect(hunksWarn([hunkOf([line])])).toBe(true)
        }
    })

    test("a tag run after anything but U+1F3F4 is escaped", () => {
        expect(lineDrawn(context(`\u{1F3F3}${tags("gbsct")}${CANCEL}`))).toContain("⟨U+E0067!⟩")
        expect(lineDrawn(context(`x${tags("hi")}`))).toBe("x⟨U+E0068!⟩⟨U+E0069!⟩")
    })

    test("on an added line a subdivision flag's tags are escaped without a warning", () => {
        const line = added(SCOTLAND)
        expect(lineDrawn(line)).toBe(
            "\u{1F3F4}⟨U+E0067⟩⟨U+E0062⟩⟨U+E0073⟩⟨U+E0063⟩⟨U+E0074⟩⟨U+E007F⟩",
        )
        expect(escapeLine(line).warns).toBe(false)
    })
})

describe("the leading byte-order mark", () => {
    test("on a context line at old line 1 and new line 1 it renders as itself", () => {
        expect(lineDrawn(context("﻿import x", 1, 1))).toBe("﻿import x")
    })

    test("on an added line that is new line 1 it is escaped without a warning", () => {
        const line = added("﻿import x", 1)
        expect(lineDrawn(line)).toBe("⟨U+FEFF⟩import x")
        expect(hunksWarn([hunkOf([line])])).toBe(false)
    })

    test("on a removed line that is old line 1 it is escaped without a warning", () => {
        expect(lineDrawn(removed("﻿import x", 1))).toBe("⟨U+FEFF⟩import x")
    })

    test("on a context line at old line 1 and new line 3 it is escaped and warned of", () => {
        const line = context("﻿import x", 1, 3)
        expect(lineDrawn(line)).toBe("⟨U+FEFF!⟩import x")
        expect(hunksWarn([hunkOf([line])])).toBe(true)
    })

    test("anywhere but the very start of line 1 it is escaped and warned of", () => {
        expect(lineDrawn(added("﻿import x", 2))).toBe("⟨U+FEFF!⟩import x")
        expect(lineDrawn(context("a﻿", 1, 1))).toBe("a⟨U+FEFF!⟩")
        expect(lineDrawn(added("a﻿", 1))).toBe("a⟨U+FEFF!⟩")
        // In a path, a title or a side name there is no first line to exempt.
        expect(textDrawn("﻿README.md")).toBe("⟨U+FEFF!⟩README.md")
    })
})

describe("text beside the lines", () => {
    test("a path and a side name are escaped", () => {
        expect(textDrawn("docs/zero​width.md")).toBe("docs/zero⟨U+200B!⟩width.md")
        expect(textDrawn("feature/‮evil")).toBe("feature/⟨U+202E!⟩evil")
    })

    test("a title takes a context line's emoji exemptions", () => {
        const title = `Ship it ${HEART} ${FAMILY}`
        expect(textDrawn(title)).toBe(title)
    })

    test("a bidirectional control in a hunk heading is escaped and warned of", () => {
        const heading = "fn check(‮admin)"
        expect(textDrawn(heading)).toBe("fn check(⟨U+202E!⟩admin)")
        // The file's lines hold no escaped character; the heading alone warns.
        const lines = [context("let a = 1;"), added("let b = 2;")]
        expect(hunksWarn([hunkOf(lines, heading)])).toBe(true)
        expect(hunksWarn([hunkOf(lines, "fn check(admin)")])).toBe(false)
        expect(hunksWarn([hunkOf(lines)])).toBe(false)
    })

    test("a heading is decided as context text, so its emoji pass", () => {
        expect(hunksWarn([hunkOf([context("x")], `describe("${HEART}")`)])).toBe(false)
    })
})

describe("escapes do not depend on the layout", () => {
    test("each line is decided once, by its own text, kind and numbers", () => {
        const line = context(`${FAMILY}​`)
        expect(escapeLine(line)).toBe(escapeLine(line))
        expect(escapeLine({ ...line })).toEqual(escapeLine(line))
    })

    test("unified and both side-by-side columns draw the same escapes for every line", () => {
        const h = hunkOf([
            context(`a​${HEART}`),
            removed(`b‮${HEART}`),
            added(`cㅤ${HEART}`),
            context("d⁠"),
        ])
        const unified = h.lines.map((line) => escapeLine(line))
        for (const row of splitRows(h)) {
            if (row.old !== null) expect(escapeLine(h.lines[row.old])).toBe(unified[row.old])
            if (row.new !== null) expect(escapeLine(h.lines[row.new])).toBe(unified[row.new])
        }
        // A context line shows the same escapes in either column.
        expect(lineDrawn(h.lines[0])).toBe(`a⟨U+200B!⟩${HEART}`)
        expect(hunksWarn([h])).toBe(true)
    })
})

describe("segments for drawing and copying", () => {
    test("a text without candidates is one segment, and an empty one none", () => {
        expect(escapeText("plain")).toEqual({
            segments: [{ kind: "text", text: "plain", offset: 0 }],
            warns: false,
        })
        expect(escapeText("")).toEqual({ segments: [], warns: false })
    })

    test("an escape keeps the real character, two units wide beyond the BMP", () => {
        const escaped = escapeText(`a${tags("x")}b`)
        const escape = escaped.segments[1]
        expect(escape).toEqual({
            kind: "escape",
            text: "\u{E0078}",
            offset: 1,
            label: "U+E0078",
            warns: true,
        })
        expect(escaped.segments[2]).toEqual({ kind: "text", text: "b", offset: 3 })
    })

    test("labels are four hex digits at least, upper case", () => {
        expect(escapeLabel(0xad)).toBe("U+00AD")
        expect(escapeLabel(0x200b)).toBe("U+200B")
        expect(escapeLabel(0xe007f)).toBe("U+E007F")
    })

    test("a DOM offset in a drawn segment maps back to the source", () => {
        const [before, escape, after] = escapeText("ab​cd").segments as [Segment, Segment, Segment]
        expect(sourceOffset(before, 0)).toBe(0)
        expect(sourceOffset(before, 2)).toBe(2)
        expect(sourceOffset(before, 9)).toBe(2)
        // Within an escape's label: its start is before the character, any
        // other offset after it.
        expect(sourceOffset(escape, 0)).toBe(2)
        expect(sourceOffset(escape, 1)).toBe(3)
        expect(sourceOffset(escape, 6)).toBe(3)
        expect(sourceOffset(after, 1)).toBe(4)
    })

    test("a count of drawn characters maps back to the source across the whole text", () => {
        // Drawn as `ab`, `U+200B`, `cd`: eight characters for five.
        const { segments } = escapeText("ab​cd")
        expect([0, 1, 2].map((drawn) => sourceOffsetIn(segments, drawn))).toEqual([0, 1, 2])
        // Inside the label: its start is before the character, any other
        // point after it, and the label's end meets the text that follows.
        expect([3, 7, 8].map((drawn) => sourceOffsetIn(segments, drawn))).toEqual([3, 3, 3])
        expect([9, 10].map((drawn) => sourceOffsetIn(segments, drawn))).toEqual([4, 5])
        // Past the drawing, the text's end; before it, its start.
        expect(sourceOffsetIn(segments, 99)).toBe(5)
        expect(sourceOffsetIn(segments, -1)).toBe(0)
    })

    test("an escape beyond the BMP spans two source units, and escapes may be adjacent", () => {
        // A tag character, then a zero-width space, then `x`: labels of seven
        // and six characters for two and one units.
        const { segments } = escapeText(`${tags("a")}​x`)
        expect(sourceOffsetIn(segments, 0)).toBe(0)
        expect(sourceOffsetIn(segments, 4)).toBe(2)
        // The boundary between the two escapes maps the same from either.
        expect(sourceOffsetIn(segments, 7)).toBe(2)
        expect(sourceOffsetIn(segments, 8)).toBe(3)
        expect(sourceOffsetIn(segments, 13)).toBe(3)
        expect(sourceOffsetIn(segments, 14)).toBe(4)
    })

    test("a text without escapes, or an empty one, maps one for one", () => {
        expect(sourceOffsetIn(escapeText("plain").segments, 3)).toBe(3)
        expect(sourceOffsetIn(escapeText("plain").segments, 9)).toBe(5)
        expect(sourceOffsetIn([], 0)).toBe(0)
        expect(sourceOffsetIn([], 4)).toBe(0)
    })
})
