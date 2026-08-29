import type { MouseEvent as ReactMouseEvent } from "react"
import {
    createContext,
    forwardRef,
    memo,
    useCallback,
    useContext,
    useEffect,
    useId,
    useImperativeHandle,
    useLayoutEffect,
    useRef,
    useState,
    useSyncExternalStore,
    type ReactNode,
} from "react"
import type {
    ArtifactKind,
    ChangeData,
    ChangeInstance,
    DivergenceLabel,
    LogicalChange,
    PaletteColor,
    RepoView,
    Section,
    SpecCommitState,
    Task,
    TreeSelection,
    WorkspaceFolder,
    WorkspaceView,
} from "../types"
import { identChipClass } from "../changeIdentity"
import { stripInlineMarkdown } from "../markdown"
import { EmptyState } from "./EmptyState"
import { RelativeTime } from "./RelativeTime"
import { ChevronDown, ChevronRight, CompletionMark, Star } from "./icons"
import { partitionFavorites, type RowFavorite } from "./favorites"
import {
    getCollapsedTreeNodeIds,
    getExpandedTreeNodeIds,
    getFavoriteChangeIds,
    setCollapsedTreeNodeIds,
    setExpandedTreeNodeIds,
    updateFavoriteChangeIds,
    updateFavoriteChangeIdsOnPageHide,
} from "../api"

/// How a row was activated. `reader` means the user asked for the row's
/// document in its own window rather than in the detail pane — a Cmd/Ctrl-click
/// — so the selection is NOT published and the tree, the pane and the history
/// are all left exactly as they were (`reader-window`: *Launching a Reader
/// Window*).
export interface SelectOptions {
    reader?: boolean
}

interface WorkspaceTreeProps {
    views: WorkspaceView[]
    selectedNodeId: string | null
    onSelect: (nodeId: string, selection: TreeSelection, options?: SelectOptions) => void
}

/// Imperative surface for reveal — a navigation to an addressed node opens
/// its ancestors (a TRANSIENT overlay above the persisted `collapsed`/
/// `expanded` sets, never written to settings) and marks it selected via the
/// existing `selectedNodeId` prop, which the caller already updates in the
/// same pass. One entry point rather than an effect that reacts to view/
/// selection changes (`view-routing`: *Navigation Reveal Is Transient*).
export interface WorkspaceTreeHandle {
    /// Open every id in `path` (the addressed node's ancestors, root-to-leaf,
    /// ENDING with the addressed node's own id — see
    /// `src/routing/nodeId.ts`'s `addressToNodePath`, the only intended
    /// producer of this array; it is built by construction from the same
    /// compositional id helpers this file exports, never by splitting a
    /// single leaf id string — A2: those ids embed absolute filesystem
    /// paths, so a "/"-split ancestor can equal an unrelated real node's id
    /// whenever one registered path is a directory prefix of another, e.g.
    /// this very repo's own `.claude/worktrees/<name>` layout). Once the
    /// path's rows exist in the DOM, the leaf is focused and scrolled into
    /// view via the same `focusRow` keyboard nav already uses. `null`/empty
    /// clears the transient reveal — nothing is force-opened, and previously
    /// revealed ancestors fall back to their persisted state.
    reveal: (path: string[] | null) => void
}

// -------------------------------------------------------------------------
// Node-ID helpers — stable React keys + entries in the collapsed-set (also
// persisted to settings, so they need to round-trip across app restarts). Each
// helper composes on the one above so a section node ID embeds its
// containing change, artifact, etc.
// -------------------------------------------------------------------------

// Exported: `src/routing/nodeId.ts` reuses these verbatim (rather than
// reimplementing the scheme) so an Address-derived node id can never drift
// from what this file actually renders/persists — see `addressToNodeId`.
export const flatWorkspaceId = (uri: string) => `flat:${uri}`
export const repoId = (id: string) => `repo:${id}`
export const logicalChangeId = (rid: string, name: string) =>
    `${repoId(rid)}/lc:${name}`
export const instanceId = (rid: string, name: string, wt: string) =>
    `${logicalChangeId(rid, name)}/inst:${wt}`
/// `containerId` is either a flat-workspace id, a logical-change id (when
/// singleton-flattened), or an instance id. It scopes the artifact/section/
/// task subtree to its host.
export const changeRowId = (containerId: string, changeId: string) =>
    `${containerId}/change:${changeId}`
export const artifactNodeId = (
    containerId: string,
    changeId: string,
    kind: ArtifactKind,
) => `${changeRowId(containerId, changeId)}/artifact:${kind}`
const sectionNodeId = (
    containerId: string,
    changeId: string,
    sectionIndex: number,
) => `${artifactNodeId(containerId, changeId, "tasks")}/section:${sectionIndex}`
const taskNodeId = (
    containerId: string,
    changeId: string,
    sectionIndex: number,
    taskIndex: number,
) => `${sectionNodeId(containerId, changeId, sectionIndex)}/task:${taskIndex}`
export const specNodeId = (
    containerId: string,
    changeId: string,
    capability: string,
) => `${artifactNodeId(containerId, changeId, "specs")}/spec:${capability}`

// -------------------------------------------------------------------------
// Selection store — selection identity reaches rows through a tiny external
// store instead of a `selectedNodeId` prop threaded through every node
// component. Each Row subscribes for its own boolean, so a selection change
// re-renders exactly the two rows whose bit flipped while the memoized node
// components above them are skipped entirely (*Keyboard focus movement does
// not re-render the whole tree* in the spec-browser spec).
// -------------------------------------------------------------------------

interface SelectionStore {
    getSelected: () => string | null
    subscribe: (cb: () => void) => () => void
    set: (id: string | null) => void
}

function createSelectionStore(): SelectionStore {
    let selected: string | null = null
    const listeners = new Set<() => void>()
    return {
        getSelected: () => selected,
        subscribe: (cb) => {
            listeners.add(cb)
            return () => listeners.delete(cb)
        },
        set: (id) => {
            if (id === selected) return
            selected = id
            listeners.forEach((cb) => cb())
        },
    }
}

const SelectionContext = createContext<SelectionStore | null>(null)

// -------------------------------------------------------------------------
// Keyboard navigation — WAI-ARIA tree pattern with a roving tabindex. The
// "visible row list" is the DOM itself: `[role="treeitem"]` elements in
// document order, which equals visual order for this nested rendering (a
// future CSS reordering would break that invariant — keep them aligned).
// Focus is held by the DOM; React state never tracks the current row, so
// arrowing produces zero React renders.
// -------------------------------------------------------------------------

/// Settle delay before resting keyboard focus on a content row opens it in
/// the detail pane — long enough that a held arrow key skims rows without
/// loading any of them, short enough to feel immediate on release.
const FOLLOW_FOCUS_DELAY_MS = 150

function visibleRows(tree: HTMLElement): HTMLElement[] {
    return Array.from(tree.querySelectorAll<HTMLElement>('[role="treeitem"]'))
}

/// Node IDs embed filesystem paths, so they are matched by dataset compare
/// rather than interpolated into a CSS selector.
function rowById(rows: HTMLElement[], id: string): HTMLElement | undefined {
    return rows.find((r) => r.dataset.nodeId === id)
}

function rowLevel(row: HTMLElement): number {
    return parseInt(row.getAttribute("aria-level") ?? "1", 10)
}

/// Expansion toggles reuse the chevron's own click handler so the keyboard
/// path shares the exact pointer contract (override-set choice, persistence).
function clickChevron(row: HTMLElement) {
    row.querySelector<HTMLElement>(
        ":scope > .chevron:not(.chevron-spacer)",
    )?.click()
}

function focusRow(row: HTMLElement | undefined) {
    if (!row) return
    row.focus()
    row.scrollIntoView({ block: "nearest" })
}

// -------------------------------------------------------------------------
// Per-node default expansion. Most nodes default to open. The Tasks artifact
// node defaults to *closed* for every change (so expanding a change doesn't
// spill its task rows into the tree — see `ArtifactNode`), while a Section
// node flips to "closed" only once its own work is complete. A user override
// (collapse against a default-open node, or expand against a default-closed
// node) is honoured by `WorkspaceTree`'s two persisted ID sets.
// -------------------------------------------------------------------------

function defaultIsOpenForSection(section: Section): boolean {
    return !(
        section.tasks.length > 0 && section.tasks.every((t) => t.completed)
    )
}

