// The pull-request view's decisions (`pull-request-viewer`: *Pull-Request
// View*, *Changed Files in the Pull-Request View*, *Linked Change in the
// Pull-Request View*, *Detail Reads Are Scoped to the Snapshot*, *Shared
// Backoff and Detail Budget*, *Review Progress*; design D5, D8, D9 and D11).
//
// When the view asks the service for a read, and what each answer leaves on
// screen; what its notices say, the deferral's time included; what the header
// says about review progress; which check earns a link; how a review thread
// names its place, and which file it sits with; and which change the header
// names. Each is a pure function here, for the reason `pullRequestOpen.ts`
// gives: JSX is not exercised by `bun test` and a frontend-only diff skips the
// mutation gate, so these tests are the only coverage the decisions get.

import { fileKey } from "./diffFiles"
import { worktreeDestination, worktreeName, worktreesForPullRequest } from "./pullRequestLinks"
import { pullRequestHref } from "./pullRequestMarkdown"
import { pullRequestAddressFor } from "./pullRequestOpen"
import type { PullRequestAddress } from "./routing/address"
import type { PullRequestResolution } from "./routing/resolve"
import type {
    ChecksState,
    DiffFile,
    FileReviewProgress,
    PullRequestCheckState,
    PullRequestDetail,
    PullRequestDetailOutcome,
    PullRequestLinks,
    PullRequestProvider,
    PullRequestReference,
    PullRequestSummary,
    ReviewProgress,
    ReviewState,
    ReviewThread,
    WorkspaceView,
} from "./types"

/// Each provider's name, as the view, its notices and the header chips say it.
export const PROVIDER_NAMES: Record<PullRequestProvider, string> = {
    bitbucket: "BitBucket",
    github: "GitHub",
}

/// A pull request's identity as the view is keyed by it: its provider, its
/// owner and repository with ASCII letters lowered, and its number. Two
/// spellings of one pull request, which `sameReference` holds equal, share one
/// key, so replacing a case variant with its row's spelling re-renders the view
/// and its diff rather than remounting them.
export function referenceKey(reference: PullRequestReference): string {
    return [
        reference.provider,
        asciiLower(reference.owner),
        asciiLower(reference.repo),
        reference.number,
    ].join("/")
}

function asciiLower(text: string): string {
    return text.replace(/[A-Z]+/g, (run) => run.toLowerCase())
}

/// The seconds of the Unix epoch now, as the service counts them.
export function nowUnix(): number {
    return Math.floor(Date.now() / 1000)
}

// ---- Reads -----------------------------------------------------------------

/// What the freshness rule watches of a pull request's row: its updated time
/// and, on GitHub, its checks and its count of unresolved conversations. A
/// BitBucket row carries neither of the last two, so its updated time alone
/// decides. Nothing else takes part: a retitle alone brings no read.
export interface RowSignals {
    updatedAtUnix: number
    checks: ChecksState | null
    unresolvedThreads: number
}

export function rowSignals(row: PullRequestSummary): RowSignals {
    return {
        updatedAtUnix: row.updatedAtUnix,
        checks: row.checks,
        unresolvedThreads: row.unresolvedThreads,
    }
}

export function sameSignals(a: RowSignals, b: RowSignals): boolean {
    return (
        a.updatedAtUnix === b.updatedAtUnix &&
        a.checks === b.checks &&
        a.unresolvedThreads === b.unresolvedThreads
    )
}

/// What the view says after an answer that brought no fresh detail:
///
/// - `deferred` — a deadline or the hourly budget holds until `untilUnix`, and
///   the service sent nothing;
/// - `transient` — the read failed and may succeed later;
/// - `unauthenticated` — the provider did not accept the credential;
/// - `unavailable` — the provider says the pull request cannot be read;
/// - `notListed` — the provider's list does not hold it and nothing is cached;
/// - `refused` — its provider is switched off.
export type ReadNotice =
    | { kind: "deferred"; untilUnix: number }
    | { kind: "transient" }
    | { kind: "unauthenticated" }
    | { kind: "unavailable" }
    | { kind: "notListed" }
    | { kind: "refused" }

