import { useEffect, useMemo, useRef, useState } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import { getIdentity, onChangeArchived } from "../api"
import { useCommitGarden } from "../hooks/useCommitGarden"
import { useMediaQuery } from "../hooks/useDarkScheme"
import { useDashboard } from "../hooks/useDashboard"
import { CommitGarden } from "./CommitGarden"
import type { ShipRowState } from "../workspaceRows"
import type { HeatmapCell, IdentityInfo, ShipEntry, TodayProgress } from "../types"
import { EmptyState } from "./EmptyState"
import { RelativeTime } from "./RelativeTime"
import { barPercent, capBreakdown, remainderLabel } from "./repoBreakdown"

/// `YYYY-MM-DD` for a Date in the viewer's local time zone, matching the
/// commit-graph rail's local-day grouping.
function localDayKey(d: Date): string {
    const y = d.getFullYear()
    const m = String(d.getMonth() + 1).padStart(2, "0")
    const day = String(d.getDate()).padStart(2, "0")
    return `${y}-${m}-${day}`
}

function formatDuration(secs: number): string {
    const days = secs / 86_400
    if (days >= 1) return days < 10 ? `${days.toFixed(1)}d` : `${Math.round(days)}d`
    const hours = secs / 3_600
    if (hours >= 1) return `${Math.round(hours)}h`
    return `${Math.max(1, Math.round(secs / 60))}m`
}

/// What a ships row's click will do, as its tooltip. Every row is clickable —
/// one whose repository can't be opened leads to Settings instead, which is
/// where a parked row is brought back and an unregistered one re-added.
function shipRowHint(state: ShipRowState, label: string): string | undefined {
    switch (state.kind) {
        case "openable":
            return undefined
        case "parked":
            return `${label} is disabled — hidden from the tree, so this change can't be opened. Opens Settings, where you can enable it again.`
        case "unavailable":
            return `${label} is no longer available in the tree. Opens Settings.`
    }
}

function greeting(): string {
    const h = new Date().getHours()
    if (h < 5) return "Still at it"
    if (h < 12) return "Good morning"
    if (h < 18) return "Good afternoon"
    return "Good evening"
}

/// Tracks the viewer's reduced-motion preference, reactively.
function usePrefersReducedMotion(): boolean {
    return useMediaQuery("(prefers-reduced-motion: reduce)")
}

/// Animates an integer toward `target` on each change. Honors reduced motion by
/// jumping straight to the final value.
function useCountUp(target: number, durationMs = 750): number {
    const reduced = usePrefersReducedMotion()
    const [value, setValue] = useState(target)
    const fromRef = useRef(target)

    useEffect(() => {
        const from = fromRef.current
        fromRef.current = target
        if (reduced || from === target) {
            setValue(target)
            return
        }
        let raf = 0
        const start = performance.now()
        const tick = (now: number) => {
            const t = Math.min(1, (now - start) / durationMs)
            const eased = 1 - Math.pow(1 - t, 3)
            setValue(Math.round(from + (target - from) * eased))
            if (t < 1) raf = requestAnimationFrame(tick)
        }
        raf = requestAnimationFrame(tick)
        return () => cancelAnimationFrame(raf)
    }, [target, reduced, durationMs])

    return reduced ? target : value
}

// ----------------------------------------------------------------------------
// Developer profile — identicon avatar + identity
// ----------------------------------------------------------------------------

/// FNV-1a hash of a string → 32-bit unsigned. Deterministic, no crypto needed —
/// it only seeds the identicon's pattern and hue.
function hashKey(s: string): number {
    let h = 0x811c9dc5
    for (let i = 0; i < s.length; i++) {
        h ^= s.charCodeAt(i)
        h = Math.imul(h, 0x01000193)
    }
    return h >>> 0
}

