import { useEffect, useMemo, useRef, useState } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import { getIdentity, onChangeArchived } from "../api"
import { useDashboard } from "../hooks/useDashboard"
import type {
    ActivityBucket,
    HeatmapCell,
    IdentityInfo,
    LeaderboardEntry,
    Milestone,
    SeasonObjective,
    SeasonRecap,
    SeasonStanding,
    TodayProgress,
    TreatmentDescriptor,
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
/// locally — no network, no email leaves the machine.
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
                { width: size, height: size, "--ident-color": color } as React.CSSProperties
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
// Per-author leaderboard (shared repositories)
// ----------------------------------------------------------------------------

/// Ranks authors by ships/tasks/commits. Rendered only for a genuine contest —
/// history with more than one distinct author; a solo repo shows nothing.
function Leaderboard({
    entries,
    title = "Leaderboard · last year",
}: {
    entries: LeaderboardEntry[]
    title?: string
}) {
    if (entries.length <= 1) return null
    return (
        <section className="dashboard-panel">
            <h2 className="dashboard-panel-title">{title}</h2>
            <ol className="leaderboard">
                {entries.map((e, i) => (
                    <li
                        key={e.authorKey}
                        className={`leaderboard-row${e.isMe ? " leaderboard-row--me" : ""}`}
                    >
                        <span className="leaderboard-rank">{i + 1}</span>
                        <Identicon keyStr={e.authorKey} size={26} />
                        <span className="leaderboard-name">
                            {e.display}
                            {e.isMe && <span className="leaderboard-you"> you</span>}
                        </span>
                        <span className="leaderboard-stats">
                            <span title="changes shipped">🏆 {e.ships}</span>
                            <span title="tasks completed">✔ {e.tasks}</span>
                            <span title="commits">⎇ {e.commits}</span>
                        </span>
                    </li>
                ))}
            </ol>
        </section>
    )
}

// ----------------------------------------------------------------------------
// Seasons — battle pass home, treatments, recap
// ----------------------------------------------------------------------------

/// Human countdown to a season's end from its end timestamp (unix seconds).
function formatCountdown(endTs: number): string {
    const secs = endTs - Date.now() / 1000
    if (secs <= 0) return "ending"
    const days = Math.floor(secs / 86_400)
    if (days >= 1) return `${days}d left`
    const hours = Math.floor(secs / 3_600)
    return `${Math.max(1, hours)}h left`
}

/// A badge treatment rendered as a small swatch — the finish the locker shows
/// and (via a shared class) the finish applied over earned milestone badges.
/// Purely local: hues come from the descriptor's palette indices, no network.
function TreatmentSwatch({
    treatment,
    size = 26,
    title,
}: {
    treatment: TreatmentDescriptor
    size?: number
    title?: string
}) {
    const hue = (treatment.palette[0] ?? 0) * 30
    const hue2 = (treatment.palette[1] ?? 6) * 30
    return (
        <span
            className={`treatment treatment--${treatment.effect} treatment--${treatment.rarity}`}
            title={title ?? `${treatment.rarity} · ${treatment.effect}`}
            aria-hidden
            style={
                {
                    width: size,
                    height: size,
                    "--treat-hue": `${hue}`,
                    "--treat-hue2": `${hue2}`,
                } as React.CSSProperties
            }
        />
    )
}

function ObjectiveRow({ objective }: { objective: SeasonObjective }) {
    const pct = Math.min(
        100,
        Math.round((objective.progress / Math.max(1, objective.target)) * 100),
    )
    return (
        <li
            className={`season-objective${
                objective.complete ? " season-objective--done" : ""
            }`}
        >
            <span className="season-objective-check" aria-hidden>
                {objective.complete ? "✓" : "○"}
            </span>
            <span className="season-objective-body">
                <span className="season-objective-title">{objective.title}</span>
                <span className="season-objective-track">
                    <span
                        className="season-objective-fill"
                        style={{ width: `${pct}%` }}
                    />
                </span>
            </span>
            <span className="season-objective-count">
                {objective.progress}/{objective.target}
            </span>
        </li>
    )
}

/// The season home: name + countdown, band/tier + career tier, the battle-pass
/// track with the next unlock previewed, and objectives. The treatment wardrobe
/// (browse + equip) lives in Settings → Badge finishes; badges still wear the
/// equipped finish here.
function SeasonPanel({ season }: { season: SeasonStanding }) {
    const { ladder, objectives, nextTreatment } = season
    const trackPct = ladder.overflow
        ? 100
        : Math.min(
              100,
              Math.round(
                  ((ladder.perTier - ladder.gapToNext) / Math.max(1, ladder.perTier)) * 100,
              ),
          )
    return (
        <section className="dashboard-panel season-panel">
            <div className="season-head">
                <div className="season-title-wrap">
                    <span className="season-eyebrow">Season {season.season.index}</span>
                    <h2 className="season-name">{season.season.name}</h2>
                </div>
                <span className="season-countdown">
                    {formatCountdown(season.season.endTs)}
                </span>
            </div>

            <div className="season-band-row">
                <span className={`season-band season-band--${ladder.band.toLowerCase()}`}>
                    {ladder.label}
                </span>
            </div>

            <div className="season-track">
                <div className="season-track-bar">
                    <div className="season-track-fill" style={{ width: `${trackPct}%` }} />
                </div>
                <div className="season-track-meta">
                    <span>
                        tier {ladder.tier}
                        {ladder.overflow ? "+" : ""}
                    </span>
                    {!ladder.overflow && (
                        <span className="season-next">
                            {nextTreatment && (
                                <TreatmentSwatch
                                    treatment={nextTreatment}
                                    size={18}
                                    title="next unlock"
                                />
                            )}
                            {ladder.gapToNext} pts → tier {ladder.tier + 1}
                        </span>
                    )}
                </div>
            </div>

            <ul className="season-objectives">
                {objectives.map((o, i) => (
                    <ObjectiveRow key={`${o.archetype}-${i}`} objective={o} />
                ))}
            </ul>
        </section>
    )
}

/// The auto-minted "wrapped" card, surfaced once when a season rolls over.
function SeasonRecapCard({
    recap,
    onDismiss,
}: {
    recap: SeasonRecap
    onDismiss: () => void
}) {
    return (
        <section className="season-recap" role="status">
            <button
                type="button"
                className="season-recap-close"
                onClick={onDismiss}
                aria-label="Dismiss recap"
            >
                ×
            </button>
            <span className="season-recap-eyebrow">{recap.season.name} · wrapped</span>
            <div className="season-recap-stats">
                <span>
                    <strong>{recap.shipped}</strong> shipped
                </span>
                <span>
                    <strong>{recap.tasksCompleted}</strong> tasks
                </span>
                <span>
                    <strong>{recap.bestStreak}</strong> best streak
                </span>
                <span>
                    reached <strong>{recap.band}</strong>
                </span>
                <span>
                    <strong>{recap.objectivesCompleted}</strong> objectives
                </span>
                <span>
                    <strong>{recap.treatmentsUnlocked}</strong> treatments
                </span>
            </div>
        </section>
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

function Milestones({
    milestones,
    equipped,
}: {
    milestones: Milestone[]
    /// The equipped treatment finish, applied as a CSS class over each earned
    /// badge glyph. Null leaves the badges in their plain rendering.
    equipped: TreatmentDescriptor | null
}) {
    const shown = milestones.slice(0, 6)
    const finishClass = equipped
        ? ` treatment-finish treatment--${equipped.effect} treatment--${equipped.rarity}`
        : ""
    const finishStyle = equipped
        ? ({
              "--treat-hue": `${(equipped.palette[0] ?? 0) * 30}`,
              "--treat-hue2": `${(equipped.palette[1] ?? 6) * 30}`,
          } as React.CSSProperties)
        : undefined
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
                            <span
                                className={`milestone-glyph${finishClass}`}
                                style={finishStyle}
                                aria-hidden
                            >
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

    const [recapDismissed, setRecapDismissed] = useState(false)

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

    // Live tier-up acknowledgement: when the season tier ticks up while the
    // Dashboard is open (never on the first load, never under reduced motion),
    // flash a brief banner. Backfilled crossings present as already-unlocked, so
    // they never trip this.
    const prevTier = useRef<number | null>(null)
    const [tierUp, setTierUp] = useState<string | null>(null)
    const curTier = data?.season?.ladder.tier ?? null
    const curBandLabel = data?.season?.ladder.label ?? ""
    useEffect(() => {
        if (curTier == null) return
        const prev = prevTier.current
        prevTier.current = curTier
        if (prev != null && curTier > prev && !reduced) {
            setTierUp(curBandLabel)
            const t = setTimeout(() => setTierUp(null), 2400)
            return () => clearTimeout(t)
        }
    }, [curTier, curBandLabel, reduced])

    // A fresh recap (new season index) clears any prior dismissal.
    const recapIndex = data?.recap?.season.index ?? null
    useEffect(() => {
        if (recapIndex != null) setRecapDismissed(false)
    }, [recapIndex])

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
    const { season, recap, seasonLeaderboard } = data
    const totalArchived = repos.reduce((sum, r) => sum + r.archivedCount, 0)
    const noWorkspaces = summary.repoCount === 0 && summary.flatCount === 0
    const maxRepoActive = Math.max(1, ...repos.map((r) => r.activeCount))

    // The equipped finish (managed in Settings → Badge finishes) styles the
    // earned milestone badges here.
    const equippedDescriptor: TreatmentDescriptor | null = data.equipped

    // Master switch: when gamification is off (default), the Dashboard shows
    // only its analytics — no season, streak, heatmap, milestones, leaderboard,
    // badges, or celebrations.
    const gamified = data.gamificationEnabled

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
                        {gamified && season?.career && (
                            <span
                                className="career-rank"
                                title={`${season.career.ships} changes shipped all-time · permanent`}
                            >
                                <span className="career-rank-mark" aria-hidden>
                                    ◆
                                </span>
                                {season.career.label}
                            </span>
                        )}
                    </div>
                </div>
                {gamified && (
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
                )}
            </header>

            {gamified && (
                <>
                    {tierUp && (
                        <div className="season-tierup" role="status">
                            <span aria-hidden>▲</span> Tier up — {tierUp}
                        </div>
                    )}

                    {recap && !recapDismissed && (
                        <SeasonRecapCard
                            recap={recap}
                            onDismiss={() => setRecapDismissed(true)}
                        />
                    )}

                    {season && <SeasonPanel season={season} />}

                    <TodayHaul
                        today={progress.today}
                        activeChanges={progress.inFlight}
                        glowTasks={glowTasks}
                    />

                    <Heatmap cells={progress.heatmap} />

                    <Leaderboard entries={data.leaderboard} />
                    <Leaderboard
                        entries={seasonLeaderboard}
                        title="Leaderboard · this season"
                    />
                </>
            )}

            <div className="dashboard-grid">
                {gamified && (
                    <Milestones
                        milestones={progress.milestones}
                        equipped={equippedDescriptor}
                    />
                )}

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

            {gamified && <Celebration />}
        </div>
    )
}
