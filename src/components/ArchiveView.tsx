import { useEffect, useMemo, useState } from "react"
import {
    archivedArtifactStatus,
    listArchived,
    onChangeArchived,
    onLogicalChangeArchived,
} from "../api"
import type {
    ArchivedChangeSummary,
    ArtifactReadKind,
    ArtifactRenderTarget,
    ArtifactStatus,
    RegisteredWorkspace,
} from "../types"
import { DetailPane } from "./DetailPane"
import { EmptyState } from "./EmptyState"
import { ChevronRight } from "./icons"

interface ArchiveViewProps {
    /// The registered workspaces — the dropdown's options. The archive is
    /// browsed one workspace at a time; this mirrors the Settings list.
    workspaces: RegisteredWorkspace[]
    /// Optional deep-link from the dashboard's today's-ships feed: on mount,
    /// select this workspace and open this dated archive directory's change.
    initialSelection?: { workspaceUri: string; archiveDir: string } | null
}

/// Reconstruct the on-disk archive directory name from a summary. The listing
/// strips the `YYYY-MM-DD-` prefix into `id` / `date`, and that strip is
/// exactly reversible: `<date>-<id>` for a dated entry, or the bare `id` for a
/// legacy (un-dated) one.
function archiveDirName(s: ArchivedChangeSummary): string {
    return s.date ? `${s.date}-${s.id}` : s.id
}

