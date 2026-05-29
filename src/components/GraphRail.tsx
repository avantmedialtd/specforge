import type { CommitGraph, CommitRef, LaidOutCommit } from "../types"
import { EmptyState } from "./EmptyState"

// Layout constants. ROW_H must match the absolute row positioning so the SVG
// node centers line up with the subject rows.
const ROW_H = 26
// Height reserved for each day-separator band. The same value feeds both the
// SVG geometry and the subject column, so nodes never drift from their rows.
const SEP_H = 22
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

// Viewer-local calendar-day key for a commit's author date — the field the
// rail already sorts by (`--date-order`), so day boundaries always coincide
// with where rows change date.
function dayKey(iso: string): string {
    const d = new Date(iso)
    if (Number.isNaN(d.getTime())) return iso
    return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`
}

// Compact day label, e.g. "Fri, May 29". Falls back to the raw string for
// unparseable dates so a separator is never blank.
function dayLabel(iso: string): string {
    const d = new Date(iso)
    if (Number.isNaN(d.getTime())) return iso
    return d.toLocaleDateString(undefined, {
        weekday: "short",
        month: "short",
        day: "numeric",
    })
}

interface DaySeparator {
    key: string
    label: string
    y: number
}

interface GraphGeometry {
    /// rowTop[i] = y of the top of commit i's row, including separators above.
    rowTop: number[]
    separators: DaySeparator[]
    totalHeight: number
}

// One pass over the commits (newest first): reserve a SEP_H band above the
// first commit of each calendar day (and above the very first row), recording
// every commit's top y. Both the SVG gutter and the subject column read this
// single map, so a separator band lengthens crossing lane edges instead of
// desynchronising nodes from their subjects.
function computeGeometry(commits: LaidOutCommit[]): GraphGeometry {
    const rowTop: number[] = []
    const separators: DaySeparator[] = []
    let y = 0
    for (let i = 0; i < commits.length; i++) {
        const key = dayKey(commits[i].date)
        if (i === 0 || key !== dayKey(commits[i - 1].date)) {
            separators.push({ key, label: dayLabel(commits[i].date), y })
            y += SEP_H
        }
        rowTop[i] = y
        y += ROW_H
    }
    return { rowTop, separators, totalHeight: y }
}

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
    const { rowTop, separators, totalHeight } = computeGeometry(commits)
    // Node/edge center for commit at `row`, sourced from the shared geometry so
    // separator bands shift the SVG and the rows identically.
    const cy = (row: number) => rowTop[row] + ROW_H / 2

    return (
        <div className="graph-rail">
            <div className="graph-rail-body" style={{ minHeight: totalHeight }}>
                <div
                    className="graph-rail-gutter"
                    style={{ width: gutterDisplayWidth }}
                >
                    <svg
                        className="graph-rail-svg"
                        width={gutterContentWidth}
                        height={totalHeight}
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
                <div className="graph-rail-rows" style={{ height: totalHeight }}>
                    {separators.map((sep) => (
                        <div
                            key={`day-${sep.key}`}
                            className="graph-day-separator"
                            style={{ top: sep.y, height: SEP_H }}
                            aria-hidden="true"
                        >
                            <span className="graph-day-separator-label">
                                {sep.label}
                            </span>
                        </div>
                    ))}
                    {commits.map((c) => (
                        <button
                            key={c.id}
                            type="button"
                            className={`graph-row${c.id === selectedSha ? " selected" : ""}`}
                            style={{ top: rowTop[c.row], height: ROW_H }}
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
