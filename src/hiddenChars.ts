/// Characters that render as nothing, shown instead as visible, marked escapes
/// (`diff-view`: *Hidden Characters Are Shown*; design D6). Every character
/// with the Unicode property Default_Ignorable_Code_Point is a candidate: the
/// bidirectional controls, the zero-width characters, the tag characters,
/// variation selectors and Hangul fillers, the soft hyphen and the invisible
/// operators. Rendered raw, code would read differently from what it compiles
/// to (Trojan Source), and tag characters can carry instructions hidden from
/// people but read by agents; stripped, the view would hide what the file
/// holds.
///
/// The diff view applies these escapes to its lines, hunk headings, paths and
/// side names, and `pull-request-viewer` reuses them for titles and branch
/// names. Each decision is taken from a text's own characters and, for a diff
/// line, from that line's own kind and numbers, never from the layout or a
/// facing cell, so a line shows the same escapes in either layout and either
/// column. Pure, for the reason `diffLayout.ts` gives in its header; the
/// property escapes need only the `u` flag, which every WebView the app runs
/// in supports.

import type { Hunk, Line } from "./types"

/// A run of a text as it is drawn: characters that render as themselves, or
/// one character shown as an escape. Both keep their real text and its offset
/// in the source, so a copy can map a DOM offset back to the model
/// (`sourceOffset`) and put the real character on the clipboard, never the
/// escape.
export type Segment =
    | {
          kind: "text"
          text: string
          /// UTF-16 offset of `text` in the source.
          offset: number
      }
    | {
          kind: "escape"
          /// The real character: one code point, one or two UTF-16 units.
          text: string
          offset: number
          /// What the escape shows in its place: its code point, as `U+200B`.
          label: string
          /// Whether it raises the file's warning. False only for a character
          /// escaped because its line is added or removed, which the
          /// exemptions would pass on a context line.
          warns: boolean
      }

export interface Escaped {
    segments: readonly Segment[]
    /// Whether any of its escapes raises the warning.
    warns: boolean
}

const DEFAULT_IGNORABLE = /\p{Default_Ignorable_Code_Point}/u
const EMOJI = /\p{Extended_Pictographic}/u

const ZWJ = 0x200d
const VS15 = 0xfe0e
const VS16 = 0xfe0f
const BOM = 0xfeff
const COMBINING_KEYCAP = 0x20e3
const BLACK_FLAG = 0x1f3f4
const CANCEL_TAG = 0xe007f

/// The tag runs that may follow U+1F3F4: the three RGI subdivision flags,
/// England, Scotland and Wales, each spelled in tag characters and closed by
/// the cancel tag. Any other run is escaped whole, because the emoji
/// tag-sequence grammar accepts any run of tags and smuggled text would
/// otherwise pass as a flag.
const SUBDIVISION_FLAG_TAGS = ["gbeng", "gbsct", "gbwls"].map((name) => [
    ...[...name].map((letter) => 0xe0000 + letter.charCodeAt(0)),
    CANCEL_TAG,
])

/// Escapes for text that is not a diff line: a path, a side name, a title or
/// a branch name, and a hunk's section heading, which is decided as context
/// text. Every exemption of a context line applies except the leading
/// byte-order mark's, which belongs to a file's first line alone. Every escape
/// raises the warning.
export function escapeText(text: string): Escaped {
    return escape(text, false, false)
}

const escapedLines = new WeakMap<Line, Escaped>()

/// A diff line's escapes, decided once per line object, so both layouts and
/// the header's warning read the same result.
///
/// On a context line, the exemptions render as themselves: a U+200D between
/// two emoji; one U+FE0E or U+FE0F directly after an emoji; the U+FE0F of a
/// keycap; the tags of the three subdivision flags; and a U+FEFF at the very
/// start of a line that is old line 1 and new line 1. Every other candidate is
/// escaped and raises the warning.
///
/// On an added or removed line nothing is exempt, so a change that only adds
/// or removes such a character never shows as two identical lines. A
/// character escaped only because its line changed, one the exemptions would
/// pass were the line context, raises no warning; a U+FEFF counts so at the
/// very start of a line that is line 1 of its own side.
export function escapeLine(line: Line): Escaped {
    const cached = escapedLines.get(line)
    if (cached) return cached
    const escaped =
        line.kind === "context"
            ? escape(line.text, false, line.oldNo === 1 && line.newNo === 1)
            : escape(line.text, true, (line.kind === "removed" ? line.oldNo : line.newNo) === 1)
    escapedLines.set(line, escaped)
    return escaped
}

/// Whether a file's header carries the warning: some line or some hunk
/// heading holds a character escaped for itself.
export function hunksWarn(hunks: readonly Hunk[]): boolean {
    return hunks.some(
        (hunk) =>
            (hunk.section !== null && escapeText(hunk.section).warns) ||
            hunk.lines.some((line) => escapeLine(line).warns),
    )
}

/// The source offset of a DOM offset inside a drawn segment. A text segment
/// draws its characters as they are, so the two agree. An escape draws its
/// label in place of one character: an offset at the label's start is the
/// character's own, and any other lies after it.
export function sourceOffset(segment: Segment, drawnOffset: number): number {
    if (segment.kind === "escape") {
        return drawnOffset <= 0 ? segment.offset : segment.offset + segment.text.length
    }
    return segment.offset + Math.min(Math.max(drawnOffset, 0), segment.text.length)
}

