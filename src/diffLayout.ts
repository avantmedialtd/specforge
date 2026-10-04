/// The diff view's layout decisions: how a hunk's lines pair into side-by-side
/// rows, when a file takes one column, when side by side is in effect and when
/// the navigator folds for it, the surface's stored choice, each line's
/// side-qualified identity and the place a switch keeps by it, and which side a
/// selection is on and what copying it yields (`diff-view`: *Unified and
/// Side-by-Side Layouts*, *The Layout Choice Is Per Surface*, *Narrow Views
/// Fall Back to Unified*, *Side-Qualified Line Identity and Switching*,
/// *Selection and Copying*; design D8).
///
/// Kept out of `DiffView` for the reason `commitHistory.ts` gives in its own
/// header: this repository has no component-test infrastructure, and a
/// `src/`-only diff short-circuits the mutation gate, so this module and its
/// tests are these decisions' only automated coverage. Every export is a total
/// function of its arguments. `splitRows` memoises per hunk object, which the
/// model allows because nothing mutates it once it has arrived; the only
/// ambient thing touched is `localStorage`, and only through the two
/// stored-choice helpers, which take an injectable store.

import type { Hunk, Line, LineKind } from "./types"

/// A layout of the diff. Side by side is `split`, the exact value the stored
/// choice holds for it.
export type DiffLayout = "unified" | "split"

/// One side of a diff: the old revision or the new one.
export type Side = "old" | "new"

// -------------------------------------------------------------------------
// Side-by-side rows
// -------------------------------------------------------------------------

/// One side-by-side row: the index, in its hunk's `lines`, of the line each
/// half shows, or null where that half is a filler. A context row names the
/// same line on both sides. Keyed by `Side`, so `row[side]` reads one half.
export interface SplitRow {
    readonly old: number | null
    readonly new: number | null
}

const rowsByHunk = new WeakMap<Hunk, readonly SplitRow[]>()

/// A hunk's side-by-side rows, computed once per hunk object, so every render
/// and a switch back and forth reuse them.
///
/// A context line fills both halves of one row. A change block, a maximal run
/// of removed and added lines that no context line interrupts, is walked with
/// one open slot: the earliest left-only row of the block not yet given a
/// partner. A removed line opens a left-only row; an added line fills the open
/// slot's right half, or opens a right-only row when no slot is open. So a
/// block git writes, k removed lines then m added ones, pairs by position in
/// max(k, m) rows with the shorter side's fillers at the bottom; interleaved
/// provider text such as −a +b −c +d pairs a with b and c with d; and an added
/// line never pairs with a removed line that comes after it.
///
/// Within each half, rows keep that side's lines in the model's order, which
/// is what lets a copy of one column read its lines from the model.
export function splitRows(hunk: Hunk): readonly SplitRow[] {
    const cached = rowsByHunk.get(hunk)
    if (cached) return cached
    const rows: { old: number | null; new: number | null }[] = []
    // The block's left-only rows in the order they opened; `slot` indexes the
    // open one, and runs past the end while none is open.
    let opened: number[] = []
    let slot = 0
    hunk.lines.forEach((line, index) => {
        if (line.kind === "context") {
            opened = []
            slot = 0
            rows.push({ old: index, new: index })
        } else if (line.kind === "removed") {
            opened.push(rows.length)
            rows.push({ old: index, new: null })
        } else if (slot < opened.length) {
            rows[opened[slot]].new = index
            slot += 1
        } else {
            rows.push({ old: null, new: index })
        }
    })
    rowsByHunk.set(hunk, rows)
    return rows
}

/// The side whose column a file fills alone, side by side, or null when it
/// keeps both columns.
///
/// A file whose every line is on one side, with no context line at all, takes
/// that side's column alone: every added or deleted file, and one emptied or
/// filled from empty. Half its width would otherwise be fillers. Any context
/// line keeps both columns, even in a file whose hunks only add or only
/// remove, so its context lines face each other with both numbers. A file
/// with no lines lays out no rows, and keeps the default of two.
export function oneColumnSide(hunks: readonly Hunk[]): Side | null {
    let only: Side | null = null
    for (const hunk of hunks) {
        for (const line of hunk.lines) {
            const side = changedSide(line)
            if (side === null || (only !== null && side !== only)) return null
            only = side
        }
    }
    return only
}

/// The one side a changed line belongs to, or null for a context line, which
/// belongs to both.
function changedSide(line: Line): Side | null {
    if (line.kind === "removed") return "old"
    if (line.kind === "added") return "new"
    return null
}

