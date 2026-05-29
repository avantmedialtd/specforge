import type { CommitGraph, CommitRef, LaidOutCommit } from "../types"
import { EmptyState } from "./EmptyState"

// Layout constants. ROW_H must match the absolute row positioning so the SVG
// node centers line up with the subject rows.
const ROW_H = 26
const LANE_W = 14
const NODE_R = 3.5
// The graph gutter's max on-screen width; wider DAGs scroll horizontally
// inside the gutter without moving the subject column.
const GUTTER_MAX = 180

// Distinct lane colors, cycled by column. Tuned for the dark theme.
const LANE_COLORS = [
    "#7c9cff",
    "#5cc6b0",
    "#69c267",
    "#d9a441",
    "#e0795b",
    "#c678b4",
    "#56b6e0",
    "#b58df0",
]

function laneColor(column: number): string {
    return LANE_COLORS[column % LANE_COLORS.length]
}

const cx = (column: number) => column * LANE_W + LANE_W / 2
const cy = (row: number) => row * ROW_H + ROW_H / 2

interface GraphRailProps {
    repoId: string | null
    graph: CommitGraph | null
    loading: boolean
    error: string | null
    selectedSha: string | null
    onSelectCommit: (commit: LaidOutCommit) => void
    onLoadMore: () => void
}

export function GraphRail({
    repoId,
    graph,
    loading,
    error,
    selectedSha,
    onSelectCommit,
    onLoadMore,
}: GraphRailProps) {
    if (!repoId) {
        return (
            <EmptyState
                title="No repository"
                body="Select a change in a git repository to see its commit history."
            />
        )
    }

    if (error) {
        return (
            <EmptyState
                title="Couldn't load history"
                body={<code className="detail-pane-error">{error}</code>}
            />
        )
    }

    if (loading && !graph) {
        return <div className="detail-pane-status">Loading history…</div>
    }

    if (!graph || graph.commits.length === 0) {
        return (
            <EmptyState
                title="No commits"
                body="This repository has no commits yet, or git is unavailable."
            />
        )
    }

    const { commits, edges, laneCount } = graph
    const gutterContentWidth = Math.max(LANE_W, laneCount * LANE_W)
    const gutterDisplayWidth = Math.min(gutterContentWidth, GUTTER_MAX)
    const height = commits.length * ROW_H

    return (
        <div className="graph-rail">
            <div className="graph-rail-body" style={{ minHeight: height }}>
                <div
                    className="graph-rail-gutter"
                    style={{ width: gutterDisplayWidth }}
                >
                    <svg
                        className="graph-rail-svg"
                        width={gutterContentWidth}
                        height={height}
                    >
                        {edges.map((e, i) => {
                            const x1 = cx(e.fromColumn)
                            const y1 = cy(e.band)
                            const x2 = cx(e.toColumn)
                            const y2 = cy(e.band + 1)
                            const midY = (y1 + y2) / 2
                            // Smooth S-curve for bends; degenerates to a
                            // straight vertical when the columns match.
                            const d = `M ${x1} ${y1} C ${x1} ${midY}, ${x2} ${midY}, ${x2} ${y2}`
                            return (
                                <path
                                    key={i}
                                    d={d}
                                    fill="none"
                                    stroke={laneColor(e.toColumn)}
                                    strokeWidth={1.5}
                                />
                            )
                        })}
                        {commits.map((c) => (
                            <circle
                                key={c.id}
                                cx={cx(c.column)}
                                cy={cy(c.row)}
                                r={NODE_R}
                                fill={laneColor(c.column)}
                                stroke="var(--surface)"
                                strokeWidth={1}
                            />
                        ))}
                    </svg>
                </div>
                <div className="graph-rail-rows" style={{ height }}>
                    {commits.map((c) => (
                        <button
                            key={c.id}
                            type="button"
                            className={`graph-row${c.id === selectedSha ? " selected" : ""}`}
                            style={{ top: c.row * ROW_H, height: ROW_H }}
                            title={`${c.author} · ${formatTimestamp(c.date)} · ${shortSha(c.id)}`}
                            onClick={() => onSelectCommit(c)}
                        >
                            {c.refs.map((ref, i) => (
                                <RefChip key={i} commitRef={ref} />
                            ))}
                            <span className="graph-row-subject">{c.subject}</span>
                        </button>
                    ))}
                </div>
            </div>
            {graph.truncated && (
                <button
                    type="button"
                    className="graph-rail-more"
                    onClick={onLoadMore}
                >
                    Load more history
                </button>
            )}
        </div>
    )
}

function RefChip({ commitRef }: { commitRef: CommitRef }) {
    return (
        <span className={`graph-ref graph-ref--${commitRef.kind}`}>
            {commitRef.name}
        </span>
    )
}

function shortSha(id: string): string {
    return id.slice(0, 7)
}

function formatTimestamp(iso: string): string {
    const date = new Date(iso)
    if (Number.isNaN(date.getTime())) return iso
    return date.toLocaleString()
}
