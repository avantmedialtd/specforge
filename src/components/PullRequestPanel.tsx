import { useEffect, useState } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import {
    getBitbucketPullRequests,
    getGithubPullRequests,
    onBitbucketPullRequestsUpdated,
    onGithubPullRequestsUpdated,
} from "../api"
import { worktreeMarker, worktreesForPullRequest } from "../pullRequestLinks"
import { pullRequestAddressFor } from "../pullRequestOpen"
import { formatRelativeTime, nextTickDelayMs } from "../relativeTime"
import type { PullRequestAddress } from "../routing/address"
import type {
    BitbucketPullRequestsState,
    ChecksState,
    GithubPullRequestsState,
    LinkedWorktree,
    PanelMovedPayload,
    PanelPosition,
    PullRequestLinks,
    PullRequestProvider,
    PullRequestSummary,
    ReviewSummary,
} from "../types"
import { ChevronDown, ChevronRight, CommentIcon } from "./icons"
import { PullRequestControl } from "./PullRequestControl"

/// Collapsed-or-expanded is per-viewer view state, persisted like pane
/// visibility and never an application setting, and kept per provider
/// (`bitbucket-pull-requests` / `github-pull-requests`: *Pull-Request Panel*).
/// BitBucket keeps the key it shipped with, so existing collapse state
/// survives the second panel's arrival.
export const COLLAPSED_KEYS: Record<PullRequestProvider, string> = {
    bitbucket: "specforge.pullRequestsCollapsed",
    github: "specforge.githubPullRequestsCollapsed",
}

// -------------------------------------------------------------------------
// The row model — pure, exported, and unit-tested in PullRequestPanel.test.ts.
// JSX is not exercised by `bun test` and a frontend-only diff never reaches the
// mutation gate, so anything decided here rather than in markup is decided in
// one of these functions.
// -------------------------------------------------------------------------

/// A provider's snapshot, tagged with the provider it came from, so every
/// pure function below narrows on one discriminant.
export type PanelSnapshot =
    | { provider: "bitbucket"; snapshot: BitbucketPullRequestsState }
    | { provider: "github"; snapshot: GithubPullRequestsState }

/// One titled (or, for BitBucket, untitled) list of rows in the panel body.
/// `showAuthor` is set where the author is someone else — the rows awaiting
/// the viewer's review.
export interface PanelSection {
    key: string
    title: string | null
    rows: PullRequestSummary[]
    showAuthor: boolean
}

/// What the panel's body shows below its header.
export type PanelBodyState = "rows" | "unauthenticated" | "unavailable" | "empty"

/// The header's title, and the panel's accessible name: each names its
/// provider, so two panels are never ambiguous (`spec-browser`: *Side Panes
/// Host the Pull-Request Panel*).
export const PANEL_TITLES: Record<PullRequestProvider, string> = {
    bitbucket: "BitBucket pull requests",
    github: "GitHub pull requests",
}

/// The body's sections. BitBucket has one untitled list. GitHub has "Yours"
/// and "To review", and a section with no rows is not rendered at all.
export function panelSections(panel: PanelSnapshot): PanelSection[] {
    if (panel.provider === "bitbucket") {
        return [
            {
                key: "all",
                title: null,
                rows: panel.snapshot.pullRequests,
                showAuthor: false,
            },
        ]
    }
    const sections: PanelSection[] = [
        { key: "authored", title: "Yours", rows: panel.snapshot.authored, showAuthor: false },
        {
            key: "review-requested",
            title: "To review",
            rows: panel.snapshot.reviewRequested,
            showAuthor: true,
        },
    ]
    return sections.filter((section) => section.rows.length > 0)
}

/// Every row the panel lists, in body order.
export function panelRows(panel: PanelSnapshot): PullRequestSummary[] {
    return panelSections(panel).flatMap((section) => section.rows)
}

/// The body state a snapshot calls for, or `null` when the panel is not
/// rendered at all (the feature is disabled).
export function panelBodyState(panel: PanelSnapshot): PanelBodyState | null {
    switch (panel.snapshot.status) {
        case "disabled":
            return null
        case "unauthenticated":
            return "unauthenticated"
        case "unavailable":
            return "unavailable"
        case "ok":
            return panelRows(panel).length > 0 ? "rows" : "empty"
    }
}

