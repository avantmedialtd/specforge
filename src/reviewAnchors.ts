// Where a file's review threads and the reviewer's own draft items render: on
// their line in the diff, in the file's preamble, or after the files
// (`pull-request-viewer`: *Changed Files in the Pull-Request View`; design
// D8, D11). Pure, so the choice is tested without a DOM.

import { fileKey } from "./diffFiles"
import type {
    DiffFile,
    DiffSide,
    DraftItem,
    PlacedInlineItem,
    PlacedReplyItem,
    ReviewThread,
    UnplacedInlineItem,
    UnplacedReplyItem,
} from "./types"

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

/// The draft comments one side-qualified line carries, in creation order.
export interface InlineDraftSlot {
    side: DiffSide
    line: number
    items: readonly PlacedInlineItem[]
}

/// A file's placed inline draft comments, split by whether its diff renders
/// their line.
export interface InlineDraftAnchors {
    /// One entry per side-qualified line that carries draft comments.
    slots: readonly InlineDraftSlot[]
    /// Placed comments the file's diff cannot render on their line — a folded
    /// hunk, a withheld file, or a line this detail does not render — for the
    /// file's preamble, where each states its anchor.
    preamble: readonly PlacedInlineItem[]
}

/// Where every item of one draft renders.
export interface DraftAnchors {
    /// Placed inline comments, by the diff-file identity they name.
    inlineByFile: ReadonlyMap<string, readonly PlacedInlineItem[]>
    /// Unplaced inline comments whose former path still belongs to a listed
    /// file, by that file's identity: the file's preamble.
    unplacedInlineByFile: ReadonlyMap<string, readonly UnplacedInlineItem[]>
    /// Placed comments whose file this detail does not list, and unplaced
    /// comments whose former path belongs to no listed file: after the files.
    inlineAfter: readonly (PlacedInlineItem | UnplacedInlineItem)[]
    /// Placed replies, by the thread they answer.
    repliesByThread: ReadonlyMap<string, readonly PlacedReplyItem[]>
    /// Replies stale recovery left without a target: after the files.
    unplacedReplies: readonly UnplacedReplyItem[]
    /// Placed replies whose target this detail no longer lists: after the
    /// files, rather than vanishing with a thread that is not here.
    orphanReplies: readonly PlacedReplyItem[]
}

const NO_FOLDED: ReadonlySet<number> = new Set()

/// What a file with no threads gets: no slot to build and no hunk to walk.
const NO_THREADS: ThreadAnchors = { slots: [], preamble: [] }

/// What a file with no draft comments gets.
const NO_DRAFTS: InlineDraftAnchors = { slots: [], preamble: [] }

/// What a draft with no items gets.
const NO_DRAFT_ANCHORS: DraftAnchors = {
    inlineByFile: new Map(),
    unplacedInlineByFile: new Map(),
    inlineAfter: [],
    repliesByThread: new Map(),
    unplacedReplies: [],
    orphanReplies: [],
}

/// A slot being filled, whose threads are still being added to.
interface BuildingSlot extends ThreadSlot {
    threads: ReviewThread[]
}

/// A draft slot being filled, whose items are still being added to.
interface BuildingDraftSlot extends InlineDraftSlot {
    items: PlacedInlineItem[]
}

/// Every path a listed file answers to, mapped to that file's identity: its
/// new path first, then its old path when no file now claims it. A renamed
/// file answers to both, so a former path still finds it.
export function fileKeysByPath(files: readonly DiffFile[]): ReadonlyMap<string, string> {
    const keyByPath = new Map<string, string>()
    for (const file of files) {
        if (file.newPath !== null) keyByPath.set(file.newPath, fileKey(file))
    }
    for (const file of files) {
        if (file.oldPath !== null && !keyByPath.has(file.oldPath)) {
            keyByPath.set(file.oldPath, fileKey(file))
        }
    }
    return keyByPath
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
    rendered: Record<DiffSide, ReadonlySet<number>> = renderedLines(file, folded),
): ThreadAnchors {
    if (threads.length === 0) return NO_THREADS
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

/// Splits a file's placed draft comments by whether the file's diff renders
/// their line, with the same rule as `anchorThreads`: a folded hunk, a
/// withheld file or a line this detail does not render keeps the comment in
/// the file's preamble, where its anchor is stated.
export function anchorInlineDrafts(
    items: readonly PlacedInlineItem[],
    file: DiffFile,
    folded: ReadonlySet<number> = NO_FOLDED,
    rendered: Record<DiffSide, ReadonlySet<number>> = renderedLines(file, folded),
): InlineDraftAnchors {
    if (items.length === 0) return NO_DRAFTS
    const slots: InlineDraftSlot[] = []
    const byPlace = new Map<string, BuildingDraftSlot>()
    const preamble: PlacedInlineItem[] = []
    for (const item of items) {
        if (item.path !== fileKey(file) || !rendered[item.side].has(item.line)) {
            preamble.push(item)
            continue
        }
        const place = `${item.side}\u0000${item.line}`
        const sitting = byPlace.get(place)
        if (sitting) {
            sitting.items.push(item)
            continue
        }
        const slot: BuildingDraftSlot = { side: item.side, line: item.line, items: [item] }
        byPlace.set(place, slot)
        slots.push(slot)
    }
    return { slots, preamble }
}

/// Groups one draft's items by where the detail's files and threads let them
/// render. A placed item names a current file identity or replyable thread, so
/// it is placed as stored; an unplaced item keeps its former anchor and is
/// never moved to a line or thread by path, number or text.
export function anchorDraftItems(
    items: readonly DraftItem[],
    files: readonly DiffFile[],
    threads: readonly ReviewThread[],
): DraftAnchors {
    if (items.length === 0) return NO_DRAFT_ANCHORS
    const keyByPath = fileKeysByPath(files)
    const listed = new Set(files.map(fileKey))
    const threadIds = new Set(threads.map((thread) => thread.id))
    const inlineByFile = new Map<string, PlacedInlineItem[]>()
    const unplacedInlineByFile = new Map<string, UnplacedInlineItem[]>()
    const inlineAfter: (PlacedInlineItem | UnplacedInlineItem)[] = []
    const repliesByThread = new Map<string, PlacedReplyItem[]>()
    const unplacedReplies: UnplacedReplyItem[] = []
    const orphanReplies: PlacedReplyItem[] = []
    for (const item of items) {
        switch (item.kind) {
            case "placedInline": {
                if (listed.has(item.path)) {
                    push(inlineByFile, item.path, item)
                } else {
                    // A current placement on a file this detail does not list:
                    // keep it visible rather than dropping its prose.
                    inlineAfter.push(item)
                }
                break
            }
            case "unplacedInline": {
                const key = keyByPath.get(item.path)
                if (key === undefined) inlineAfter.push(item)
                else push(unplacedInlineByFile, key, item)
                break
            }
            case "placedReply": {
                if (threadIds.has(item.target)) push(repliesByThread, item.target, item)
                else orphanReplies.push(item)
                break
            }
            case "unplacedReply":
                unplacedReplies.push(item)
                break
        }
    }
    return {
        inlineByFile,
        unplacedInlineByFile,
        inlineAfter,
        repliesByThread,
        unplacedReplies,
        orphanReplies,
    }
}

function push<V>(map: Map<string, V[]>, key: string, value: V): void {
    const sitting = map.get(key)
    if (sitting) sitting.push(value)
    else map.set(key, [value])
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
export function renderedLines(
    file: DiffFile,
    folded: ReadonlySet<number> = NO_FOLDED,
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