/// A hunk header's ranges as git writes them, every count spelled out:
/// `@@ -10,3 +10,4 @@`. Its section heading is drawn after it, through the
/// escapes.
export function hunkRange(hunk: Hunk): string {
    return `@@ -${hunk.oldStart},${hunk.oldLines} +${hunk.newStart},${hunk.newLines} @@`
}

/// How many digits a file's widest line number takes, so its gutters keep one
/// width down the whole file. At least one.
export function lineNumberDigits(hunks: readonly Hunk[]): number {
    let widest = 1
    for (const hunk of hunks) {
        for (const line of hunk.lines) {
            widest = Math.max(widest, line.oldNo ?? 0, line.newNo ?? 0)
        }
    }
    return String(widest).length
}

// -------------------------------------------------------------------------
// Line identity and switching
// -------------------------------------------------------------------------

/// A rendered line's side-qualified identity, the same in both layouts: old
/// line n for a removed line, new line n for an added one, and both for a
/// context line. A later anchor finds its line by side and number, as
/// GitHub's `diffSide` and BitBucket's `inline.from` and `inline.to` give it,
/// without knowing the layout.
export interface LineIdentity {
    readonly old: number | null
    readonly new: number | null
}

/// A unified line's identity: its own numbers on the sides it belongs to.
export function lineIdentity(line: Line): LineIdentity {
    return {
        old: line.kind === "added" ? null : line.oldNo,
        new: line.kind === "removed" ? null : line.newNo,
    }
}

/// A side-by-side row's identity: its left line's old number and its right
/// line's new number. A context row carries both of its one line's numbers,
/// and a row facing a filler carries its one side's.
export function splitRowIdentity(hunk: Hunk, row: SplitRow): LineIdentity {
    return {
        old: row.old === null ? null : hunk.lines[row.old].oldNo,
        new: row.new === null ? null : hunk.lines[row.new].newNo,
    }
}

/// The one side and number a switch keeps the reader's place by: the old
/// number when the line or row has one, else the new. Whichever layout comes
/// next holds a line or row carrying it, and a row pairing old 40 with new 41
/// becomes, in unified, the removed old 40 directly above the added new 41.
export function placeKey(identity: LineIdentity): { side: Side; line: number } | null {
    if (identity.old !== null) return { side: "old", line: identity.old }
    if (identity.new !== null) return { side: "new", line: identity.new }
    return null
}

/// The topmost visible of `count` rows laid out top to bottom: the first whose
/// bottom edge lies below `line`, the top of the scrolling ancestor's port, or
/// `count` when none does. Bisected, so a switch on a page at the budget reads
/// a handful of boxes rather than every row's.
export function topmostVisible(
    count: number,
    bottomOf: (index: number) => number,
    line: number,
): number {
    let low = 0
    let high = count
    while (low < high) {
        const middle = (low + high) >> 1
        if (bottomOf(middle) > line) high = middle
        else low = middle + 1
    }
    return low
}

/// The name a line-number cell carries for assistive technology, whose
/// visible digits are hidden from it: side, number and kind, as "old line 12,
/// removed" (`diff-view`: *Keyboard and Accessibility*).
export function lineNumberLabel(side: Side, number: number, kind: LineKind): string {
    return `${side} line ${number}, ${kind}`
}

// -------------------------------------------------------------------------
// Narrow views
// -------------------------------------------------------------------------

/// Side by side comes into effect once the sections column is this wide, in
/// `ch` of the code font (`diff-view`: *Narrow Views Fall Back to Unified*).
export const SPLIT_ENTER_CH = 104

/// Once in effect, side by side leaves only below this width. The gap keeps a
/// scrollbar that appears after a switch from flipping the layout straight
/// back.
export const SPLIT_LEAVE_CH = 96

/// What the layout control says, as visible text and as its accessible
/// description, while side by side is chosen but the view is too narrow.
export const NARROW_FALLBACK_TEXT = "Too narrow — showing unified"

/// A width in CSS pixels as `ch` of the code font, given the width of one `ch`
/// as the view's hidden probe measures it. A probe that measures nothing
/// (detached, or not laid out yet) gives NaN, which every threshold here reads
/// as too narrow, never as infinitely wide.
export function widthInCh(widthPx: number, chPx: number): number {
    return chPx > 0 && Number.isFinite(chPx) ? widthPx / chPx : Number.NaN
}