/// Whether a provider's panel renders anything: `false` until its first
/// snapshot has been read and while the feature is disabled. `App` derives
/// this from the snapshot it reads itself, so presence — which decides whether
/// the rail exists at all (`spec-browser`: *Rail Exists Only While Occupied*)
/// — never depends on the panel having been mounted (design D3).
export function panelPresent(panel: PanelSnapshot | null): boolean {
    return panel !== null && panelBodyState(panel) !== null
}

/// The one quiet line each non-row body state shows, per provider. The
/// unauthenticated one points at Settings, where the credential is entered.
export const PANEL_MESSAGES: Record<
    PullRequestProvider,
    Record<Exclude<PanelBodyState, "rows">, string>
> = {
    bitbucket: {
        unauthenticated: "Credentials need attention — check BitBucket in Settings.",
        unavailable: "BitBucket is unavailable right now.",
        empty: "No open pull requests.",
    },
    github: {
        unauthenticated: "Token needs attention — check GitHub in Settings.",
        unavailable: "GitHub is unavailable right now.",
        empty: "No open pull requests and nothing to review.",
    },
}

/// The header's count: the number of rows for BitBucket, and `yours · to
/// review` for GitHub — shown only while there is a list (`ok`, stale or
/// not), so a collapsed panel still carries its counts. `label` says the same
/// in words for the tooltip and assistive technology.
export function headerCounts(panel: PanelSnapshot): { text: string; label: string } | null {
    if (panel.snapshot.status !== "ok") return null
    if (panel.provider === "bitbucket") {
        const n = panel.snapshot.pullRequests.length
        return { text: String(n), label: `${n} open` }
    }
    const yours = panel.snapshot.authored.length
    const toReview = panel.snapshot.reviewRequested.length
    return { text: `${yours} · ${toReview}`, label: `${yours} yours, ${toReview} to review` }
}

/// A checks state in words, for the dot's tooltip and accessible label.
export function checksLabel(checks: ChecksState): string {
    switch (checks) {
        case "passing":
            return "Checks passing"
        case "failing":
            return "Checks failing"
        case "pending":
            return "Checks pending"
    }
}

/// The unresolved-conversation count in words.
export function conversationsLabel(count: number): string {
    return `${count} unresolved conversation${count === 1 ? "" : "s"}`
}

/// The review cell: approvals, changes requested and pending reviewers, always
/// in that order and always all three, so a zero reads as a zero. An absent
/// summary is "unknown" — the response carried no review data — which must
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
/// relative-time vocabulary. A row whose updated time could not be read
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

/// The header's tooltip. What the panel could not list — BitBucket's skipped
/// workspaces, GitHub's withheld results — is visible here, on request, and
/// never rendered as an error; a stale list says why it is de-emphasised.
export function panelHeaderTitle(panel: PanelSnapshot): string {
    if (panel.provider === "bitbucket") {
        const { snapshot } = panel
        const lines = ["Your open BitBucket pull requests"]
        if (snapshot.stale) {
            lines.push("Showing the last list — BitBucket could not be reached.")
        }
        if (snapshot.skippedWorkspaces.length > 0) {
            lines.push(`Skipped (no access): ${snapshot.skippedWorkspaces.join(", ")}`)
        }
        return lines.join("\n")
    }
    const { snapshot } = panel
    const lines = ["Your open GitHub pull requests, and those awaiting your review"]
    const counts = headerCounts(panel)
    if (counts) lines.push(counts.label)
    if (snapshot.stale) {
        lines.push("Showing the last lists — GitHub could not be reached.")
    }
    if (snapshot.withheld > 0) {
        const n = snapshot.withheld
        lines.push(
            `${n} result${n === 1 ? "" : "s"} withheld by GitHub. An organisation may ` +
                "require the token to be authorised for single sign-on, and a " +
                "fine-grained token sees only the one account or organisation it " +
                "was created for.",
        )
    }
    return lines.join("\n")
}

/// Which panel a `pull-request-panel-moved` payload re-seats, and where —
/// or `null` for a frame that names no known provider or no position (an
/// unparseable SSE frame arrives as `undefined`). A move re-seats only the
/// panel it names (`github-pull-requests`: *GitHub Panel Position Is a
/// Persisted Setting*).
export function routePanelMove(
    payload: PanelMovedPayload | undefined | null,
): { provider: PullRequestProvider; position: PanelPosition } | null {
    if (!payload?.position) return null
    if (payload.provider !== "bitbucket" && payload.provider !== "github") return null
    return { provider: payload.provider, position: payload.position }
}

