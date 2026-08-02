import type { QuotaWindow } from "../types"

/// Provider-neutral pieces of a quota-window meter row, shared by every
/// provider's sidebar-footer pill (`QuotaPill` for Claude, `ChatGptQuotaPill`
/// for ChatGPT) so the visual grammar — and the `quota-*` CSS — stays
/// identical across providers. Extracted from `QuotaPill.tsx`, which
/// originally defined these only for itself.

/// Threshold colour class, matching the TUI gauge: green < 70, orange ≥ 70,
/// red ≥ 90.
export function fillClass(util: number): string {
    if (util >= 90) return "quota-meter-fill--red"
    if (util >= 70) return "quota-meter-fill--orange"
    return "quota-meter-fill--green"
}

/// `h:mm` (or `Nd` beyond 48h) until the window resets, computed live so an
/// exhausted-window countdown ticks down between polls.
export function countdown(resetsAtUnix: number | null, nowMs: number): string {
    if (resetsAtUnix == null) return "full"
    const mins = Math.max(0, Math.floor((resetsAtUnix * 1000 - nowMs) / 60000))
    if (mins >= 48 * 60) return `${Math.floor(mins / 1440)}d`
    return `${Math.floor(mins / 60)}:${String(mins % 60).padStart(2, "0")}`
}

/// Fraction (0..1) of a fixed-length window that has elapsed, or `null` when the
/// reset time is unknown (→ no segments, no marker). Computed live off `nowMs` so
/// the marker glides between polls, like the countdown.
export function elapsedFraction(
    resetsAtUnix: number | null,
    nowMs: number,
    lengthSecs: number,
): number | null {
    if (resetsAtUnix == null) return null
    const frac = 1 - (resetsAtUnix * 1000 - nowMs) / (lengthSecs * 1000)
    return Math.min(1, Math.max(0, frac))
}

/// One provider-neutral quota-window meter row: a label, the utilization fill
/// (colour-thresholded), segment ticks + a live "now" marker derived from the
/// window's reset time and length, and the percentage (or a reset countdown
/// once the window is fully consumed).
export function WindowRow({
    label,
    win,
    nowMs,
    segments,
    lengthSecs,
}: {
    label: string
    win: QuotaWindow
    nowMs: number
    segments: number
    lengthSecs: number
}) {
    const value =
        win.utilization >= 100
            ? countdown(win.resetsAtUnix, nowMs)
            : `${win.utilization}%`
    // The time axis: hour/day segment ticks plus a live "now" marker. Both ride
    // with the reset time — absent it, the bar stays the plain utilization fill.
    const marker = elapsedFraction(win.resetsAtUnix, nowMs, lengthSecs)
    return (
        <div className="quota-row">
            <span className="quota-row-label">{label}</span>
            <div className="quota-meter">
                <div
                    className={`quota-meter-fill ${fillClass(win.utilization)}`}
                    style={{ width: `${Math.min(100, Math.max(0, win.utilization))}%` }}
                />
                {marker != null && (
                    <>
                        {Array.from({ length: segments - 1 }, (_, i) => (
                            <span
                                key={i}
                                className="quota-meter-tick"
                                style={{ left: `${((i + 1) / segments) * 100}%` }}
                            />
                        ))}
                        <span
                            className="quota-meter-now"
                            style={{ left: `${marker * 100}%` }}
                            title={`${Math.round(marker * 100)}% through the window`}
                        />
                    </>
                )}
            </div>
            <span className="quota-row-value">{value}</span>
        </div>
    )
}
