import { useEffect, useState } from "react"
import { getClaudeQuota, onQuotaUpdated } from "../api"
import type { ClaudeQuotaState, QuotaWindow } from "../types"

/// Threshold colour class, matching the TUI gauge: green < 70, orange ≥ 70,
/// red ≥ 90.
function fillClass(util: number): string {
    if (util >= 90) return "quota-meter-fill--red"
    if (util >= 70) return "quota-meter-fill--orange"
    return "quota-meter-fill--green"
}

/// `h:mm` (or `Nd` beyond 48h) until the window resets, computed live so an
/// exhausted-window countdown ticks down between polls.
function countdown(resetsAtUnix: number | null, nowMs: number): string {
    if (resetsAtUnix == null) return "full"
    const mins = Math.max(0, Math.floor((resetsAtUnix * 1000 - nowMs) / 60000))
    if (mins >= 48 * 60) return `${Math.floor(mins / 1440)}d`
    return `${Math.floor(mins / 60)}:${String(mins % 60).padStart(2, "0")}`
}

function WindowRow({
    label,
    win,
    nowMs,
}: {
    label: string
    win: QuotaWindow
    nowMs: number
}) {
    const value =
        win.utilization >= 100
            ? countdown(win.resetsAtUnix, nowMs)
            : `${win.utilization}%`
    return (
        <div className="quota-row">
            <span className="quota-row-label">{label}</span>
            <div className="quota-meter">
                <div
                    className={`quota-meter-fill ${fillClass(win.utilization)}`}
                    style={{ width: `${Math.min(100, Math.max(0, win.utilization))}%` }}
                />
            </div>
            <span className="quota-row-value">{value}</span>
        </div>
    )
}

/// The opt-in Claude usage-quota gauge, pinned in the sidebar footer. Renders
/// nothing when the feature is disabled; re-reads on each `quota-updated` event.
export function QuotaPill() {
    const [quota, setQuota] = useState<ClaudeQuotaState | null>(null)
    const [nowMs, setNowMs] = useState(() => Date.now())

    useEffect(() => {
        let mounted = true
        const refresh = () =>
            getClaudeQuota()
                .then((q) => {
                    if (mounted) setQuota(q)
                })
                .catch(() => {})
        refresh()
        let unlisten: (() => void) | undefined
        onQuotaUpdated(() => refresh()).then((u) => {
            if (mounted) unlisten = u
            else u()
        })
        // Keep exhausted-window countdowns roughly live between poll events.
        const id = setInterval(() => setNowMs(Date.now()), 30_000)
        return () => {
            mounted = false
            unlisten?.()
            clearInterval(id)
        }
    }, [])

    if (!quota || quota.status === "disabled") return null

    if (quota.status === "unauthenticated") {
        return (
            <div className="quota-strip quota-strip--muted" title="Sign in with the Claude Code CLI to show your usage quota">
                Claude quota · sign in
            </div>
        )
    }
    if (quota.status === "unavailable") {
        return (
            <div className="quota-strip quota-strip--muted" title="Claude usage could not be read right now">
                Claude quota · unavailable
            </div>
        )
    }

    return (
        <div className={`quota-strip${quota.stale ? " quota-strip--stale" : ""}`}>
            <span className="quota-strip-label">
                Claude{quota.stale ? " · updating…" : ""}
            </span>
            {quota.fiveHour && (
                <WindowRow label="5h" win={quota.fiveHour} nowMs={nowMs} />
            )}
            {quota.sevenDay && (
                <WindowRow label="wk" win={quota.sevenDay} nowMs={nowMs} />
            )}
        </div>
    )
}