/// The side pane a slot belongs to, or `null` before the setting is read.
export function paneOf(position: PanelPosition | null): "sidebar" | "rail" | null {
    switch (position) {
        case "left-top":
        case "left-bottom":
            return "sidebar"
        case "right-top":
        case "right-bottom":
            return "rail"
        case null:
            return null
    }
}

/// Whether `pane` takes the height reserve: only while a panel that is
/// actually rendering sits in it. A panel merely *positioned* there does not
/// count — a default install has both features off at `left-bottom` and must
/// lay out exactly as it did without the panels (`spec-browser`: *Side Panes
/// Host the Pull-Request Panel*; design D9).
export function paneTakesReserve(
    pane: "sidebar" | "rail",
    panels: { position: PanelPosition | null; present: boolean }[],
): boolean {
    return panels.some((panel) => panel.present && paneOf(panel.position) === pane)
}

function readCollapsed(provider: PullRequestProvider): boolean {
    try {
        return globalThis.localStorage?.getItem(COLLAPSED_KEYS[provider]) === "true"
    } catch {
        return false
    }
}

function writeCollapsed(provider: PullRequestProvider, collapsed: boolean): void {
    try {
        globalThis.localStorage?.setItem(COLLAPSED_KEYS[provider], String(collapsed))
    } catch {
        // Storage blocked: the panel still collapses, just not persistently.
    }
}

// -------------------------------------------------------------------------
// Provider adapters: where each panel's snapshot comes from and when to
// re-read it. Everything else is shared.
// -------------------------------------------------------------------------

interface ProviderSource {
    fetch: () => Promise<PanelSnapshot>
    onUpdated: (handler: () => void) => Promise<UnlistenFn>
}

/// Which getter and which update event each provider's panel uses — the
/// routing behind "a GitHub refresh does not make the BitBucket panel
/// re-read". Exported so a test pins it: swapping two entries would pass
/// every rendering check while each panel re-read on the other's event.
export const PANEL_SOURCES: Record<PullRequestProvider, ProviderSource> = {
    bitbucket: {
        fetch: () =>
            getBitbucketPullRequests().then((snapshot) => ({
                provider: "bitbucket" as const,
                snapshot,
            })),
        onUpdated: onBitbucketPullRequestsUpdated,
    },
    github: {
        fetch: () =>
            getGithubPullRequests().then((snapshot) => ({
                provider: "github" as const,
                snapshot,
            })),
        onUpdated: onGithubPullRequestsUpdated,
    },
}

// -------------------------------------------------------------------------
// The component
// -------------------------------------------------------------------------

