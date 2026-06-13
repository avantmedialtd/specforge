import {
    createContext,
    memo,
    useCallback,
    useContext,
    useEffect,
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
    Task,
    TreeSelection,
    WorkspaceFolder,
    WorkspaceView,
} from "../types"
import { stripInlineMarkdown } from "../markdown"
import { EmptyState } from "./EmptyState"
import { Check, ChevronDown, ChevronRight } from "./icons"
import {
    getCollapsedTreeNodeIds,
    getExpandedTreeNodeIds,
    setCollapsedTreeNodeIds,
    setExpandedTreeNodeIds,
} from "../api"

interface WorkspaceTreeProps {
    views: WorkspaceView[]
    selectedNodeId: string | null
    onSelect: (nodeId: string, selection: TreeSelection) => void
}

// -------------------------------------------------------------------------
// Node-ID helpers — stable React keys + entries in the collapsed-set (also
// persisted to settings, so they need to round-trip across app restarts). Each
// helper composes on the one above so a section node ID embeds its
// containing change, artifact, etc.
// -------------------------------------------------------------------------

const flatWorkspaceId = (uri: string) => `flat:${uri}`
const repoId = (id: string) => `repo:${id}`
const logicalChangeId = (rid: string, name: string) =>
    `${repoId(rid)}/lc:${name}`
const instanceId = (rid: string, name: string, wt: string) =>
    `${logicalChangeId(rid, name)}/inst:${wt}`
/// `containerId` is either a flat-workspace id, a logical-change id (when
/// singleton-flattened), or an instance id. It scopes the artifact/section/
/// task subtree to its host.
const changeRowId = (containerId: string, changeId: string) =>
    `${containerId}/change:${changeId}`
const artifactNodeId = (
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
const specNodeId = (
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

export function WorkspaceTree({
    views,
    selectedNodeId,
    onSelect,
}: WorkspaceTreeProps) {
    // Two override sets, one per direction of default:
    //   `collapsed` — user-closed against a default-open node.
    //   `expanded`  — user-opened against a default-closed node (today only
    //                 a completed Tasks artifact or completed Section).
    // Most nodes default to open, so most clicks land in `collapsed`. The
    // `expanded` set only fills up as users opt back into seeing done work.
    const [collapsed, setCollapsed] = useState<Set<string>>(new Set())
    const [expanded, setExpanded] = useState<Set<string>>(new Set())
    const [hydrated, setHydrated] = useState(false)

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
        (nodeId: string, selection: TreeSelection) =>
            onSelectRef.current(nodeId, selection),
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
        ])
            .then(([collapsedIds, expandedIds]) => {
                if (cancelled) return
                setCollapsed(new Set(collapsedIds))
                setExpanded(new Set(expandedIds))
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

    const toggle = useCallback((id: string, defaultOpen: boolean) => {
        const setter = defaultOpen ? setCollapsed : setExpanded
        setter((prev) => {
            const next = new Set(prev)
            if (next.has(id)) next.delete(id)
            else next.add(id)
            return next
        })
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
                            toggle={toggle}
                            onSelect={stableOnSelect}
                        />
                    ),
                )}
            </div>
        </SelectionContext.Provider>
    )
}

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
    onSelect?: () => void
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
    /// Workspace-colour rail in the inline-start border slot (only on two-line
    /// change rows). Selection overrides it to --accent via higher specificity.
    const railClass =
        detail != null && primarySwatch
            ? ` tree-row--rail-${primarySwatch}`
            : ""
    const swatchClass = swatch ? `row-swatch row-swatch--${swatch}` : ""
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
                    </span>
                    <span className="row-line row-line--detail">{detail}</span>
                </span>
            ) : (
                <>
                    <span className="row-label">{label}</span>
                    {meta != null && <span className="row-meta">{meta}</span>}
                </>
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
    /// `defaultOpen` selects which override set the click mutates:
    /// `true`  → xor into `collapsed` (the default for almost every node).
    /// `false` → xor into `expanded`  (used only by the two auto-collapse
    ///           node types — Tasks artifact and Section — when their work
    ///           is complete).
    toggle: (id: string, defaultOpen: boolean) => void
    onSelect: (nodeId: string, selection: TreeSelection) => void
}

// -------------------------------------------------------------------------
// Repo group + descendants
// -------------------------------------------------------------------------

interface RepoNodeProps extends NodeProps {
    repo: RepoView & { kind: "repo" }
}