/// The view's read state: what is on screen, and what the freshness rule
/// remembers between announcements.
export interface ViewRead {
    /// The detail on screen, or null while there is none to show.
    detail: PullRequestDetail | null
    /// What the view says beside the detail, or in its place.
    notice: ReadNotice | null
    /// The row's signals when the view last asked for a read: null before its
    /// first ask, and when it asked while the pull request was not listed.
    askedWith: RowSignals | null
    /// When the view's last read was deferred, the time it named; null once
    /// the view asks again.
    deferredUntilUnix: number | null
}

export const NO_READ: ViewRead = {
    detail: null,
    notice: null,
    askedWith: null,
    deferredUntilUnix: null,
}

/// The read state once the view has asked for a read, under `row` while the
/// pull request is listed. A deferral ends here: the read it held back has now
/// been asked for, so a later unchanged announcement asks for nothing more.
/// What is on screen stays until the answer lands, so a re-read never puts a
/// loading state in the detail's place.
export function askedRead(read: ViewRead, row: PullRequestSummary | null): ViewRead {
    return { ...read, askedWith: row ? rowSignals(row) : null, deferredUntilUnix: null }
}

/// The read state once an answer has landed:
///
/// - a detail replaces whatever was on screen, notice and all;
/// - a deferral shows the cached detail it carries, else keeps the one on
///   screen, and remembers its time for the freshness rule;
/// - a cache-only call with nothing cached paints nothing and changes nothing;
/// - a transient failure keeps any detail on screen;
/// - not listed, refused, unauthenticated and unavailable replace the detail
///   with their notice: none of them leaves anything the service would serve.
export function answeredRead(read: ViewRead, outcome: PullRequestDetailOutcome): ViewRead {
    switch (outcome.kind) {
        case "detail":
            return { ...read, detail: withShownFiles(read.detail, outcome.detail), notice: null }
        case "deferred":
            return {
                ...read,
                detail: outcome.detail ? withShownFiles(read.detail, outcome.detail) : read.detail,
                notice: { kind: "deferred", untilUnix: outcome.untilUnix },
                deferredUntilUnix: outcome.untilUnix,
            }
        case "notCached":
            return read
        case "transient":
            return { ...read, notice: { kind: "transient" } }
        case "notListed":
        case "refused":
        case "unauthenticated":
        case "unavailable":
            return { ...read, detail: null, notice: { kind: outcome.kind } }
    }
}

/// `next` as it replaces `shown`: read at the same head and base commits, the
/// two details' files are the same diff, so the files on screen are kept, the
/// same array. The diff view then keeps every withheld file the reader has
/// loaded and highlights nothing again, while its conversation, threads and
/// checks, which change without a push, are taken from `next`.
function withShownFiles(shown: PullRequestDetail | null, next: PullRequestDetail): PullRequestDetail {
    return shown !== null &&
        shown.headCommit === next.headCommit &&
        shown.baseCommit === next.baseCommit
        ? { ...next, files: shown.files }
        : next
}

/// What can make the view ask for a read (`pull-request-viewer`: *Detail Reads
/// Are Scoped to the Snapshot*). No timer is among them, so an idle view asks
/// for nothing:
///
/// - `open` — the view opened, in either presentation, having first painted
///   what a cache-only call returned;
/// - `announcement` — its provider announced a snapshot, in which the pull
///   request's row is `row`;
/// - `fileRefused` — the service refused a withheld file the view asked for,
///   because the cached detail moved on from the one the view shows;
/// - `manual` — the reader asked for a refresh.
export type ReadTrigger =
    | { kind: "open" }
    | { kind: "announcement"; row: PullRequestSummary; nowUnix: number }
    | { kind: "fileRefused" }
    | { kind: "manual" }

const AUTOMATIC = { manual: false } as const
const MANUAL = { manual: true } as const

/// The read `trigger` calls for, and whether it is a manual refresh, which
/// the service lets past its 60-second freshness rule; `null` when it calls
/// for none.
export function readFor(read: ViewRead, trigger: ReadTrigger): { manual: boolean } | null {
    switch (trigger.kind) {
        case "open":
        case "fileRefused":
            return AUTOMATIC
        case "manual":
            return MANUAL
        case "announcement":
            return announcementCallsForRead(read, trigger.row, trigger.nowUnix) ? AUTOMATIC : null
    }
}