/// A deterministic identicon for an identity key: a 5×5 vertically-mirrored
/// grid (GitHub-style), hue derived from the same hash. Generated entirely
/// locally — no network, no email leaves the machine. The avatar is rendered
/// plainly: it carries no finish, overlay, or rank ornament.
function Identicon({ keyStr, size = 44 }: { keyStr: string; size?: number }) {
    const h = hashKey(keyStr.trim().toLowerCase() || "you")
    const hue = h % 360
    const color = `hsl(${hue} 52% 58%)`
    // 5 rows × 3 unique columns mirrored to 5; 15 bits from the hash.
    const grid: boolean[][] = []
    for (let r = 0; r < 5; r++) {
        grid.push([0, 1, 2, 1, 0].map((c) => ((h >>> (r * 3 + c)) & 1) === 1))
    }
    return (
        <div
            className="identicon"
            aria-hidden
            style={
                {
                    width: size,
                    height: size,
                    "--ident-color": color,
                } as React.CSSProperties
            }
        >
            {grid.flat().map((on, i) => (
                <span key={i} className={`identicon-cell${on ? " on" : ""}`} />
            ))}
        </div>
    )
}

/// The key the identicon and "me" resolution key on: primary alias email, then
/// name, then display name.
function identityKeyOf(identity: IdentityInfo | null): string {
    const primary = identity?.config.aliases[0]
    return primary?.email ?? primary?.name ?? identity?.config.displayName ?? "you"
}

function identityNameOf(identity: IdentityInfo | null): string | null {
    return (
        identity?.config.displayName ??
        identity?.config.aliases[0]?.name ??
        identity?.config.aliases[0]?.email ??
        null
    )
}

// ----------------------------------------------------------------------------
// Today's Progress hero
// ----------------------------------------------------------------------------

function DeltaBadge({ today, avgCenti }: { today: number; avgCenti: number }) {
    if (avgCenti === 0) return null
    const avg = avgCenti / 100
    const diff = today - avg
    if (Math.abs(diff) < 0.05) {
        return <span className="haul-delta haul-delta--flat">≈ your average</span>
    }
    const up = diff > 0
    const rounded = Math.round(Math.abs(diff) * 10) / 10
    return (
        <span className={`haul-delta ${up ? "haul-delta--up" : "haul-delta--down"}`}>
            {up ? "▲" : "▼"} {up ? "+" : "−"}
            {rounded} vs avg
        </span>
    )
}

function HaulTile({
    glyph,
    value,
    avgCenti,
    label,
    glow,
}: {
    glyph: string
    value: number
    /// Trailing daily average (×100) backing the comparison badge. Omit for a
    /// live state count (e.g. "in flight"), which has no daily average and so
    /// renders without a comparison badge.
    avgCenti?: number
    label: string
    glow?: boolean
}) {
    const shown = useCountUp(value)
    return (
        <div className={`haul-tile${glow ? " haul-tile--glow" : ""}`}>
            <span className="haul-glyph" aria-hidden>
                {glyph}
            </span>
            <span className="haul-value">{shown}</span>
            <span className="haul-label">{label}</span>
            {avgCenti !== undefined && <DeltaBadge today={value} avgCenti={avgCenti} />}
        </div>
    )
}

function TodayHaul({
    today,
    activeChanges,
    glowTasks,
}: {
    today: TodayProgress
    activeChanges: number
    glowTasks: boolean
}) {
    // The encouraging zero state keys on the three today-flow counts only.
    // "In flight" is a live state count, not something done today, so a backlog
    // of active changes should not suppress the fresh-day nudge.
    const nothingYet =
        today.changesArchived === 0 &&
        today.commitsLanded === 0 &&
        today.tasksCompleted === 0

    return (
        <section className="dashboard-haul-section">
            <div className="dashboard-haul">
                <HaulTile
                    glyph="🏆"
                    value={today.changesArchived}
                    avgCenti={today.changesArchivedAvgCenti}
                    label="shipped"
                />
                <HaulTile glyph="✚" value={activeChanges} label="in flight" />
                <HaulTile
                    glyph="⎇"
                    value={today.commitsLanded}
                    avgCenti={today.commitsAvgCenti}
                    label="commits"
                />
                <HaulTile
                    glyph="✔"
                    value={today.tasksCompleted}
                    avgCenti={today.tasksAvgCenti}
                    label="tasks done"
                    glow={glowTasks}
                />
            </div>
            {nothingYet && (
                <p className="dashboard-haul-nudge">
                    A fresh day — check off a task or land a commit to get the ball rolling.
                </p>
            )}
        </section>
    )
}

