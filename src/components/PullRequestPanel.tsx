import { useEffect, useRef, useState } from "react"
import { getMyPullRequests, isWeb, onPullRequestsUpdated, openPullRequest } from "../api"
import { formatRelativeTime, nextTickDelayMs } from "../relativeTime"
import type { PullRequestSummary, PullRequestsState, ReviewSummary } from "../types"
import { ChevronDown, ChevronRight } from "./icons"

/// Collapsed-or-expanded is per-viewer view state, persisted like pane
/// visibility and never an application setting (`bitbucket-pull-requests`:
/// *Pull-Request Panel*).
const COLLAPSED_KEY = "specforge.pullRequestsCollapsed"

/// How long the quiet "could not open" line stays up after a desktop open
/// fails — the same transient tone as a refused artifact link.
const OPEN_FAILURE_MS = 1600

// -------------------------------------------------------------------------
// The row model — pure, exported, and unit-tested in PullRequestPanel.test.ts.
// JSX is not exercised by `bun test` and a frontend-only diff never reaches the
// mutation gate, so anything decided here rather than in markup is decided in
// one of these functions.
// -------------------------------------------------------------------------

/// What the panel's body shows below its header.
export type PanelBodyState = "rows" | "unauthenticated" | "unavailable" | "empty"

/// The body state a snapshot calls for, or `null` when the panel is not
/// rendered at all (the feature is disabled).
export function panelBodyState(snapshot: PullRequestsState): PanelBodyState | null {
    switch (snapshot.status) {
        case "disabled":
            return null
        case "unauthenticated":
            return "unauthenticated"
        case "unavailable":
            return "unavailable"
        case "ok":
            return snapshot.pullRequests.length > 0 ? "rows" : "empty"
    }
}

/// The one quiet line each non-row body state shows. The unauthenticated one
/// points at Settings, where the credentials are entered.
export const PANEL_MESSAGES: Record<Exclude<PanelBodyState, "rows">, string> = {
    unauthenticated: "Credentials need attention — check BitBucket in Settings.",
    unavailable: "BitBucket is unavailable right now.",
    empty: "No open pull requests.",
}

/// The review cell: approvals, changes requested and pending reviewers, always
/// in that order and always all three, so a zero reads as a zero. An absent
/// summary is "unknown" — the response carried no participants — which must
/// not read as "no activity".
export function reviewCellText(review: ReviewSummary | null): string {
    if (!review) return "?"
    return `✓${review.approvals} ✗${review.changesRequested} ○${review.pending}`
}

/// The review cell's tooltip and accessible label — the same three numbers in
/// words.
export function reviewCellTitle(review: ReviewSummary | null): string {
    if (!review) return "Review state unknown"
    return (
        `${review.approvals} approved · ` +
        `${review.changesRequested} changes requested · ` +
        `${review.pending} pending`
    )
}

/// The row's updated time relative to `nowMs`, in the application's one
/// relative-time vocabulary. A row whose `updated_on` could not be read
/// carries `0` — shown as a dash rather than as fifty-six years ago.
export function relativeUpdated(nowMs: number, updatedAtUnix: number): string {
    return updatedAtUnix > 0 ? formatRelativeTime(updatedAtUnix, nowMs) : "—"
}

/// How long until the soonest of the rows' relative labels changes, or `null`
/// when no row has a readable time. One timer for the whole panel, landing on
/// the next boundary instead of polling on a fixed interval.
export function nextRelabelDelayMs(
    nowMs: number,
    rows: PullRequestSummary[],
): number | null {
    const delays = rows
        .filter((row) => row.updatedAtUnix > 0)
        .map((row) => nextTickDelayMs(row.updatedAtUnix, nowMs))
    return delays.length > 0 ? Math.min(...delays) : null
}

/// The header's tooltip. Skipped workspaces are visible here, on request, and
/// never rendered as an error; a stale list says why it is de-emphasised.
export function panelHeaderTitle(snapshot: PullRequestsState): string {
    const lines = ["Your open BitBucket pull requests"]
    if (snapshot.stale) {
        lines.push("Showing the last list — BitBucket could not be reached.")
    }
    if (snapshot.skippedWorkspaces.length > 0) {
        lines.push(`Skipped (no access): ${snapshot.skippedWorkspaces.join(", ")}`)
    }
    return lines.join("\n")
}

function readCollapsed(): boolean {
    try {
        return globalThis.localStorage?.getItem(COLLAPSED_KEY) === "true"
    } catch {
        return false
    }
}

function writeCollapsed(collapsed: boolean): void {
    try {
        globalThis.localStorage?.setItem(COLLAPSED_KEY, String(collapsed))
    } catch {
        // Storage blocked: the panel still collapses, just not persistently.
    }
}

// -------------------------------------------------------------------------
// The component
// -------------------------------------------------------------------------