/// One provider's opt-in pull-request panel, rendered by `App` in whichever of
/// the four side-pane slots that provider's position setting names. Renders
/// nothing while the feature is disabled, so a disabled feature leaves the
/// layout exactly as it was.
///
/// `panel` is the provider's snapshot, read by `App` through
/// `usePullRequestSnapshot` (and re-read there on each of the provider's
/// `*-pull-requests-updated` events) rather than here: `App` needs to know
/// whether the panel is present before deciding whether the rail exists, and a
/// panel that read its own snapshot could only report that once mounted
/// (design D3). It also means a panel re-mounted in another slot renders from
/// the snapshot it had instead of re-reading.
///
/// `onOpenPullRequest` is App's `openPullRequest`: a row's click, Enter or
/// Space shows its pull request in the center pane, at its address
/// (`pull-request-viewer`: *Opening a Pull Request Like a Document*).
///
/// `links` and `onOpenWorktree` give a linked row its worktree marker: a
/// sibling control beside the row that navigates SpecForge to the change in
/// that worktree, while the row itself opens the pull request
/// (`pull-request-worktree-links`: *Pull-Request Rows Lead to Their Worktree*).
export function PullRequestPanel({
    provider,
    panel,
    onOpenPullRequest,
    links = null,
    onOpenWorktree,
}: {
    provider: PullRequestProvider
    panel: PanelSnapshot | null
    onOpenPullRequest: (address: PullRequestAddress) => void
    links?: PullRequestLinks | null
    onOpenWorktree?: (repoId: string, worktreePath: string) => void
}) {
    const [collapsed, setCollapsed] = useState(() => readCollapsed(provider))
    const [nowMs, setNowMs] = useState(() => Date.now())

    // A fresh snapshot restarts the relative-time clock, as a fresh read did
    // when the panel fetched its own.
    useEffect(() => {
        setNowMs(Date.now())
    }, [panel])

    useEffect(() => {
        writeCollapsed(provider, collapsed)
    }, [provider, collapsed])

    const body = panel ? panelBodyState(panel) : null

    // Keep the relative times current: wake exactly when the soonest label
    // changes, and not at all while the list is folded away.
    const rows = panel ? panelRows(panel) : undefined
    const relabelDelay = rows && !collapsed ? nextRelabelDelayMs(nowMs, rows) : null
    useEffect(() => {
        if (relabelDelay === null) return
        const timer = window.setTimeout(() => setNowMs(Date.now()), relabelDelay)
        return () => window.clearTimeout(timer)
    }, [relabelDelay, nowMs])

    if (!panel || body === null) return null

    const counts = headerCounts(panel)
    const title = PANEL_TITLES[provider]

    return (
        <section
            className={`pull-request-panel${panel.snapshot.stale ? " pull-request-panel--stale" : ""}`}
            aria-label={title}
        >
            <button
                type="button"
                className="pull-request-panel-header"
                aria-expanded={!collapsed}
                onClick={() => setCollapsed((c) => !c)}
                title={panelHeaderTitle(panel)}
            >
                {collapsed ? (
                    <ChevronRight width={14} height={14} />
                ) : (
                    <ChevronDown width={14} height={14} />
                )}
                <span className="pull-request-panel-title">{title}</span>
                {counts && (
                    <span className="pull-request-panel-count" aria-label={counts.label}>
                        {counts.text}
                    </span>
                )}
            </button>
            {!collapsed &&
                (body === "rows" ? (
                    // One scroll container for the whole body — section
                    // headings included — so a squeezed panel scrolls rather
                    // than clipping rows, and two sections never add up to two
                    // height caps.
                    <div className="pull-request-list">
                        {panelSections(panel).map((section) => (
                            <div key={section.key} className="pull-request-section">
                                {section.title && (
                                    <h3 className="pull-request-section-title">
                                        {section.title}
                                    </h3>
                                )}
                                <ul className="pull-request-section-rows">
                                    {section.rows.map((pr) => {
                                        const worktrees = onOpenWorktree
                                            ? worktreesForPullRequest(links, pr.url)
                                            : []
                                        return (
                                            <li
                                                key={`${pr.url}|${pr.repoFullName}#${pr.id}`}
                                                // Only a linked row becomes a
                                                // two-column grid; an unlinked
                                                // row renders exactly as before.
                                                className={
                                                    worktrees.length > 0
                                                        ? "pull-request-item--linked"
                                                        : undefined
                                                }
                                            >
                                                <PullRequestRow
                                                    provider={provider}
                                                    pr={pr}
                                                    nowMs={nowMs}
                                                    showAuthor={section.showAuthor}
                                                    onOpen={onOpenPullRequest}
                                                />
                                                {onOpenWorktree && (
                                                    <WorktreeMarker
                                                        worktrees={worktrees}
                                                        onOpen={onOpenWorktree}
                                                    />
                                                )}
                                            </li>
                                        )
                                    })}
                                </ul>
                            </div>
                        ))}
                    </div>
                ) : (
                    <p className="pull-request-panel-message">
                        {PANEL_MESSAGES[provider][body]}
                    </p>
                ))}
        </section>
    )
}

/// A linked row's worktree marker: its own control, a SIBLING of the row's
/// control — interactive content cannot nest inside the row's `<button>` or
/// `<a>` — naming the first linked worktree and listing every one in its
/// tooltip and accessible name. It navigates in-app on both transports and
/// never opens the pull request. Renders nothing for an unlinked row.
function WorktreeMarker({
    worktrees,
    onOpen,
}: {
    worktrees: LinkedWorktree[]
    onOpen: (repoId: string, worktreePath: string) => void
}) {
    const marker = worktreeMarker(worktrees)
    const first = worktrees[0]
    if (!marker || !first) return null
    return (
        <button
            type="button"
            className="pull-request-worktree"
            title={marker.label}
            aria-label={marker.label}
            onClick={() => onOpen(first.repoId, first.worktreePath)}
        >
            <span className="pull-request-worktree-glyph" aria-hidden="true">
                ⤷
            </span>
            <span className="pull-request-worktree-name">{marker.text}</span>
        </button>
    )
}