/// The Archive view: a global, footer-reached surface for browsing a single
/// workspace's archived changes. Loads on mount and on workspace change, so
/// the archive never touches the tree render or the watcher hot path.
export function ArchiveView({
    workspaces,
    initialSelection,
}: ArchiveViewProps) {
    const [selectedUri, setSelectedUri] = useState<string | null>(
        initialSelection?.workspaceUri ?? workspaces[0]?.uri ?? null,
    )
    const [summaries, setSummaries] = useState<ArchivedChangeSummary[]>([])
    const [loading, setLoading] = useState(false)
    const [error, setError] = useState<string | null>(null)
    const [filter, setFilter] = useState("")
    const [openChange, setOpenChange] = useState<ArchivedChangeSummary | null>(
        null,
    )
    // Bumped by archive events to force a re-fetch of the open listing.
    const [reload, setReload] = useState(0)
    // Which artifacts the open change has on disk, and which one is shown.
    const [artifactStatus, setArtifactStatus] = useState<ArtifactStatus | null>(
        null,
    )
    const [activeArtifact, setActiveArtifact] = useState<{
        kind: ArtifactReadKind
        capability?: string
    }>({ kind: "proposal" })
    // A deep-linked archive directory (from the dashboard) to open once its
    // workspace listing has loaded. Consumed and cleared on first match.
    const [pendingOpenDir, setPendingOpenDir] = useState<string | null>(
        initialSelection?.archiveDir ?? null,
    )

    // Keep the selection valid if the workspace set changes underneath us.
    useEffect(() => {
        if (selectedUri && workspaces.some((w) => w.uri === selectedUri)) return
        setSelectedUri(workspaces[0]?.uri ?? null)
    }, [workspaces, selectedUri])

    // Auto-open a deep-linked change once its workspace's listing has loaded.
    useEffect(() => {
        if (!pendingOpenDir) return
        const match = summaries.find(
            (s) => archiveDirName(s) === pendingOpenDir,
        )
        if (match) {
            setOpenChange(match)
            setPendingOpenDir(null)
        }
    }, [summaries, pendingOpenDir])

    // Load the selected workspace's archive listing on demand.
    useEffect(() => {
        if (!selectedUri) {
            setSummaries([])
            return
        }
        let cancelled = false
        setLoading(true)
        setError(null)
        listArchived(selectedUri)
            .then((rows) => {
                if (cancelled) return
                setSummaries(rows)
                setLoading(false)
            })
            .catch((e) => {
                if (cancelled) return
                setError(String(e))
                setLoading(false)
            })
        return () => {
            cancelled = true
        }
    }, [selectedUri, reload])

    // Live refresh while open: re-fetch when a change is archived. Closing the
    // view unmounts this effect, so no archive work happens while it's closed.
    useEffect(() => {
        const bump = () => setReload((n) => n + 1)
        const unsubs = [onChangeArchived(bump), onLogicalChangeArchived(bump)]
        return () => {
            for (const u of unsubs) void u.then((f) => f())
        }
    }, [])

    // When a change is opened, reset to its proposal and fetch which artifacts
    // it has on disk so the reader can offer per-artifact tabs. On-demand and
    // per-change — never on the aggregation path.
    useEffect(() => {
        if (!openChange || !selectedUri) {
            setArtifactStatus(null)
            return
        }
        setActiveArtifact({ kind: "proposal" })
        setArtifactStatus(null)
        let cancelled = false
        archivedArtifactStatus(selectedUri, archiveDirName(openChange))
            .then((s) => {
                if (!cancelled) setArtifactStatus(s)
            })
            .catch(() => {
                if (!cancelled) setArtifactStatus(null)
            })
        return () => {
            cancelled = true
        }
    }, [openChange, selectedUri])

    const filtered = useMemo(() => {
        const q = filter.trim().toLowerCase()
        if (!q) return summaries
        return summaries.filter(
            (s) =>
                s.id.toLowerCase().includes(q) ||
                (s.title?.toLowerCase().includes(q) ?? false),
        )
    }, [summaries, filter])

    if (workspaces.length === 0) {
        return (
            <EmptyState
                title="No workspaces registered"
                body="Add a workspace from Settings to browse its archive."
            />
        )
    }

    // Reading one archived change: reuse the artifact renderer with a change_id
    // that points into the archive subtree (read_artifact permits it).
    if (openChange && selectedUri) {
        const target: ArtifactRenderTarget = {
            kind: "artifact",
            workspace: selectedUri,
            changeId: `archive/${archiveDirName(openChange)}`,
            artifactKind: activeArtifact.kind,
            capability: activeArtifact.capability,
        }
        const isActive = (kind: ArtifactReadKind, capability?: string) =>
            activeArtifact.kind === kind &&
            activeArtifact.capability === capability
        return (
            <div className="archive-view archive-view--reading">
                <div className="archive-header">
                    <button
                        className="archive-back"
                        onClick={() => setOpenChange(null)}
                    >
                        ← Archive
                    </button>
                    <span className="archive-reading-title">
                        {openChange.date ? `${openChange.date} · ` : ""}
                        {openChange.title ?? openChange.id}
                    </span>
                </div>
                <div className="archive-artifact-tabs">
                    {/* Proposal is the default; show it unless we know it's absent. */}
                    {artifactStatus?.proposal !== false && (
                        <button
                            className={`archive-tab${isActive("proposal") ? " archive-tab--active" : ""}`}
                            onClick={() =>
                                setActiveArtifact({ kind: "proposal" })
                            }
                        >
                            Proposal
                        </button>
                    )}
                    {artifactStatus?.design && (
                        <button
                            className={`archive-tab${isActive("design") ? " archive-tab--active" : ""}`}
                            onClick={() => setActiveArtifact({ kind: "design" })}
                        >
                            Design
                        </button>
                    )}
                    {artifactStatus?.tasks && (
                        <button
                            className={`archive-tab${isActive("tasks") ? " archive-tab--active" : ""}`}
                            onClick={() => setActiveArtifact({ kind: "tasks" })}
                        >
                            Tasks
                        </button>
                    )}
                    {artifactStatus?.specs.map((cap) => (
                        <button
                            key={cap}
                            className={`archive-tab${isActive("spec", cap) ? " archive-tab--active" : ""}`}
                            onClick={() =>
                                setActiveArtifact({
                                    kind: "spec",
                                    capability: cap,
                                })
                            }
                        >
                            {cap}
                        </button>
                    ))}
                </div>
                <DetailPane target={target} scrollAnchor={null} />
            </div>
        )
    }

    return (
        <div className="archive-view">
            <div className="archive-header">
                <h2 className="archive-title">Archive</h2>
                {workspaces.length > 1 ? (
                    <select
                        className="archive-workspace-select"
                        value={selectedUri ?? ""}
                        onChange={(e) => {
                            setSelectedUri(e.target.value)
                            setOpenChange(null)
                            setFilter("")
                        }}
                        aria-label="Workspace"
                    >
                        {workspaces.map((w) => (
                            <option key={w.uri} value={w.uri}>
                                {w.displayName ?? w.name}
                            </option>
                        ))}
                    </select>
                ) : (
                    <span className="archive-workspace-single">
                        {workspaces[0]?.displayName ?? workspaces[0]?.name}
                    </span>
                )}
            </div>

            <input
                className="archive-search"
                type="text"
                placeholder="Search the archive…"
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
            />

            {loading ? (
                <div className="archive-status">Loading…</div>
            ) : error ? (
                <div className="archive-status archive-status--error">
                    {error}
                </div>
            ) : summaries.length === 0 ? (
                <EmptyState
                    title="No archived changes"
                    body="This workspace has nothing in openspec/changes/archive/."
                />
            ) : filtered.length === 0 ? (
                <div className="archive-status">
                    No changes match “{filter}”.
                </div>
            ) : (
                <ul className="archive-list">
                    {filtered.map((s) => (
                        <li key={archiveDirName(s)}>
                            <button
                                className="archive-row"
                                onClick={() => setOpenChange(s)}
                            >
                                <span className="archive-date">
                                    {s.date ?? "—"}
                                </span>
                                <span className="archive-name">
                                    {s.title ?? s.id}
                                </span>
                                <ChevronRight
                                    className="archive-row-chevron"
                                    width={14}
                                    height={14}
                                />
                            </button>
                        </li>
                    ))}
                </ul>
            )}
        </div>
    )
}