/// True iff every parsed task in the change is complete (and at least one
/// task exists). Drives the trailing completion glyph on both the
/// flat-change row and the per-instance row; centralised so the rule
/// can't drift between the two paths.
function allTasksDone(change: ChangeData): boolean {
    return (
        change.artifacts.tasks &&
        change.totalTasks > 0 &&
        change.completedTasks === change.totalTasks
    )
}


// -------------------------------------------------------------------------
// Tree root
// -------------------------------------------------------------------------

export const WorkspaceTree = forwardRef<WorkspaceTreeHandle, WorkspaceTreeProps>(
    function WorkspaceTree({ views, selectedNodeId, onSelect }, ref) {
    // Two override sets, one per direction of default:
    //   `collapsed` — user-closed against a default-open node.
    //   `expanded`  — user-opened against a default-closed node (today only
    //                 a completed Tasks artifact or completed Section).
    // Most nodes default to open, so most clicks land in `collapsed`. The
    // `expanded` set only fills up as users opt back into seeing done work.
    const [collapsed, setCollapsed] = useState<Set<string>>(new Set())
    const [expanded, setExpanded] = useState<Set<string>>(new Set())
    // Favorited changes, keyed by position-independent change identity
    // (`logicalChangeId` for repo-group changes, the flat change-row id for
    // flat-workspace changes) — never an instance id, so a favorite survives
    // singleton↔multi promotion (*Favorite Identity and Persistence*).
    const [favorites, setFavorites] = useState<Set<string>>(new Set())
    // Mirror for reads outside render (same pattern as `forcedOpenRef`).
    const favoritesRef = useRef(favorites)
    favoritesRef.current = favorites
    // Un-flushed favorite toggles, id → desired state. Persistence sends
    // these as a DELTA (`update_favorite_change_ids`), never the whole set:
    // a failed hydration can't echo-write an empty list over stored
    // favorites, a stale client can't erase favorites another client
    // persisted since this one hydrated, and a toggle made before hydration
    // lands is preserved. `favoriteOpsVersion` only triggers the debounced
    // flush effect.
    const pendingFavoriteOpsRef = useRef<Map<string, boolean>>(new Map())
    const [favoriteOpsVersion, setFavoriteOpsVersion] = useState(0)
    const [hydrated, setHydrated] = useState(false)

    // Transient reveal overlay — see `WorkspaceTreeHandle`. Deliberately its
    // own `useState`, never folded into `collapsed`/`expanded`: it is never
    // read by the two persistence effects below, so a reveal can never
    // itself trigger a settings write (*Navigation Reveal Is Transient*).
    const [forcedOpen, setForcedOpen] = useState<Set<string>>(new Set())
    // Mirrors `forcedOpen` for `toggle` (a `useCallback` with an empty
    // dependency array, so it can't close over the state value itself
    // without going stale) to read synchronously — same "ref updated inline
    // during render" pattern `onSelectRef` already uses above.
    const forcedOpenRef = useRef(forcedOpen)
    forcedOpenRef.current = forcedOpen
    // The leaf id from the most recent `reveal()` call, to focus + scroll
    // into view once its row exists in the DOM (E3) — cleared once used, so
    // a LATER, unrelated `forcedOpen` change (e.g. `toggle` closing a
    // revealed node, see below) doesn't re-trigger it.
    const revealTargetRef = useRef<string | null>(null)
    useImperativeHandle(
        ref,
        () => ({
            reveal: (path) => {
                setForcedOpen(path && path.length > 0 ? new Set(path) : new Set())
                revealTargetRef.current = path && path.length > 0 ? path[path.length - 1]! : null
            },
        }),
        [],
    )

    // Selection store (see header above) — synced from the prop pre-paint so
    // the highlight never lags a render behind.
    const storeRef = useRef<SelectionStore | null>(null)
    if (storeRef.current === null) storeRef.current = createSelectionStore()
    useLayoutEffect(() => {
        storeRef.current!.set(selectedNodeId)
    }, [selectedNodeId])

    // `onSelect` comes from App as a fresh closure every render; the nodes
    // receive this stable wrapper instead so React.memo can hold.
    const onSelectRef = useRef(onSelect)
    onSelectRef.current = onSelect
    const stableOnSelect = useCallback(
        (nodeId: string, selection: TreeSelection, options?: SelectOptions) =>
            onSelectRef.current(nodeId, selection, options),
        [],
    )

    // Roving-focus bookkeeping. All DOM, no state: the current row's id, its
    // ancestor-id chain (root → self, captured while the elements still
    // exist, for fallback when a refresh removes the row), whether focus is
    // inside the tree, and the pending follow-focus timer.
    const treeRef = useRef<HTMLDivElement>(null)
    const focusedIdRef = useRef<string | null>(null)
    const chainRef = useRef<string[]>([])
    const focusInsideRef = useRef(false)
    const followTimerRef = useRef<number | null>(null)
    // Input-modality gates for follow-focus: the spec scopes it to KEYBOARD
    // focus, but rows are click-focusable too (tabIndex=-1), so a chevron
    // click would otherwise arm the timer and hijack the detail pane 150ms
    // later. Pointer-induced and programmatic-restore focus skip the timer.
    const pointerDownRef = useRef(false)
    const restoringRef = useRef(false)

    const clearFollowTimer = () => {
        if (followTimerRef.current !== null) {
            window.clearTimeout(followTimerRef.current)
            followTimerRef.current = null
        }
    }
    useEffect(() => clearFollowTimer, [])

    // E3: focus + scroll the revealed leaf into view once its row actually
    // exists in the DOM — `forcedOpen` changing is exactly that signal (React
    // commits the newly-open ancestors' DOM before running this effect).
    // Reuses `focusRow` (the same function keyboard nav already calls) rather
    // than adding a second `scrollIntoView` path.
    useEffect(() => {
        const targetId = revealTargetRef.current
        if (!targetId) return
        revealTargetRef.current = null
        const tree = treeRef.current
        if (!tree) return
        focusRow(rowById(visibleRows(tree), targetId))
    }, [forcedOpen])

    const handleFocusIn = (e: React.FocusEvent<HTMLDivElement>) => {
        const row = (e.target as HTMLElement).closest<HTMLElement>(
            '[role="treeitem"]',
        )
        if (!row) return
        focusInsideRef.current = true
        const id = row.dataset.nodeId ?? null
        if (id !== focusedIdRef.current) {
            const tree = treeRef.current
            if (tree) {
                for (const prev of tree.querySelectorAll<HTMLElement>(
                    '[role="treeitem"][tabindex="0"]',
                )) {
                    if (prev !== row) prev.tabIndex = -1
                }
            }
            row.tabIndex = 0
            focusedIdRef.current = id
            // Capture the ancestor chain by scanning back through document
            // order for strictly decreasing levels.
            if (tree && id !== null) {
                const rows = visibleRows(tree)
                const chain: string[] = []
                let need = rowLevel(row)
                for (let i = rows.indexOf(row); i >= 0 && need > 0; i--) {
                    if (rowLevel(rows[i]!) === need) {
                        const rid = rows[i]!.dataset.nodeId
                        if (rid) chain.unshift(rid)
                        need--
                    }
                }
                chainRef.current = chain
            }
        }
        // Debounced follow-focus: resting KEYBOARD focus on a content row
        // opens it as a click would. Disclosure-only and disabled rows never
        // start the timer; pointer-induced focus (mousedown precedes focusin)
        // and the roving effect's programmatic restore are excluded so a
        // chevron click or a watcher refresh can't navigate the detail pane;
        // the activeElement and selection re-checks at expiry guard against
        // focus having moved on and against re-selecting.
        clearFollowTimer()
        const viaPointer = pointerDownRef.current
        pointerDownRef.current = false
        if (
            !viaPointer &&
            !restoringRef.current &&
            row.dataset.grouping !== "true" &&
            row.getAttribute("aria-disabled") !== "true"
        ) {
            followTimerRef.current = window.setTimeout(() => {
                followTimerRef.current = null
                if (
                    document.activeElement === row &&
                    storeRef.current!.getSelected() !== row.dataset.nodeId
                ) {
                    row.click()
                }
            }, FOLLOW_FOCUS_DELAY_MS)
        }
    }

    const handleFocusOut = (e: React.FocusEvent<HTMLDivElement>) => {
        const next = e.relatedTarget as HTMLElement | null
        if (!next || !treeRef.current?.contains(next)) {
            focusInsideRef.current = false
            clearFollowTimer()
        }
    }

    const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
        // Keyboard activity ends any pointer gesture (covers a mousedown
        // whose focusin never fired, e.g. on an already-focused row).
        pointerDownRef.current = false
        const tree = treeRef.current
        const row = (e.target as HTMLElement).closest<HTMLElement>(
            '[role="treeitem"]',
        )
        if (!tree || !row) return
        // Cmd/Ctrl+D toggles the focused row's favorite state — checked
        // before the visibleRows() scan, which this branch never needs.
        // Matched on e.code (the physical key) so the chord works on layouts
        // where that key types a different character, and gated on !e.repeat
        // so OS auto-repeat can't flip-flop the state while the chord is
        // held. preventDefault unconditionally so the browser's bookmark
        // shortcut never fires while the tree has focus in the served web
        // UI; only favoritable rows render the button, so the chord is inert
        // elsewhere. Re-dispatching through the button's own click handler
        // mirrors `clickChevron`.
        if (
            (e.metaKey || e.ctrlKey) &&
            !e.altKey &&
            !e.shiftKey &&
            e.code === "KeyD"
        ) {
            e.preventDefault()
            if (!e.repeat) {
                row.querySelector<HTMLElement>(".row-favorite")?.click()
            }
            return
        }

        const rows = visibleRows(tree)
        const index = rows.indexOf(row)

        switch (e.key) {
            case "ArrowDown":
                e.preventDefault()
                focusRow(rows[index + 1])
                break
            case "ArrowUp":
                e.preventDefault()
                focusRow(rows[index - 1])
                break
            case "Home":
                e.preventDefault()
                focusRow(rows[0])
                break
            case "End":
                e.preventDefault()
                focusRow(rows[rows.length - 1])
                break
            case "ArrowRight": {
                e.preventDefault()
                const state = row.getAttribute("aria-expanded")
                if (state === "false") {
                    clickChevron(row)
                } else if (state === "true") {
                    const next = rows[index + 1]
                    if (next && rowLevel(next) === rowLevel(row) + 1) {
                        focusRow(next)
                    }
                }
                break
            }
            case "ArrowLeft": {
                e.preventDefault()
                if (row.getAttribute("aria-expanded") === "true") {
                    clickChevron(row)
                } else {
                    for (let i = index - 1; i >= 0; i--) {
                        if (rowLevel(rows[i]!) < rowLevel(row)) {
                            focusRow(rows[i])
                            break
                        }
                    }
                }
                break
            }
            case "Enter":
            case " ": {
                e.preventDefault()
                if (row.getAttribute("aria-disabled") === "true") break
                clearFollowTimer()
                if (row.dataset.grouping === "true") clickChevron(row)
                else row.click()
                break
            }
            default: {
                // First-letter typeahead over the visible rows after the
                // current one, wrapping past the end.
                if (
                    e.key.length !== 1 ||
                    e.metaKey ||
                    e.ctrlKey ||
                    e.altKey ||
                    !/\S/.test(e.key)
                ) {
                    break
                }
                const needle = e.key.toLowerCase()
                for (let step = 1; step <= rows.length; step++) {
                    const candidate = rows[(index + step) % rows.length]!
                    const label = candidate
                        .querySelector(".row-label")
                        ?.textContent?.trim()
                        .toLowerCase()
                    if (label?.startsWith(needle)) {
                        e.preventDefault()
                        focusRow(candidate)
                        break
                    }
                }
            }
        }
    }

    // Keep the roving tabindex coherent after every render: exactly one row
    // carries tabIndex=0. When a refresh removed the current row, fall back
    // along the captured ancestor chain — and if focus was lost to the body
    // because the focused element vanished, restore it.
    useEffect(() => {
        const tree = treeRef.current
        if (!tree) return
        const rows = visibleRows(tree)
        if (rows.length === 0) {
            focusedIdRef.current = null
            chainRef.current = []
            return
        }
        let current = focusedIdRef.current
            ? rowById(rows, focusedIdRef.current)
            : undefined
        if (!current) {
            for (let i = chainRef.current.length - 1; i >= 0 && !current; i--) {
                current = rowById(rows, chainRef.current[i]!)
            }
            current ??= rows[0]!
        }
        // Focus was lost to the body because the focused element was removed
        // or remounted (a remount keeps the node ID, so `current` resolves
        // above — the restore must still happen). The restoring gate keeps
        // this programmatic focus from arming the follow-focus timer.
        if (
            focusInsideRef.current &&
            document.activeElement === document.body
        ) {
            restoringRef.current = true
            focusRow(current)
            restoringRef.current = false
        }
        for (const prev of tree.querySelectorAll<HTMLElement>(
            '[role="treeitem"][tabindex="0"]',
        )) {
            if (prev !== current) prev.tabIndex = -1
        }
        current.tabIndex = 0
        focusedIdRef.current = current.dataset.nodeId ?? null
    })

    // Hydrate both sets in parallel. While the call is in flight the tree
    // renders against empty override sets, which means default-closed nodes
    // (completed groups) appear closed and default-open nodes appear open —
    // both correct under the new default — and only the user's persisted
    // overrides snap once hydration lands. Errors still flip `hydrated` so
    // the tree becomes interactive even if settings are unreadable.
    useEffect(() => {
        let cancelled = false
        Promise.all([
            getCollapsedTreeNodeIds().catch(() => [] as string[]),
            getExpandedTreeNodeIds().catch(() => [] as string[]),
            getFavoriteChangeIds().catch(() => [] as string[]),
        ])
            .then(([collapsedIds, expandedIds, favoriteIds]) => {
                if (cancelled) return
                setCollapsed(new Set(collapsedIds))
                setExpanded(new Set(expandedIds))
                // Fold toggles made before hydration landed over the fetched
                // list, so a pre-hydration star doesn't visibly revert.
                setFavorites(() => {
                    const next = new Set(favoriteIds)
                    for (const [id, active] of pendingFavoriteOpsRef.current) {
                        if (active) next.add(id)
                        else next.delete(id)
                    }
                    return next
                })
                setHydrated(true)
            })
            .catch(() => {
                if (cancelled) return
                setHydrated(true)
            })
        return () => {
            cancelled = true
        }
    }, [])

    // Two debounced-persistence effects, identical shape. Each writes its own
    // set; a click against a default-open node never touches `expanded`, so
    // the writes naturally stay independent.
    useEffect(() => {
        if (!hydrated) return
        const timer = setTimeout(() => {
            void setCollapsedTreeNodeIds([...collapsed])
        }, 150)
        return () => clearTimeout(timer)
    }, [collapsed, hydrated])

    useEffect(() => {
        if (!hydrated) return
        const timer = setTimeout(() => {
            void setExpandedTreeNodeIds([...expanded])
        }, 150)
        return () => clearTimeout(timer)
    }, [expanded, hydrated])

    // Favorites persist through a different mechanism than the two sets
    // above: pending toggles are drained and sent as a delta, and the backend
    // merges them under its settings lock. Draining and marshalling live in
    // one helper because the debounced flush and the page-dismissal flush
    // must never disagree about what "taking the pending ops" means.
    const takePendingFavoriteOps = useCallback(() => {
        const pending = pendingFavoriteOpsRef.current
        if (pending.size === 0) return null
        pendingFavoriteOpsRef.current = new Map()
        const add: string[] = []
        const remove: string[] = []
        for (const [id, active] of pending) {
            if (active) add.push(id)
            else remove.push(id)
        }
        return { pending, add, remove }
    }, [])

    const flushFavoriteOps = useCallback(() => {
        const taken = takePendingFavoriteOps()
        if (!taken) return
        updateFavoriteChangeIds(taken.add, taken.remove)
            .then((merged) => {
                // The backend's merged list is authoritative; fold any ops
                // recorded while the update was in flight over it.
                setFavorites(() => {
                    const next = new Set(merged)
                    for (const [id, active] of pendingFavoriteOpsRef.current) {
                        if (active) next.add(id)
                        else next.delete(id)
                    }
                    return next
                })
            })
            .catch(() => {
                // Restore the unsent ops (newer ops win) so the next toggle's
                // flush retries them instead of silently dropping the delta.
                const current = pendingFavoriteOpsRef.current
                for (const [id, active] of taken.pending) {
                    if (!current.has(id)) current.set(id, active)
                }
            })
    }, [takePendingFavoriteOps])

    useEffect(() => {
        if (favoriteOpsVersion === 0) return
        const timer = setTimeout(flushFavoriteOps, 150)
        return () => clearTimeout(timer)
    }, [favoriteOpsVersion, flushFavoriteOps])

    // The debounce alone would drop a toggle made within 150ms of the page
    // going away or the tree unmounting — flush on both paths through the
    // page-dismissal variant (sendBeacon on the web, which survives page
    // teardown; fire-and-forget invoke in the native shell).
    useEffect(() => {
        const flushOnDismiss = () => {
            const taken = takePendingFavoriteOps()
            if (taken) updateFavoriteChangeIdsOnPageHide(taken.add, taken.remove)
        }
        window.addEventListener("pagehide", flushOnDismiss)
        return () => {
            window.removeEventListener("pagehide", flushOnDismiss)
            flushOnDismiss()
        }
    }, [takePendingFavoriteOps])

    // A1: a forced-open node's chevron must unambiguously mean "close it" —
    // NOT the usual XOR of the persisted set. The row is rendered open
    // *because* reveal is overriding whatever `collapsed`/`expanded` already
    // says (that override is exactly WHY reveal exists: the common case is
    // an ancestor the user had previously collapsed), so XOR-ing that
    // existing value here could coincidentally leave it computing "open"
    // once the reveal clears — the chevron would look dead (no visual
    // change, since `forcedOpen` was still winning) and the click would
    // silently persist a state the user never actually chose. Explicitly
    // closing instead — and dropping the reveal for just this one id — makes
    // the chevron respond immediately AND makes the persisted write
    // trustworthy (it now matches what the click visibly did).
    const toggle = useCallback((id: string, defaultOpen: boolean) => {
        const setter = defaultOpen ? setCollapsed : setExpanded
        if (forcedOpenRef.current.has(id)) {
            setForcedOpen((prev) => {
                if (!prev.has(id)) return prev
                const next = new Set(prev)
                next.delete(id)
                return next
            })
            setter((prev) => {
                const has = prev.has(id)
                if (defaultOpen ? has : !has) return prev
                const next = new Set(prev)
                if (defaultOpen) next.add(id)
                else next.delete(id)
                return next
            })
            return
        }
        setter((prev) => {
            const next = new Set(prev)
            if (next.has(id)) next.delete(id)
            else next.add(id)
            return next
        })
    }, [])

    const toggleFavorite = useCallback((id: string) => {
        const nextActive = !favoritesRef.current.has(id)
        setFavorites((prev) => {
            const next = new Set(prev)
            if (nextActive) next.add(id)
            else next.delete(id)
            return next
        })
        pendingFavoriteOpsRef.current.set(id, nextActive)
        setFavoriteOpsVersion((v) => v + 1)
    }, [])

    if (views.length === 0) {
        return (
            <EmptyState
                title="No workspaces registered"
                body="Add a folder containing an openspec/ directory from settings."
            />
        )
    }

    return (
        <SelectionContext.Provider value={storeRef.current}>
            <div
                className="tree"
                role="tree"
                aria-label="Workspaces"
                ref={treeRef}
                onKeyDown={handleKeyDown}
                onFocus={handleFocusIn}
                onBlur={handleFocusOut}
                // Capture-phase so the flag is set before the browser focuses
                // the row (mousedown → focus → focusin → click).
                onMouseDownCapture={() => {
                    pointerDownRef.current = true
                }}
                onMouseUp={() => {
                    pointerDownRef.current = false
                }}
            >
                {views.map((view) =>
                    view.kind === "repo" ? (
                        <RepoNode
                            key={repoId(view.repoId)}
                            repo={view}
                            collapsed={collapsed}
                            expanded={expanded}
                            forcedOpen={forcedOpen}
                            favorites={favorites}
                            toggleFavorite={toggleFavorite}
                            toggle={toggle}
                            onSelect={stableOnSelect}
                        />
                    ) : (
                        <FlatWorkspaceNode
                            key={flatWorkspaceId(view.workspace.uri)}
                            workspace={view.workspace}
                            changes={view.changes}
                            displayName={view.displayName}
                            color={view.color}
                            collapsed={collapsed}
                            expanded={expanded}
                            forcedOpen={forcedOpen}
                            favorites={favorites}
                            toggleFavorite={toggleFavorite}
                            toggle={toggle}
                            onSelect={stableOnSelect}
                        />
                    ),
                )}
            </div>
        </SelectionContext.Provider>
    )
    },
)