/// Whether an announcement whose row is `row` calls for a read: the row's
/// updated time, checks or unresolved-conversation count changed since the
/// view last asked, or the view's last read was deferred and the time it
/// named has passed — the first announcement after that time reads whether or
/// not the row changed, since a poller announces only news and an unchanged
/// row could otherwise leave a deferred view waiting indefinitely (design D8).
/// While the deferral holds, no announcement asks: the view sends nothing, and
/// says when a read becomes possible.
export function announcementCallsForRead(
    read: ViewRead,
    row: PullRequestSummary,
    nowUnix: number,
): boolean {
    if (read.deferredUntilUnix !== null) return nowUnix >= read.deferredUntilUnix
    return read.askedWith === null || !sameSignals(read.askedWith, rowSignals(row))
}

// ---- Notices ---------------------------------------------------------------

/// Where every notice that needs the reader to act sends them.
export const SETTINGS_INTEGRATIONS = "Settings › Integrations"

/// Why the view, or the address it would show, says something other than the
/// pull request: an address's resolution (`providerOff`, and the list's
/// `listUnauthenticated` or `listUnavailable`) or a read's answer.
export type PullRequestNotice =
    | ReadNotice
    | { kind: "providerOff" }
    | { kind: "listUnauthenticated" }
    | { kind: "listUnavailable" }

/// The notice an address's resolution calls for in place of the pull request:
/// its provider switched off, or its list unauthenticated or unavailable.
/// `null` for the outcomes that are no notice of the address's own — pending
/// shows "Loading…", and listed and not listed show the view, which asks for
/// the detail (`view-routing`: *Pull-Request Addresses*).
export function resolutionNotice(resolution: PullRequestResolution): PullRequestNotice | null {
    switch (resolution.status) {
        case "providerOff":
            return { kind: "providerOff" }
        case "unavailable":
            return {
                kind:
                    resolution.reason === "unauthenticated"
                        ? "listUnauthenticated"
                        : "listUnavailable",
            }
        case "pending":
        case "listed":
        case "notListed":
            return null
    }
}

/// Whether a notice in place of the pull request offers a refresh: when a
/// later read may bring the detail — after a failure, a deferral, or a
/// credential or repository fixed elsewhere. Not when the pull request is not
/// listed, where the service answers from its cache alone, nor when its
/// provider is off, where it answers nothing.
export function noticeOffersRefresh(notice: ReadNotice): boolean {
    return notice.kind !== "notListed" && notice.kind !== "refused"
}

/// A notice's words, and whether it points to Settings › Integrations, which
/// its text then names: the center pane adds a link there, while the
/// pull-request window, which cannot navigate the main window, leaves the name
/// as text (`pull-request-viewer`: *Pull-Request Window*).
export interface NoticeWords {
    title: string
    text: string
    settings: boolean
}

/// What the view needs to word a notice: whether it keeps a detail on screen
/// beside it, the time now, and how a time of day is written.
export interface NoticeContext {
    kept: boolean
    nowUnix: number
    clock?: (unixSeconds: number) => string
}

