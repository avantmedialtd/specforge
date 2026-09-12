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
    ChangeData,
    ChangeInstance,
    LogicalChange,
    PaletteColor,
    RepoView,
    SpecCommitState,
    TreeSelection,
    WorkspaceFolder,
    WorkspaceView,
} from "../types"
import { identChipClass } from "../changeIdentity"
import { defaultInstanceFor } from "../changeNavigation"
import { stripInlineMarkdown } from "../markdown"
import { isNewWindowModifier } from "../platform"
import { EmptyState } from "./EmptyState"
import { RelativeTime } from "./RelativeTime"
import { ChevronDown, ChevronRight, CompletionMark, Star } from "./icons"
import { partitionFavorites, type RowFavorite } from "./favorites"
import { DivergenceChip, TaskProgress } from "./changeMarks"
import {
    getFavoriteChangeIds,
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

/// Imperative surface for reveal — a navigation to an addressed change opens
/// its top-level row (a TRANSIENT overlay above the session's own disclosure
/// state, and with nothing persisted there is nothing it could write) and
/// marks the change row selected via the existing `selectedNodeId` prop, which
/// the caller already updates in the same pass. One entry point rather than an
/// effect that reacts to view/selection changes (`view-routing`: *Navigation
/// Reveal Is Transient*).
export interface WorkspaceTreeHandle {
    /// Open every id in `path` — `[containerId, changeRowId]` for an artifact
    /// address, `[containerId]` for a files address (see
    /// `src/routing/nodeId.ts`'s `addressToNodePath`, the only intended
    /// producer of this array; it is built by construction from the same
    /// compositional id helpers this file exports, never by splitting a
    /// single leaf id string — A2: those ids embed absolute filesystem
    /// paths, so a "/"-split ancestor can equal an unrelated real node's id
    /// whenever one registered path is a directory prefix of another, e.g.
    /// this very repo's own `.claude/worktrees/<name>` layout). Once the
    /// path's rows exist in the DOM, the leaf is focused and scrolled into
    /// view via the same `focusRow` keyboard nav already uses. `null`/empty
    /// clears the transient reveal — nothing is force-opened, and a row the
    /// reveal had opened falls back to the disclosure state it had.
    reveal: (path: string[] | null) => void
}

// -------------------------------------------------------------------------
// Node-ID helpers — stable React keys and the rows' keyboard identity. Each
// helper composes on the one above, so a change row's ID embeds its container.
// Nothing is persisted under these ids any more (design D6): disclosure is
// session state, and favorites key on the change row id in their own list.
// -------------------------------------------------------------------------

// Exported: `src/routing/nodeId.ts` reuses these verbatim (rather than
// reimplementing the scheme) so an Address-derived node id can never drift
// from what this file actually renders — see `addressToNodePath`.
export const flatWorkspaceId = (uri: string) => `flat:${uri}`
export const repoId = (id: string) => `repo:${id}`
/// A repo group's change row: one per LOGICAL change, whatever its rendered
/// instance count (`spec-browser`: *Workspace Tree Hierarchy*).
export const logicalChangeId = (rid: string, name: string) =>
    `${repoId(rid)}/lc:${name}`
/// A flat workspace's change row. `containerId` is the flat-workspace id.
export const changeRowId = (containerId: string, changeId: string) =>
    `${containerId}/change:${changeId}`

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
// Keyboard navigation — WAI-ARIA tree pattern with a roving tabindex over the
// tree's TWO levels. The "visible row list" is the DOM itself:
// `[role="treeitem"]` elements in document order, which equals visual order
// for this nested rendering (a future CSS reordering would break that
// invariant — keep them aligned). Focus is held by the DOM; React state never
// tracks the current row, so arrowing produces zero React renders.
// -------------------------------------------------------------------------

/// Settle delay before resting keyboard focus on a change row opens it in
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

/// Whether a row has a disclosure to drive at all. A change row is a leaf and
/// an empty top-level row renders as one, so neither exposes `aria-expanded`.
function isDisclosure(row: HTMLElement): boolean {
    return row.getAttribute("aria-expanded") !== null
}

/// Expansion toggles reuse the chevron's own click handler so the keyboard
/// path shares the exact pointer contract.
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

/// True iff every parsed task in the change is complete (and at least one
/// task exists). Drives the trailing completion glyph on both change-row
/// shapes; centralised so the rule can't drift between the two paths.
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
    // Top-level rows open by default; a row the user closes stays closed for
    // the rest of the SESSION and is never persisted (design D6 —
    // `spec-browser`: *Workspace Tree Hierarchy*, "no disclosure state of any
    // kind survives a restart, and the tree performs no settings write on a
    // toggle"). The `collapsedTreeNodeIds`/`expandedTreeNodeIds` settings
    // fields are left in place, unread, so an older settings file still parses.
    const [closed, setClosed] = useState<Set<string>>(new Set())
    // Favorited changes, keyed by position-independent change identity
    // (`logicalChangeId` for repo-group changes, the flat change-row id for
    // flat-workspace changes), so a favorite survives a change gaining or
    // losing a worktree (*Favorite Identity and Persistence*).
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

    // Transient reveal overlay — see `WorkspaceTreeHandle`. Deliberately its
    // own `useState`, never folded into `closed`: a reveal must be able to
    // open a row the user closed without becoming that user's choice.
    const [forcedOpen, setForcedOpen] = useState<Set<string>>(new Set())
    // Mirrors `forcedOpen` for `toggle` (a `useCallback` with an empty
    // dependency array, so it can't close over the state value itself
    // without going stale) to read synchronously — same "ref updated inline
    // during render" pattern `onSelectRef` already uses below.
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
    // commits the newly-open top-level row's DOM before running this effect).
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
            // order for strictly decreasing levels — over two levels that is
            // the row's own top-level row, which is where focus falls back to
            // when a refresh removes a focused change row.
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
        // Debounced follow-focus: resting KEYBOARD focus on a change row
        // opens it as a click would. Top-level rows are disclosure-first and
        // never start the timer; pointer-induced focus (mousedown precedes
        // focusin) and the roving effect's programmatic restore are excluded
        // so a chevron click or a watcher refresh can't navigate the detail
        // pane; the activeElement and selection re-checks at expiry guard
        // against focus having moved on and against re-selecting.
        clearFollowTimer()
        const viaPointer = pointerDownRef.current
        pointerDownRef.current = false
        if (!viaPointer && !restoringRef.current && row.dataset.grouping !== "true") {
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
                    // A change row, or a closed/leaf top-level row: jump to
                    // the row's top-level row, and do not move when there is
                    // none above it.
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
                clearFollowTimer()
                // A top-level row WITH changes toggles, exactly as its
                // chevron click does. An EMPTY top-level row has no
                // disclosure, so it selects and opens the workspace file
                // browser, as a click on it does. A change row navigates.
                if (row.dataset.grouping === "true" && isDisclosure(row)) {
                    clickChevron(row)
                } else {
                    row.click()
                }
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
    // along the captured ancestor chain — for a change row that is its own
    // top-level row — and if focus was lost to the body because the focused
    // element vanished, restore it.
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

    // Hydrate the favorites. Disclosure state is deliberately NOT hydrated:
    // it does not survive a restart at all (design D6), so the tree is
    // interactive from its first paint with every top-level row open.
    useEffect(() => {
        let cancelled = false
        getFavoriteChangeIds()
            .then((favoriteIds) => {
                if (cancelled) return
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
            })
            .catch(() => {
                // An unreadable settings file costs the session its stars,
                // never its tree.
            })
        return () => {
            cancelled = true
        }
    }, [])

    // Favorites persist as a delta: pending toggles are drained and sent, and
    // the backend merges them under its settings lock. Draining and
    // marshalling live in one helper because the debounced flush and the
    // page-dismissal flush must never disagree about what "taking the pending
    // ops" means.
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

    // A1: a forced-open row's chevron must unambiguously mean "close it".
    // The row is rendered open *because* reveal is overriding whatever the
    // session's own `closed` set says (that override is exactly WHY reveal
    // exists: the common case is a row the user had closed), so toggling
    // `closed` alone could leave it computing "open" once the reveal clears —
    // the chevron would look dead. Closing explicitly, and dropping the
    // reveal for just this one id, makes the chevron respond immediately.
    const toggle = useCallback((id: string) => {
        if (forcedOpenRef.current.has(id)) {
            setForcedOpen((prev) => {
                if (!prev.has(id)) return prev
                const next = new Set(prev)
                next.delete(id)
                return next
            })
            setClosed((prev) => (prev.has(id) ? prev : new Set(prev).add(id)))
            return
        }
        setClosed((prev) => {
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
                            closed={closed}
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
                            closed={closed}
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
    /// Stable hierarchical node ID — React key material AND the row's
    /// keyboard identity (`data-node-id`, selection-store subscription).
    nodeId: string
    /// 0 for a top-level row, 1 for a change row. Drives the indent and
    /// `aria-level` (1 and 2 — the tree has no other depths).
    depth: number
    isLeaf?: boolean
    isExpanded?: boolean
    /// Disclosure-first row (a repo group or a non-git workspace): Enter and
    /// Space toggle expansion instead of selecting, and follow-focus never
    /// opens it in the detail pane — mirroring the pointer contract, where
    /// clicking it changes no content beyond opening the file browser.
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
    /// chevron and label. Top-level rows only — change rows pass nothing.
    /// `null` / undefined = no swatch, label slots in directly after the
    /// chevron.
    swatch?: PaletteColor | null
    /// Optional `title` attribute for the row — used to surface the path on
    /// renamed top-level rows so they remain disambiguatable.
    title?: string
    /// Marks a completed two-line change row: its inline-start rail switches
    /// from the workspace-colour rail to the --ok completion rail (mutually
    /// exclusive with the swatch rail). No effect on single-line rows, which
    /// carry no rail. Selection still overrides the rail to --accent.
    complete?: boolean
    /// Favorite-toggle wiring — change rows only. Renders the star button in
    /// a reserved slot at the primary line's extreme trailing edge, after any
    /// meta (*Change-Row Favorite Toggle*).
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
            className={`tree-row${isSelected ? " selected" : ""}${topLevelClass}${twoLineClass}${railClass}`}
            style={{ paddingLeft: depth * 12 + 4 }}
            onClick={onSelect}
            title={title}
            role="treeitem"
            data-node-id={nodeId}
            data-grouping={grouping ? "true" : undefined}
            // All rows render -1; the roving effect in WorkspaceTree promotes
            // exactly one to 0. React never re-renders -1 over it because the
            // vdom value is unchanged.
            tabIndex={-1}
            // 1 for a top-level row, 2 for a change row — the tree's only two
            // levels. No row carries `aria-disabled`: every rendered row
            // responds to activation, now that absent artifacts have no row.
            aria-level={depth + 1}
            aria-selected={isSelected}
            aria-expanded={isLeaf ? undefined : isExpanded}
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
// Common props shared by the two top-level node kinds
// -------------------------------------------------------------------------

interface NodeProps {
    /// Top-level rows the user closed this session. Open by default, never
    /// persisted (design D6).
    closed: Set<string>
    /// Transient reveal overlay (see `WorkspaceTreeHandle.reveal`) — a row
    /// whose id is in this set renders open regardless of `closed`.
    forcedOpen: Set<string>
    toggle: (id: string) => void
    favorites: Set<string>
    toggleFavorite: (id: string) => void
    onSelect: (nodeId: string, selection: TreeSelection, options?: SelectOptions) => void
}

// -------------------------------------------------------------------------
// Repo group + its change rows
// -------------------------------------------------------------------------

interface RepoNodeProps extends NodeProps {
    repo: RepoView & { kind: "repo" }
}

/// Memoized: `repo` keeps its identity within a views generation, `toggle` /
/// `onSelect` are stable, so a selection change in App skips this whole
/// subtree — the affected Rows re-render through their store subscription.
const RepoNode = memo(function RepoNode({
    repo,
    closed,
    forcedOpen,
    favorites,
    toggleFavorite,
    toggle,
    onSelect,
}: RepoNodeProps) {
    const nodeId = repoId(repo.repoId)
    const isEmpty = repo.active.length === 0
    const isOpen = forcedOpen.has(nodeId) || !closed.has(nodeId)
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
                onToggle={isEmpty ? undefined : () => toggle(nodeId)}
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
                            favorites={favorites}
                            toggleFavorite={toggleFavorite}
                            onSelect={onSelect}
                        />
                    ))}
                </div>
            )}
        </div>
    )
})