// ----------------------------------------------------------------------------
// Contribution heatmap
// ----------------------------------------------------------------------------

/// Long-form date for the drill-down detail strip, from a `YYYY-MM-DD` key.
function formatDayKey(key: string): string {
    const [y, m, d] = key.split("-").map(Number)
    if (!y || !m || !d) return key
    return new Date(y, m - 1, d).toLocaleDateString(undefined, {
        weekday: "short",
        month: "short",
        day: "numeric",
    })
}

function HeatmapDetail({ cell }: { cell: HeatmapCell }) {
    const parts: string[] = []
    if (cell.ships > 0) parts.push(`🏆 ${cell.ships} shipped`)
    if (cell.created > 0) parts.push(`✚ ${cell.created} started`)
    if (cell.commits > 0) parts.push(`⎇ ${cell.commits} commit${cell.commits === 1 ? "" : "s"}`)
    if (cell.tasks > 0) parts.push(`✔ ${cell.tasks} task${cell.tasks === 1 ? "" : "s"}`)
    return (
        <div className="heatmap-detail">
            <span className="heatmap-detail-date">{formatDayKey(cell.day)}</span>
            {parts.length > 0 ? (
                <span className="heatmap-detail-parts">{parts.join(" · ")}</span>
            ) : (
                <span className="heatmap-detail-empty">Nothing logged</span>
            )}
        </div>
    )
}

function Heatmap({ cells }: { cells: HeatmapCell[] }) {
    const todayKey = localDayKey(new Date())
    const [selected, setSelected] = useState<string | null>(null)
    const max = Math.max(1, ...cells.map((c) => c.count))
    const level = (count: number): number => {
        if (count <= 0) return 0
        return Math.min(4, Math.ceil((count / max) * 4))
    }
    const totalDays = cells.filter((c) => c.count > 0).length
    const selectedCell =
        cells.find((c) => c.day === selected) ??
        cells.find((c) => c.day === todayKey) ??
        cells[cells.length - 1]

    return (
        <section className="dashboard-panel dashboard-heatmap-panel">
            <h2 className="dashboard-panel-title">
                Activity · {cells.length} days · {totalDays} active
            </h2>
            <div
                className="heatmap-grid"
                role="group"
                aria-label={`${totalDays} active days`}
                style={
                    {
                        // ⌈cells / 7 rows⌉ — bounds the grid width so a sparse
                        // (few-week) heatmap stays compact instead of blowing up.
                        "--heatmap-cols": Math.max(1, Math.ceil(cells.length / 7)),
                    } as React.CSSProperties
                }
            >
                {cells.map((c) => (
                    <button
                        type="button"
                        key={c.day}
                        className={`heatmap-cell heatmap-cell--l${level(c.count)}${
                            c.day === todayKey ? " heatmap-cell--today" : ""
                        }${c.day === selectedCell?.day ? " heatmap-cell--selected" : ""}`}
                        title={`${c.day}: ${c.count} ${c.count === 1 ? "thing" : "things"} done`}
                        aria-label={`${formatDayKey(c.day)}: ${c.count} done`}
                        onClick={() => setSelected(c.day)}
                    />
                ))}
            </div>
            {selectedCell && <HeatmapDetail cell={selectedCell} />}
        </section>
    )
}

// ----------------------------------------------------------------------------
// Live celebration — confetti on a ship while the Dashboard is open.
// ----------------------------------------------------------------------------

const CONFETTI_COLORS = ["#6366f1", "#22c55e", "#f59e0b", "#ec4899", "#38bdf8"]

interface Particle {
    dx: number
    dy: number
    rot: number
    color: string
    delay: number
}