export function noticeWords(
    provider: PullRequestProvider,
    notice: PullRequestNotice,
    context: NoticeContext,
): NoticeWords {
    const name = PROVIDER_NAMES[provider]
    const credential = provider === "github" ? "token" : "credentials"
    switch (notice.kind) {
        case "providerOff":
        case "refused":
            return {
                title: `${name} is switched off`,
                text: `Turn on ${name} pull requests in ${SETTINGS_INTEGRATIONS} to show this pull request.`,
                settings: true,
            }
        case "listUnauthenticated":
            return {
                title: `${name} needs attention`,
                text: `${name} didn't accept the ${credential}, so its pull requests can't be listed. Check ${name} in ${SETTINGS_INTEGRATIONS}.`,
                settings: true,
            }
        case "unauthenticated":
            return {
                title: `${name} didn't accept the ${credential}`,
                text:
                    provider === "github"
                        ? `GitHub refused the token for this pull request. Check GitHub in ${SETTINGS_INTEGRATIONS}.`
                        : `BitBucket refused the credentials for this pull request; the token may lack a read scope it needs. Check BitBucket in ${SETTINGS_INTEGRATIONS}.`,
                settings: true,
            }
        case "listUnavailable":
            return {
                title: `${name} is unavailable`,
                text: `${name}'s pull requests can't be read right now. This one shows once they can.`,
                settings: false,
            }
        case "unavailable":
            return {
                title: "This pull request can't be read",
                text: `${name} says it's unavailable: it may have been moved or deleted, or its repository renamed.`,
                settings: false,
            }
        case "notListed":
            return {
                title: `Not in ${name}'s list`,
                text:
                    provider === "github"
                        ? "SpecForge reads only the open GitHub pull requests you authored or were asked to review, and this isn't one of them."
                        : "SpecForge reads only the open BitBucket pull requests you authored, and this isn't one of them.",
                settings: false,
            }
        case "transient":
            return {
                title: `${name} couldn't be reached`,
                text: context.kept
                    ? `${name} couldn't be reached just now, so this is the last read. Refresh to try again.`
                    : `${name} couldn't be reached just now. Refresh to try again.`,
                settings: false,
            }
        case "deferred":
            return {
                title: "Reading paused",
                text: deferralText(
                    provider,
                    notice.untilUnix,
                    context.nowUnix,
                    context.kept,
                    context.clock,
                ),
                settings: false,
            }
    }
}

/// The time of day a Unix time falls at, as the reader's own clock writes it.
export function clockTime(unixSeconds: number): string {
    return new Date(unixSeconds * 1000).toLocaleTimeString(undefined, {
        hour: "2-digit",
        minute: "2-digit",
    })
}

/// What the view says while a read is deferred: when a read becomes possible,
/// as a time of day rather than a countdown, which would go stale with no
/// timer to advance it, and no timer fires a read at that time either. Once the
/// time has passed it says that a refresh reads, since the next announcement,
/// which also would, may be a while coming. `kept` says whether a detail stays
/// on screen meanwhile (`pull-request-viewer`: *Shared Backoff and Detail
/// Budget*).
export function deferralText(
    provider: PullRequestProvider,
    untilUnix: number,
    nowUnix: number,
    kept: boolean,
    clock: (unixSeconds: number) => string = clockTime,
): string {
    const name = PROVIDER_NAMES[provider]
    const shown = kept ? "Showing the last read. " : ""
    return nowUnix < untilUnix
        ? `${shown}Reading from ${name} is paused until ${clock(untilUnix)}, to stay within its rate limits. Refresh after then to read this pull request.`
        : `${shown}Reading from ${name} was paused until ${clock(untilUnix)}. Refresh to read this pull request now.`
}

// ---- The header ------------------------------------------------------------

/// The row the header takes its title and signals from: the live row while
/// the pull request is listed, and once it is gone, the row its cached detail
/// was read through (`pull-request-viewer`: *Pull-Request View*).
export function shownRow(
    row: PullRequestSummary | null,
    detail: PullRequestDetail | null,
): PullRequestSummary | null {
    return row ?? detail?.row ?? null
}

/// Where the header's "Open on GitHub" or "Open on BitBucket" control leads,
/// and through which desktop command: `snapshot` while the pull request is
/// listed, through `openPullRequest`, which accepts only a URL of the current
/// snapshot; and `link` once it is no longer listed, through
/// `openPullRequestLink` with the URL its cached detail keeps. `null` when
/// neither has a URL. The browser skin links to the same URL either way.
export function providerPage(
    row: PullRequestSummary | null,
    detail: PullRequestDetail | null,
): { url: string; through: "snapshot" | "link" } | null {
    if (row) return row.url === "" ? null : { url: row.url, through: "snapshot" }
    if (detail && detail.row.url !== "") return { url: detail.row.url, through: "link" }
    return null
}