WorkspaceTree.displayName = "WorkspaceTree"

// -------------------------------------------------------------------------
// Row primitive
// -------------------------------------------------------------------------

interface RowProps {
    /// Stable hierarchical node ID — React key material, the persisted
    /// collapse-set entry, AND the row's keyboard identity (`data-node-id`,
    /// selection-store subscription).
    nodeId: string
    depth: number
    isLeaf?: boolean
    isExpanded?: boolean
    /// Disclosure-only row (workspace / repo / logical change / flat change /
    /// the Specs artifact): Enter and Space toggle expansion instead of
    /// selecting, and follow-focus never opens it in the detail pane —
    /// mirroring the pointer contract, where clicking it changes no content.
    grouping?: boolean
    label: ReactNode
    meta?: ReactNode
    /// Optional second line rendered beneath the label as part of the same
    /// selectable row. When present the row becomes a two-line "sole change
    /// row" (see the *Two-Line Sole-Change-Row Layout* spec): line 1 is the
    /// label, line 2 is this `detail` node (worktree identity + status). The
    /// chevron/swatch stay in the leading gutter; selection + hover span both
    /// lines because both live inside one `.tree-row`.
    detail?: ReactNode
    /// Palette colour dot rendered at the start of the primary line (line 1),
    /// only on two-line rows. Ties a change to its workspace's identity colour
    /// so the change name — not the branch chip — anchors the eye.
    primarySwatch?: PaletteColor | null
    onToggle?: () => void
    onSelect?: (event: ReactMouseEvent) => void
    /// Workspace identity glyph rendered as an 8px filled circle between
    /// chevron and label. Top-level rows only — child rows pass nothing.
    /// `null` / undefined = no swatch, label slots in directly after the
    /// chevron.
    swatch?: PaletteColor | null
    /// Optional `title` attribute for the row — used to surface the path on
    /// renamed top-level rows so they remain disambiguatable.
    title?: string
    /// Renders the row in a dim + inert state — used as a slot indicator
    /// for missing artifacts. The row layout footprint is preserved, but
    /// the click handler is suppressed at the React level and (via CSS)
    /// `pointer-events: none` cancels hover/cursor as well.
    dim?: boolean
    /// Strikes through + dims the row's label — the completed-state signal
    /// for leaf-task rows, which carry no leading glyph. Presentation-level
    /// (the caller decides *when*); composes with selection/hover via CSS.
    struck?: boolean
    /// Marks a completed two-line change row: its inline-start rail switches
    /// from the workspace-colour rail to the --ok completion rail (mutually
    /// exclusive with the swatch rail). No effect on single-line rows, which
    /// carry no rail. Selection still overrides the rail to --accent.
    complete?: boolean
    /// Favorite-toggle wiring — favoritable change rows only. Renders the
    /// star button in a reserved slot at the primary line's extreme trailing
    /// edge, after any meta (*Change-Row Favorite Toggle*).
    favorite?: RowFavorite
}