/// The layout in effect, decided again whenever the width or the choice
/// changes. With `widthCh` the sections column's width in `ch` of the code
/// font, never the window's, and `previous` the layout in effect until then
/// (unified before the first decision), side by side is in effect only while
/// it is chosen, and then from 104 ch, or from 96 ch when it already was:
///
/// s' ⇔ chosen ∧ (w ≥ 104 ∨ (s ∧ w ≥ 96))
export function layoutInEffect(
    chosen: DiffLayout,
    widthCh: number,
    previous: DiffLayout,
): DiffLayout {
    if (chosen !== "split") return "unified"
    const threshold = previous === "split" ? SPLIT_LEAVE_CH : SPLIT_ENTER_CH
    return widthCh >= threshold ? "split" : "unified"
}

/// Whether the narrow-view fallback holds: side by side chosen, unified in
/// effect. The control then keeps "Side by side" checked and shows
/// `NARROW_FALLBACK_TEXT`; the stored choice is never rewritten, so widening
/// the view brings side by side back.
export function fallbackHolds(chosen: DiffLayout, inEffect: DiffLayout): boolean {
    return chosen === "split" && inEffect === "unified"
}

/// Whether the navigator folds above the sections for side by side's sake:
/// while side by side is chosen, wherever keeping the navigator beside them
/// would leave the sections under the entry threshold, so widening the view
/// never turns side by side off.
///
/// Both widths are in `ch` of the code font, which is why script decides this
/// fold and sets the attribute the stylesheet folds on: the `ch` in a
/// container query's condition resolves in the view's own font. The
/// navigator's other fold, where it does not fit beside the sections at all,
/// is the stylesheet's container query, whatever the layout.
export function navigatorFoldsForSplit(
    chosen: DiffLayout,
    viewCh: number,
    navigatorCh: number,
): boolean {
    return chosen === "split" && viewCh - navigatorCh < SPLIT_ENTER_CH
}

// -------------------------------------------------------------------------
// The stored choice
// -------------------------------------------------------------------------

/// Where a surface keeps its layout choice. Per-surface view state, like the
/// rail's `specforge.railHidden`, and never an application setting: every
/// desktop window shares one origin and so one choice, while each browser that
/// opens a served instance keeps its own.
export const DIFF_LAYOUT_STORAGE_KEY = "specforge.diffLayout"

/// The subset of `Storage` the stored choice uses, so tests can pass a fake
/// and the helpers never depend on a DOM being present.
export interface DiffLayoutStore {
    getItem(key: string): string | null
    setItem(key: string, value: string): void
}

/// The ambient store, or `null` where there isn't one. Reading
/// `globalThis.localStorage` can itself throw (blocked site data, a non-browser
/// runtime), so the access is guarded, not just the call.
function ambientStore(): DiffLayoutStore | null {
    try {
        return globalThis.localStorage ?? null
    } catch {
        return null
    }
}

/// The surface's stored choice, read once as a view mounts. Only the exact
/// `split` reads as side by side; no value, any other value (`Split`, `split `,
/// `side-by-side`) and a store that throws all read as unified, the default.
/// Never throws.
export function readStoredLayout(store: DiffLayoutStore | null = ambientStore()): DiffLayout {
    try {
        return store?.getItem(DIFF_LAYOUT_STORAGE_KEY) === "split" ? "split" : "unified"
    } catch {
        return "unified"
    }
}

/// Store a choice for the diffs this surface opens next. Best-effort, and it
/// reports whether the write landed: a view whose write is refused keeps the
/// choice for itself alone, and the next diff opens in the layout stored
/// before. Never throws.
export function storeLayout(
    layout: DiffLayout,
    store: DiffLayoutStore | null = ambientStore(),
): boolean {
    if (!store) return false
    try {
        store.setItem(DIFF_LAYOUT_STORAGE_KEY, layout)
        return true
    } catch {
        return false
    }
}

// -------------------------------------------------------------------------
// Selection and copying
// -------------------------------------------------------------------------

/// What the side-naming rule hears from the view's pointer handlers.
export type SideNamingEvent =
    /// A pointer went down. `codeCell` is the side of the side-by-side code
    /// cell under it, `"unified"` for a code cell of the unified layout, which
    /// has no side, or null anywhere that is not a code cell.
    | { type: "pointerDown"; codeCell: Side | "unified" | null }
    /// A pointer was released. `collapsed` is true when the selection is
    /// collapsed or absent, and `insideView` when it lies within the view.
    | { type: "pointerUp"; collapsed: boolean; insideView: boolean }

