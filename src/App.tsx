import { useEffect, useMemo, useRef, useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { SplitPane } from "./components/SplitPane"
import {
    WorkspaceTree,
    type SelectOptions,
    type WorkspaceTreeHandle,
} from "./components/WorkspaceTree"
import { DetailPane, type ScrollAnchor } from "./components/DetailPane"
import { GraphRail } from "./components/GraphRail"
import { CommitDetailView } from "./components/CommitDetailView"
import { DashboardView } from "./components/DashboardView"
import { SettingsView } from "./components/SettingsView"
import { ArchiveView } from "./components/ArchiveView"
import { DisabledAddressNotice } from "./components/DisabledAddressNotice"
import { FileBrowserView } from "./components/FileBrowserView"
import { QuotaPill } from "./components/QuotaPill"
import { ChatGptQuotaPill } from "./components/ChatGptQuotaPill"
import { EmptyState } from "./components/EmptyState"
import {
    Archive as ArchiveIcon,
    Dashboard as DashboardIcon,
    Settings as SettingsIcon,
} from "./components/icons"
import { isTauri, onToggleCommitRail, onToggleSidebar, openReaderWindow } from "./api"
import { useWorkspaces } from "./hooks/useWorkspaces"
import { useCommitGraph } from "./hooks/useCommitGraph"
import { useAddress } from "./hooks/useAddress"
import { useDocumentWidth } from "./hooks/useDocumentWidth"
import { encodeAddress } from "./routing/codec"
import { addressToNodePath } from "./routing/nodeId"
import { readerTitle } from "./readerTitle"
import {
    findViewByRoot,
    findWorkspaceMatch,
    renderTargetToAddress,
    resolveAddress,
    type ResolveResult,
} from "./routing/resolve"
import { archiveSlugFor, shortHash } from "./routing/slug"
import { disabledRowCount, shipRowState } from "./workspaceRows"
import type { Address } from "./routing/address"
import type {
    ArtifactReadKind,
    ChangeData,
    CommitRenderTarget,
    LaidOutCommit,
    RenderTarget,
    ShipEntry,
    TreeSelection,
    WorkspaceView,
} from "./types"
import "./App.css"

// Commit-graph window: how many commits to load at first, and how much each
// "load more" click grows the window.
const GRAPH_PAGE = 200
const RAIL_WIDTH_KEY = "specforge.railWidth"
const SIDEBAR_HIDDEN_KEY = "specforge.sidebarHidden"
const RAIL_HIDDEN_KEY = "specforge.railHidden"

const HOME: Address = { kind: "home" }
const DASHBOARD_TARGET: RenderTarget = { kind: "dashboard" }

function initialRailWidth(): number {
    const stored = localStorage.getItem(RAIL_WIDTH_KEY)
    const parsed = stored ? parseInt(stored, 10) : NaN
    return Number.isFinite(parsed) ? parsed : 260
}

/// Whether a side pane starts hidden. Visibility persists across sessions the
/// same way the rail width does — frontend view state, never a setting and
/// never part of the Address (`spec-browser`: *Side-Pane Visibility Toggles*).
function initialHidden(key: string): boolean {
    return localStorage.getItem(key) === "true"
}

/// The RenderTarget a tree selection asks for — `null` for the disclosure-
/// only rows (change / logical-change grouping, the Specs artifact node)
/// that render nothing of their own (`view-routing`: *Addressable Viewing
/// State*'s "a non-rendering node has no address"). Building the target here
/// (rather than setting view state directly) lets `handleSelect` publish the
/// same Address `renderTargetToAddress` would hand back to it.
function renderTargetForSelection(
    tree: TreeSelection,
    views: WorkspaceView[],
): RenderTarget | null {
    switch (tree.kind) {
        // Disclosure-only / grouping nodes: no detail-pane effect.
        case "change":
        case "logicalChange":
            return null
        case "workspace": {
            const match = views.find(
                (view) => view.kind === "flat" && view.workspace.uri === tree.workspaceUri,
            )
            return match ? { kind: "files", root: tree.workspaceUri } : null
        }
        case "repo": {
            const match = views.find((view) => view.kind === "repo" && view.repoId === tree.repoId)
            return match && match.kind === "repo" ? { kind: "files", root: match.mainWorktree } : null
        }
        case "instance": {
            // Clicking an instance row opens whichever artifact actually
            // exists, preferring proposal.md — gives the user something
            // useful when they click the change they're working on. The row
            // itself proves the change is real, so this must never resolve
            // to not-found the way a hard-coded "always proposal" target
            // would for a change that happens to have no proposal.md (E1).
            const repo = views.find((v) => v.kind === "repo" && v.repoId === tree.repoId)
            const lc =
                repo && repo.kind === "repo" ? repo.active.find((l) => l.name === tree.changeName) : undefined
            const inst = lc?.instances.find((i) => i.worktreePath === tree.worktreePath)
            const artifact = inst ? firstPresentArtifact(inst.change) : null
            if (!artifact) return null
            return {
                kind: "artifact",
                workspace: tree.worktreePath,
                changeId: tree.changeName,
                ...artifact,
            }
        }
        case "artifact":
            if (tree.artifactKind === "specs") return null
            return {
                kind: "artifact",
                workspace: tree.workspaceUri,
                changeId: tree.changeId,
                artifactKind: tree.artifactKind,
            }
        case "spec":
            return {
                kind: "artifact",
                workspace: tree.workspaceUri,
                changeId: tree.changeId,
                artifactKind: "spec",
                capability: tree.capability,
            }
        case "section":
            return {
                kind: "artifact",
                workspace: tree.workspaceUri,
                changeId: tree.changeId,
                artifactKind: "tasks",
            }
        case "task":
            return {
                kind: "artifact",
                workspace: tree.workspaceUri,
                changeId: tree.changeId,
                artifactKind: "tasks",
            }
    }
}

/// The first artifact kind actually present on `change` — proposal, then
/// design, then tasks, then the first capability spec — or `null` when the
/// change has none at all (pathological, but a click must still degrade to
/// "do nothing" rather than to a guaranteed not-found address).
function firstPresentArtifact(
    change: ChangeData,
): { artifactKind: ArtifactReadKind; capability?: string } | null {
    if (change.artifacts.proposal) return { artifactKind: "proposal" }
    if (change.artifacts.design) return { artifactKind: "design" }
    if (change.artifacts.tasks) return { artifactKind: "tasks" }
    const [capability] = change.artifacts.specs
    return capability ? { artifactKind: "spec", capability } : null
}

/// Whether resolving `address` needs the registered-workspace list at all —
/// `home`/`settings` (and an archive address with no pre-selection) resolve
/// the same way regardless of what's registered, so they must never be held
/// behind `loading` (E2: a cold start must render the Dashboard immediately,
/// not flash "Loading…" for an address that never touches `views`).
function addressNeedsViews(address: Address): boolean {
    switch (address.kind) {
        case "home":
        case "settings":
            return false
        case "archive":
            return address.selection !== null
        case "files":
        case "file":
        case "artifact":
            return true
    }
}

/// The display label of the workspace a tree selection belongs to — what a
/// reader window's title ends with. Falls back to the raw path so a title is
/// never empty, which is better than a window called "SpecForge" among five
/// others called the same.
/// An artifact's `workspace` is the WORKTREE it was read from, which is not a
/// browse root, so `labelForRoot` would miss it and fall back to the raw path.
/// `findWorkspaceMatch` is the lookup that knows about worktrees, and it is
/// what the tree itself uses.
function labelForArtifactWorkspace(
    workspace: string,
    changeId: string,
    views: WorkspaceView[],
): string {
    const found = findWorkspaceMatch(workspace, views, changeId)
    if (!found) return labelForRoot(workspace, views)
    return found.view.kind === "repo"
        ? (found.view.displayName ?? found.view.name)
        : (found.view.displayName ?? found.view.workspace.name)
}

function labelForSelection(tree: TreeSelection, views: WorkspaceView[]): string {
    const target = renderTargetForSelection(tree, views)
    if (target?.kind === "artifact") {
        return labelForArtifactWorkspace(target.workspace, target.changeId, views)
    }
    if (target?.kind === "files") return labelForRoot(target.root, views)
    return ""
}

/// The scroll target a tree selection asks for, alongside its RenderTarget —
/// unaddressed (design.md: fragment/scroll anchors are out of scope), so it
/// travels as plain view state rather than through the router.
function scrollAnchorForSelection(tree: TreeSelection): ScrollAnchor {
    switch (tree.kind) {
        case "section":
            return { kind: "section", index: tree.sectionIndex }
        case "task":
            return { kind: "task", lineNumber: tree.lineNumber }
        default:
            return null
    }
}

/// Resolve the repository a tree selection belongs to, DIRECTLY from the raw
/// `TreeSelection` — independent of whether the click actually navigates
/// anywhere (D4: a "change"/"logicalChange" disclosure row carries no
/// RenderTarget at all, but the rail still needs to re-scope to whichever
/// repo's subtree the user is browsing — expanding a change in repo B while
/// an artifact from repo A is still showing must not leave the rail on A).
/// `repoIdForTarget` below covers the complementary case (a cold-load/deep-
/// link that never went through a click at all); both write through the
/// same `applyGraphRepoId` so neither can leave the other's result stale.
function repoIdForSelection(views: WorkspaceView[], sel: TreeSelection): string | null {
    switch (sel.kind) {
        case "repo":
        case "logicalChange":
        case "instance":
            return sel.repoId
        case "workspace":
            return null
        case "change":
        case "artifact":
        case "spec":
        case "section":
        case "task": {
            const found = findWorkspaceMatch(sel.workspaceUri, views, sel.changeId)
            return found && found.view.kind === "repo" ? found.view.repoId : null
        }
    }
}

/// Resolve the repository a *resolved* render target belongs to, for scoping
/// the rail. Repo-hosted targets carry only a worktree path (`workspace`/
/// `root`), so we find the repo view that owns that worktree; a commit
/// target already carries `repoId` directly. Flat (non-git) workspaces and
/// the Dashboard return null (unscoped).
function repoIdForTarget(views: WorkspaceView[], target: RenderTarget | null): string | null {
    if (!target) return null
    switch (target.kind) {
        case "commit":
            return target.repoId
        case "dashboard":
            return null
        case "files": {
            const view = findViewByRoot(target.root, views)
            return view && view.kind === "repo" ? view.repoId : null
        }
        case "artifact": {
            const found = findWorkspaceMatch(target.workspace, views)
            return found && found.view.kind === "repo" ? found.view.repoId : null
        }
    }
}

/// The file browser's display label, re-derived from the workspace views
/// rather than carried on `FilesRenderTarget` (`view-routing`: routable
/// render targets are identifier-only — see `types.ts`'s `FilesRenderTarget`).
function labelForRoot(root: string, views: WorkspaceView[]): string {
    const view = findViewByRoot(root, views)
    if (!view) return root
    return view.kind === "repo" ? (view.displayName ?? view.name) : (view.displayName ?? view.workspace.name)
}

/// Whether `target` is (or is inside) a live text-editing control — an
/// `<input>`/`<textarea>`/`contenteditable` element. Global keyboard
/// gestures (the desktop back/forward handler) must not fire while the user
/// is mid-edit there, the same allowance the Escape handler gets for free
/// via `e.defaultPrevented` on those fields' own key handlers.
function isEditableTarget(target: EventTarget | null): boolean {
    if (!(target instanceof HTMLElement)) return false
    const tag = target.tagName
    return tag === "INPUT" || tag === "TEXTAREA" || target.isContentEditable
}

/// Explicitly call startDragging() on mousedown over the titlebar strip.
/// The `data-tauri-drag-region` attribute is meant to handle this but
/// hasn't been reliable here; calling the API directly removes the
/// dependency on Tauri's runtime click delegation.
function handleTitlebarMouseDown(event: React.MouseEvent<HTMLDivElement>) {
    // Native window dragging is a Tauri-only affordance. In a browser tab there
    // is no native titlebar, and `getCurrentWindow()` would throw with no Tauri
    // runtime — so bail out before touching it.
    if (!isTauri()) return
    if (event.button !== 0) return
    if (event.detail === 2) {
        // Double-click toggles maximize on macOS native titlebars.
        void getCurrentWindow().toggleMaximize()
        return
    }
    void getCurrentWindow().startDragging()
}

function App() {
    const { workspaces, views, refresh, loading } = useWorkspaces()
    const { address, navigate, back, forward } = useAddress()

    // Reconciles the pre-mount stamp against the stored preference and adopts
    // changes made in another window. Held here rather than inside
    // SettingsView so the reconciliation happens whether or not Settings is
    // open, and so only one listener exists per window.
    const [documentWidth, chooseDocumentWidth] = useDocumentWidth()

    // Commit selection is deliberately unaddressed (design.md: commit
    // permalinks are a non-goal — `CommitRenderTarget` keeps its preloaded
    // payload and gets no Address). It overlays whatever the address
    // resolves to, mirroring how the old single `centerTarget` union let a
    // commit click override the last tree selection — "last write wins",
    // just spread across two state sources instead of one.
    const [selectedCommit, setSelectedCommit] = useState<CommitRenderTarget | null>(null)
    const [scrollAnchor, setScrollAnchor] = useState<ScrollAnchor>(null)
    const [graphLimit, setGraphLimit] = useState(GRAPH_PAGE)
    const [graphRepoId, setGraphRepoId] = useState<string | null>(null)
    const prevGraphRepoRef = useRef<string | null>(null)

    // Side-pane visibility — ambient view state, deliberately outside the
    // Address so navigation (including Back/Forward) never changes it
    // (`spec-browser`: *Side-Pane Visibility Toggles*).
    const [sidebarHidden, setSidebarHidden] = useState(() => initialHidden(SIDEBAR_HIDDEN_KEY))
    const [railHidden, setRailHidden] = useState(() => initialHidden(RAIL_HIDDEN_KEY))
    useEffect(() => {
        localStorage.setItem(SIDEBAR_HIDDEN_KEY, String(sidebarHidden))
    }, [sidebarHidden])
    useEffect(() => {
        localStorage.setItem(RAIL_HIDDEN_KEY, String(railHidden))
    }, [railHidden])

    // Pane-toggle input, exactly one source per surface (design D4): on the
    // macOS desktop the View menu's native accelerators own Cmd+B / Cmd+Alt+B
    // and arrive here as Tauri events; every other surface (web UI on any OS,
    // desktop Windows/Linux — where no menu exists) gets this keydown handler
    // for the same combos instead. Registering both on one surface would
    // double-toggle a single keypress.
    useEffect(() => {
        if (isTauri() && document.body.dataset.platform === "mac") {
            const unlistens = [
                onToggleSidebar(() => setSidebarHidden((h) => !h)),
                onToggleCommitRail(() => setRailHidden((h) => !h)),
            ]
            return () => {
                for (const p of unlistens) void p.then((unlisten) => unlisten())
            }
        }
        const onKeyDown = (e: KeyboardEvent) => {
            if (e.defaultPrevented) return
            const mod = e.metaKey || e.ctrlKey
            // `code`, not `key`: with Alt held, macOS browsers report the
            // typed character ("∫" for Alt+B), so `key` never reads "b".
            if (!mod || e.shiftKey || e.code !== "KeyB") return
            e.preventDefault()
            if (e.altKey) setRailHidden((h) => !h)
            else setSidebarHidden((h) => !h)
        }
        window.addEventListener("keydown", onKeyDown)
        return () => window.removeEventListener("keydown", onKeyDown)
    }, [])

    // A commit selection belongs to no address, so ANY address change ends
    // it — not just the ones that happen to run through `go()` or the
    // desktop keyboard handler. A served UI's native `popstate` (the
    // browser's own Back/Forward button) changes `address` through neither
    // of those, so without this a stale `CommitDetailView` keeps winning the
    // center pane after Back until the user clicks the tree again.
    useEffect(() => {
        setSelectedCommit(null)
    }, [address])

    // B1: the tree highlight needs BOTH the exact clicked node id and the
    // address-derived one, not just the latter. Several distinct tree rows
    // resolve to the SAME address (a section/task row maps to its Tasks
    // artifact; an instance row maps to whichever artifact it happens to
    // open) — deriving the highlight from the address alone lands it on the
    // wrong row (the coarser artifact/instance ancestor) instead of the row
    // the user actually clicked. `clickedNodeId` wins the highlight for
    // exactly the one address transition the click itself caused;
    // `clickPendingRef` is how the effect below tells "this address change
    // was that click" apart from any other reason the address could have
    // changed (Back/Forward, a cold load, Dashboard, a rail commit click) —
    // in every one of those OTHER cases there is no click to remember, so
    // the address-derived id (`nodePath`'s last element, computed further
    // below) is the only source, exactly as it must be on a cold load.
    const [clickedNodeId, setClickedNodeId] = useState<string | null>(null)
    const clickPendingRef = useRef(false)
    useEffect(() => {
        if (clickPendingRef.current) {
            clickPendingRef.current = false
            return
        }
        setClickedNodeId(null)
    }, [address])

    // The address to restore when the settings/archive overlay is dismissed
    // via Escape or by re-clicking its own footer button, for when there is
    // no history entry it's safe to pop back to (see `enteredOverlayViaPushRef`
    // below) — tracks whatever non-overlay address was current before an
    // overlay opened. Never used when a real push exists to pop instead,
    // specifically so a fresh browser tab with no in-app history to return
    // to can never navigate the tab away from the app.
    const lastNonOverlayAddressRef = useRef<Address>(HOME)
    if (address.kind !== "settings" && address.kind !== "archive" && address.kind !== "unresolvable") {
        lastNonOverlayAddressRef.current = address
    }

    // Whether the CURRENT settings/archive visit was reached by `go` itself
    // pushing a new entry from a non-overlay address (as opposed to a cold
    // load landing directly on one, or arriving via Back/Forward) — true
    // exactly when there is a real history entry one `back()` away that
    // reproduces `lastNonOverlayAddressRef`. Switching between settings and
    // archive (always a replace, see `go`) doesn't change this: replacing
    // doesn't add or remove a stack level, so the entry one step behind the
    // current position is unaffected either way. Reset the moment we leave
    // overlay territory entirely, so it never survives to a later,
    // unrelated overlay visit that didn't itself push.
    const enteredOverlayViaPushRef = useRef(false)

    // Navigate to `next`, clearing the two pieces of view state that are
    // deliberately NOT part of the Address (commit selection, scroll
    // anchor) so neither leaks across a real navigation. Leaving an overlay
    // pane (settings/archive) replaces its entry rather than pushing a new
    // one by default — it was a transient detour, not a place worth a
    // dedicated Back stop — unless the caller explicitly asks otherwise
    // (used when presenting a disambiguation candidate: picking one
    // canonicalises the address in place — *History Entry Discipline*).
    const go = (next: Address, options?: { replace?: boolean }) => {
        setSelectedCommit(null)
        setScrollAnchor(null)
        const wasOverlay = address.kind === "settings" || address.kind === "archive"
        const enteringOverlay = next.kind === "settings" || next.kind === "archive"
        const replace = options?.replace ?? wasOverlay
        if (!replace && !wasOverlay && enteringOverlay) {
            enteredOverlayViaPushRef.current = true
        } else if (!enteringOverlay) {
            enteredOverlayViaPushRef.current = false
        }
        navigate(next, { replace })
    }

    // Closing the overlay via Escape or its own footer button. When we know
    // opening it pushed a real entry, popping it is exact (D2: `go`'s
    // replace-on-leave default would otherwise overwrite that entry with
    // the SAME address already sitting one step behind it, leaving two
    // identical adjacent entries — a Back gesture would then move the index
    // without changing the rendered path, requiring a second press).
    // Otherwise (a cold load landing directly on the overlay) there is
    // nothing to safely pop, so fall back to replacing with the tracked
    // non-overlay address.
    const closeOverlay = () => {
        if (enteredOverlayViaPushRef.current) {
            enteredOverlayViaPushRef.current = false
            setSelectedCommit(null)
            setScrollAnchor(null)
            back()
            return
        }
        go(lastNonOverlayAddressRef.current)
    }

    // Escape dismisses the Settings / Archive pane. Outermost fallback only:
    // controls that consume Escape themselves (e.g. the settings rename
    // inputs abandoning an edit) stopPropagation, so the event never
    // reaches this window listener.
    useEffect(() => {
        if (address.kind !== "settings" && address.kind !== "archive") return
        const onKeyDown = (e: KeyboardEvent) => {
            if (e.key !== "Escape" || e.defaultPrevented) return
            closeOverlay()
        }
        window.addEventListener("keydown", onKeyDown)
        return () => window.removeEventListener("keydown", onKeyDown)
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [address])

    // Desktop-only back/forward gesture (`view-routing`: *Desktop Back and
    // Forward Gestures*). The served web UI leaves this to the browser's own
    // gesture, which the in-memory adapter this replaces on desktop has no
    // equivalent of — so a single gesture never navigates twice.
    useEffect(() => {
        if (!isTauri()) return
        const onKeyDown = (e: KeyboardEvent) => {
            if (e.defaultPrevented) return
            const mod = e.metaKey || e.ctrlKey
            if (!mod || (e.key !== "[" && e.key !== "]")) return
            // Mid-edit in a text field (e.g. a Settings rename input), the
            // gesture must not fire out from under uncommitted text — the
            // Escape handler above the same fields makes the same
            // allowance via `e.defaultPrevented`; a plain bracket key never
            // reaches `defaultPrevented` on its own, so this is checked
            // directly against the focused element instead.
            if (isEditableTarget(e.target)) return
            e.preventDefault()
            setSelectedCommit(null)
            setScrollAnchor(null)
            if (e.key === "[") back()
            else forward()
        }
        window.addEventListener("keydown", onKeyDown)
        return () => window.removeEventListener("keydown", onKeyDown)
    }, [back, forward])

    // A hidden rail does no work: `null` suppresses the fetch entirely while
    // `applyGraphRepoId` keeps tracking the selection, so restoring the rail
    // fetches the repository the user is on NOW, not the one they were on
    // when they hid it (`commit-graph`: *Commit-Graph Rail Pane*).
    const { graph, loading: graphLoading, error: graphError } = useCommitGraph(
        railHidden ? null : graphRepoId,
        graphLimit,
    )

    // Cold-load / navigated-to resolution (`view-routing`: *Cold-Load
    // Address Resolution*). `loading` (from `useWorkspaces`) gates whether
    // `views` is real data or just "not fetched yet" — resolving against an
    // empty `views` before the first fetch lands would falsely report every
    // files/artifact/archive address as not-found. `home`/`settings` (and an
    // archive address with no selection) resolve without consulting `views`
    // at all — gating them on `loading` too would flash "Loading…" before
    // the Dashboard on every cold start, for no reason tied to the address
    // actually being opened.
    const resolution: ResolveResult | { status: "pending" } = useMemo(() => {
        if (address.kind === "unresolvable") return { status: "notFound" }
        if (loading && addressNeedsViews(address)) return { status: "pending" }
        // `workspaces` (unfiltered) is what lets a miss against `views`
        // distinguish a PARKED workspace from one that is genuinely gone.
        return resolveAddress(address, views, workspaces)
    }, [loading, address, views, workspaces])

    const centerTarget: RenderTarget | null =
        resolution.status === "resolved"
            ? resolution.view.kind === "home"
                ? DASHBOARD_TARGET
                : resolution.view.kind === "target"
                  ? resolution.view.target
                  : null
            : null

    const showSettings = resolution.status === "resolved" && resolution.view.kind === "settings"
    const archiveView =
        resolution.status === "resolved" && resolution.view.kind === "archive" ? resolution.view : null
    const showArchive = archiveView !== null

    // Tree reveal — a pure function of the resolved address, never
    // independently tracked state (`view-routing`: *Navigation Reveal Is
    // Transient*). `null` (home/settings/archive/an unresolved address)
    // clears any prior reveal, returning ancestors to their persisted
    // collapse state. `nodePath` is root-to-leaf inclusive.
    const nodePath = useMemo(
        () => (address.kind === "unresolvable" ? null : addressToNodePath(address, views)),
        [address, views],
    )
    const treeRef = useRef<WorkspaceTreeHandle>(null)
    useEffect(() => {
        treeRef.current?.reveal(nodePath)
    }, [nodePath])

    // The highlight: the exact clicked node id when there is one (B1), else
    // the address-derived leaf (cold load, Back/Forward, Dashboard, …).
    const selectedNodeId = clickedNodeId ?? (nodePath ? nodePath[nodePath.length - 1]! : null)

    // Re-scope the rail, resetting its page window only when the repo
    // actually changes — shared by the reactive effect below (a cold-load/
    // deep-link landing directly on a repo-hosted artifact, with no click to
    // hook into) and `handleSelect`'s imperative call (every tree click,
    // INCLUDING a disclosure-only one — D4: the rail tracks what's being
    // BROWSED, not just what's rendered, so expanding a change row in a
    // different repo must re-scope it even though nothing else navigates).
    const applyGraphRepoId = (next: string | null) => {
        if (next === prevGraphRepoRef.current) return
        prevGraphRepoRef.current = next
        setGraphRepoId(next)
        setGraphLimit(GRAPH_PAGE)
    }

    // Covers cold-load/deep-link only — every click-driven case is handled
    // imperatively inside `handleSelect` instead, synchronously with the
    // click rather than waiting on the address round-trip. Settings/
    // archive/pending/ambiguous/not-found carry no target — the rail is
    // left exactly as it was, matching the pre-routing behaviour where
    // opening Settings never touched it (the rail is an ambient element,
    // not 1:1 with the overlay).
    useEffect(() => {
        if (!centerTarget) return
        applyGraphRepoId(repoIdForTarget(views, centerTarget))
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [centerTarget, views])

    /// Open `target` in its own reader window. Shared by the header control and
    /// the Cmd/Ctrl-click gesture so both mint the same address and the same
    /// title — two spellings of one operation would be two things to keep in
    /// step.
    const openReaderForTarget = (target: RenderTarget) => {
        const address = renderTargetToAddress(target, views)
        if (!address || (address.kind !== "artifact" && address.kind !== "file")) return
        const label =
            target.kind === "artifact"
                ? labelForArtifactWorkspace(target.workspace, target.changeId, views)
                : target.kind === "files"
                  ? labelForRoot(target.root, views)
                  : ""
        openReaderWindow(encodeAddress(address), readerTitle(address, label))
    }

    const handleSelect = (
        nodeId: string,
        tree: TreeSelection,
        options?: SelectOptions,
    ) => {
        // A Cmd/Ctrl-click asks for the row's document in its own window. It
        // navigates nothing, so it returns BEFORE any of the state below is
        // touched: the tree's highlight, the commit rail's scope, the detail
        // pane and the history all stay exactly as they were
        // (`reader-window`: *Launching a Reader Window* — "The launching
        // surface is undisturbed"). A row with no document of its own — a
        // grouping row, a change row, the Specs node — reaches no address here
        // and so opens nothing.
        if (options?.reader) {
            const target = renderTargetForSelection(tree, views)
            if (!target) return
            const address = renderTargetToAddress(target, views)
            if (!address || (address.kind !== "artifact" && address.kind !== "file")) return
            openReaderWindow(
                encodeAddress(address),
                readerTitle(address, labelForSelection(tree, views)),
            )
            return
        }
        // Highlight the row the user actually clicked immediately — even a
        // disclosure-only row (no navigation follows) gets this, matching
        // ordinary tree UX. `clickPendingRef` is set separately, ONLY when a
        // navigation actually follows, below — a disclosure click has no
        // corresponding address change for the B1 effect to distinguish
        // from an unrelated one, so it must never set it.
        setClickedNodeId(nodeId)
        applyGraphRepoId(repoIdForSelection(views, tree))
        const target = renderTargetForSelection(tree, views)
        if (!target) return
        const address = renderTargetToAddress(target, views)
        if (!address) return
        clickPendingRef.current = true
        go(address)
        setScrollAnchor(scrollAnchorForSelection(tree))
    }

    // Rail commit click: the rail drives the center pane too. Last selection
    // wins — this overlays whatever artifact the tree last showed, and the
    // tree's own highlight is left intact so clicking an artifact returns.
    const handleSelectCommit = (commit: LaidOutCommit) => {
        if (!graphRepoId) return
        setSelectedCommit({ kind: "commit", repoId: graphRepoId, commit })
    }

    // The pinned Dashboard entry returns the center pane to the global
    // overview.
    const selectDashboard = () => go(HOME)

    // Today's-ships feed click: open the archived change in the Archive
    // browser, pre-selected. An archived change isn't in the active read
    // path, so this routes to the archive address rather than the tree-
    // selection contract. `worktreeHint` preserves the ship's EXACT worktree
    // (C2) — this repo archives changes from inside their own feature
    // worktrees, so the repo's main worktree can easily not (yet) have the
    // archival commit merged in, and resolving straight to `mainWorktree`
    // would silently show an archive listing without the very change
    // clicked. That hint is only worth minting because `resolve.ts` can invert
    // it — against the repo's active instances AND its registered folders,
    // the second of which is what reaches a worktree hosting nothing but the
    // archived change (see `worktreeForHint`). `archiveSlugFor` (not `slugFor`)
    // matches how `resolve.ts` resolves archive addresses — across both pools
    // together, not per-kind (C1).
    //
    // The Dashboard is deliberately unfiltered (design.md D7), so ships from a
    // PARKED repository render here too — and their row has no view to address.
    // Settings is where the switch that brings it back lives, so the click goes
    // there rather than nowhere: a rendered row is never an inert control.
    const handleOpenShip = (entry: ShipEntry) => {
        const state = shipRowState(entry, views, workspaces)
        if (state.kind !== "openable") {
            go({ kind: "settings" })
            return
        }
        const worktreeHint =
            state.view.kind === "repo" ? shortHash(entry.worktreePath) : undefined
        go({
            kind: "archive",
            selection: {
                workspace: archiveSlugFor(state.view, views),
                archiveDir: entry.archiveDir,
                worktreeHint,
            },
        })
    }

    const selectedSha = selectedCommit?.commit.id ?? null

    return (
        <div className="app-shell" data-sidebar-hidden={sidebarHidden || undefined}>
            {/* Drag region for macOS hidden-inset titlebar. Pointer events
                are gated by `body[data-platform="mac"]` so the strip is
                inert on Windows/Linux where a normal titlebar exists. */}
            <div
                className="titlebar-drag-region"
                data-tauri-drag-region
                onMouseDown={handleTitlebarMouseDown}
            />
            <SplitPane
                initialFarWidth={initialRailWidth()}
                onFarWidthChange={(w) =>
                    localStorage.setItem(RAIL_WIDTH_KEY, String(Math.round(w)))
                }
                leftHidden={sidebarHidden}
                farHidden={railHidden}
                onToggleLeft={() => setSidebarHidden((h) => !h)}
                onToggleFar={() => setRailHidden((h) => !h)}
                left={
                    <>
                        <button
                            className={`sidebar-header-button${address.kind === "home" ? " active" : ""}`}
                            onClick={selectDashboard}
                            aria-label="Show dashboard"
                            title="Dashboard"
                        >
                            <DashboardIcon width={18} height={18} />
                            <span>Dashboard</span>
                        </button>
                        <div className="sidebar-tree">
                            <WorkspaceTree
                                ref={treeRef}
                                views={views}
                                selectedNodeId={selectedNodeId}
                                onSelect={handleSelect}
                            />
                        </div>
                        <button
                            className={`sidebar-footer-button${showArchive ? " active" : ""}`}
                            onClick={() =>
                                showArchive ? closeOverlay() : go({ kind: "archive", selection: null })
                            }
                            aria-label="Toggle archive"
                            title="Archive"
                        >
                            <ArchiveIcon width={18} height={18} />
                            <span>Archive</span>
                        </button>
                        <button
                            className={`sidebar-footer-button${showSettings ? " active" : ""}`}
                            onClick={() => (showSettings ? closeOverlay() : go({ kind: "settings" }))}
                            aria-label="Toggle settings"
                            title="Settings"
                        >
                            <SettingsIcon width={18} height={18} />
                            <span>Settings</span>
                        </button>
                        <QuotaPill />
                        <ChatGptQuotaPill />
                    </>
                }
                right={
                    selectedCommit ? (
                        <CommitDetailView target={selectedCommit} />
                    ) : showSettings ? (
                        <SettingsView
                            workspaces={workspaces}
                            onWorkspacesChanged={refresh}
                            onClose={closeOverlay}
                            documentWidth={documentWidth}
                            onDocumentWidthChange={chooseDocumentWidth}
                        />
                    ) : archiveView ? (
                        <ArchiveView
                            views={views}
                            workspaces={workspaces}
                            initialSelection={archiveView.selection}
                        />
                    ) : resolution.status === "pending" ? (
                        <div className="detail-pane-status">Loading…</div>
                    ) : resolution.status === "disabled" ? (
                        <DisabledAddressNotice
                            workspaces={resolution.workspaces}
                            onOpenSettings={() => go({ kind: "settings" })}
                        />
                    ) : resolution.status === "notFound" ? (
                        <EmptyState
                            title="Address not found"
                            body={
                                <>
                                    {/* Not "anything currently registered": a
                                        registered workspace can still be missing
                                        the change or artifact this link names,
                                        and a disabled one has its own notice
                                        above. */}
                                    <p>
                                        This link doesn&rsquo;t match any workspace or change
                                        currently available.
                                    </p>
                                    <button className="archive-back" onClick={() => go(HOME)}>
                                        Go to Dashboard
                                    </button>
                                </>
                            }
                        />
                    ) : resolution.status === "ambiguous" ? (
                        <EmptyState
                            title="Which one did you mean?"
                            body={
                                <ul className="archive-list">
                                    {resolution.candidates.map((candidate, index) => (
                                        <li key={index}>
                                            <button
                                                className="archive-row"
                                                onClick={() => go(candidate.address, { replace: true })}
                                            >
                                                <span className="archive-name">{candidate.label}</span>
                                            </button>
                                        </li>
                                    ))}
                                </ul>
                            }
                        />
                    ) : centerTarget?.kind === "dashboard" ? (
                        <DashboardView
                            onOpenShip={handleOpenShip}
                            shipState={(entry) => shipRowState(entry, views, workspaces)}
                            disabledCount={disabledRowCount(workspaces)}
                        />
                    ) : centerTarget?.kind === "files" ? (
                        <FileBrowserView
                            root={centerTarget.root}
                            label={labelForRoot(centerTarget.root, views)}
                            // The selection is the address, not local state, so
                            // it is linkable and restorable and the back gesture
                            // returns to the previously previewed file
                            // (`workspace-file-browser`: *The Selected File Is
                            // Addressable*).
                            selectedPath={centerTarget.selectedPath ?? null}
                            onSelectFile={(path) => {
                                const next = renderTargetToAddress(
                                    { kind: "files", root: centerTarget.root, selectedPath: path },
                                    views,
                                )
                                if (next) go(next)
                            }}
                            onOpenReader={(path) => {
                                const next = renderTargetToAddress(
                                    { kind: "files", root: centerTarget.root, selectedPath: path },
                                    views,
                                )
                                if (next)
                                    openReaderWindow(
                                        encodeAddress(next),
                                        readerTitle(next, labelForRoot(centerTarget.root, views)),
                                    )
                            }}
                        />
                    ) : (
                        <DetailPane
                            target={
                                centerTarget?.kind === "artifact" ? centerTarget : null
                            }
                            scrollAnchor={scrollAnchor}
                            views={views}
                            // The visible twin of the Cmd/Ctrl-click gesture,
                            // acting on whatever the pane is showing. Absent
                            // when the artifact has no address to detach —
                            // which is also why the Archive reader, rendering
                            // through this same pane without this prop, offers
                            // no control.
                            onOpenReader={
                                centerTarget?.kind === "artifact"
                                    ? () => openReaderForTarget(centerTarget)
                                    : undefined
                            }
                        />
                    )
                }
                far={
                    <GraphRail
                        repoId={graphRepoId}
                        graph={graph}
                        loading={graphLoading}
                        error={graphError}
                        selectedSha={selectedSha}
                        onSelectCommit={handleSelectCommit}
                        onLoadMore={() =>
                            setGraphLimit((l) => l + GRAPH_PAGE)
                        }
                    />
                }
            />
        </div>
    )
}

export default App