interface LogicalChangeRowProps {
    repoId: string
    logical: LogicalChange
    /// The owning repo's palette colour, surfaced as a dot on the change-name
    /// line so a change reads as belonging to its workspace.
    color: PaletteColor | null
    favorites: Set<string>
    toggleFavorite: (id: string) => void
    onSelect: (nodeId: string, selection: TreeSelection, options?: SelectOptions) => void
}

/// The sole row for a git logical change — a LEAF, whatever its rendered
/// instance count (`spec-browser`: *Workspace Tree Hierarchy*).
///
/// Two lines: the proposal title on line 1 (full width), worktree identity and
/// status on line 2 (*Two-Line Sole-Change-Row Layout*). A change living in
/// several worktrees is not a disclosure parent with a row per instance — it
/// is this same row carrying an instance-count chip where a singleton carries
/// its branch, and the instance to read is chosen in the change header
/// (*Instance Switcher in the Change Header*).
function LogicalChangeRow({
    repoId: rid,
    logical,
    color,
    favorites,
    toggleFavorite,
    onSelect,
}: LogicalChangeRowProps) {
    const nodeId = logicalChangeId(rid, logical.name)
    // The favorite keys on the lc-level id, so the star follows the change
    // across a worktree being added or removed.
    const favorite: RowFavorite = {
        active: favorites.has(nodeId),
        onToggle: () => toggleFavorite(nodeId),
    }

    // Everything the row SAYS about the change is the default instance's —
    // the same instance a click on the row opens (`changeNavigation.ts`), so
    // the row cannot describe one copy while opening another.
    const instance = defaultInstanceFor(logical.instances)
    if (!instance) return null
    const count = logical.instances.length
    const isMulti = count > 1
    const identity = worktreeIdentity(instance)
    const complete = allTasksDone(instance.change)

    const detail = (
        <>
            {isMulti ? (
                // The instance-count chip stands where a singleton's branch
                // chip stands, in the same outlined-chip treatment but
                // UNTINTED and naming no branch: the instances' branches are
                // named in the change header's switcher.
                <span className={identChipClass(null, "row-worktree")}>
                    {count} worktrees
                </span>
            ) : (
                identity && (
                    // Appearance and tint come from `ident-chip`, this row's
                    // layout from `row-worktree`. The class list is built by
                    // the shared helper, not spelled out here, so the detail
                    // pane's chip cannot drift from this one — see
                    // `identChipClass`.
                    <span className={identChipClass(color, "row-worktree")}>
                        {identity}
                    </span>
                )
            )}
            <span className="row-meta">
                {instance.change.artifacts.tasks &&
                    instance.change.totalTasks > 0 &&
                    !complete && (
                        <TaskProgress
                            completed={instance.change.completedTasks}
                            total={instance.change.totalTasks}
                        />
                    )}
                {complete && <CompletionMark />}
                <span
                    className="row-mtime"
                    // The tooltip takes the same guard as the text. Without it
                    // a row reading "—" reveals `1970-01-01T00:00:00.000Z` on
                    // hover — the application stating a fabricated date in
                    // exactly the confident tone it states real ones, which is
                    // the failure the service's `Option<u64>` and the header's
                    // absent label both exist to avoid.
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
                {/* Divergence is a property of an INSTANCE, so a row naming
                    several names none of them: it is shown per instance in
                    the change header's switcher instead
                    (`spec-browser`: *Per-Instance Divergence Label*). */}
                {!isMulti && instance.divergence && (
                    <DivergenceChip label={instance.divergence} />
                )}
                <SpecStateChip state={instance.specCommitState} />
            </span>
        </>
    )

    return (
        <Row
            nodeId={nodeId}
            depth={1}
            isLeaf
            label={stripInlineMarkdown(instance.change.title ?? logical.name)}
            title={instance.change.title ? logical.name : undefined}
            primarySwatch={color}
            complete={complete}
            detail={detail}
            favorite={favorite}
            onSelect={(e) =>
                onSelect(
                    nodeId,
                    {
                        kind: "change",
                        container: { kind: "repo", repoId: rid },
                        changeName: logical.name,
                    },
                    { reader: isNewWindowModifier(e) },
                )
            }
        />
    )
}