/// The address the center pane's pop-out control opens the window at: the
/// matched row's spelling while the pull request is listed, as the new-window
/// gesture encodes it, so one pull request has one window; and once it is no
/// longer listed, the spelling of the row its cached detail was read through.
export function popOutAddress(
    provider: PullRequestProvider,
    row: PullRequestSummary | null,
    detail: PullRequestDetail | null,
): PullRequestAddress | null {
    if (row) return pullRequestAddressFor(provider, row)
    return detail ? { kind: "pullRequest", ...detail.reference } : null
}

/// The header's review progress: `4 of 10 files viewed`, then, when any file
/// changed since it was viewed, `1 changed since viewed`, dated by the head
/// commit at the last mark (`since you last marked, at abc1234`). The counts
/// are the service's, from the keys alone. `null` when there are no files.
export interface ProgressWords {
    viewed: string
    changed: string | null
    since: string | null
}

export function progressWords(progress: ReviewProgress | null): ProgressWords | null {
    if (!progress || progress.total === 0) return null
    const viewed = `${progress.viewed} of ${progress.total} ${progress.total === 1 ? "file" : "files"} viewed`
    if (progress.changedSinceViewed === 0) return { viewed, changed: null, since: null }
    return {
        viewed,
        changed: `${progress.changedSinceViewed} changed since viewed`,
        since:
            progress.lastMarkedHead === null
                ? null
                : `since you last marked, at ${progress.lastMarkedHead.slice(0, 7)}`,
    }
}

/// The linked change the header names (`pull-request-viewer`: *Linked Change in
/// the Pull-Request View*): the change the panel's worktree marker would land
/// on, by the marker's own rule (`worktreeDestination`), or the first linked
/// worktree's branch alone when it hosts no change to open.
export type LinkedChange =
    | { kind: "change"; name: string; repoId: string; worktreePath: string }
    | { kind: "branch"; name: string }

/// The first linked worktree's one active change, else its most recently
/// modified, else its branch; `null` for a pull request linked to no worktree.
/// `url` is the pull request's web URL, which the links snapshot is keyed by.
export function linkedChange(
    views: WorkspaceView[],
    links: PullRequestLinks | null,
    url: string,
): LinkedChange | null {
    const first = worktreesForPullRequest(links, url)[0]
    if (!first) return null
    const destination = worktreeDestination(views, first.repoId, first.worktreePath)
    return destination?.kind === "artifact"
        ? {
              kind: "change",
              name: destination.changeId,
              repoId: first.repoId,
              worktreePath: first.worktreePath,
          }
        : { kind: "branch", name: worktreeName(first) }
}

// ---- Conversation and checks -----------------------------------------------

const REVIEW_STATE_LABELS: Record<ReviewState, string> = {
    approved: "Approved",
    changesRequested: "Changes requested",
    commented: "Commented",
    dismissed: "Dismissed",
}

/// A review summary's state, as its conversation entry names it.
export function reviewStateLabel(state: ReviewState): string {
    return REVIEW_STATE_LABELS[state]
}

/// What a minimised comment shows in place of its body: GitHub's stated
/// reason, or that it was hidden when GitHub states none.
export function minimisedText(reason: string): string {
    return reason === "" ? "This comment was hidden." : `This comment was marked as ${reason}.`
}

const CHECK_STATE_LABELS: Record<PullRequestCheckState, string> = {
    passing: "Passed",
    failing: "Failed",
    pending: "Pending",
    neutral: "Neutral",
    skipped: "Skipped",
    cancelled: "Cancelled",
    unknown: "Unknown",
}

/// A check's state in words.
export function checkStateLabel(state: PullRequestCheckState): string {
    return CHECK_STATE_LABELS[state]
}

/// The link a check earns: its URL when that parses as an absolute `http` or
/// `https` URL with a host, as a link in pull-request content must, and `null`
/// for anything else — a `file:`, `data:` or `javascript:` URL, a custom
/// scheme, a relative or empty one — which then shows no link at all
/// (*A check link with another scheme opens nothing*).
export function checkLink(url: string | null): string | null {
    if (url === null) return null
    const link = pullRequestHref(url)
    return link.kind === "open" ? link.url : null
}

// ---- Files, threads and progress -------------------------------------------

