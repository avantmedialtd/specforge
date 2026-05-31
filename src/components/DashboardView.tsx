import { useEffect, useMemo, useRef, useState } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import { onChangeArchived } from "../api"
import { useDashboard } from "../hooks/useDashboard"
import type {
    ActivityBucket,
    HeatmapCell,
    Milestone,
    TodayProgress,
} from "../types"
import { EmptyState } from "./EmptyState"

/// `YYYY-MM-DD` for a Date in the viewer's local time zone, matching the
/// commit-graph rail's local-day grouping.
function localDayKey(d: Date): string {
    const y = d.getFullYear()
    const m = String(d.getMonth() + 1).padStart(2, "0")
    const day = String(d.getDate()).padStart(2, "0")
    return `${y}-${m}-${day}`
}

/// The day axis the chart renders: `windowDays` local calendar days ending
/// today (newest last), so empty days still occupy a column.
function buildAxis(windowDays: number): Date[] {
    const today = new Date()
    today.setHours(0, 0, 0, 0)
    const days: Date[] = []
    for (let i = windowDays - 1; i >= 0; i--) {
        const d = new Date(today)
        d.setDate(today.getDate() - i)
        days.push(d)
    }
    return days
}

function formatDuration(secs: number): string {
    const days = secs / 86_400
    if (days >= 1) return days < 10 ? `${days.toFixed(1)}d` : `${Math.round(days)}d`
    const hours = secs / 3_600
    if (hours >= 1) return `${Math.round(hours)}h`
    return `${Math.max(1, Math.round(secs / 60))}m`
}

function relativeTime(unixSecs: number): string {
    if (!unixSecs) return ""
    const diff = Date.now() / 1000 - unixSecs
    if (diff < 60) return "just now"
    const mins = Math.floor(diff / 60)
    if (mins < 60) return `${mins}m ago`
    const hours = Math.floor(mins / 60)
    if (hours < 24) return `${hours}h ago`
    return `${Math.floor(hours / 24)}d ago`
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
    const [reduced, setReduced] = useState(
        () =>
            typeof window !== "undefined" &&
            window.matchMedia("(prefers-reduced-motion: reduce)").matches,
    )
    useEffect(() => {
        const mq = window.matchMedia("(prefers-reduced-motion: reduce)")
        const handler = () => setReduced(mq.matches)
        mq.addEventListener("change", handler)
        return () => mq.removeEventListener("change", handler)
    }, [])
    return reduced
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
    avgCenti: number
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
            <DeltaBadge today={value} avgCenti={avgCenti} />
        </div>
    )
}