function Row({
    nodeId,
    depth,
    isLeaf,
    isExpanded,
    grouping,
    label,
    meta,
    detail,
    primarySwatch,
    onToggle,
    onSelect,
    swatch,
    title,
    dim,
    struck,
    complete,
    favorite,
}: RowProps) {
    // Per-row selection subscription — see the SelectionStore header.
    const store = useContext(SelectionContext)!
    const isSelected = useSyncExternalStore(
        store.subscribe,
        () => store.getSelected() === nodeId,
    )
    const topLevelClass = depth === 0 ? " tree-row--top-level" : ""
    const dimClass = dim ? " tree-row--dim" : ""
    const struckClass = struck ? " tree-row--struck" : ""
    const twoLineClass = detail != null ? " tree-row--two-line" : ""
    /// Inline-start rail (only on two-line change rows). A completed change
    /// takes the --ok completion rail; otherwise it takes its workspace-colour
    /// rail. Selection overrides either to --accent via higher specificity.
    const railClass =
        detail != null && complete
            ? " tree-row--complete"
            : detail != null && primarySwatch
              ? ` tree-row--rail-${primarySwatch}`
              : ""
    const swatchClass = swatch ? `row-swatch row-swatch--${swatch}` : ""
    // Screen-reader-only description target for the treeitem-level favorite
    // state (aria-describedby has universal AT support; the draft
    // aria-description does not). useId because node IDs may contain spaces
    // (they embed filesystem paths) and aria-describedby is space-separated.
    const favoriteDescId = useId()
    // The star is a nested control like the chevron: stopPropagation keeps a
    // toggle from selecting the row. tabIndex=-1 keeps the tree's single Tab
    // stop, and mousedown-preventDefault keeps a click from moving focus to
    // the (hover-hidden) button — the roving focus stays on the row, per the
    // spec's "the toggle itself is never focusable". The accessible name is
    // invariant ("Favorite"); aria-pressed alone carries the state, per the
    // ARIA toggle-button pattern.
    const favoriteButton = favorite ? (
        <button
            type="button"
            className={`row-favorite${favorite.active ? " row-favorite--active" : ""}`}
            tabIndex={-1}
            aria-pressed={favorite.active}
            aria-label="Favorite"
            title="Favorite"
            onMouseDown={(e) => e.preventDefault()}
            onClick={(e) => {
                e.stopPropagation()
                favorite.onToggle()
            }}
        >
            <Star width={13} height={13} filled={favorite.active} />
        </button>
    ) : null
    return (
        <div
            className={`tree-row${isSelected ? " selected" : ""}${topLevelClass}${dimClass}${struckClass}${twoLineClass}${railClass}`}
            style={{ paddingLeft: depth * 12 + 4 }}
            onClick={dim ? undefined : onSelect}
            title={title}
            role="treeitem"
            data-node-id={nodeId}
            data-grouping={grouping ? "true" : undefined}
            // All rows render -1; the roving effect in WorkspaceTree promotes
            // exactly one to 0. React never re-renders -1 over it because the
            // vdom value is unchanged.
            tabIndex={-1}
            aria-level={depth + 1}
            aria-selected={isSelected}
            aria-expanded={isLeaf ? undefined : isExpanded}
            aria-disabled={dim || undefined}
            // Treeitem-level favorite state: screen readers often flatten a
            // nested button's aria-pressed when reading the row, so the state
            // is also conveyed on the treeitem itself.
            aria-describedby={favorite?.active ? favoriteDescId : undefined}
        >
            {isLeaf ? (
                <span className="chevron chevron-spacer" />
            ) : (
                <span
                    className={`chevron${isExpanded ? " open" : ""}`}
                    aria-hidden="true"
                    onClick={(e) => {
                        e.stopPropagation()
                        onToggle?.()
                    }}
                >
                    {isExpanded ? <ChevronDown /> : <ChevronRight />}
                </span>
            )}
            {swatch && <span className={swatchClass} aria-hidden="true" />}
            {detail != null ? (
                <span className="row-stack">
                    <span className="row-line row-line--primary">
                        <span className="row-label">{label}</span>
                        {meta != null && (
                            <span className="row-meta">{meta}</span>
                        )}
                        {favoriteButton}
                    </span>
                    <span className="row-line row-line--detail">{detail}</span>
                </span>
            ) : (
                <>
                    <span className="row-label">{label}</span>
                    {meta != null && <span className="row-meta">{meta}</span>}
                    {favoriteButton}
                </>
            )}
            {favorite?.active && (
                <span id={favoriteDescId} className="sr-only">
                    Favorite
                </span>
            )}
        </div>
    )
}