function ConfettiBurst({ onDone }: { onDone: () => void }) {
    const particles = useMemo<Particle[]>(
        () =>
            Array.from({ length: 40 }, () => {
                const angle = Math.random() * Math.PI * 2
                const dist = 120 + Math.random() * 160
                return {
                    dx: Math.cos(angle) * dist,
                    dy: Math.sin(angle) * dist - 80,
                    rot: Math.random() * 720 - 360,
                    color: CONFETTI_COLORS[
                        Math.floor(Math.random() * CONFETTI_COLORS.length)
                    ],
                    delay: Math.random() * 80,
                }
            }),
        [],
    )

    useEffect(() => {
        const t = setTimeout(onDone, 1400)
        return () => clearTimeout(t)
    }, [onDone])

    return (
        <div className="confetti-burst" aria-hidden>
            {particles.map((p, i) => (
                <span
                    key={i}
                    className="confetti-particle"
                    style={
                        {
                            background: p.color,
                            "--dx": `${p.dx}px`,
                            "--dy": `${p.dy}px`,
                            "--rot": `${p.rot}deg`,
                            animationDelay: `${p.delay}ms`,
                        } as React.CSSProperties
                    }
                />
            ))}
        </div>
    )
}

/// Fires a confetti burst whenever a change is archived while the Dashboard is
/// the active surface. Suppressed entirely under reduced motion. Mounting is
/// scoped to the Dashboard, so no celebration plays when it isn't shown.
function Celebration() {
    const reduced = usePrefersReducedMotion()
    const [bursts, setBursts] = useState<number[]>([])
    const seq = useRef(0)

    useEffect(() => {
        if (reduced) return
        let mounted = true
        let unlisten: UnlistenFn | undefined
        void onChangeArchived(() => {
            seq.current += 1
            const id = seq.current
            setBursts((b) => [...b, id])
        }).then((u) => {
            if (mounted) unlisten = u
            else u()
        })
        return () => {
            mounted = false
            unlisten?.()
        }
    }, [reduced])

    if (reduced || bursts.length === 0) return null
    return (
        <div className="confetti-layer">
            {bursts.map((id) => (
                <ConfettiBurst
                    key={id}
                    onDone={() => setBursts((b) => b.filter((x) => x !== id))}
                />
            ))}
        </div>
    )
}

// ----------------------------------------------------------------------------
// Analytics band — the per-repository breakdown, under a rule carrying the
// change-lifecycle figures.
// ----------------------------------------------------------------------------

interface DashboardViewProps {
    /// Act on a today's-ships entry: open it in the Archive browser when its
    /// repository is reachable, else take the user to where it can be brought
    /// back. Called for EVERY ship row — the caller, not this component,
    /// decides where a row that cannot be opened leads, so no row is inert.
    onOpenShip: (entry: ShipEntry) => void
    /// Whether a ship's top-level row can be opened, is parked, or is gone —
    /// what the row renders itself as.
    shipState: (entry: ShipEntry) => ShipRowState
    /// How many top-level rows (repository groups and flat workspaces) are
    /// parked — the count of rows the tree drops, not of registered folders,
    /// which over-counts a repository registered at several worktrees. The
    /// Dashboard is deliberately *unfiltered* — a disabled workspace still
    /// counts here even though it has left the tree and the tray badge — so
    /// when this is non-zero the footnote says so, and the gap between the two
    /// surfaces reads as intent rather than as a bug.
    disabledCount: number
}

