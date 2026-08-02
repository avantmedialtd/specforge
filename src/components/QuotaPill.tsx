import { useEffect, useState } from "react"
import { getClaudeQuota, onQuotaUpdated } from "../api"
import type { ClaudeQuotaState } from "../types"
import { WindowRow } from "./quotaMeter"

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
                <WindowRow
                    label="5h"
                    win={quota.fiveHour}
                    nowMs={nowMs}
                    segments={5}
                    lengthSecs={5 * 3600}
                />
            )}
            {quota.sevenDay && (
                <WindowRow
                    label="wk"
                    win={quota.sevenDay}
                    nowMs={nowMs}
                    segments={7}
                    lengthSecs={7 * 86400}
                />
            )}
            {/* Per-model scoped weekly windows (e.g. Fable) — same weekly axis,
                labeled by the model name. Empty for most snapshots. */}
            {quota.scoped.map((w) => (
                <WindowRow
                    key={w.model}
                    label={w.model}
                    win={{ utilization: w.utilization, resetsAtUnix: w.resetsAtUnix }}
                    nowMs={nowMs}
                    segments={7}
                    lengthSecs={7 * 86400}
                />
            ))}
        </div>
    )
}
