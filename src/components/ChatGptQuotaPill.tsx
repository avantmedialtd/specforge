import { useEffect, useState } from "react"
import { getChatGptQuota, onQuotaUpdated } from "../api"
import type { ChatGptQuotaState } from "../types"
import { WindowRow } from "./quotaMeter"

// Fallbacks used only when the response omits `limit_window_seconds` — see
// the *Missing window length falls back to standard durations* scenario in
// the `chatgpt-quota` spec.
const FALLBACK_PRIMARY_SECS = 5 * 3600
const FALLBACK_SECONDARY_SECS = 7 * 86400

/// Segment count + axis length + a terse label for one window, derived from
/// the server-reported window length: hours for windows up to 24h, days
/// beyond (mirrors the `chatgpt-quota` spec's *Time axis derives from the
/// reported window length* scenario, and the TUI's identical derivation in
/// `ui.rs`). Floored at 1 segment so a very short/zero length still renders a
/// bar instead of dividing by zero.
function axisFor(
    windowSecs: number | null,
    fallbackSecs: number,
): { label: string; segments: number; lengthSecs: number } {
    const secs = Math.max(1, windowSecs ?? fallbackSecs)
    if (secs <= 24 * 3600) {
        const hours = Math.max(1, Math.round(secs / 3600))
        return { label: windowLabel(secs, `${hours}h`), segments: hours, lengthSecs: secs }
    }
    const days = Math.max(1, Math.round(secs / 86400))
    return { label: windowLabel(secs, `${days}d`), segments: days, lengthSecs: secs }
}

/// The standard window lengths borrow the Claude pill's vocabulary — `wk` for a
/// week, `5h` for five hours — so the two provider strips name the same period
/// the same way. Matched within a tolerance because `limit_window_seconds` need
/// not be exactly 604800 / 18000 (mirrors `chatgpt_window_label` in the TUI).
function windowLabel(secs: number, derived: string): string {
    if (Math.abs(secs - 7 * 86400) <= 3600) return "wk"
    if (Math.abs(secs - 5 * 3600) <= 600) return "5h"
    return derived
}

/// The opt-in ChatGPT usage-quota gauge, pinned in the sidebar footer beside
/// the Claude pill. Renders nothing while disabled; re-reads on the same
/// `quota-updated` event the Claude pill listens for — the ChatGPT poller
/// emits the identical `CacheEvent::QuotaUpdated` (see `chatgpt_quota.rs`).
export function ChatGptQuotaPill() {
    const [quota, setQuota] = useState<ChatGptQuotaState | null>(null)
    const [nowMs, setNowMs] = useState(() => Date.now())

    useEffect(() => {
        let mounted = true
        const refresh = () =>
            getChatGptQuota()
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
            <div className="quota-strip quota-strip--muted" title="Sign in with the Codex CLI to show your ChatGPT usage quota">
                ChatGPT quota · sign in
            </div>
        )
    }
    if (quota.status === "unavailable") {
        return (
            <div className="quota-strip quota-strip--muted" title="ChatGPT usage could not be read right now">
                ChatGPT quota · unavailable
            </div>
        )
    }

    const primaryAxis = axisFor(quota.primary?.windowSecs ?? null, FALLBACK_PRIMARY_SECS)
    const secondaryAxis = axisFor(quota.secondary?.windowSecs ?? null, FALLBACK_SECONDARY_SECS)

    return (
        <div className={`quota-strip${quota.stale ? " quota-strip--stale" : ""}`}>
            <span className="quota-strip-label">
                ChatGPT{quota.stale ? " · updating…" : ""}
            </span>
            {quota.primary && (
                <WindowRow
                    label={primaryAxis.label}
                    win={quota.primary}
                    nowMs={nowMs}
                    segments={primaryAxis.segments}
                    lengthSecs={primaryAxis.lengthSecs}
                />
            )}
            {quota.secondary && (
                <WindowRow
                    label={secondaryAxis.label}
                    win={quota.secondary}
                    nowMs={nowMs}
                    segments={secondaryAxis.segments}
                    lengthSecs={secondaryAxis.lengthSecs}
                />
            )}
        </div>
    )
}