// -------------------------------------------------------------------------
// Task-progress meter — a fixed-width outlined track with an --ok fill whose
// width is completed/total. Renders no inline digits; the exact count lives
// in the `title` tooltip and the `progressbar` aria attributes. Renders
// nothing when there are no parseable tasks. Callers hide it at 100% (the
// trailing ✓ takes over), so the meter only ever depicts in-progress work.
// -------------------------------------------------------------------------

function TaskProgress({
    completed,
    total,
}: {
    completed: number
    total: number
}) {
    if (total <= 0) return null
    const fraction = Math.max(0, Math.min(1, completed / total))
    const label = `${completed} of ${total} tasks`
    return (
        <span
            className="task-progress"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={total}
            aria-valuenow={completed}
            aria-label={label}
            title={label}
        >
            <span
                className="task-progress-fill"
                style={{ width: `${fraction * 100}%` }}
            />
        </span>
    )
}

// -------------------------------------------------------------------------
// Common props shared by container nodes
// -------------------------------------------------------------------------

interface NodeProps {
    collapsed: Set<string>
    expanded: Set<string>
    /// Transient reveal overlay (see `WorkspaceTreeHandle.reveal`) — a node
    /// whose id is in this set renders open regardless of `collapsed`/
    /// `expanded`, and it is never itself written to either.
    forcedOpen: Set<string>
    /// `defaultOpen` selects which override set the click mutates:
    /// `true`  → xor into `collapsed` (the default for almost every node).
    /// `false` → xor into `expanded`  (used only by the two auto-collapse
    ///           node types — Tasks artifact and Section — when their work
    ///           is complete).
    toggle: (id: string, defaultOpen: boolean) => void
    onSelect: (nodeId: string, selection: TreeSelection, options?: SelectOptions) => void
}

// -------------------------------------------------------------------------
// Repo group + descendants
// -------------------------------------------------------------------------

interface RepoNodeProps extends NodeProps {
    repo: RepoView & { kind: "repo" }
    favorites: Set<string>
    toggleFavorite: (id: string) => void
}

/// Memoized: `repo` keeps its identity within a views generation, `toggle` /
/// `onSelect` are stable, so a selection change in App skips this whole
/// subtree — the affected Rows re-render through their store subscription.
const RepoNode = memo(function RepoNode({
    repo,
    collapsed,
    expanded,
    forcedOpen,
    favorites,
    toggleFavorite,
    toggle,
    onSelect,
}: RepoNodeProps) {
    const nodeId = repoId(repo.repoId)
    const isEmpty = repo.active.length === 0
    const isOpen = forcedOpen.has(nodeId) || !collapsed.has(nodeId)
    const label = repo.displayName ?? repo.name

    return (
        <div>
            <Row
                nodeId={nodeId}
                depth={0}
                isLeaf={isEmpty}
                isExpanded={!isEmpty && isOpen}
                grouping
                label={label}
                swatch={repo.color}
                title={repo.mainWorktree}
                meta={
                    <>
                        {repo.defaultBranch && (
                            <span className="row-branch">
                                {repo.defaultBranch}
                            </span>
                        )}
                        <RepoStatusDots
                            dirty={repo.dirty}
                            hasUncommittedSpecs={repo.hasUncommittedSpecs}
                            dirtyWorktrees={repo.dirtyWorktrees}
                        />
                        <span className="row-count">{repo.active.length}</span>
                    </>
                }
                onToggle={isEmpty ? undefined : () => toggle(nodeId, true)}
                onSelect={() =>
                    onSelect(nodeId, { kind: "repo", repoId: repo.repoId })
                }
            />
            {!isEmpty && isOpen && (
                <div role="group">
                    {partitionFavorites(repo.active, favorites, (lc) =>
                        logicalChangeId(repo.repoId, lc.name),
                    ).map((lc) => (
                        <LogicalChangeRow
                            key={logicalChangeId(repo.repoId, lc.name)}
                            repoId={repo.repoId}
                            logical={lc}
                            color={repo.color}
                            collapsed={collapsed}
                            expanded={expanded}
                            forcedOpen={forcedOpen}
                            favorites={favorites}
                            toggleFavorite={toggleFavorite}
                            toggle={toggle}
                            onSelect={onSelect}
                        />
                    ))}
                </div>
            )}
        </div>
    )
})

interface LogicalChangeRowProps extends NodeProps {
    repoId: string
    logical: LogicalChange
    /// The owning repo's palette colour, surfaced as a dot on the change-name
    /// line so a change reads as belonging to its workspace.
    color: PaletteColor | null
    favorites: Set<string>
    toggleFavorite: (id: string) => void
}