/// Worktree identity for a change row's detail line: the branch name, falling
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
// Flat (non-git) workspace + its change rows
// -------------------------------------------------------------------------

interface FlatWorkspaceNodeProps extends NodeProps {
    workspace: WorkspaceFolder
    changes: ChangeData[]
    displayName: string | null
    color: PaletteColor | null
}

/// Memoized for the same reason as RepoNode.
const FlatWorkspaceNode = memo(function FlatWorkspaceNode({
    workspace,
    changes,
    displayName,
    color,
    closed,
    forcedOpen,
    favorites,
    toggleFavorite,
    toggle,
    onSelect,
}: FlatWorkspaceNodeProps) {
    const nodeId = flatWorkspaceId(workspace.uri)
    const isEmpty = changes.length === 0
    const isOpen = forcedOpen.has(nodeId) || !closed.has(nodeId)
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
                onToggle={isEmpty ? undefined : () => toggle(nodeId)}
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
                            favorites={favorites}
                            toggleFavorite={toggleFavorite}
                            onSelect={onSelect}
                        />
                    ))}
                </div>
            )}
        </div>
    )
})

interface FlatChangeNodeProps {
    containerId: string
    workspaceUri: string
    change: ChangeData
    /// Owning workspace's palette colour — rendered as a dot on the change-name
    /// line, matching the git change row's treatment.
    color: PaletteColor | null
    favorites: Set<string>
    toggleFavorite: (id: string) => void
    onSelect: (nodeId: string, selection: TreeSelection, options?: SelectOptions) => void
}

/// A flat-workspace change row — a LEAF, like its git counterpart. Line 2
/// carries the change's own identifier where a git row carries worktree
/// identity, and the completion ✓ where a git row carries its status cluster
/// (*Two-Line Sole-Change-Row Layout*).
function FlatChangeNode({
    containerId,
    workspaceUri,
    change,
    color,
    favorites,
    toggleFavorite,
    onSelect,
}: FlatChangeNodeProps) {
    const nodeId = changeRowId(containerId, change.changeId)
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
        <Row
            nodeId={nodeId}
            depth={1}
            isLeaf
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
            onSelect={(e) =>
                onSelect(
                    nodeId,
                    {
                        kind: "change",
                        container: { kind: "flat", workspaceUri },
                        changeName: change.changeId,
                    },
                    { reader: isNewWindowModifier(e) },
                )
            }
        />
    )
}
