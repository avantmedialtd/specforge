// Where a file's review threads render: on their line in the diff, or in the
// file's preamble when the diff cannot render that line
// (`pull-request-viewer`: *Changed Files in the Pull-Request View*; design
// D11). Pure, so the choice is tested without a DOM.

import type { DiffFile, DiffSide, ReviewThread } from "./types"

/// The threads one side-qualified line carries, in the order the detail lists
/// them: a line may hold more than one thread.
export interface ThreadSlot {
    side: DiffSide
    line: number
    threads: readonly ReviewThread[]
}

/// A file's threads, split by where its diff renders them.
export interface ThreadAnchors {
    /// One entry per side-qualified line that carries threads, in the order
    /// the first thread on each line appears.
    slots: readonly ThreadSlot[]
    /// The threads the file's diff cannot render on their line, for the
    /// file's preamble, where each states its original anchor: outdated,
    /// ranged, without a line, or on a line the diff does not render.
    preamble: readonly ReviewThread[]
}

const NO_FOLDED: ReadonlySet<number> = new Set()

/// What a file with no threads gets: no slot to build and no hunk to walk.
const NO_THREADS: ThreadAnchors = { slots: [], preamble: [] }

/// A slot being filled, whose threads are still being added to.
interface BuildingSlot extends ThreadSlot {
    threads: ReviewThread[]
}

/// Splits `threads` — all on `file`, as `splitThreads` gives them — by
/// whether the file's diff renders their anchor.
///
/// A thread is placed only when it is current, names one line, and that line
/// is rendered. A hunk the host folds renders none of its lines, and a
/// withheld, too-large or binary file renders none at all, so their threads
/// keep the preamble rather than vanishing inside the diff. Every unplaced
/// thread stays in the preamble, and a placed one moves to its line slot.
export function anchorThreads(
    threads: readonly ReviewThread[],
    file: DiffFile,
    folded: ReadonlySet<number> = NO_FOLDED,
): ThreadAnchors {
    if (threads.length === 0) return NO_THREADS
    const rendered = renderedLines(file, folded)
    const slots: ThreadSlot[] = []
    const byPlace = new Map<string, BuildingSlot>()
    const preamble: ReviewThread[] = []
    for (const thread of threads) {
        const { side, line } = thread
        if (
            thread.outdated ||
            line === null ||
            spansRange(thread) ||
            !rendered[side].has(line)
        ) {
            preamble.push(thread)
            continue
        }
        const place = `${side}\u0000${line}`
        const sitting = byPlace.get(place)
        if (sitting) {
            sitting.threads.push(thread)
            continue
        }
        const slot: BuildingSlot = { side, line, threads: [thread] }
        byPlace.set(place, slot)
        slots.push(slot)
    }
    return { slots, preamble }
}

/// A thread that names a range rather than one line, as `threadPlace` spells
/// it: a start line, or a start side, different from the thread's own.
function spansRange(thread: ReviewThread): boolean {
    if (thread.startLine === null) return false
    return thread.startLine !== thread.line || (thread.startSide ?? thread.side) !== thread.side
}

/// The line numbers the file's diff renders on each side: a context line on
/// both, a removed line on old alone and an added one on new alone. Hunks the
/// host folds contribute none of theirs.
function renderedLines(
    file: DiffFile,
    folded: ReadonlySet<number>,
): Record<DiffSide, ReadonlySet<number>> {
    const old = new Set<number>()
    const next = new Set<number>()
    if (file.content.kind === "hunks") {
        file.content.hunks.forEach((hunk, index) => {
            if (folded.has(index)) return
            for (const line of hunk.lines) {
                if (line.oldNo !== null) old.add(line.oldNo)
                if (line.newNo !== null) next.add(line.newNo)
            }
        })
    }
    return { old, new: next }
}