/// Memoized: `repo` keeps its identity within a views generation, `toggle` /
/// `onSelect` are stable, so a selection change in App skips this whole
/// subtree — the affected Rows re-render through their store subscription.
const RepoNode = memo(function RepoNode({
    repo,
    collapsed,
    expanded,
    toggle,
    onSelect,
}: RepoNodeProps) {
    const nodeId = repoId(repo.repoId)
    const isEmpty = repo.active.length === 0
    const isOpen = !collapsed.has(nodeId)
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
                    {repo.active.map((lc) => (
                        <LogicalChangeRow
                            key={logicalChangeId(repo.repoId, lc.name)}
                            repoId={repo.repoId}
                            logical={lc}
                            color={repo.color}
                            collapsed={collapsed}
                            expanded={expanded}
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
}

/// Either a flattened single-instance row (no parent disclosure) or a
/// parent disclosure with one child per instance.
function LogicalChangeRow({
    repoId: rid,
    logical,
    color,
    collapsed,
    expanded,
    toggle,
    onSelect,
}: LogicalChangeRowProps) {
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
                toggle={toggle}
                onSelect={onSelect}
            />
        )
    }

    const nodeId = logicalChangeId(rid, logical.name)
    const isOpen = !collapsed.has(nodeId)

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
    isOpen: boolean
    onToggle: () => void
    onSelect: () => void
    children: ReactNode
}

function DisclosureGroup({
    id,
    depth,
    label,
    title,
    meta,
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
    toggle,
    onSelect,
}: InstanceNodeProps) {
    const nodeId = instanceId(rid, changeName, instance.worktreePath)
    const isOpen = !collapsed.has(nodeId)

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
                <Check className="icon-checked" />
            )}
            <span
                className="row-mtime"
                title={new Date(instance.modifiedAt * 1000).toISOString()}
            >
                <RelativeTime unixSeconds={instance.modifiedAt} />
            </span>
            {instance.divergence && (
                <DivergenceChip label={instance.divergence} />
            )}
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
                        className={
                            color
                                ? `row-worktree row-worktree--${color}`
                                : "row-worktree"
                        }
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
                    detail={detail}
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

const REL_THRESHOLDS: [number, string][] = [
    [60, "s"],
    [3600, "m"],
    [86400, "h"],
    [604800, "d"],
    [2592000, "w"],
]

function formatRelativeTime(unixSeconds: number): string {
    if (unixSeconds === 0) return "—"
    const nowSec = Math.floor(Date.now() / 1000)
    const delta = Math.max(0, nowSec - unixSeconds)
    for (let i = 0; i < REL_THRESHOLDS.length; i++) {
        const [threshold, unit] = REL_THRESHOLDS[i]!
        if (delta < threshold) {
            const prev = i === 0 ? 1 : REL_THRESHOLDS[i - 1]![0]
            return `${Math.max(1, Math.floor(delta / prev))}${unit} ago`
        }
    }
    const months = Math.floor(delta / 2592000)
    return `${months}mo ago`
}

/// Self-ticking relative time. The memoized node components above stop
/// re-rendering on App-level state changes (by design), which would freeze a
/// render-time `formatRelativeTime` string for the life of a quiet session —
/// so the label owns a minute tick and re-renders only itself.
function RelativeTime({ unixSeconds }: { unixSeconds: number }) {
    const [, setTick] = useState(0)
    useEffect(() => {
        const timer = window.setInterval(
            () => setTick((n) => n + 1),
            60_000,
        )
        return () => window.clearInterval(timer)
    }, [])
    return <>{formatRelativeTime(unixSeconds)}</>
}

function DivergenceChip({ label }: { label: DivergenceLabel }) {
    const text = label === "diverged" ? "diverged" : "stale"
    const tone = label === "diverged" ? "chip--warn" : "chip--muted"
    return <span className={`chip ${tone}`}>{text}</span>
}

// -------------------------------------------------------------------------
// Flat (non-git) workspace + descendants
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
    collapsed,
    expanded,
    toggle,
    onSelect,
}: FlatWorkspaceNodeProps) {
    const nodeId = flatWorkspaceId(workspace.uri)
    const isEmpty = changes.length === 0
    const isOpen = !collapsed.has(nodeId)
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
                    {changes.map((change) => (
                        <FlatChangeNode
                            key={changeRowId(nodeId, change.changeId)}
                            containerId={nodeId}
                            workspaceUri={workspace.uri}
                            change={change}
                            color={color}
                            collapsed={collapsed}
                            expanded={expanded}
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
}

function FlatChangeNode({
    containerId,
    workspaceUri,
    change,
    color,
    collapsed,
    expanded,
    toggle,
    onSelect,
}: FlatChangeNodeProps) {
    const nodeId = changeRowId(containerId, change.changeId)
    const isOpen = !collapsed.has(nodeId)
    const isCompleted = allTasksDone(change)

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
                detail={
                    <>
                        <span className="row-changeid" title={change.changeId}>
                            {change.changeId}
                        </span>
                        {isCompleted && (
                            <span className="row-meta">
                                <Check className="icon-checked" />
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
                            <Check className="icon-checked" />
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
    const isOpen = defaultOpen ? !collapsed.has(nodeId) : expanded.has(nodeId)

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
                        ? () =>
                              onSelect(nodeId, {
                                  kind: "artifact",
                                  workspaceUri,
                                  changeId: change.changeId,
                                  artifactKind: kind,
                              })
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
    onSelect: (nodeId: string, selection: TreeSelection) => void
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
            onSelect={() =>
                onSelect(nodeId, {
                    kind: "spec",
                    workspaceUri,
                    changeId: cid,
                    capability,
                })
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
    toggle,
    onSelect,
}: SectionNodeProps) {
    const nodeId = sectionNodeId(containerId, cid, sectionIndex)
    const allTasksDone =
        section.tasks.length > 0 && section.tasks.every((t) => t.completed)
    const defaultOpen = defaultIsOpenForSection(section)
    const isOpen = defaultOpen ? !collapsed.has(nodeId) : expanded.has(nodeId)
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
                        <Check className="icon-checked" />
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
    onSelect: (nodeId: string, selection: TreeSelection) => void
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