/// One row as a control that opens its pull request in SpecForge
/// (`PullRequestControl`), or a static block when there is nothing to open.
/// It carries no tooltip: the provider's URL is no longer where a click goes,
/// and the provider's page is one control away, in the pull request's view.
function PullRequestRow({
    provider,
    pr,
    nowMs,
    showAuthor,
    onOpen,
}: {
    provider: PullRequestProvider
    pr: PullRequestSummary
    nowMs: number
    showAuthor: boolean
    onOpen: (address: PullRequestAddress) => void
}) {
    const content = <PullRequestRowContent pr={pr} nowMs={nowMs} showAuthor={showAuthor} />
    const address = pullRequestAddressFor(provider, pr)
    if (!address) {
        // No https link in the response, or nothing a path could name: no
        // address, so nothing to open by a click or by the gesture, and
        // nothing to activate.
        return <div className="pull-request-row pull-request-row--static">{content}</div>
    }
    return (
        <PullRequestControl
            className="pull-request-row"
            address={address}
            title={pr.title}
            onOpen={onOpen}
        >
            {content}
        </PullRequestControl>
    )
}

/// One row's content: repository (and, where it is someone else, the author),
/// draft marker, conflict marker and updated time; the title; branches, the
/// review cell, the checks dot, and — when non-zero — the open-task or
/// unresolved-conversation count. A value a provider cannot know renders
/// nothing. Phrasing content only, so it can sit inside a `<button>` or an
/// `<a>`.
function PullRequestRowContent({
    pr,
    nowMs,
    showAuthor,
}: {
    pr: PullRequestSummary
    nowMs: number
    showAuthor: boolean
}) {
    return (
        <>
            <span className="pull-request-row-top">
                <span className="pull-request-repo">{pr.repoFullName || "—"}</span>
                {showAuthor && pr.author && (
                    <span className="pull-request-author">{pr.author}</span>
                )}
                <PullRequestMarkers pr={pr} />
                <span className="pull-request-updated">
                    {relativeUpdated(nowMs, pr.updatedAtUnix)}
                </span>
            </span>
            <span className="pull-request-title">{pr.title}</span>
            <span className="pull-request-row-meta">
                <span className="pull-request-branches">
                    {pr.sourceBranch} → {pr.destinationBranch}
                </span>
                <PullRequestSignalCells pr={pr} />
            </span>
        </>
    )
}

/// A pull request's draft and conflict markers, as its row shows them. The
/// pull-request view's header repeats them from the same component, so the
/// header and the row cannot treat one signal two ways (`pull-request-viewer`:
/// *The header carries the row's signals*). Phrasing content only.
export function PullRequestMarkers({ pr }: { pr: PullRequestSummary }) {
    return (
        <>
            {pr.draft && <span className="pull-request-draft">Draft</span>}
            {pr.conflicting && (
                <span className="pull-request-conflict" title="Merge conflicts">
                    Conflicts
                </span>
            )}
        </>
    )
}

/// A pull request's signal cells, as its row shows them: the review cell, the
/// checks dot, and the open-task or unresolved-conversation count when it is
/// not zero. Shared with the pull-request view's header for the reason
/// `PullRequestMarkers` is. Phrasing content only.
export function PullRequestSignalCells({ pr }: { pr: PullRequestSummary }) {
    return (
        <>
            <span
                className={`pull-request-review${pr.review ? "" : " pull-request-review--unknown"}`}
                title={reviewCellTitle(pr.review)}
                aria-label={reviewCellTitle(pr.review)}
            >
                {reviewCellText(pr.review)}
            </span>
            {pr.checks && (
                <span
                    className={`pull-request-checks pull-request-checks--${pr.checks}`}
                    title={checksLabel(pr.checks)}
                    aria-label={checksLabel(pr.checks)}
                    role="img"
                />
            )}
            {pr.openTasks > 0 && (
                <span
                    className="pull-request-tasks"
                    title={`${pr.openTasks} open task${pr.openTasks === 1 ? "" : "s"}`}
                >
                    ☐ {pr.openTasks}
                </span>
            )}
            {pr.unresolvedThreads > 0 && (
                <span
                    className="pull-request-conversations"
                    title={conversationsLabel(pr.unresolvedThreads)}
                    aria-label={conversationsLabel(pr.unresolvedThreads)}
                >
                    <CommentIcon width={11} height={11} />
                    {pr.unresolvedThreads}
                </span>
            )}
        </>
    )
}
