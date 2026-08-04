import { useEffect, useMemo, useRef, useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { SplitPane } from "./components/SplitPane"
import { WorkspaceTree, type WorkspaceTreeHandle } from "./components/WorkspaceTree"
import { DetailPane, type ScrollAnchor } from "./components/DetailPane"
import { GraphRail } from "./components/GraphRail"
import { CommitDetailView } from "./components/CommitDetailView"
import { DashboardView } from "./components/DashboardView"
import { SettingsView } from "./components/SettingsView"
import { ArchiveView } from "./components/ArchiveView"
import { FileBrowserView } from "./components/FileBrowserView"
import { QuotaPill } from "./components/QuotaPill"
import { ChatGptQuotaPill } from "./components/ChatGptQuotaPill"
import { EmptyState } from "./components/EmptyState"
import {
    Archive as ArchiveIcon,
    Dashboard as DashboardIcon,
    Settings as SettingsIcon,
} from "./components/icons"
import { isTauri } from "./api"
import { useWorkspaces } from "./hooks/useWorkspaces"
import { useCommitGraph } from "./hooks/useCommitGraph"
import { useAddress } from "./hooks/useAddress"
import { addressToNodeId } from "./routing/nodeId"
import {
    findViewByRoot,
    findWorkspaceMatch,
    renderTargetToAddress,
    resolveAddress,
    type ResolveResult,
} from "./routing/resolve"
import { slugFor } from "./routing/slug"
import type { Address } from "./routing/address"
import type {
    CommitRenderTarget,
    LaidOutCommit,
    RenderTarget,
    TreeSelection,
    WorkspaceView,
} from "./types"
import "./App.css"

// Commit-graph window: how many commits to load at first, and how much each
// "load more" click grows the window.
const GRAPH_PAGE = 200
const RAIL_WIDTH_KEY = "specforge.railWidth"

const HOME: Address = { kind: "home" }
const DASHBOARD_TARGET: RenderTarget = { kind: "dashboard" }

function initialRailWidth(): number {
    const stored = localStorage.getItem(RAIL_WIDTH_KEY)
    const parsed = stored ? parseInt(stored, 10) : NaN
    return Number.isFinite(parsed) ? parsed : 260
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
        case "instance":
            // Clicking an instance row opens its proposal.md by default —
            // gives the user something useful when they click the change
            // they're working on.
            return {
                kind: "artifact",
                workspace: tree.worktreePath,
                changeId: tree.changeName,
                artifactKind: "proposal",
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

    // The address to restore when the settings/archive overlay is dismissed
    // via Escape or by re-clicking its own footer button — tracks whatever
    // non-overlay address was current before one opened. A genuine Back
    // gesture doesn't need this (opening an overlay already pushes a real
    // history entry — see `go` below and *Back closes the settings pane*);
    // this is only for the two convenience "close" paths, which use it
    // rather than `history.back()` so a fresh browser tab with no in-app
    // history to return to can never navigate the tab away from the app.
    const lastNonOverlayAddressRef = useRef<Address>(HOME)
    if (address.kind !== "settings" && address.kind !== "archive" && address.kind !== "unresolvable") {
        lastNonOverlayAddressRef.current = address
    }

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
        const leavingOverlay = address.kind === "settings" || address.kind === "archive"
        navigate(next, { replace: options?.replace ?? leavingOverlay })
    }

    const closeOverlay = () => go(lastNonOverlayAddressRef.current)

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
            const mod = e.metaKey || e.ctrlKey
            if (!mod || (e.key !== "[" && e.key !== "]")) return
            e.preventDefault()
            setSelectedCommit(null)
            setScrollAnchor(null)
            if (e.key === "[") back()
            else forward()
        }
        window.addEventListener("keydown", onKeyDown)
        return () => window.removeEventListener("keydown", onKeyDown)
    }, [back, forward])

    const { graph, loading: graphLoading, error: graphError } = useCommitGraph(
        graphRepoId,
        graphLimit,
    )

    // Cold-load / navigated-to resolution (`view-routing`: *Cold-Load
    // Address Resolution*). `loading` (from `useWorkspaces`) gates whether
    // `views` is real data or just "not fetched yet" — resolving against an
    // empty `views` before the first fetch lands would falsely report every
    // files/artifact/archive address as not-found.
    const resolution: ResolveResult | { status: "pending" } = useMemo(() => {
        if (loading) return { status: "pending" }
        if (address.kind === "unresolvable") return { status: "notFound" }
        return resolveAddress(address, views)
    }, [loading, address, views])

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

    // Tree reveal + selection highlight — a pure function of the resolved
    // address, never independently tracked state (`view-routing`:
    // *Navigation Reveal Is Transient*). `null` (home/settings/archive/an
    // unresolved address) clears any prior reveal, returning ancestors to
    // their persisted collapse state.
    const selectedNodeId = useMemo(
        () => (address.kind === "unresolvable" ? null : addressToNodeId(address, views)),
        [address, views],
    )
    const treeRef = useRef<WorkspaceTreeHandle>(null)
    useEffect(() => {
        treeRef.current?.reveal(selectedNodeId)
    }, [selectedNodeId])

    // Rail re-scoping: reactive to the resolved target (covers both a click
    // and a cold-load/deep-link landing directly on a repo-hosted artifact)
    // rather than computed only at click time. Settings/archive/pending/
    // ambiguous/not-found carry no target — the rail is left exactly as it
    // was, matching the pre-routing behaviour where opening Settings never
    // touched it (the rail is an ambient element, not 1:1 with the overlay).
    useEffect(() => {
        if (!centerTarget) return
        const next = repoIdForTarget(views, centerTarget)
        if (next !== prevGraphRepoRef.current) {
            prevGraphRepoRef.current = next
            setGraphRepoId(next)
            setGraphLimit(GRAPH_PAGE)
        }
    }, [centerTarget, views])

    const handleSelect = (_nodeId: string, tree: TreeSelection) => {
        const target = renderTargetForSelection(tree, views)
        if (!target) return
        const address = renderTargetToAddress(target, views)
        if (!address) return
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
    // selection contract. Archived changes carry no per-worktree
    // distinction, so the owning workspace/repo (not necessarily the exact
    // worktree the ship was recorded against) is what gets addressed — see
    // `routing/resolve.ts`'s archive resolution.
    const handleOpenShip = (worktreePath: string, archiveDir: string) => {
        const view = findWorkspaceMatch(worktreePath, views)?.view ?? findViewByRoot(worktreePath, views)
        if (!view) return
        go({ kind: "archive", selection: { workspace: slugFor(view, views), archiveDir } })
    }

    const selectedSha = selectedCommit?.commit.id ?? null

    return (
        <div className="app-shell">
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
                        />
                    ) : archiveView ? (
                        <ArchiveView workspaces={workspaces} initialSelection={archiveView.selection} />
                    ) : resolution.status === "pending" ? (
                        <div className="detail-pane-status">Loading…</div>
                    ) : resolution.status === "notFound" ? (
                        <EmptyState
                            title="Address not found"
                            body={
                                <>
                                    <p>This link doesn&rsquo;t match anything currently registered.</p>
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
                        <DashboardView onOpenShip={handleOpenShip} />
                    ) : centerTarget?.kind === "files" ? (
                        <FileBrowserView
                            root={centerTarget.root}
                            label={labelForRoot(centerTarget.root, views)}
                        />
                    ) : (
                        <DetailPane
                            target={
                                centerTarget?.kind === "artifact" ? centerTarget : null
                            }
                            scrollAnchor={scrollAnchor}
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