function TodayHaul({ today, glowTasks }: { today: TodayProgress; glowTasks: boolean }) {
    const nothingYet =
        today.tasksCompleted === 0 &&
        today.changesArchived === 0 &&
        today.commitsLanded === 0 &&
        today.changesCreated === 0

    return (
        <section className="dashboard-haul-section">
            <div className="dashboard-haul">
                <HaulTile
                    glyph="✔"
                    value={today.tasksCompleted}
                    avgCenti={today.tasksAvgCenti}
                    label="tasks done"
                    glow={glowTasks}
                />
                <HaulTile
                    glyph="🏆"
                    value={today.changesArchived}
                    avgCenti={today.changesArchivedAvgCenti}
                    label="shipped"
                />
                <HaulTile
                    glyph="⎇"
                    value={today.commitsLanded}
                    avgCenti={today.commitsAvgCenti}
                    label="commits"
                />
                <HaulTile
                    glyph="✚"
                    value={today.changesCreated}
                    avgCenti={today.changesCreatedAvgCenti}
                    label="started"
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
    if (cell.tasks > 0) parts.push(`✔ ${cell.tasks} task${cell.tasks === 1 ? "" : "s"}`)
    if (cell.ships > 0) parts.push(`🏆 ${cell.ships} shipped`)
    if (cell.commits > 0) parts.push(`⎇ ${cell.commits} commit${cell.commits === 1 ? "" : "s"}`)
    if (cell.created > 0) parts.push(`✚ ${cell.created} started`)
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
            <div className="heatmap-grid" role="group" aria-label={`${totalDays} active days`}>
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
// Milestones
// ----------------------------------------------------------------------------

function milestoneGlyph(kind: string): string {
    switch (kind) {
        case "firstShip":
            return "🎉"
        case "ships":
            return "🏆"
        case "streak":
            return "🔥"
        default:
            return "🏅"
    }
}

function Milestones({ milestones }: { milestones: Milestone[] }) {
    const shown = milestones.slice(0, 6)
    return (
        <section className="dashboard-panel">
            <h2 className="dashboard-panel-title">Milestones</h2>
            {shown.length === 0 ? (
                <p className="dashboard-empty-note">
                    No milestones yet — they unlock as you ship and rack up tasks.
                </p>
            ) : (
                <ul className="dashboard-milestones">
                    {shown.map((m) => (
                        <li key={m.id} className="milestone-row">
                            <span className="milestone-glyph" aria-hidden>
                                {milestoneGlyph(m.kind)}
                            </span>
                            <span className="milestone-label">{m.label}</span>
                            {m.achievedAt != null && (
                                <span className="milestone-time">
                                    {relativeTime(m.achievedAt)}
                                </span>
                            )}
                        </li>
                    ))}
                </ul>
            )}
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
// Demoted analytics (existing snapshot, now below the progress band)
// ----------------------------------------------------------------------------

function ActivityChart({
    activity,
    windowDays,
}: {
    activity: ActivityBucket[]
    windowDays: number
}) {
    const counts = new Map(activity.map((b) => [b.day, b.commitCount]))
    const axis = buildAxis(windowDays)
    const max = Math.max(1, ...axis.map((d) => counts.get(localDayKey(d)) ?? 0))
    const total = axis.reduce((sum, d) => sum + (counts.get(localDayKey(d)) ?? 0), 0)

    if (total === 0) {
        return (
            <div className="dashboard-chart dashboard-chart--empty">
                <span>No commits in the last {windowDays} days</span>
            </div>
        )
    }

    return (
        <div
            className="dashboard-chart"
            role="img"
            aria-label={`${total} commits over ${windowDays} days`}
        >
            {axis.map((d) => {
                const key = localDayKey(d)
                const count = counts.get(key) ?? 0
                const height = Math.round((count / max) * 100)
                return (
                    <div
                        key={key}
                        className="dashboard-bar-col"
                        title={`${key}: ${count} commit${count === 1 ? "" : "s"}`}
                    >
                        <div
                            className="dashboard-bar"
                            style={{ height: `${Math.max(count > 0 ? 6 : 0, height)}%` }}
                        />
                    </div>
                )
            })}
        </div>
    )
}

interface DashboardViewProps {
    /// Navigate to a change's proposal — wired to the same selection contract
    /// the tree uses. Driven by the recent-activity feed.
    onOpenChange: (worktreePath: string, changeId: string) => void
}

export function DashboardView({ onOpenChange }: DashboardViewProps) {
    const { data, error } = useDashboard()

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

    const { summary, repos, activity, activityWindowDays, lifecycle, recent, progress } = data
    const totalArchived = repos.reduce((sum, r) => sum + r.archivedCount, 0)
    const noWorkspaces = summary.repoCount === 0 && summary.flatCount === 0
    const maxRepoActive = Math.max(1, ...repos.map((r) => r.activeCount))

    if (noWorkspaces) {
        return (
            <EmptyState
                title="No workspaces yet"
                body="Register an OpenSpec workspace from Settings to see your progress here."
            />
        )
    }

    const streak = progress.streak.current

    return (
        <div className="dashboard">
            <header className="dashboard-hero">
                <div className="dashboard-hero-greeting">
                    <span className="dashboard-hero-date">
                        {new Date().toLocaleDateString(undefined, {
                            weekday: "long",
                            month: "long",
                            day: "numeric",
                        })}
                    </span>
                    <h1>{greeting()}</h1>
                </div>
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
            </header>

            <TodayHaul today={progress.today} glowTasks={glowTasks} />

            <Heatmap cells={progress.heatmap} />

            <div className="dashboard-grid">
                <Milestones milestones={progress.milestones} />

                <section className="dashboard-panel">
                    <h2 className="dashboard-panel-title">Recent</h2>
                    {recent.length === 0 ? (
                        <p className="dashboard-empty-note">No active changes.</p>
                    ) : (
                        <ul className="dashboard-recent">
                            {recent.map((entry) => (
                                <li key={`${entry.worktreePath}:${entry.changeId}`}>
                                    <button
                                        type="button"
                                        className="dashboard-recent-row"
                                        onClick={() =>
                                            onOpenChange(entry.worktreePath, entry.changeId)
                                        }
                                    >
                                        <span className="dashboard-recent-title">
                                            {entry.title ?? entry.changeId}
                                        </span>
                                        <span className="dashboard-recent-meta">
                                            <span className="dashboard-recent-ws">
                                                {entry.workspaceLabel}
                                            </span>
                                            {entry.modifiedAt > 0 && (
                                                <span className="dashboard-recent-time">
                                                    {relativeTime(entry.modifiedAt)}
                                                </span>
                                            )}
                                        </span>
                                    </button>
                                </li>
                            ))}
                        </ul>
                    )}
                </section>
            </div>

            <div className="dashboard-analytics">
                <span className="dashboard-analytics-divider">Overview</span>

                <section className="dashboard-cards">
                    <div className="dashboard-card">
                        <span className="dashboard-card-value">{summary.activeChanges}</span>
                        <span className="dashboard-card-label">Active changes</span>
                    </div>
                    <div className="dashboard-card">
                        <span className="dashboard-card-value">
                            {summary.completedTasks}
                            <span className="dashboard-card-value-sub">
                                {" "}
                                / {summary.totalTasks}
                            </span>
                        </span>
                        <span className="dashboard-card-label">
                            Tasks · {summary.taskPercent}%
                        </span>
                        <div className="dashboard-meter">
                            <div
                                className="dashboard-meter-fill"
                                style={{ width: `${summary.taskPercent}%` }}
                            />
                        </div>
                    </div>
                    <div className="dashboard-card">
                        <span className="dashboard-card-value">{summary.specsTouching}</span>
                        <span className="dashboard-card-label">Changes touch specs</span>
                    </div>
                    <div className="dashboard-card">
                        <span className="dashboard-card-value">{summary.repoCount}</span>
                        <span className="dashboard-card-label">
                            {summary.repoCount === 1 ? "repo" : "repos"} ·{" "}
                            {summary.worktreeCount} worktrees
                            {summary.flatCount > 0 ? ` · ${summary.flatCount} flat` : ""}
                        </span>
                    </div>
                </section>

                <div className="dashboard-grid">
                    <section className="dashboard-panel">
                        <h2 className="dashboard-panel-title">
                            Commits · last {activityWindowDays} days
                        </h2>
                        <ActivityChart activity={activity} windowDays={activityWindowDays} />
                        <div className="dashboard-lifecycle">
                            <span>
                                <strong>{lifecycle.archivedInWindow}</strong> archived this window
                            </span>
                            <span>
                                avg time-to-archive{" "}
                                <strong>
                                    {lifecycle.avgTimeToArchiveSecs != null
                                        ? formatDuration(lifecycle.avgTimeToArchiveSecs)
                                        : "—"}
                                </strong>
                            </span>
                        </div>
                    </section>

                    <section className="dashboard-panel">
                        <h2 className="dashboard-panel-title">Per repository</h2>
                        <ul className="dashboard-breakdown">
                            {repos.map((repo) => (
                                <li key={repo.label} className="dashboard-breakdown-row">
                                    <span className="dashboard-breakdown-label">
                                        {repo.label}
                                    </span>
                                    <span className="dashboard-breakdown-track">
                                        <span
                                            className="dashboard-breakdown-fill"
                                            style={{
                                                width: `${Math.round((repo.activeCount / maxRepoActive) * 100)}%`,
                                            }}
                                        />
                                    </span>
                                    <span className="dashboard-breakdown-counts">
                                        {repo.activeCount} active · {repo.archivedCount} archived
                                    </span>
                                </li>
                            ))}
                        </ul>
                    </section>
                </div>
            </div>

            <span className="dashboard-subtitle dashboard-footnote">
                {summary.activeChanges} active · {totalArchived} archived
            </span>

            <Celebration />
        </div>
    )
}
