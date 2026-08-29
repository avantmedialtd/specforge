import type { CommitGraph, CommitRef, LaidOutCommit } from "../types"
import { computeGeometry, ROW_H, SEP_H } from "./graphGeometry"
import { EmptyState } from "./EmptyState"

// ROW_H / SEP_H live in `graphGeometry.ts` beside the layout pass that uses
// them — they must match the absolute row positioning so the SVG node centres
// line up with the subject rows.
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

// One shared, locale-aware relative-day formatter — with `numeric: "auto"` it
// yields "today"/"yesterday" (localized) for day offsets 0 and -1.
const relativeDay = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" })

// Local-midnight anchor for DST-safe whole-day arithmetic: the difference
// between two of these, divided by a day and rounded, is an exact calendar-day
// count even across the 23h/25h DST-transition days.
function startOfDay(d: Date): Date {
    return new Date(d.getFullYear(), d.getMonth(), d.getDate())
}

// Day-separator label, relative to the viewer's current calendar day in local
// time: "Today"/"Yesterday" for the two newest days, the plain weekday name for
// 2–6 days back (unambiguous — those six days plus today span the seven weekday
// names once each, so the same weekday a week ago lands in the absolute case),
// and the compact absolute date ("Fri, May 29") for 7+ days back or any
// future-dated commit. Falls back to the raw string for unparseable dates so a
// separator is never blank. Time-dependent: a "Today" label can go stale if the
// window is left open past midnight, self-correcting on the next re-render.
function dayLabel(iso: string): string {
    const d = new Date(iso)
    if (Number.isNaN(d.getTime())) return iso
    const diff = Math.round(
        (startOfDay(d).getTime() - startOfDay(new Date()).getTime()) / 86_400_000,
    )
    if (diff === 0 || diff === -1) return relativeDay.format(diff, "day")
    if (diff <= -2 && diff >= -6) {
        return d.toLocaleDateString(undefined, { weekday: "long" })
    }
    return d.toLocaleDateString(undefined, {
        weekday: "short",
        month: "short",
        day: "numeric",
    })
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
    const { rowTop, separators, totalHeight } = computeGeometry(commits, dayLabel)
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
                            key={sep.key}
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