/// Either a flattened single-instance row (no parent disclosure) or a
/// parent disclosure with one child per instance.
function LogicalChangeRow({
    repoId: rid,
    logical,
    color,
    collapsed,
    expanded,
    forcedOpen,
    favorites,
    toggleFavorite,
    toggle,
    onSelect,
}: LogicalChangeRowProps) {
    // The favorite keys on the lc-level id whichever shape renders, so the
    // star follows the change across singleton↔multi promotion.
    const favoriteKey = logicalChangeId(rid, logical.name)
    const favorite: RowFavorite = {
        active: favorites.has(favoriteKey),
        onToggle: () => toggleFavorite(favoriteKey),
    }

    if (logical.instances.length === 1) {
        return (
            <InstanceNode
                repoId={rid}
                changeName={logical.name}
                instance={logical.instances[0]!}
                color={color}
                isPrimary={true}
                isSingleton={true}
                depth={1}
                collapsed={collapsed}
                expanded={expanded}
                forcedOpen={forcedOpen}
                favorite={favorite}
                toggle={toggle}
                onSelect={onSelect}
            />
        )
    }

    const nodeId = logicalChangeId(rid, logical.name)
    const isOpen = forcedOpen.has(nodeId) || !collapsed.has(nodeId)

    // The parent names the change for all its instances. Prefer the proposal
    // title of the primary instance (instances are sorted most-recently-
    // modified first, so [0] is the primary the active indicator pins to),
    // falling back to the logical change name. When the title is shown, the
    // hover tooltip keeps the change name recoverable — mirroring the
    // singleton row in `InstanceNode`.
    const primaryTitle = logical.instances[0]?.change.title ?? null

    return (
        <DisclosureGroup
            id={nodeId}
            depth={1}
            label={stripInlineMarkdown(primaryTitle ?? logical.name)}
            title={primaryTitle ? logical.name : undefined}
            meta={
                <span className="row-count">
                    {logical.instances.length}
                </span>
            }
            favorite={favorite}
            isOpen={isOpen}
            onToggle={() => toggle(nodeId, true)}
            onSelect={() =>
                onSelect(nodeId, {
                    kind: "logicalChange",
                    repoId: rid,
                    changeName: logical.name,
                })
            }
        >
            {logical.instances.map((inst, idx) => (
                <InstanceNode
                    key={instanceId(rid, logical.name, inst.worktreePath)}
                    repoId={rid}
                    changeName={logical.name}
                    instance={inst}
                    color={color}
                    isPrimary={idx === 0}
                    isSingleton={false}
                    depth={2}
                    collapsed={collapsed}
                    expanded={expanded}
                    forcedOpen={forcedOpen}
                    toggle={toggle}
                    onSelect={onSelect}
                />
            ))}
        </DisclosureGroup>
    )
}

interface DisclosureGroupProps {
    id: string
    depth: number
    label: ReactNode
    /// Optional `title` tooltip for the group's row — used by the multi-
    /// instance change parent to keep the change name recoverable when the
    /// label shows the proposal title instead.
    title?: string
    meta?: ReactNode
    /// Favorite-toggle wiring — present only on the multi-instance change
    /// parent (the one favoritable DisclosureGroup caller).
    favorite?: RowFavorite
    isOpen: boolean
    onToggle: () => void
    onSelect: (event: ReactMouseEvent) => void
    children: ReactNode
}

function DisclosureGroup({
    id,
    depth,
    label,
    title,
    meta,
    favorite,
    isOpen,
    onToggle,
    onSelect,
    children,
}: DisclosureGroupProps) {
    return (
        <div>
            <Row
                nodeId={id}
                depth={depth}
                isExpanded={isOpen}
                grouping
                label={label}
                title={title}
                meta={meta}
                favorite={favorite}
                onToggle={onToggle}
                onSelect={onSelect}
            />
            {isOpen && <div role="group">{children}</div>}
        </div>
    )
}

interface InstanceNodeProps extends NodeProps {
    repoId: string
    changeName: string
    instance: ChangeInstance
    /// Owning repo's palette colour — rendered as a dot on the singleton's
    /// change-name line. Unused by multi-instance children (single-line).
    color: PaletteColor | null
    isPrimary: boolean
    isSingleton: boolean
    depth: number
    /// Favorite-toggle wiring, passed only for the flattened singleton (the
    /// sole favoritable InstanceNode shape) — multi-instance children carry
    /// no star (*Change-Row Favorite Toggle*).
    favorite?: RowFavorite
}

function InstanceNode({
    repoId: rid,
    changeName,
    instance,
    color,
    isPrimary,
    isSingleton,
    depth,
    collapsed,
    expanded,
    forcedOpen,
    favorite,
    toggle,
    onSelect,
}: InstanceNodeProps) {
    const nodeId = instanceId(rid, changeName, instance.worktreePath)
    const isOpen = forcedOpen.has(nodeId) || !collapsed.has(nodeId)

    // Shared status elements: the task-progress meter (in progress) or the
    // completion ✓, the relative modification time, and the divergence label.
    // The active-instance dot is NOT here — it is a multi-instance-child
    // element, prepended in the child branch below.
    const statusCluster = (
        <>
            {instance.change.artifacts.tasks &&
                instance.change.totalTasks > 0 &&
                !allTasksDone(instance.change) && (
                    <TaskProgress
                        completed={instance.change.completedTasks}
                        total={instance.change.totalTasks}
                    />
                )}
            {allTasksDone(instance.change) && (
                <CompletionMark />
            )}
            <span
                className="row-mtime"
                // The tooltip takes the same guard as the text. Without it a row
                // reading "—" reveals `1970-01-01T00:00:00.000Z` on hover — the
                // application stating a fabricated date in exactly the confident
                // tone it states real ones, which is the failure the service's
                // `Option<u64>` and the header's absent label both exist to
                // avoid.
                title={
                    instance.modifiedAt === 0
                        ? undefined
                        : new Date(instance.modifiedAt * 1000).toISOString()
                }
            >
                {instance.modifiedAt === 0 ? (
                    NO_MTIME
                ) : (
                    <RelativeTime unixSeconds={instance.modifiedAt} />
                )}
            </span>
            {instance.divergence && (
                <DivergenceChip label={instance.divergence} />
            )}
            <SpecStateChip state={instance.specCommitState} />
        </>
    )

    const subtree = isOpen && (
        <div role="group">
            <ArtifactSubtree
                containerId={nodeId}
                workspaceUri={instance.worktreePath}
                change={instance.change}
                depth={depth + 1}
                collapsed={collapsed}
                expanded={expanded}
                forcedOpen={forcedOpen}
                toggle={toggle}
                onSelect={onSelect}
            />
        </div>
    )

    const select = () =>
        onSelect(nodeId, {
            kind: "instance",
            repoId: rid,
            changeName,
            worktreePath: instance.worktreePath,
        })

    // A flattened singleton is the sole row for its change → two lines: the
    // proposal title (falling back to the change name) on line 1 (full width),
    // worktree identity + status on line 2 (*Two-Line Sole-Change-Row
    // Layout*). This keeps the branch out of the greedy ellipsizing label,
    // where a long change name used to clip it. When the title is shown, the
    // hover tooltip keeps the change name recoverable.
    if (isSingleton) {
        const identity = worktreeIdentity(instance)
        const detail = (
            <>
                {identity && (
                    <span
                        // Appearance and tint come from `ident-chip`, this
                        // row's layout from `row-worktree`. The class list is
                        // built by the shared helper, not spelled out here,
                        // so the detail pane's chip cannot drift from this
                        // one — see `identChipClass`.
                        className={identChipClass(color, "row-worktree")}
                    >
                        {identity}
                    </span>
                )}
                <span className="row-meta">{statusCluster}</span>
            </>
        )
        return (
            <div>
                <Row
                    nodeId={nodeId}
                    depth={depth}
                    isExpanded={isOpen}
                    label={stripInlineMarkdown(
                        instance.change.title ?? changeName,
                    )}
                    title={
                        instance.change.title ? changeName : undefined
                    }
                    primarySwatch={color}
                    complete={allTasksDone(instance.change)}
                    detail={detail}
                    favorite={favorite}
                    onToggle={() => toggle(nodeId, true)}
                    onSelect={select}
                />
                {subtree}
            </div>
        )
    }

    // A multi-instance child row stays single-line: the branch (or path
    // basename) is its label, and the active-instance dot — only on the
    // primary — leads the meta slot.
    const meta = (
        <>
            {isPrimary && (
                <span
                    className="status-dot status-dot--ok"
                    title="Most recently modified"
                    aria-label="Most recently modified"
                />
            )}
            {statusCluster}
        </>
    )
    return (
        <div>
            <Row
                nodeId={nodeId}
                depth={depth}
                isExpanded={isOpen}
                label={labelForInstance(instance)}
                meta={meta}
                onToggle={() => toggle(nodeId, true)}
                onSelect={select}
            />
            {subtree}
        </div>
    )
}

