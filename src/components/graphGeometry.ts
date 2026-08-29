/// The commit rail's vertical layout, as a pure function of the commit list.
///
/// Extracted from `GraphRail.tsx` because it is the only part of the rail that
/// can be tested without a DOM: the repository has no component-test
/// infrastructure, so logic that stays inside a component is logic nothing can
/// assert on. Same reasoning as `figureZoom.ts`.
///
/// See the `commit-graph` capability's *Day Separators* requirement for the
/// contract this implements.

/// Height reserved for each day-separator band, and for each commit row. Both
/// feed the SVG gutter and the subject column from this one place, so a
/// separator band lengthens crossing lane edges instead of desynchronising
/// nodes from their subjects.
export const ROW_H = 26
export const SEP_H = 22

export interface DaySeparator {
    /// React key for this band.
    ///
    /// Identifies the **band**, not the day it labels — which is why the row
    /// index is in it. A single calendar day can legitimately get more than one
    /// band: the rail renders `git log --all --date-order`, and `--date-order`
    /// puts the topological constraint first ("no parents before all of their
    /// children"), so the emitted date sequence is not monotonically
    /// decreasing. When a branch's commits carry the reader back to a day
    /// already passed and then forward again, that day gets a second band —
    /// correctly, because the alternative is reordering commits, which the
    /// *Day Separators* requirement forbids outright.
    ///
    /// Keying on the day alone therefore handed React duplicate keys on any
    /// repository whose history interleaves branches, which is most of them.
    key: string
    label: string
    y: number
}

export interface GraphGeometry {
    /// rowTop[i] = y of the top of commit i's row, including separators above.
    rowTop: number[]
    separators: DaySeparator[]
    totalHeight: number
}

/// Viewer-local calendar-day key for a commit's author date — the field the
/// rail already sorts by (`--date-order`), so day boundaries always coincide
/// with where rows change date.
///
/// The month is 1-based: `getMonth()` is 0-indexed, so the obvious spelling
/// produces a key that reads as the wrong month. Nothing compares this against
/// a date formatted elsewhere, so that was cosmetic — but a value that says
/// June when it means July is a trap for whoever reads it next in a DOM
/// inspector.
export function dayKey(iso: string): string {
    const d = new Date(iso)
    if (Number.isNaN(d.getTime())) return iso
    return `${d.getFullYear()}-${d.getMonth() + 1}-${d.getDate()}`
}

/// One pass over the commits (newest first): reserve a SEP_H band above the
/// first commit of each run of a calendar day (and above the very first row),
/// recording every commit's top y. Both the SVG gutter and the subject column
/// read this single map.
///
/// "Each run of a day", not "each day" — see [`DaySeparator.key`].
export function computeGeometry(
    commits: Array<{ date: string }>,
    label: (iso: string) => string,
): GraphGeometry {
    const rowTop: number[] = []
    const separators: DaySeparator[] = []
    let y = 0
    for (let i = 0; i < commits.length; i++) {
        const key = dayKey(commits[i]!.date)
        if (i === 0 || key !== dayKey(commits[i - 1]!.date)) {
            separators.push({ key: `${key}-${i}`, label: label(commits[i]!.date), y })
            y += SEP_H
        }
        rowTop[i] = y
        y += ROW_H
    }
    return { rowTop, separators, totalHeight: y }
}