/// The source offset of a point in a whole text as drawn, given as the count
/// of drawn characters before it, an escape counting as its label. The view
/// reads that count off the page, so it needs no element per segment; the
/// segment holding the point then decides, by `sourceOffset`. A point on the
/// boundary of two segments maps the same from either side, and one past the
/// drawing maps to the text's end.
export function sourceOffsetIn(segments: readonly Segment[], drawnOffset: number): number {
    let drawnStart = 0
    for (const segment of segments) {
        const drawnLength = segment.kind === "escape" ? segment.label.length : segment.text.length
        if (drawnOffset <= drawnStart + drawnLength) {
            return sourceOffset(segment, drawnOffset - drawnStart)
        }
        drawnStart += drawnLength
    }
    const last = segments[segments.length - 1]
    return last ? last.offset + last.text.length : 0
}

/// The label an escape shows: its code point in hexadecimal, at least four
/// digits wide.
export function escapeLabel(codePoint: number): string {
    return `U+${codePoint.toString(16).toUpperCase().padStart(4, "0")}`
}

/// Split `text` into segments. `changed` is true for an added or removed line,
/// which escapes every candidate and raises the warning only for one the
/// exemptions would not pass. `leadingMarkExempt` is whether a U+FEFF at the
/// very start passes: for context text, as itself; for a changed line, as a
/// warning-free escape.
function escape(text: string, changed: boolean, leadingMarkExempt: boolean): Escaped {
    if (!DEFAULT_IGNORABLE.test(text)) {
        return { segments: text === "" ? [] : [{ kind: "text", text, offset: 0 }], warns: false }
    }
    const chars = [...text]
    const codePoints = chars.map((char) => char.codePointAt(0) ?? 0)
    const flagTags = subdivisionFlagTags(codePoints)
    const segments: Segment[] = []
    let warns = false
    let offset = 0
    let runStart = 0
    chars.forEach((char, index) => {
        const at = offset
        offset += char.length
        if (!DEFAULT_IGNORABLE.test(char)) return
        const exempt =
            exemptAsContext(codePoints, index, flagTags) ||
            (index === 0 && codePoints[0] === BOM && leadingMarkExempt)
        if (exempt && !changed) return
        if (at > runStart) {
            segments.push({ kind: "text", text: text.slice(runStart, at), offset: runStart })
        }
        segments.push({
            kind: "escape",
            text: char,
            offset: at,
            label: escapeLabel(codePoints[index]),
            warns: !exempt,
        })
        warns ||= !exempt
        runStart = offset
    })
    if (runStart < text.length) {
        segments.push({ kind: "text", text: text.slice(runStart), offset: runStart })
    }
    return { segments, warns }
}

/// Whether the candidate at `index` passes as itself on context text, by the
/// exemptions other than the leading byte-order mark's.
function exemptAsContext(
    codePoints: readonly number[],
    index: number,
    flagTags: ReadonlySet<number>,
): boolean {
    const before = codePoints[index - 1]
    switch (codePoints[index]) {
        case ZWJ:
            // Between two emoji: one follows directly, and one precedes,
            // directly or carrying one emoji modifier or one U+FE0F.
            return (
                isEmoji(codePoints[index + 1]) &&
                (isEmoji(before) ||
                    ((isEmojiModifier(before) || before === VS16) &&
                        isEmoji(codePoints[index - 2])))
            )
        case VS15:
            return isEmoji(before)
        case VS16:
            return (
                isEmoji(before) ||
                (isKeycapBase(before) && codePoints[index + 1] === COMBINING_KEYCAP)
            )
        default:
            return flagTags.has(index)
    }
}

/// The indices of the tag characters that spell one of the three subdivision
/// flags after U+1F3F4. A run of tags is taken whole, to its last tag, so a
/// flag followed by more tags passes none of them.
function subdivisionFlagTags(codePoints: readonly number[]): Set<number> {
    const passing = new Set<number>()
    codePoints.forEach((codePoint, index) => {
        if (codePoint !== BLACK_FLAG) return
        let end = index + 1
        while (end < codePoints.length && isTag(codePoints[end])) end += 1
        const run = codePoints.slice(index + 1, end)
        const isFlag = SUBDIVISION_FLAG_TAGS.some(
            (tags) => tags.length === run.length && tags.every((tag, k) => tag === run[k]),
        )
        if (isFlag) for (let k = index + 1; k < end; k += 1) passing.add(k)
    })
    return passing
}

function isEmoji(codePoint: number | undefined): boolean {
    return codePoint !== undefined && EMOJI.test(String.fromCodePoint(codePoint))
}

/// The five skin-tone modifiers, which are not themselves Extended_Pictographic.
function isEmojiModifier(codePoint: number | undefined): boolean {
    return codePoint !== undefined && codePoint >= 0x1f3fb && codePoint <= 0x1f3ff
}

/// A keycap's base: `0`–`9`, `#` or `*`.
function isKeycapBase(codePoint: number | undefined): boolean {
    return (
        codePoint !== undefined &&
        ((codePoint >= 0x30 && codePoint <= 0x39) || codePoint === 0x23 || codePoint === 0x2a)
    )
}

/// The Tags block, U+E0000–U+E007F.
function isTag(codePoint: number): boolean {
    return codePoint >= 0xe0000 && codePoint <= CANCEL_TAG
}