/// Label for a multi-instance child row: the branch distinguishes the worktree,
/// falling back to the worktree path basename (detached HEAD / no git context)
/// and finally the full path. Singletons do NOT use this — they render two
/// lines via the sole-change-row layout, with the change name as their label.
function labelForInstance(instance: ChangeInstance): ReactNode {
    return (
        instance.branch ??
        basename(instance.worktreePath) ??
        instance.worktreePath
    )
}

/// Worktree identity for a singleton's detail line: the branch name, falling
/// back to the worktree folder basename when the branch is unknown (detached
/// HEAD / no git context). Null when neither is available (should not happen
/// for a real worktree).
function worktreeIdentity(instance: ChangeInstance): string | null {
    return instance.branch ?? basename(instance.worktreePath)
}

function basename(path: string): string | null {
    const parts = path.split("/").filter(Boolean)
    return parts.length > 0 ? parts[parts.length - 1]! : null
}

/// An instance with no recorded modification time. `ChangeInstance.modifiedAt`
/// uses 0 rather than a null for "unknown", so the em dash is this surface's own
/// answer to that — the Dashboard renders nothing and the identity header omits
/// its label entirely, which is why the shared formatter leaves the unknown case
/// to its callers instead of picking one presentation for all three.
const NO_MTIME = "—"

function DivergenceChip({ label }: { label: DivergenceLabel }) {
    const text = label === "diverged" ? "diverged" : "stale"
    const tone = label === "diverged" ? "chip--warn" : "chip--muted"
    return <span className={`chip ${tone}`}>{text}</span>
}

/// Per-instance commit-state chip. Rendered only when this worktree's copy of
/// the change is uncommitted; a committed instance shows nothing, to keep the
/// row quiet in the common case.
function SpecStateChip({ state }: { state: SpecCommitState }) {
    if (state === "committed") return null
    const text = state === "untracked" ? "untracked" : "modified"
    return <span className="chip chip--warn">{text}</span>
}

/// Repo-node working-tree rollup. Two distinct signals: a muted dot when any
/// worktree is dirty (the familiar source-control dot), and an accent mark when
/// that dirt includes uncommitted *specs* — so "a dirty source file" reads
/// differently from "an uncommitted spec". Both are suppressed when the repo is
/// clean.
function RepoStatusDots({
    dirty,
    hasUncommittedSpecs,
    dirtyWorktrees,
}: {
    dirty: boolean
    hasUncommittedSpecs: boolean
    dirtyWorktrees: string[]
}) {
    if (!dirty && !hasUncommittedSpecs) return null
    const dirtyTitle =
        dirtyWorktrees.length > 0
            ? `Uncommitted changes in:\n${dirtyWorktrees.join("\n")}`
            : "Uncommitted changes"
    return (
        <>
            {dirty && (
                <span
                    className="status-dot status-dot--muted"
                    title={dirtyTitle}
                    aria-label="Uncommitted changes"
                />
            )}
            {hasUncommittedSpecs && (
                <span
                    className="status-dot status-dot--warn"
                    title="Uncommitted spec changes"
                    aria-label="Uncommitted spec changes"
                />
            )}
        </>
    )
}

// -------------------------------------------------------------------------
// Flat (non-git) workspace + descendants
// -------------------------------------------------------------------------

interface FlatWorkspaceNodeProps extends NodeProps {
    workspace: WorkspaceFolder
    changes: ChangeData[]
    displayName: string | null
    color: PaletteColor | null
    favorites: Set<string>
    toggleFavorite: (id: string) => void
}

/// Memoized for the same reason as RepoNode.
const FlatWorkspaceNode = memo(function FlatWorkspaceNode({
    workspace,
    changes,
    displayName,
    color,
    collapsed,
    expanded,
    forcedOpen,
    favorites,
    toggleFavorite,
    toggle,
    onSelect,
}: FlatWorkspaceNodeProps) {
    const nodeId = flatWorkspaceId(workspace.uri)
    const isEmpty = changes.length === 0
    const isOpen = forcedOpen.has(nodeId) || !collapsed.has(nodeId)
    const label = displayName ?? workspace.name

    return (
        <div>
            <Row
                nodeId={nodeId}
                depth={0}
                isLeaf={isEmpty}
                isExpanded={!isEmpty && isOpen}
                grouping
                label={label}
                swatch={color}
                title={workspace.uri}
                meta={<span className="row-count">{changes.length}</span>}
                onToggle={isEmpty ? undefined : () => toggle(nodeId, true)}
                onSelect={() =>
                    onSelect(nodeId, {
                        kind: "workspace",
                        workspaceUri: workspace.uri,
                    })
                }
            />
            {!isEmpty && isOpen && (
                <div role="group">
                    {partitionFavorites(changes, favorites, (change) =>
                        changeRowId(nodeId, change.changeId),
                    ).map((change) => (
                        <FlatChangeNode
                            key={changeRowId(nodeId, change.changeId)}
                            containerId={nodeId}
                            workspaceUri={workspace.uri}
                            change={change}
                            color={color}
                            collapsed={collapsed}
                            expanded={expanded}
                            forcedOpen={forcedOpen}
                            favorites={favorites}
                            toggleFavorite={toggleFavorite}
                            toggle={toggle}
                            onSelect={onSelect}
                        />
                    ))}
                </div>
            )}
        </div>
    )
})

interface FlatChangeNodeProps extends NodeProps {
    containerId: string
    workspaceUri: string
    change: ChangeData
    /// Owning workspace's palette colour — rendered as a dot on the change-name
    /// line, matching the git singleton treatment.
    color: PaletteColor | null
    favorites: Set<string>
    toggleFavorite: (id: string) => void
}

function FlatChangeNode({
    containerId,
    workspaceUri,
    change,
    color,
    collapsed,
    expanded,
    forcedOpen,
    favorites,
    toggleFavorite,
    toggle,
    onSelect,
}: FlatChangeNodeProps) {
    const nodeId = changeRowId(containerId, change.changeId)
    const isOpen = forcedOpen.has(nodeId) || !collapsed.has(nodeId)
    const isCompleted = allTasksDone(change)
    // The flat change-row id doubles as the favorite key — already
    // position-independent (workspace uri + change id).
    const favorite: RowFavorite = {
        active: favorites.has(nodeId),
        onToggle: () => toggleFavorite(nodeId),
    }

    const label = change.title
        ? stripInlineMarkdown(change.title)
        : change.changeId

    return (
        <div>
            <Row
                nodeId={nodeId}
                depth={1}
                isExpanded={isOpen}
                grouping
                label={label}
                primarySwatch={color}
                complete={isCompleted}
                favorite={favorite}
                detail={
                    <>
                        <span className="row-changeid" title={change.changeId}>
                            {change.changeId}
                        </span>
                        {isCompleted && (
                            <span className="row-meta">
                                <CompletionMark />
                            </span>
                        )}
                    </>
                }
                onToggle={() => toggle(nodeId, true)}
                onSelect={() =>
                    onSelect(nodeId, {
                        kind: "change",
                        workspaceUri,
                        changeId: change.changeId,
                    })
                }
            />
            {isOpen && (
                <div role="group">
                    <ArtifactSubtree
                        containerId={nodeId}
                        workspaceUri={workspaceUri}
                        change={change}
                        depth={2}
                        collapsed={collapsed}
                        expanded={expanded}
                        forcedOpen={forcedOpen}
                        toggle={toggle}
                        onSelect={onSelect}
                    />
                </div>
            )}
        </div>
    )
}

// -------------------------------------------------------------------------
// Artifact / Specs / Sections / Tasks subtree (shared by Flat and Instance)
// -------------------------------------------------------------------------

interface ArtifactSubtreeProps extends NodeProps {
    containerId: string
    workspaceUri: string
    change: ChangeData
    depth: number
}

