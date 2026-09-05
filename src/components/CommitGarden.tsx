import type { CommitRef, GardenCommit, WorkspaceGarden } from "../types"

// Compact rail-style geometry for the dashboard plots.
const ROW_H = 24
const LANE_W = 14
const NODE_R = 3.5
// Max on-screen gutter width; wider day-DAGs scroll inside the gutter without
// moving the subject column.
const GUTTER_MAX = 140

/// FNV-1a hash → 32-bit, mirroring the dashboard identicon's hue seed so a
/// developer's nodes and their identicon share a colour family. Local-only.
function hashKey(s: string): number {
    let h = 0x811c9dc5
    for (let i = 0; i < s.length; i++) {
        h ^= s.charCodeAt(i)
        h = Math.imul(h, 0x01000193)
    }
    return h >>> 0
}

/// A node's colour: the app accent for the developer ("me"), otherwise a stable
/// hue derived from the author's normalised attribution key.
function nodeColor(c: GardenCommit): string {
    if (c.isMe) return "var(--accent)"
    const seed = (c.authorKey || c.author || "anon").trim().toLowerCase()
    return `hsl(${hashKey(seed) % 360} 52% 58%)`
}

const cx = (column: number) => column * LANE_W + LANE_W / 2

function commitTime(iso: string): string {
    const d = new Date(iso)
    if (Number.isNaN(d.getTime())) return iso
    return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })
}

function RefChip({ commitRef }: { commitRef: CommitRef }) {
    return (
        <span className={`garden-ref garden-ref--${commitRef.kind}`}>
            {commitRef.name}
        </span>
    )
}

/// One workspace's plot: a faithful today-scoped commit graph (lanes, nodes,
/// edges, refs, subjects), with nodes coloured by committer. Read-only.
function Plot({ plant }: { plant: WorkspaceGarden }) {
    const { commits, edges, laneCount } = plant
    const gutterContent = Math.max(LANE_W, laneCount * LANE_W)
    const gutterDisplay = Math.min(gutterContent, GUTTER_MAX)
    const totalH = commits.length * ROW_H
    const cy = (row: number) => row * ROW_H + ROW_H / 2
    const n = commits.length
    // Distinct *authors*, not humans: without a roster, one teammate's two git
    // identities are two keys and count twice (`commit-garden`: *Author-Colored
    // Graph Nodes*). The caption says "authors" so the figure is honest.
    const authors = new Set(commits.map((c) => c.authorKey)).size

    return (
        <figure className="garden-plot">
            <figcaption className="garden-plot-head">
                <span className="garden-plot-label">{plant.label}</span>
                <span className="garden-plot-count">
                    {n} commit{n === 1 ? "" : "s"}
                    {authors > 1 ? ` · ${authors} authors` : ""}
                </span>
            </figcaption>
            <div className="garden-plot-body" style={{ minHeight: totalH }}>
                <div className="garden-plot-gutter" style={{ width: gutterDisplay }}>
                    <svg
                        className="garden-plot-svg"
                        width={gutterContent}
                        height={totalH}
                    >
                        {edges.map((e, i) => {
                            const x1 = cx(e.fromColumn)
                            const y1 = cy(e.band)
                            const x2 = cx(e.toColumn)
                            const y2 = cy(e.band + 1)
                            const midY = (y1 + y2) / 2
                            const d = `M ${x1} ${y1} C ${x1} ${midY}, ${x2} ${midY}, ${x2} ${y2}`
                            return (
                                <path
                                    key={i}
                                    className="garden-edge"
                                    d={d}
                                    fill="none"
                                />
                            )
                        })}
                        {commits.map((c) => (
                            <circle
                                key={c.id}
                                cx={cx(c.column)}
                                cy={cy(c.row)}
                                r={NODE_R}
                                style={{ fill: nodeColor(c) }}
                                stroke="var(--surface)"
                                strokeWidth={1}
                            >
                                <title>{`${c.author} · ${commitTime(c.date)} · ${c.subject}`}</title>
                            </circle>
                        ))}
                    </svg>
                </div>
                <div className="garden-plot-rows" style={{ height: totalH }}>
                    {commits.map((c) => (
                        <div
                            key={c.id}
                            className="garden-row"
                            style={{ top: c.row * ROW_H, height: ROW_H }}
                            title={`${c.author} · ${commitTime(c.date)} · ${c.subject}`}
                        >
                            {c.refs.map((ref, i) => (
                                <RefChip key={i} commitRef={ref} />
                            ))}
                            <span className="garden-row-subject">{c.subject}</span>
                        </div>
                    ))}
                </div>
            </div>
        </figure>
    )
}

/// The commit garden: each registered workspace's faithful today-scoped commit
/// graph, stacked at the bottom of the Dashboard. Read-only; refreshes live as
/// commits land and resets at local midnight (see `useCommitGarden`).
export function CommitGarden({ plants }: { plants: WorkspaceGarden[] }) {
    // Only workspaces that actually moved today are shown; quiet, non-git, and
    // git-unavailable entries are all dormant and omitted. With none active the
    // whole section disappears rather than leaving a lonely heading.
    const active = plants.filter((p) => !p.dormant && p.commits.length > 0)
    if (active.length === 0) return null
    return (
        <section className="dashboard-garden-section" aria-label="Today's commits">
            <h2 className="dashboard-panel-title">Today&rsquo;s commits</h2>
            <div className="garden-plots">
                {active.map((plant, i) => (
                    <Plot key={`${plant.label}:${i}`} plant={plant} />
                ))}
            </div>
        </section>
    )
}