export function DashboardView({ onOpenShip, shipState, disabledCount }: DashboardViewProps) {
    const { data, error } = useDashboard()

    // The garden refreshes on its own cadence (today-scoped + midnight tick), so
    // it has its own hook rather than riding the dashboard payload.
    const plants = useCommitGarden()

    // The developer's identity for the profile band (avatar + display name).
    // Fetched once; it changes only via Settings, which remounts on navigation.
    const [identity, setIdentity] = useState<IdentityInfo | null>(null)
    useEffect(() => {
        let cancelled = false
        void getIdentity()
            .then((i) => {
                if (!cancelled) setIdentity(i)
            })
            .catch(() => {})
        return () => {
            cancelled = true
        }
    }, [])

    // Glow the tasks tile when today's completed-task count ticks up while the
    // Dashboard is open. Derived from the data delta so it never fires for the
    // backfilled history present on the first load.
    const reduced = usePrefersReducedMotion()
    const prevTasks = useRef<number | null>(null)
    const [glowTasks, setGlowTasks] = useState(false)
    const todayTasks = data?.progress.today.tasksCompleted ?? null
    useEffect(() => {
        if (todayTasks == null) return
        const prev = prevTasks.current
        prevTasks.current = todayTasks
        if (prev != null && todayTasks > prev && !reduced) {
            setGlowTasks(true)
            const t = setTimeout(() => setGlowTasks(false), 1200)
            return () => clearTimeout(t)
        }
    }, [todayTasks, reduced])

    if (error && !data) {
        return (
            <EmptyState
                title="Couldn't load the dashboard"
                body={<code className="detail-pane-error">{error}</code>}
            />
        )
    }

    if (!data) {
        return <div className="detail-pane-status">Loading…</div>
    }

    const { summary, repos, lifecycleWindowDays, lifecycle, todaysShips, progress } = data
    // Summed over the WHOLE payload, not the capped slice the card renders —
    // the breakdown withholds rows for height, and this footnote is the
    // registry-wide total (`dashboard`: *Per-Repository Breakdown*).
    const totalArchived = repos.reduce((sum, r) => sum + r.archivedCount, 0)
    const noWorkspaces = summary.repoCount === 0 && summary.flatCount === 0
    const breakdown = capBreakdown(repos)
    const remainder = remainderLabel(breakdown)
    // Bars are normalised against the largest active count among the entries
    // actually presented, so the top row fills its track.
    const maxShownActive = Math.max(1, ...breakdown.shown.map((r) => r.activeCount))

    if (noWorkspaces) {
        return (
            <EmptyState
                title="No workspaces yet"
                body="Register an OpenSpec workspace from Settings to see your progress here."
            />
        )
    }

    const streak = progress.streak.current
    const displayName = identityNameOf(identity)

    return (
        <div className="dashboard">
            <header className="dashboard-hero">
                <div className="dashboard-hero-greeting">
                    <Identicon keyStr={identityKeyOf(identity)} />
                    <div className="dashboard-hero-greeting-text">
                        <span className="dashboard-hero-date">
                            {new Date().toLocaleDateString(undefined, {
                                weekday: "long",
                                month: "long",
                                day: "numeric",
                            })}
                        </span>
                        <h1>
                            {greeting()}
                            {displayName ? `, ${displayName}` : ""}
                        </h1>
                    </div>
                </div>
                <div className="dashboard-hero-right">
                    <div
                        className={`dashboard-streak${streak > 0 ? " dashboard-streak--lit" : ""}`}
                        title={`Longest streak: ${progress.streak.longest} days`}
                    >
                        <span className="dashboard-streak-flame" aria-hidden>
                            🔥
                        </span>
                        <span className="dashboard-streak-count">{streak}</span>
                        <span className="dashboard-streak-label">
                            day{streak === 1 ? "" : "s"} streak
                        </span>
                    </div>
                </div>
            </header>

            <TodayHaul
                today={progress.today}
                activeChanges={progress.inFlight}
                glowTasks={glowTasks}
            />

            {/* Ships sits directly under the haul, above the heatmap: the feed
                answering "what did I finish today" leads the slower-moving
                surfaces. Its position is fixed — the quiet-day note below keeps
                the section rendered on a day with no ships, so nothing below it
                moves up the page (`dashboard`: *Dashboard Section Order*). */}
            <section className="dashboard-panel">
                <h2 className="dashboard-panel-title">Today's ships</h2>
                {todaysShips.length === 0 ? (
                    <p className="dashboard-empty-note">
                        Nothing shipped yet today.
                    </p>
                ) : (
                    <ul className="dashboard-ships">
                        {todaysShips.map((entry) => {
                            // A ship from a parked repository still belongs
                            // here (the Dashboard is the record, not an
                            // attention surface), but it must show why it
                            // doesn't open the archive — and still lead
                            // somewhere.
                            const state = shipState(entry)
                            return (
                                <li key={`${entry.worktreePath}:${entry.archiveDir}`}>
                                    <button
                                        type="button"
                                        className={`dashboard-ships-row${
                                            state.kind === "openable"
                                                ? ""
                                                : " dashboard-ships-row--parked"
                                        }`}
                                        title={shipRowHint(state, entry.workspaceLabel)}
                                        onClick={() => onOpenShip(entry)}
                                    >
                                        <span className="dashboard-ships-title">
                                            {entry.title ?? entry.changeId}
                                        </span>
                                        <span className="dashboard-ships-meta">
                                            <span className="dashboard-ships-ws">
                                                {entry.workspaceLabel}
                                            </span>
                                            {state.kind !== "openable" && (
                                                <span className="chip chip--muted">
                                                    {state.kind === "parked"
                                                        ? "disabled"
                                                        : "unavailable"}
                                                </span>
                                            )}
                                            {entry.archivedAt ? (
                                                <span className="dashboard-ships-time">
                                                    archived{" "}
                                                    <RelativeTime
                                                        unixSeconds={
                                                            entry.archivedAt
                                                        }
                                                    />
                                                </span>
                                            ) : null}
                                        </span>
                                    </button>
                                </li>
                            )
                        })}
                    </ul>
                )}
            </section>

            <Heatmap cells={progress.heatmap} />

            <div className="dashboard-analytics">
                {/* The band's rule carries its summary: the lifecycle figures
                    have no card of their own, and they name their own window —
                    nothing else on screen defines it now that the commits chart
                    is gone (`dashboard`: *Analytics Band Composition*). */}
                <div className="dashboard-analytics-rule">
                    <span className="dashboard-analytics-divider">Overview</span>
                    <span className="dashboard-lifecycle">
                        <span>
                            <strong>{lifecycle.archivedInWindow}</strong> archived
                        </span>
                        <span>{lifecycleWindowDays} days</span>
                        <span>
                            avg time-to-archive{" "}
                            <strong>
                                {lifecycle.avgTimeToArchiveSecs != null
                                    ? formatDuration(lifecycle.avgTimeToArchiveSecs)
                                    : "—"}
                            </strong>
                        </span>
                    </span>
                </div>

                <section className="dashboard-panel">
                    <h2 className="dashboard-panel-title">Per repository</h2>
                    <ul className="dashboard-breakdown">
                        {breakdown.shown.map((repo) => {
                            // Two row shapes. A row with work in flight draws a
                            // bar; a row without draws no track at all and
                            // dims, so every bar on screen encodes a non-zero
                            // quantity and the drawing agrees with the sort key
                            // (`dashboard`: *Per-Repository Breakdown*).
                            const active = repo.activeCount > 0
                            return (
                                <li
                                    key={repo.label}
                                    className={`dashboard-breakdown-row${
                                        active ? "" : " dashboard-breakdown-row--quiet"
                                    }`}
                                >
                                    <span className="dashboard-breakdown-label">
                                        {repo.label}
                                    </span>
                                    {active ? (
                                        <span className="dashboard-breakdown-track">
                                            <span
                                                className="dashboard-breakdown-fill"
                                                style={{
                                                    width: `${barPercent(
                                                        repo.activeCount,
                                                        maxShownActive,
                                                    )}%`,
                                                }}
                                            />
                                        </span>
                                    ) : (
                                        <span className="dashboard-breakdown-track-empty" />
                                    )}
                                    <span className="dashboard-breakdown-counts">
                                        {active && `${repo.activeCount} active · `}
                                        {repo.archivedCount} archived
                                    </span>
                                </li>
                            )
                        })}
                    </ul>
                    {remainder && <p className="dashboard-breakdown-more">{remainder}</p>}
                </section>
            </div>

            <CommitGarden plants={plants} />

            <span className="dashboard-subtitle dashboard-footnote">
                {summary.activeChanges} active · {totalArchived} archived
                {disabledCount > 0 &&
                    ` · includes ${disabledCount} disabled workspace${
                        disabledCount === 1 ? "" : "s"
                    }`}
            </span>

            <Celebration />
        </div>
    )
}