function ArtifactSubtree({
    containerId,
    workspaceUri,
    change,
    depth,
    collapsed,
    expanded,
    forcedOpen,
    toggle,
    onSelect,
}: ArtifactSubtreeProps) {
    return (
        <>
            <ArtifactNode
                containerId={containerId}
                workspaceUri={workspaceUri}
                kind="proposal"
                label="Proposal"
                present={change.artifacts.proposal}
                change={change}
                depth={depth}
                collapsed={collapsed}
                expanded={expanded}
                forcedOpen={forcedOpen}
                toggle={toggle}
                onSelect={onSelect}
            />
            <ArtifactNode
                containerId={containerId}
                workspaceUri={workspaceUri}
                kind="specs"
                label="Specs"
                present={change.artifacts.specs.length > 0}
                change={change}
                depth={depth}
                collapsed={collapsed}
                expanded={expanded}
                forcedOpen={forcedOpen}
                toggle={toggle}
                onSelect={onSelect}
            />
            <ArtifactNode
                containerId={containerId}
                workspaceUri={workspaceUri}
                kind="design"
                label="Design"
                present={change.artifacts.design}
                change={change}
                depth={depth}
                collapsed={collapsed}
                expanded={expanded}
                forcedOpen={forcedOpen}
                toggle={toggle}
                onSelect={onSelect}
            />
            <ArtifactNode
                containerId={containerId}
                workspaceUri={workspaceUri}
                kind="tasks"
                label="Tasks"
                meta={
                    change.artifacts.tasks && change.totalTasks > 0 ? (
                        allTasksDone(change) ? (
                            <CompletionMark />
                        ) : (
                            <TaskProgress
                                completed={change.completedTasks}
                                total={change.totalTasks}
                            />
                        )
                    ) : undefined
                }
                present={change.artifacts.tasks}
                change={change}
                depth={depth}
                collapsed={collapsed}
                expanded={expanded}
                forcedOpen={forcedOpen}
                toggle={toggle}
                onSelect={onSelect}
            />
        </>
    )
}

interface ArtifactNodeProps extends NodeProps {
    containerId: string
    workspaceUri: string
    kind: ArtifactKind
    label: string
    present: boolean
    change: ChangeData
    depth: number
    /// Optional trailing meta (e.g. the Tasks node's progress meter or
    /// completion ✓). Only the Tasks node passes this today.
    meta?: ReactNode
}

function ArtifactNode({
    containerId,
    workspaceUri,
    kind,
    label,
    present,
    change,
    depth,
    meta,
    collapsed,
    expanded,
    forcedOpen,
    toggle,
    onSelect,
}: ArtifactNodeProps) {
    const nodeId = artifactNodeId(containerId, change.changeId, kind)
    const hasChildren =
        (kind === "specs" && change.artifacts.specs.length > 0) ||
        (kind === "tasks" && change.sections.length > 0)
    // The Tasks artifact node is collapsed by default for every change, so
    // expanding a change keeps its task rows out of the tree until the user
    // opts in; every other artifact stays default-open. (Progress still shows
    // in the Tasks row's meta slot whether open or closed.)
    const defaultOpen = kind !== "tasks"
    const isOpen =
        forcedOpen.has(nodeId) || (defaultOpen ? !collapsed.has(nodeId) : expanded.has(nodeId))

    return (
        <div>
            <Row
                nodeId={nodeId}
                depth={depth}
                isLeaf={!hasChildren}
                isExpanded={isOpen}
                // Clicking the Specs artifact row changes no detail-pane
                // content (App's handleSelect returns early on it) — it is
                // disclosure-only, so the keyboard treats it as grouping.
                grouping={kind === "specs"}
                label={label}
                meta={meta}
                dim={!present}
                onToggle={
                    present && hasChildren
                        ? () => toggle(nodeId, defaultOpen)
                        : undefined
                }
                onSelect={
                    present
                        ? (e) =>
                              onSelect(
                                  nodeId,
                                  {
                                      kind: "artifact",
                                      workspaceUri,
                                      changeId: change.changeId,
                                      artifactKind: kind,
                                  },
                                  { reader: e.metaKey || e.ctrlKey },
                              )
                        : undefined
                }
            />
            {isOpen && hasChildren && (
                <div role="group">
                    {kind === "specs" &&
                        change.artifacts.specs.map((capability) => (
                            <CapabilitySpecNode
                                key={specNodeId(containerId, change.changeId, capability)}
                                containerId={containerId}
                                workspaceUri={workspaceUri}
                                changeId={change.changeId}
                                capability={capability}
                                depth={depth + 1}
                                onSelect={onSelect}
                            />
                        ))}
                    {kind === "tasks" &&
                        change.sections.map((section, index) => (
                            <SectionNode
                                key={sectionNodeId(containerId, change.changeId, index)}
                                containerId={containerId}
                                workspaceUri={workspaceUri}
                                changeId={change.changeId}
                                section={section}
                                sectionIndex={index}
                                depth={depth + 1}
                                collapsed={collapsed}
                                expanded={expanded}
                                forcedOpen={forcedOpen}
                                toggle={toggle}
                                onSelect={onSelect}
                            />
                        ))}
                </div>
            )}
        </div>
    )
}

interface CapabilitySpecNodeProps {
    containerId: string
    workspaceUri: string
    changeId: string
    capability: string
    depth: number
    onSelect: (nodeId: string, selection: TreeSelection, options?: SelectOptions) => void
}

function CapabilitySpecNode({
    containerId,
    workspaceUri,
    changeId: cid,
    capability,
    depth,
    onSelect,
}: CapabilitySpecNodeProps) {
    const nodeId = specNodeId(containerId, cid, capability)
    return (
        <Row
            nodeId={nodeId}
            depth={depth}
            isLeaf
            label={capability}
            onSelect={(e) =>
                onSelect(
                    nodeId,
                    {
                        kind: "spec",
                        workspaceUri,
                        changeId: cid,
                        capability,
                    },
                    { reader: e.metaKey || e.ctrlKey },
                )
            }
        />
    )
}

interface SectionNodeProps extends NodeProps {
    containerId: string
    workspaceUri: string
    changeId: string
    section: Section
    sectionIndex: number
    depth: number
}

function SectionNode({
    containerId,
    workspaceUri,
    changeId: cid,
    section,
    sectionIndex,
    depth,
    collapsed,
    expanded,
    forcedOpen,
    toggle,
    onSelect,
}: SectionNodeProps) {
    const nodeId = sectionNodeId(containerId, cid, sectionIndex)
    const allTasksDone =
        section.tasks.length > 0 && section.tasks.every((t) => t.completed)
    const defaultOpen = defaultIsOpenForSection(section)
    const isOpen =
        forcedOpen.has(nodeId) || (defaultOpen ? !collapsed.has(nodeId) : expanded.has(nodeId))
    return (
        <div>
            <Row
                nodeId={nodeId}
                depth={depth}
                isLeaf={section.tasks.length === 0}
                isExpanded={isOpen}
                label={stripInlineMarkdown(section.title)}
                title={stripInlineMarkdown(section.title)}
                meta={
                    allTasksDone ? (
                        <CompletionMark />
                    ) : undefined
                }
                onToggle={
                    section.tasks.length > 0
                        ? () => toggle(nodeId, defaultOpen)
                        : undefined
                }
                onSelect={() =>
                    onSelect(nodeId, {
                        kind: "section",
                        workspaceUri,
                        changeId: cid,
                        sectionIndex,
                    })
                }
            />
            {isOpen && section.tasks.length > 0 && (
                <div role="group">
                    {section.tasks.map((task, idx) => (
                        <TaskNode
                            key={taskNodeId(containerId, cid, sectionIndex, idx)}
                            containerId={containerId}
                            workspaceUri={workspaceUri}
                            changeId={cid}
                            sectionIndex={sectionIndex}
                            taskIndex={idx}
                            task={task}
                            depth={depth + 1}
                            onSelect={onSelect}
                        />
                    ))}
                </div>
            )}
        </div>
    )
}

interface TaskNodeProps {
    containerId: string
    workspaceUri: string
    changeId: string
    sectionIndex: number
    taskIndex: number
    task: Task
    depth: number
    onSelect: (nodeId: string, selection: TreeSelection, options?: SelectOptions) => void
}

function TaskNode({
    containerId,
    workspaceUri,
    changeId: cid,
    sectionIndex,
    taskIndex,
    task,
    depth,
    onSelect,
}: TaskNodeProps) {
    const nodeId = taskNodeId(containerId, cid, sectionIndex, taskIndex)
    return (
        <Row
            nodeId={nodeId}
            depth={depth}
            isLeaf
            struck={task.completed}
            label={stripInlineMarkdown(task.text)}
            title={stripInlineMarkdown(task.text)}
            onSelect={() =>
                onSelect(nodeId, {
                    kind: "task",
                    workspaceUri,
                    changeId: cid,
                    sectionIndex,
                    taskIndex,
                    lineNumber: task.lineNumber,
                })
            }
        />
    )
}