/// The opt-in BitBucket pull-request panel, rendered by `App` in whichever of
/// the four side-pane slots the position setting names. Renders nothing while
/// the feature is disabled, so a disabled feature leaves the layout exactly as
/// it was; re-reads the snapshot on each `pull-requests-updated` event.
export function PullRequestPanel() {
    const [snapshot, setSnapshot] = useState<PullRequestsState | null>(null)
    const [collapsed, setCollapsed] = useState(readCollapsed)
    const [nowMs, setNowMs] = useState(() => Date.now())
    const [openFailed, setOpenFailed] = useState(false)
    const failureTimer = useRef<number | undefined>(undefined)

    useEffect(() => {
        let mounted = true
        const refresh = () =>
            getMyPullRequests()
                .then((next) => {
                    if (!mounted) return
                    setSnapshot(next)
                    setNowMs(Date.now())
                })
                .catch(() => {})
        refresh()
        let unlisten: (() => void) | undefined
        onPullRequestsUpdated(() => refresh()).then((u) => {
            if (mounted) unlisten = u
            else u()
        })
        return () => {
            mounted = false
            unlisten?.()
            window.clearTimeout(failureTimer.current)
        }
    }, [])

    useEffect(() => {
        writeCollapsed(collapsed)
    }, [collapsed])

    // Keep the relative times current: wake exactly when the soonest label
    // changes, and not at all while the list is folded away.
    const rows = snapshot?.pullRequests
    useEffect(() => {
        if (!rows || collapsed) return
        const delay = nextRelabelDelayMs(nowMs, rows)
        if (delay === null) return
        const timer = window.setTimeout(() => setNowMs(Date.now()), delay)
        return () => window.clearTimeout(timer)
    }, [rows, collapsed, nowMs])

    if (!snapshot) return null
    const body = panelBodyState(snapshot)
    if (body === null) return null

    // Desktop: through the snapshot-scoped command. A refusal or opener error
    // is reported quietly and transiently — never a navigation, never a throw.
    const openRow = (url: string) => {
        openPullRequest(url).catch(() => {
            window.clearTimeout(failureTimer.current)
            setOpenFailed(true)
            failureTimer.current = window.setTimeout(
                () => setOpenFailed(false),
                OPEN_FAILURE_MS,
            )
        })
    }
    const web = isWeb()

    return (
        <section
            className={`pull-request-panel${snapshot.stale ? " pull-request-panel--stale" : ""}`}
            aria-label="Pull requests"
        >
            <button
                type="button"
                className="pull-request-panel-header"
                aria-expanded={!collapsed}
                onClick={() => setCollapsed((c) => !c)}
                title={panelHeaderTitle(snapshot)}
            >
                {collapsed ? (
                    <ChevronRight width={14} height={14} />
                ) : (
                    <ChevronDown width={14} height={14} />
                )}
                <span className="pull-request-panel-title">Pull requests</span>
                {snapshot.status === "ok" && (
                    <span className="pull-request-panel-count">
                        {snapshot.pullRequests.length}
                    </span>
                )}
            </button>
            {!collapsed &&
                (body === "rows" ? (
                    <ul className="pull-request-list">
                        {snapshot.pullRequests.map((pr) => (
                            <li key={`${pr.url}|${pr.repoFullName}#${pr.id}`}>
                                {pr.url === "" ? (
                                    // No https link in the response: nothing
                                    // to open, so nothing to activate.
                                    <div className="pull-request-row pull-request-row--static">
                                        <PullRequestRowContent pr={pr} nowMs={nowMs} />
                                    </div>
                                ) : web ? (
                                    // Browser skin: a plain link in a new,
                                    // opener-isolated tab — the serving page
                                    // never navigates, and the server opens
                                    // nothing (`open_pull_request` has no web
                                    // dispatch arm).
                                    <a
                                        className="pull-request-row"
                                        href={pr.url}
                                        target="_blank"
                                        rel="noopener noreferrer"
                                        title={pr.url}
                                    >
                                        <PullRequestRowContent pr={pr} nowMs={nowMs} />
                                    </a>
                                ) : (
                                    <button
                                        type="button"
                                        className="pull-request-row"
                                        onClick={() => openRow(pr.url)}
                                        title={pr.url}
                                    >
                                        <PullRequestRowContent pr={pr} nowMs={nowMs} />
                                    </button>
                                )}
                            </li>
                        ))}
                    </ul>
                ) : (
                    <p className="pull-request-panel-message">{PANEL_MESSAGES[body]}</p>
                ))}
            {openFailed && (
                <p className="pull-request-panel-message" role="status">
                    Could not open that pull request.
                </p>
            )}
        </section>
    )
}

/// One row's content: repository, draft marker and updated time; the title;
/// branches, the review cell and — when non-zero — the open-task count.
/// Phrasing content only, so it can sit inside a `<button>` or an `<a>`.
function PullRequestRowContent({
    pr,
    nowMs,
}: {
    pr: PullRequestSummary
    nowMs: number
}) {
    return (
        <>
            <span className="pull-request-row-top">
                <span className="pull-request-repo">{pr.repoFullName || "—"}</span>
                {pr.draft && <span className="pull-request-draft">Draft</span>}
                <span className="pull-request-updated">
                    {relativeUpdated(nowMs, pr.updatedAtUnix)}
                </span>
            </span>
            <span className="pull-request-title">{pr.title}</span>
            <span className="pull-request-row-meta">
                <span className="pull-request-branches">
                    {pr.sourceBranch} → {pr.destinationBranch}
                </span>
                <span
                    className={`pull-request-review${pr.review ? "" : " pull-request-review--unknown"}`}
                    title={reviewCellTitle(pr.review)}
                    aria-label={reviewCellTitle(pr.review)}
                >
                    {reviewCellText(pr.review)}
                </span>
                {pr.openTasks > 0 && (
                    <span
                        className="pull-request-tasks"
                        title={`${pr.openTasks} open task${pr.openTasks === 1 ? "" : "s"}`}
                    >
                        ☐ {pr.openTasks}
                    </span>
                )}
            </span>
        </>
    )
}
