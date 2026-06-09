import { useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { SplitPane } from "./components/SplitPane"
import { WorkspaceTree } from "./components/WorkspaceTree"
import { DetailPane, type ScrollAnchor } from "./components/DetailPane"
import { GraphRail } from "./components/GraphRail"
import { CommitDetailView } from "./components/CommitDetailView"
import { DashboardView } from "./components/DashboardView"
import { SettingsView } from "./components/SettingsView"
import { ArchiveView } from "./components/ArchiveView"
import {
    Archive as ArchiveIcon,
    Dashboard as DashboardIcon,
    Settings as SettingsIcon,
} from "./components/icons"
import { useWorkspaces } from "./hooks/useWorkspaces"
import { useCommitGraph } from "./hooks/useCommitGraph"
import type {
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

function initialRailWidth(): number {
    const stored = localStorage.getItem(RAIL_WIDTH_KEY)
    const parsed = stored ? parseInt(stored, 10) : NaN
    return Number.isFinite(parsed) ? parsed : 260
}

/// Resolve the repository a tree selection belongs to, for scoping the rail.
/// Repo-grouped selections carry `repoId` directly; artifact/spec/task
/// selections carry only a `workspaceUri` (a worktree path), so we find the
/// repo view that owns that worktree. Flat (non-git) workspaces return null.
function repoIdForSelection(
    views: WorkspaceView[],
    sel: TreeSelection,
): string | null {
    switch (sel.kind) {
        case "repo":
        case "logicalChange":
        case "instance":
            return sel.repoId
        case "workspace":
        case "change":
        case "artifact":
        case "spec":
        case "section":
        case "task": {
            const uri = sel.workspaceUri
            for (const view of views) {
                if (view.kind !== "repo") continue
                if (view.mainWorktree === uri) return view.repoId
                // Archived changes aren't in the tree, so a tree selection's
                // worktree always belongs to an active change.
                for (const lc of view.active) {
                    for (const inst of lc.instances) {
                        if (inst.worktreePath === uri) return view.repoId
                    }
                }
            }
            return null
        }
    }
}

/// Explicitly call startDragging() on mousedown over the titlebar strip.
/// The `data-tauri-drag-region` attribute is meant to handle this but
/// hasn't been reliable here; calling the API directly removes the
/// dependency on Tauri's runtime click delegation.
function handleTitlebarMouseDown(event: React.MouseEvent<HTMLDivElement>) {
    if (event.button !== 0) return
    if (event.detail === 2) {
        // Double-click toggles maximize on macOS native titlebars.
        void getCurrentWindow().toggleMaximize()
        return
    }
    void getCurrentWindow().startDragging()
}

function App() {
    const { workspaces, views, refresh } = useWorkspaces()
    const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null)
    // The Dashboard is the default home surface: shown at startup and whenever
    // no artifact or commit is selected.
    const [centerTarget, setCenterTarget] = useState<RenderTarget | null>({
        kind: "dashboard",
    })
    const [scrollAnchor, setScrollAnchor] = useState<ScrollAnchor>(null)
    const [showSettings, setShowSettings] = useState(false)
    // Archive is a modal pane toggled from the footer, sibling to Settings;
    // opening either closes the other.
    const [showArchive, setShowArchive] = useState(false)
    // A pending deep-link into the Archive browser from the dashboard's
    // today's-ships feed: which archived change to pre-select on open. Cleared
    // when the archive is opened manually from the footer.
    const [archiveSelection, setArchiveSelection] = useState<{
        workspaceUri: string
        archiveDir: string
    } | null>(null)
    // Repository the rail is scoped to, derived from the tree selection.
    const [graphRepoId, setGraphRepoId] = useState<string | null>(null)
    const [graphLimit, setGraphLimit] = useState(GRAPH_PAGE)

    const { graph, loading: graphLoading, error: graphError } = useCommitGraph(
        graphRepoId,
        graphLimit,
    )

    const handleSelect = (nodeId: string, tree: TreeSelection) => {
        setSelectedNodeId(nodeId)
        // Re-scope the rail to the selection's repository (null for flat
        // workspaces / non-git selections). Reset the window so a fresh repo
        // starts from the first page.
        const nextRepo = repoIdForSelection(views, tree)
        if (nextRepo !== graphRepoId) {
            setGraphRepoId(nextRepo)
            setGraphLimit(GRAPH_PAGE)
        }

        switch (tree.kind) {
            // Disclosure-only / grouping nodes: no detail-pane effect.
            case "workspace":
            case "change":
            case "repo":
            case "logicalChange":
                return
            case "instance":
                // Clicking an instance row opens its proposal.md by default —
                // gives the user something useful when they click the change
                // they're working on.
                setCenterTarget({
                    kind: "artifact",
                    workspace: tree.worktreePath,
                    changeId: tree.changeName,
                    artifactKind: "proposal",
                })
                setScrollAnchor(null)
                break
            case "artifact":
                if (tree.artifactKind === "specs") return
                setCenterTarget({
                    kind: "artifact",
                    workspace: tree.workspaceUri,
                    changeId: tree.changeId,
                    artifactKind: tree.artifactKind,
                })
                setScrollAnchor(null)
                break
            case "spec":
                setCenterTarget({
                    kind: "artifact",
                    workspace: tree.workspaceUri,
                    changeId: tree.changeId,
                    artifactKind: "spec",
                    capability: tree.capability,
                })
                setScrollAnchor(null)
                break
            case "section":
                setCenterTarget({
                    kind: "artifact",
                    workspace: tree.workspaceUri,
                    changeId: tree.changeId,
                    artifactKind: "tasks",
                })
                setScrollAnchor({ kind: "section", index: tree.sectionIndex })
                break
            case "task":
                setCenterTarget({
                    kind: "artifact",
                    workspace: tree.workspaceUri,
                    changeId: tree.changeId,
                    artifactKind: "tasks",
                })
                setScrollAnchor({ kind: "task", lineNumber: tree.lineNumber })
                break
        }

        // Clicking a renderable item takes us out of settings — the user is
        // clearly asking to look at an artifact.
        if (showSettings) setShowSettings(false)
        if (showArchive) setShowArchive(false)
    }

    // Rail commit click: the rail drives the center pane too. Last selection
    // wins — this overwrites whatever artifact the tree last showed, and the
    // tree's own highlight is left intact so clicking an artifact returns.
    const handleSelectCommit = (commit: LaidOutCommit) => {
        if (!graphRepoId) return
        setCenterTarget({ kind: "commit", repoId: graphRepoId, commit })
        if (showSettings) setShowSettings(false)
        if (showArchive) setShowArchive(false)
    }

    // The pinned Dashboard entry returns the center pane to the global
    // overview. Clear the tree selection and unscope the rail so the home
    // surface stands on its own.
    const selectDashboard = () => {
        setSelectedNodeId(null)
        setCenterTarget({ kind: "dashboard" })
        setScrollAnchor(null)
        setShowSettings(false)
        setShowArchive(false)
        if (graphRepoId !== null) {
            setGraphRepoId(null)
            setGraphLimit(GRAPH_PAGE)
        }
    }

    // Today's-ships feed click: open the archived change in the Archive
    // browser, pre-selected. An archived change isn't in the active read path,
    // so this routes to the archive pane rather than the tree-selection
    // contract.
    const handleOpenShip = (worktreePath: string, archiveDir: string) => {
        setArchiveSelection({ workspaceUri: worktreePath, archiveDir })
        setShowSettings(false)
        setShowArchive(true)
    }

    const selectedSha =
        centerTarget?.kind === "commit" ? centerTarget.commit.id : null

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
                            className={`sidebar-header-button${centerTarget?.kind === "dashboard" && !showSettings ? " active" : ""}`}
                            onClick={selectDashboard}
                            aria-label="Show dashboard"
                            title="Dashboard"
                        >
                            <DashboardIcon width={18} height={18} />
                            <span>Dashboard</span>
                        </button>
                        <div className="sidebar-tree">
                            <WorkspaceTree
                                views={views}
                                selectedNodeId={selectedNodeId}
                                onSelect={handleSelect}
                            />
                        </div>
                        <button
                            className={`sidebar-footer-button${showArchive ? " active" : ""}`}
                            onClick={() => {
                                // A manual open starts at the top of the
                                // archive — drop any pending ship deep-link.
                                setArchiveSelection(null)
                                setShowArchive((s) => !s)
                                setShowSettings(false)
                            }}
                            aria-label="Toggle archive"
                            title="Archive"
                        >
                            <ArchiveIcon width={18} height={18} />
                            <span>Archive</span>
                        </button>
                        <button
                            className={`sidebar-footer-button${showSettings ? " active" : ""}`}
                            onClick={() => {
                                setShowSettings((s) => !s)
                                setShowArchive(false)
                            }}
                            aria-label="Toggle settings"
                            title="Settings"
                        >
                            <SettingsIcon width={18} height={18} />
                            <span>Settings</span>
                        </button>
                    </>
                }
                right={
                    showSettings ? (
                        <SettingsView
                            workspaces={workspaces}
                            onWorkspacesChanged={refresh}
                            onClose={() => setShowSettings(false)}
                        />
                    ) : showArchive ? (
                        <ArchiveView
                            workspaces={workspaces}
                            initialSelection={archiveSelection}
                        />
                    ) : centerTarget?.kind === "commit" ? (
                        <CommitDetailView target={centerTarget} />
                    ) : centerTarget?.kind === "dashboard" ? (
                        <DashboardView onOpenShip={handleOpenShip} />
                    ) : (
                        <DetailPane
                            target={
                                centerTarget?.kind === "artifact"
                                    ? centerTarget
                                    : null
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
