import { useState } from "react"
import { SplitPane } from "./components/SplitPane"
import { WorkspaceTree } from "./components/WorkspaceTree"
import { DetailPane, type RenderTarget, type ScrollAnchor } from "./components/DetailPane"
import { SettingsView } from "./components/SettingsView"
import { useWorkspaces } from "./hooks/useWorkspaces"
import type { TreeSelection } from "./types"
import "./App.css"

function App() {
    const { workspaces, changesByWorkspace, refresh } = useWorkspaces()
    const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null)
    const [renderTarget, setRenderTarget] = useState<RenderTarget | null>(null)
    const [scrollAnchor, setScrollAnchor] = useState<ScrollAnchor>(null)
    const [showSettings, setShowSettings] = useState(false)

    const handleSelect = (nodeId: string, tree: TreeSelection) => {
        setSelectedNodeId(nodeId)

        // No-op for workspace / change / specs-artifact: don't touch the
        // detail pane, don't close settings.
        switch (tree.kind) {
            case "workspace":
            case "change":
                return
            case "artifact":
                if (tree.artifactKind === "specs") return
                setRenderTarget({
                    workspace: tree.workspaceUri,
                    changeId: tree.changeId,
                    artifactKind: tree.artifactKind,
                })
                setScrollAnchor(null)
                break
            case "spec":
                setRenderTarget({
                    workspace: tree.workspaceUri,
                    changeId: tree.changeId,
                    artifactKind: "spec",
                    capability: tree.capability,
                })
                setScrollAnchor(null)
                break
            case "section":
                setRenderTarget({
                    workspace: tree.workspaceUri,
                    changeId: tree.changeId,
                    artifactKind: "tasks",
                })
                setScrollAnchor({ kind: "section", index: tree.sectionIndex })
                break
            case "task":
                setRenderTarget({
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
    }

    return (
        <div className="app-shell">
            <SplitPane
                left={
                    <WorkspaceTree
                        workspaces={workspaces}
                        changesByWorkspace={changesByWorkspace}
                        selectedNodeId={selectedNodeId}
                        onSelect={handleSelect}
                    />
                }
                right={
                    showSettings ? (
                        <SettingsView
                            workspaces={workspaces}
                            onWorkspacesChanged={refresh}
                            onClose={() => setShowSettings(false)}
                        />
                    ) : (
                        <DetailPane target={renderTarget} scrollAnchor={scrollAnchor} />
                    )
                }
            />
            <button
                className={`settings-toggle${showSettings ? " active" : ""}`}
                onClick={() => setShowSettings((s) => !s)}
                aria-label="Toggle settings"
                title="Settings"
            >
                ⚙
            </button>
        </div>
    )
}

export default App