/// The side named for selection after an event: a pure transition the view's
/// pointer handlers drive. While a side is named, every file's grid refuses
/// selection in the other column, so a drag stays in its column and carries
/// into the next file on the same side.
///
/// A pointer-down in a side-by-side code cell names that cell's side, and one
/// anywhere that is not a code cell clears it; a unified code cell does
/// neither. A pointer-up clears it when it leaves the selection collapsed, as
/// a click does, or outside the view. Nothing else clears it: the caret a
/// pointer-down leaves before a drag is collapsed too, but only a pointer-up
/// reads the selection, so a drag keeps the side it started on.
export function nextNamedSide(named: Side | null, event: SideNamingEvent): Side | null {
    if (event.type === "pointerDown") {
        if (event.codeCell === null) return null
        return event.codeCell === "unified" ? named : event.codeCell
    }
    return event.collapsed || !event.insideView ? null : named
}

/// One end of a selection, inside a code cell, mapped back to the model.
export interface CodePosition {
    /// The key of the file whose code cell holds it (`fileKey` in
    /// `diffFiles.ts`).
    file: string
    /// The hunk's index in the file's hunks.
    hunk: number
    /// The line's index in the hunk's `lines`.
    line: number
    /// A UTF-16 offset into the line's text, 0 being before its first
    /// character. A DOM offset inside a drawn escape maps to it through
    /// `sourceOffset` in `hiddenChars.ts`.
    offset: number
}

export interface CopyRequest {
    /// The selection's two ends, each null when it is not in a code cell. They
    /// may come in either order.
    start: CodePosition | null
    end: CodePosition | null
    /// The layout in effect, not the chosen one.
    inEffect: DiffLayout
    namedSide: Side | null
    /// The hunks a file shows now, by its key: its loaded content in place of
    /// a withheld file's, undefined for a file that shows none.
    hunksOf: (fileKey: string) => readonly Hunk[] | undefined
}

/// The clipboard text a copy builds from the model, or null when the
/// selection copies in document order instead.
///
/// The model is the source when both ends of the selection lie in code cells
/// of one file, and unified is in effect or a side is named. Side by side,
/// the text is the named side's selected lines: its context lines and its own
/// changed lines, in the model's order, which is that column's order. In
/// unified it is the selected lines in the order shown. Either way it is code
/// text only, the lines joined by newlines with the partial first and last
/// lines honoured, so a selection that crosses a hunk boundary yields the
/// lines alone. Every character is the line's own, so an escaped character
/// copies as itself.
///
/// Any other selection copies in document order: one with an end outside a
/// code cell or in the other side's cells, one spanning files, and, side by
/// side, one made while no side is named.
export function modelCopyText({
    start,
    end,
    inEffect,
    namedSide,
    hunksOf,
}: CopyRequest): string | null {
    if (start === null || end === null || start.file !== end.file) return null
    const side = inEffect === "split" ? namedSide : null
    if (inEffect === "split" && side === null) return null
    const hunks = hunksOf(start.file)
    if (!hunks) return null

    const [first, last] = precedes(end, start) ? [end, start] : [start, end]
    const shown = (line: Line) => side === null || changedSide(line) !== otherSide(side)
    const firstLine = hunks[first.hunk]?.lines[first.line]
    const lastLine = hunks[last.hunk]?.lines[last.line]
    if (!firstLine || !lastLine || !shown(firstLine) || !shown(lastLine)) return null

    const texts: string[] = []
    for (let h = first.hunk; h <= last.hunk; h += 1) {
        const lines = hunks[h].lines
        const from = h === first.hunk ? first.line : 0
        const to = h === last.hunk ? last.line : lines.length - 1
        for (let i = from; i <= to; i += 1) {
            const line = lines[i]
            if (!shown(line)) continue
            const begin = h === first.hunk && i === first.line ? first.offset : 0
            const finish = h === last.hunk && i === last.line ? last.offset : line.text.length
            texts.push(line.text.slice(begin, finish))
        }
    }
    return texts.join("\n")
}

function otherSide(side: Side): Side {
    return side === "old" ? "new" : "old"
}

/// Whether `a` comes before `b` in the model's order.
function precedes(a: CodePosition, b: CodePosition): boolean {
    if (a.hunk !== b.hunk) return a.hunk < b.hunk
    if (a.line !== b.line) return a.line < b.line
    return a.offset < b.offset
}