/// How a review thread names its place after its file: its side and line —
/// `old line 12`, `new lines 10–12`, or `old line 10 to new line 12` for a
/// range across sides — and, for an outdated thread, the line it was written
/// on (`originally new line 12`), since the line it named may no longer exist.
/// A thread with no line is on `the whole file`.
export function threadPlace(thread: ReviewThread): string {
    const line = thread.outdated ? (thread.originalLine ?? thread.line) : thread.line
    if (line === null) return "the whole file"
    const startLine = thread.outdated
        ? (thread.originalStartLine ?? thread.startLine)
        : thread.startLine
    const startSide = thread.startSide ?? thread.side
    const place =
        startLine === null || (startLine === line && startSide === thread.side)
            ? `${thread.side} line ${line}`
            : startSide === thread.side
              ? `${thread.side} lines ${startLine}–${line}`
              : `${startSide} line ${startLine} to ${thread.side} line ${line}`
    return thread.outdated ? `originally ${place}` : place
}

/// Which review threads sit in which listed file's preamble, by the file's
/// key, and which follow the files because their file is not among the listed
/// ones: a file past GitHub's thousandth, or one an outdated thread names that
/// the pull request no longer changes (`pull-request-viewer`: *A thread on an
/// unlisted file follows the files*). A thread matches a file by its new path,
/// else by its old path, so a thread on a renamed or deleted file's old side
/// still sits with it. Threads keep their order.
export interface ThreadSplit {
    byFile: ReadonlyMap<string, readonly ReviewThread[]>
    unlisted: readonly ReviewThread[]
}

export function splitThreads(
    threads: readonly ReviewThread[],
    files: readonly DiffFile[],
): ThreadSplit {
    const keyByPath = new Map<string, string>()
    for (const file of files) {
        if (file.newPath !== null) keyByPath.set(file.newPath, fileKey(file))
    }
    // Old paths second, so a path one file was renamed to keeps its threads
    // even when another file was renamed from it.
    for (const file of files) {
        if (file.oldPath !== null && !keyByPath.has(file.oldPath)) {
            keyByPath.set(file.oldPath, fileKey(file))
        }
    }
    const byFile = new Map<string, ReviewThread[]>()
    const unlisted: ReviewThread[] = []
    for (const thread of threads) {
        const key = keyByPath.get(thread.path)
        if (key === undefined) {
            unlisted.push(thread)
        } else {
            const sitting = byFile.get(key)
            if (sitting) sitting.push(thread)
            else byFile.set(key, [thread])
        }
    }
    return { byFile, unlisted }
}

/// What the view says of the files it does not list.
export function unlistedFilesText(count: number, provider: PullRequestProvider): string | null {
    if (count <= 0) return null
    const files = count === 1 ? "file isn't" : "files aren't"
    return `${count} more changed ${files} listed here. Open the pull request on ${PROVIDER_NAMES[provider]} to see every file.`
}

/// Each file's review progress, by its key path (`newPath ?? oldPath`), which
/// is how the service names it.
export function progressByPath(
    progress: ReviewProgress | null,
): ReadonlyMap<string, FileReviewProgress> {
    return new Map((progress?.files ?? []).map((file) => [file.path, file]))
}

/// Why a file keyed by the head commit counts as changed: it has no patch
/// text to compare, nor a GitHub blob id, so any push or retarget does.
export const KEYED_BY_HEAD_REASON =
    "it has no patch to compare, so any push or retarget counts as a change"

/// A file's viewed mark in its header: whether its toggle shows it viewed,
/// whether it is flagged changed since viewed, and why, when the reason is not
/// its patch (`pull-request-viewer`: *Review Progress*). A file progress has
/// not been read for shows unviewed.
export interface ViewedMark {
    viewed: boolean
    changed: boolean
    reason: string | null
}

export function viewedMark(file: FileReviewProgress | undefined): ViewedMark {
    if (!file) return { viewed: false, changed: false, reason: null }
    const changed = file.state === "changedSinceViewed"
    return {
        viewed: file.state === "viewed",
        changed,
        reason: changed && file.keyedByHead ? KEYED_BY_HEAD_REASON : null,
    }
}
