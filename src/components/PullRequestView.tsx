import {
    useCallback,
    useEffect,
    useLayoutEffect,
    useMemo,
    useRef,
    useState,
    type ReactNode,
} from "react"
import {
    getPullRequestDetail,
    getPullRequestFile,
    getPullRequestFileImage,
    getReviewProgress,
    isWeb,
    openImageWindow,
    onReviewProgressChanged,
    onReviewSkipPatternsChanged,
    openPullRequest as openProviderPage,
    openPullRequestLink,
    openPullRequestWindow,
    setFileIncluded,
    setFileViewed,
    setHunkViewed,
} from "../api"
import { fileKey, isImageFile, isLfsPointerFile } from "../diffFiles"
import { imageWindowAddress, imageWindowTitle, type ImageWindowSource } from "../imageWindow"
import { hunkFirstLine } from "../diffLayout"
import { pullRequestTitle } from "../pullRequestOpen"
import {
    answeredRead,
    askedRead,
    checkLink,
    checkStateLabel,
    FILE_CHANGED_TEXT,
    fileFailureText,
    fileViewedIn,
    endRowFor,
    hostFileLink,
    hunkMarkLabel,
    hunkStates,
    imageReadLinksToHost,
    linkedChange,
    minimisedText,
    newlySkipped,
    NO_READ,
    noticeOffersRefresh,
    noticeWords,
    nowUnix,
    popOutAddress,
    progressByPath,
    progressWords,
    providerPage,
    PROVIDER_NAMES,
    readFor,
    referenceKey,
    resolutionNotice,
    reviewStateLabel,
    SETTINGS_INTEGRATIONS,
    shownRow,
    skipMark,
    splitThreads,
    threadPlace,
    unlistedFilesText,
    viewedMark,
    type LinkedChange,
    type NoticeWords,
    type ReadTrigger,
    type SkipMark,
    type ViewedMark,
    type ViewRead,
} from "../pullRequestView"
import { referenceOf, sameReference, type PullRequestAddress } from "../routing/address"
import { encodeAddress } from "../routing/codec"
import type { PullRequestResolution } from "../routing/resolve"
import type {
    DiffFile,
    ImageVersions,
    PullRequestCheck,
    PullRequestComment,
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
} from "../types"
import { DiffView, EscapedText, type DiffViewHandle, type HunkSlots } from "./DiffView"
import { IdentityTrailing } from "./DocumentView"
import { EmptyState } from "./EmptyState"
import { prettifyError } from "./errors"
import { MarkdownView } from "./MarkdownView"
import { OpenReaderControl } from "./OpenReaderControl"
import { PullRequestMarkers, PullRequestSignalCells } from "./PullRequestPanel"
import { RelativeTime } from "./RelativeTime"

/// What only the center pane offers, which the pull-request window does not:
/// the pop-out control, the linked change as a way to it, and a link to
/// Settings › Integrations. The window is already detached, and it cannot
/// navigate the main window, so there the linked change is passive text and
/// the notices name Settings › Integrations as text (`pull-request-viewer`:
/// *Pull-Request View*, *Linked Change in the Pull-Request View*,
/// *Pull-Request Window*).
export interface PaneNavigation {
    /// App's `openWorktree`, which lands where the panel's worktree marker
    /// does, through `go`.
    openWorktree: (repoId: string, worktreePath: string) => void
    /// Settings › Integrations, through `go`.
    openSettings: () => void
}

export interface PullRequestViewProps {
    /// The pull request: the matched row's spelling while it is listed, else
    /// the address as it was spelt.
    address: PullRequestAddress
    /// Its row in its provider's snapshot, while the snapshot lists it; null
    /// when it does not.
    row: PullRequestSummary | null
    views: WorkspaceView[]
    links: PullRequestLinks | null
    /// The center pane's navigation; absent in the pull-request window.
    pane?: PaneNavigation
    /// Told the pull request's title as the view shows it, for the window's.
    onTitle?: (title: string | null) => void
}

/// A pull-request address as its resolution has it, the same in the center
/// pane and in the pull-request window (`view-routing`: *Pull-Request
/// Addresses*, *Cold-Load Address Resolution*; `pull-request-viewer`:
/// *Pull-Request Window*): "Loading…" while pending, never another surface; a
/// notice when its provider is off or its list cannot be read; and the view
/// when it is listed, or when it is not, where the view asks once for the
/// pull request's cached detail and reports it not listed when there is none.
///
/// The view is keyed by the pull request, so another one starts afresh, while
/// an address respelt in another case keeps the view on screen as it is.
export function PullRequestAtAddress({
    address,
    resolution,
    views,
    links,
    pane,
    onTitle,
}: {
    address: PullRequestAddress
    resolution: PullRequestResolution
    views: WorkspaceView[]
    links: PullRequestLinks | null
    pane?: PaneNavigation
    onTitle?: (title: string | null) => void
}) {
    switch (resolution.status) {
        case "pending":
            return <div className="detail-pane-status">Loading…</div>
        case "providerOff":
        case "unavailable":
            return (
                <PullRequestResolutionNotice
                    provider={address.provider}
                    resolution={resolution}
                    onOpenSettings={pane?.openSettings}
                />
            )
        case "listed":
        case "notListed": {
            const listed = resolution.status === "listed" ? resolution : null
            return (
                <PullRequestView
                    key={referenceKey(address)}
                    address={listed ? listed.address : address}
                    row={listed ? listed.row : null}
                    views={views}
                    links={links}
                    pane={pane}
                    onTitle={onTitle}
                />
            )
        }
    }
}

const OPEN: ReadTrigger = { kind: "open" }
const FILE_REFUSED: ReadTrigger = { kind: "fileRefused" }
const MANUAL: ReadTrigger = { kind: "manual" }
const TRANSIENT: PullRequestDetailOutcome = { kind: "transient" }

/// One pull request, rendered the same in the center pane and in its own
/// window from one detail (`pull-request-viewer`: *Pull-Request View*): a
/// header naming it, with its row's signals, its linked change and its review
/// progress; its description and conversation as untrusted content; its
/// checks; and its changed files through the one diff renderer, with review
/// threads above their files and a viewed mark in each file's header.
///
/// It reads only by the freshness rule, never on a timer, and keeps what it
/// shows on screen through a re-read. Nothing in it posts, approves, merges or
/// marks anything on either host: a viewed mark is this machine's alone.
///
/// The caller keys it by `referenceKey`, so another pull request starts afresh
/// while a respelt address keeps the view as it is.
export function PullRequestView({
    address,
    row,
    views,
    links,
    pane,
    onTitle,
}: PullRequestViewProps) {
    const { provider } = address
    const reference = useMemo(
        () => referenceOf(address),
        // eslint-disable-next-line react-hooks/exhaustive-deps -- the
        // address's fields, not the object a caller may rebuild per render.
        [address.provider, address.owner, address.repo, address.number],
    )
    const { read, reading, refresh, fileRefused } = usePullRequestRead(reference, row)
    const { detail, notice } = read
    const [progress, reloadProgress] = useReviewProgress(reference, detail)

    const headed = shownRow(row, detail)
    const title = headed?.title ?? null
    useEffect(() => {
        onTitle?.(title)
    }, [title, onTitle])

    const linked = useMemo(
        () => (headed ? linkedChange(views, links, headed.url) : null),
        [views, links, headed],
    )

    // Publish the header's height, so each file's header pins just below it
    // rather than behind it, wherever the header wraps. Written imperatively,
    // as `DocumentView` publishes its own, since the height changes with the
    // pane's width and is not worth a render.
    const rootRef = useRef<HTMLDivElement>(null)
    const headerRef = useRef<HTMLDivElement>(null)
    const hasHeader = headed !== null
    useLayoutEffect(() => {
        const root = rootRef.current
        const header = headerRef.current
        if (!root) return
        const publish = () => {
            root.style.setProperty("--diff-sticky-top", `${header?.offsetHeight ?? 0}px`)
        }
        publish()
        if (!header || typeof ResizeObserver === "undefined") return
        const observer = new ResizeObserver(publish)
        observer.observe(header)
        return () => observer.disconnect()
    }, [hasHeader])

    const words = notice
        ? noticeWords(provider, notice, { kept: detail !== null, nowUnix: nowUnix() })
        : null

    // In place of the detail while there is none: the notice an answer left,
    // or "Loading…" until the first answer lands.
    const placeholder =
        notice && words ? (
            <NoticeBlock
                words={words}
                onOpenSettings={pane?.openSettings}
                onRefresh={noticeOffersRefresh(notice) ? refresh : undefined}
            />
        ) : (
            <div className="detail-pane-status">Loading…</div>
        )

    if (!headed) {
        // Nothing to head the view with: not listed, and nothing cached.
        return (
            <div className="detail-pane pull-request-view" ref={rootRef}>
                {placeholder}
            </div>
        )
    }

    const page = providerPage(row, detail)
    const popOut = pane ? popOutAddress(provider, row, detail) : null
    const reviewed = progressWords(progress)
    const head = detail?.headBranch ?? headed.sourceBranch
    const base = detail?.baseBranch ?? headed.destinationBranch
    const author = detail?.author ?? headed.author

    return (
        <div className="detail-pane pull-request-view" ref={rootRef}>
            {/* The `.detail-identity` the change header is, so in the macOS
                main window it takes the same titlebar clearance as a direct
                child of the center pane's `.detail-pane`, and the pop-out
                control is revealed on its hover as the reader control is. */}
            <div className="detail-identity pull-request-view-header" ref={headerRef}>
                <div className="detail-identity-inner pull-request-view-identity">
                    <h1 className="pull-request-view-title">
                        <span className="pull-request-view-number">#{reference.number}</span>{" "}
                        <EscapedText text={headed.title} />
                    </h1>
                    <IdentityTrailing>
                        <button
                            type="button"
                            className="pull-request-view-action"
                            onClick={refresh}
                            aria-busy={reading || undefined}
                        >
                            {reading ? "Refreshing…" : "Refresh"}
                        </button>
                        {page && (
                            <ProviderControl provider={provider} page={page} reference={reference} />
                        )}
                        {popOut && (
                            <OpenReaderControl
                                onClick={() =>
                                    openPullRequestWindow(
                                        encodeAddress(popOut),
                                        pullRequestTitle(referenceOf(popOut), title),
                                    )
                                }
                            />
                        )}
                    </IdentityTrailing>
                </div>
                <div className="detail-identity-inner pull-request-view-meta">
                    <span className="pull-request-view-repo">
                        {headed.repoFullName || `${reference.owner}/${reference.repo}`}
                    </span>
                    <span className="pull-request-view-branches">
                        <EscapedText text={head} /> → <EscapedText text={base} />
                    </span>
                    {author && <span className="pull-request-view-author">by {author}</span>}
                    <PullRequestMarkers pr={headed} />
                    <PullRequestSignalCells pr={headed} />
                    {(row === null || detail?.noLongerListed) && (
                        <span
                            className="identity-missing"
                            title={`This pull request has left ${PROVIDER_NAMES[provider]}'s list. Its last read is still shown.`}
                        >
                            no longer listed
                        </span>
                    )}
                    {linked && <LinkedChangeName linked={linked} pane={pane} />}
                    {reviewed && (
                        <span className="pull-request-view-progress">
                            {reviewed.viewed}
                            {reviewed.skipped && (
                                <>
                                    {" · "}
                                    <span className="pull-request-view-progress-skipped">
                                        {reviewed.skipped}
                                    </span>
                                </>
                            )}
                            {reviewed.changed && (
                                <>
                                    {" · "}
                                    <span className="pull-request-view-progress-changed">
                                        {reviewed.changed}
                                    </span>
                                    {reviewed.since && <> ({reviewed.since})</>}
                                </>
                            )}
                        </span>
                    )}
                </div>
            </div>

            {detail === null ? (
                placeholder
            ) : (
                <>
                    <div className="pull-request-view-body">
                        {words && (
                            <p className="pull-request-view-banner" role="status">
                                {words.text}
                            </p>
                        )}
                        <section className="pull-request-view-section">
                            <h2 className="pull-request-view-heading">Description</h2>
                            {detail.description.trim() === "" ? (
                                <p className="pull-request-view-empty">No description.</p>
                            ) : (
                                <MarkdownView content={detail.description} pullRequest={reference} />
                            )}
                        </section>
                        {detail.conversation.length > 0 && (
                            <section className="pull-request-view-section">
                                <h2 className="pull-request-view-heading">Conversation</h2>
                                <ol className="pull-request-view-entries">
                                    {detail.conversation.map((entry) => (
                                        <li key={entry.comment.id}>
                                            <CommentBlock
                                                comment={entry.comment}
                                                review={entry.review}
                                                reference={reference}
                                            />
                                        </li>
                                    ))}
                                </ol>
                            </section>
                        )}
                        {detail.checks.length > 0 && (
                            <section className="pull-request-view-section">
                                <h2 className="pull-request-view-heading">Checks</h2>
                                <ul className="pull-request-view-checks">
                                    {detail.checks.map((check, index) => (
                                        <CheckRow
                                            key={`${index}:${check.name}`}
                                            check={check}
                                            reference={reference}
                                        />
                                    ))}
                                </ul>
                            </section>
                        )}
                    </div>
                    <PullRequestFiles
                        detail={detail}
                        reference={reference}
                        progress={progress}
                        reloadProgress={reloadProgress}
                        onFileRefused={fileRefused}
                    />
                </>
            )}
        </div>
    )
}

// ---- Reads -----------------------------------------------------------------

/// The view's reads (`pull-request-viewer`: *Detail Reads Are Scoped to the
/// Snapshot*, *Shared Backoff and Detail Budget*). On opening a listed pull
/// request it paints whatever a cache-only call returns, then asks for a read
/// under the freshness rule; one that is not listed asks once, which the
/// service answers from its cache alone. After that it asks only when an
/// announcement calls for a read by `readFor`'s rule, when a withheld file is
/// refused, and on a manual refresh — never on a timer. What is on screen stays
/// through every ask until its answer lands, and an answer overtaken by a later
/// ask's is dropped, so an older read never replaces a newer one.
function usePullRequestRead(reference: PullRequestReference, row: PullRequestSummary | null) {
    const [read, setRead] = useState<ViewRead>(NO_READ)
    const [inFlight, setInFlight] = useState(0)

    // Mirrors for the callbacks and effects below, which outlive any render.
    const readRef = useRef(read)
    readRef.current = read
    const referenceRef = useRef(reference)
    referenceRef.current = reference
    const rowRef = useRef(row)
    rowRef.current = row
    // Each ask's number, and the latest whose answer is on screen.
    const askedRef = useRef(0)
    const shownRef = useRef(0)
    const mountedRef = useRef(false)
    useEffect(() => {
        mountedRef.current = true
        return () => {
            mountedRef.current = false
        }
    }, [])

    const ask = useCallback((call: { manual: boolean }, cachedOnly = false): Promise<void> => {
        const asked = ++askedRef.current
        if (!cachedOnly) setRead((current) => askedRead(current, rowRef.current))
        setInFlight((n) => n + 1)
        const settle = (outcome: PullRequestDetailOutcome) => {
            if (!mountedRef.current || asked < shownRef.current) return
            shownRef.current = asked
            setRead((current) => answeredRead(current, outcome))
        }
        return getPullRequestDetail(referenceRef.current, call.manual, cachedOnly)
            .then(settle, (err: unknown) => {
                // Only the transport failing rejects; the service answers every
                // provider failure as an outcome.
                console.warn("failed to read the pull request", err)
                settle(TRANSIENT)
            })
            .finally(() => {
                if (mountedRef.current) setInFlight((n) => n - 1)
            })
    }, [])

    // Opening, and the pull request entering or leaving its list, which
    // re-opens it in the other state. Once per state, so a development build's
    // second run of every effect asks nothing twice.
    const listed = row !== null
    const openedRef = useRef<boolean | null>(null)
    useEffect(() => {
        if (openedRef.current === listed) return
        openedRef.current = listed
        // Marked asked at once, so an announcement landing while the paint is
        // out asks nothing a second time.
        setRead((current) => askedRead(current, rowRef.current))
        const call = readFor(readRef.current, OPEN)
        if (call && listed) {
            void ask(call, true).then(() => {
                if (mountedRef.current) void ask(call)
            })
        } else if (call) {
            void ask(call)
        }
    }, [listed, ask])

    // Each of its provider's announcements re-reads the snapshot, so a new row
    // object is a new announcement. The row the view opened with is not one,
    // and neither is a row appearing, which the effect above handles.
    const announcedRef = useRef(row)
    useEffect(() => {
        const previous = announcedRef.current
        if (row === previous) return
        announcedRef.current = row
        if (row === null || previous === null) return
        const call = readFor(readRef.current, { kind: "announcement", row, nowUnix: nowUnix() })
        if (call) void ask(call)
    }, [row, ask])

    const refresh = useCallback(() => {
        const call = readFor(readRef.current, MANUAL)
        if (call) void ask(call)
    }, [ask])

    const fileRefused = useCallback(() => {
        const call = readFor(readRef.current, FILE_REFUSED)
        if (call) void ask(call)
    }, [ask])

    return { read, reading: inFlight > 0, refresh, fileRefused }
}

/// The pull request's review progress on this machine, read for each detail
/// the view shows and again whenever a mark anywhere in the service changes it
/// — in this view, another window, or a browser tab — compared ignoring ASCII
/// case (`pull-request-viewer`: *Review Progress*), and whenever the skip
/// patterns change, which can change every pull request's (*Review Skip
/// Patterns*). Null until read, and when it cannot be. The reload resolves
/// once its answer is applied, with what it read (null when it could not), so
/// a mark can tell whether it completed its file.
function useReviewProgress(
    reference: PullRequestReference,
    detail: PullRequestDetail | null,
): [ReviewProgress | null, () => Promise<ReviewProgress | null>] {
    const [progress, setProgress] = useState<ReviewProgress | null>(null)
    const referenceRef = useRef(reference)
    referenceRef.current = reference
    const askedRef = useRef(0)

    const reload = useCallback((): Promise<ReviewProgress | null> => {
        const asked = ++askedRef.current
        return getReviewProgress(referenceRef.current).then(
            (next) => {
                if (asked === askedRef.current) setProgress(next)
                return next
            },
            () => {
                if (asked === askedRef.current) setProgress(null)
                return null
            },
        )
    }, [])

    useEffect(() => {
        if (detail) void reload()
    }, [detail, reload])

    useEffect(() => {
        const unlisten = onReviewProgressChanged((changed) => {
            // An unparseable SSE frame arrives as `undefined`.
            if (changed && sameReference(changed, referenceRef.current)) void reload()
        })
        return () => {
            void unlisten.then((off) => off())
        }
    }, [reload])

    // A new list skips other files of every pull request, so its payload is
    // not read: the progress is, again.
    useEffect(() => {
        const unlisten = onReviewSkipPatternsChanged(() => void reload())
        return () => {
            void unlisten.then((off) => off())
        }
    }, [reload])

    return [progress, reload]
}

/// A rejection's words, from either transport.
function errorText(err: unknown): string {
    return err instanceof Error ? err.message : prettifyError(err)
}

// ---- Header pieces -----------------------------------------------------------

/// "Open on GitHub" or "Open on BitBucket": the pull request's own page,
/// outside SpecForge. On the desktop through `openPullRequest` while it is
/// listed, and through `openPullRequestLink` with its cached detail's URL once
/// it is not; in the browser skin an opener-isolated new tab. A plain control,
/// visible at rest on every device (`pull-request-viewer`: *The provider's page
/// opens from the header*).
function ProviderControl({
    provider,
    page,
    reference,
}: {
    provider: PullRequestProvider
    page: { url: string; through: "snapshot" | "link" }
    reference: PullRequestReference
}) {
    const label = `Open on ${PROVIDER_NAMES[provider]}`
    if (isWeb()) {
        return (
            <a
                className="pull-request-view-action"
                href={page.url}
                target="_blank"
                rel="noopener noreferrer"
            >
                {label}
            </a>
        )
    }
    const open = () => {
        const opening =
            page.through === "snapshot"
                ? openProviderPage(page.url)
                : openPullRequestLink(reference, page.url)
        opening.catch((err: unknown) => console.warn(`failed to open ${page.url}`, err))
    }
    return (
        <button type="button" className="pull-request-view-action" onClick={open}>
            {label}
        </button>
    )
}

/// The change the panel's worktree marker would land on, or the worktree's
/// branch when it hosts none. In the center pane the change's name navigates
/// there, adding a history entry, so Back returns to the pull request; in the
/// window it is passive text that navigates nothing and opens no reader
/// (`pull-request-viewer`: *Linked Change in the Pull-Request View*).
function LinkedChangeName({ linked, pane }: { linked: LinkedChange; pane?: PaneNavigation }) {
    if (linked.kind === "branch") {
        return (
            <span className="pull-request-view-linked">
                <span className="pull-request-view-linked-label">Worktree</span>{" "}
                <span className="pull-request-view-linked-name">{linked.name}</span>
            </span>
        )
    }
    return (
        <span className="pull-request-view-linked">
            <span className="pull-request-view-linked-label">Change</span>{" "}
            {pane ? (
                <button
                    type="button"
                    className="pull-request-view-linked-name pull-request-view-linked-name--control"
                    onClick={() => pane.openWorktree(linked.repoId, linked.worktreePath)}
                >
                    {linked.name}
                </button>
            ) : (
                <span className="pull-request-view-linked-name">{linked.name}</span>
            )}
        </span>
    )
}

// ---- Notices -----------------------------------------------------------------

/// A notice in place of the pull request. One the reader can act on in
/// Settings names Settings › Integrations in its words, and the center pane,
/// which passes `onOpenSettings`, also links there through `go`, adding a
/// history entry; the pull-request window leaves the name as text.
function NoticeBlock({
    words,
    onOpenSettings,
    onRefresh,
}: {
    words: NoticeWords
    onOpenSettings?: () => void
    onRefresh?: () => void
}) {
    const settingsLink = words.settings ? onOpenSettings : undefined
    return (
        <EmptyState
            title={words.title}
            body={
                <>
                    <p>{words.text}</p>
                    {(settingsLink || onRefresh) && (
                        <div className="disabled-notice-actions">
                            {settingsLink && (
                                <button className="archive-back" onClick={settingsLink}>
                                    Open {SETTINGS_INTEGRATIONS}
                                </button>
                            )}
                            {onRefresh && (
                                <button className="archive-back" onClick={onRefresh}>
                                    Refresh
                                </button>
                            )}
                        </div>
                    )}
                </>
            }
        />
    )
}

/// What a pull-request address that resolved to no pull request shows: its
/// provider switched off, or its list unauthenticated or unavailable
/// (`view-routing`: *Pull-Request Addresses*). The center pane passes
/// `onOpenSettings`, which links to Settings › Integrations; the pull-request
/// window does not, and its words name the place as text.
function PullRequestResolutionNotice({
    provider,
    resolution,
    onOpenSettings,
}: {
    provider: PullRequestProvider
    resolution: PullRequestResolution
    onOpenSettings?: () => void
}) {
    const notice = resolutionNotice(resolution)
    if (!notice) return null
    return (
        <NoticeBlock
            words={noticeWords(provider, notice, { kept: false, nowUnix: nowUnix() })}
            onOpenSettings={onOpenSettings}
        />
    )
}

// ---- Conversation and checks ---------------------------------------------------

/// One comment or review summary: who, the review's state for a summary, when,
/// and its body as untrusted content. A comment GitHub reports as minimised is
/// collapsed behind its stated reason, as a conversation entry and as a thread
/// comment alike (`pull-request-viewer`: *A minimised comment stays collapsed*,
/// *A minimised thread comment stays collapsed*).
function CommentBlock({
    comment,
    review,
    reference,
}: {
    comment: PullRequestComment
    review: ReviewState | null
    reference: PullRequestReference
}) {
    return (
        <article className="pull-request-view-comment">
            <div className="pull-request-view-comment-head">
                <span className="pull-request-view-comment-author">
                    {comment.author ?? "Deleted account"}
                </span>
                {review && (
                    <span
                        className={`pull-request-view-review-state pull-request-view-review-state--${review}`}
                    >
                        {reviewStateLabel(review)}
                    </span>
                )}
                {comment.postedAtUnix > 0 && (
                    <span
                        className="pull-request-view-comment-time"
                        title={new Date(comment.postedAtUnix * 1000).toLocaleString()}
                    >
                        <RelativeTime unixSeconds={comment.postedAtUnix} />
                    </span>
                )}
            </div>
            <CommentBody comment={comment} reference={reference} />
        </article>
    )
}

function CommentBody({
    comment,
    reference,
}: {
    comment: PullRequestComment
    reference: PullRequestReference
}) {
    if (comment.deleted) {
        return <p className="pull-request-view-empty">This comment was deleted.</p>
    }
    const body = <MarkdownView content={comment.body} pullRequest={reference} />
    if (comment.minimizedReason === null) return body
    return (
        <details className="pull-request-view-minimised">
            <summary>{minimisedText(comment.minimizedReason)}</summary>
            {body}
        </details>
    )
}

/// One check: its name and its state, and a link to its page only when its
/// URL is an absolute `http` or `https` URL with a host
/// (`pull-request-viewer`: *Checks link out*, *A check link with another
/// scheme opens nothing*).
function CheckRow({
    check,
    reference,
}: {
    check: PullRequestCheck
    reference: PullRequestReference
}) {
    const link = checkLink(check.url)
    return (
        <li className="pull-request-view-check">
            <span
                className={`pull-request-view-check-dot pull-request-view-check-dot--${check.state}`}
                aria-hidden="true"
            />
            <span className="pull-request-view-check-name">{check.name}</span>
            <span className="pull-request-view-check-state">{checkStateLabel(check.state)}</span>
            {link && (
                <ContentLink href={link} reference={reference}>
                    Details
                </ContentLink>
            )}
        </li>
    )
}

/// A link out of pull-request content, as the pull-request mode opens its own:
/// through `openPullRequestLink` on the desktop, never navigating the window,
/// and as an opener-isolated new tab in the browser skin (`pull-request-viewer`:
/// *Desktop Link Opener*). `href` has already passed `checkLink`.
function ContentLink({
    href,
    reference,
    children,
}: {
    href: string
    reference: PullRequestReference
    children: ReactNode
}) {
    if (isWeb()) {
        return (
            <a
                className="markdown-link markdown-link--external"
                href={href}
                target="_blank"
                rel="noopener noreferrer"
            >
                {children}
            </a>
        )
    }
    return (
        <a
            className="markdown-link markdown-link--external"
            href={href}
            onClick={(event) => {
                event.preventDefault()
                openPullRequestLink(reference, href).catch((err: unknown) =>
                    console.warn(`failed to open ${href}`, err),
                )
            }}
        >
            {children}
        </a>
    )
}

// ---- Files -------------------------------------------------------------------

/// The changed files through `DiffView` (`pull-request-viewer`: *Changed Files
/// in the Pull-Request View*): the detail's files, named by the base and head
/// branches, in the layout commit detail uses; each file's review threads in
/// its preamble and its viewed mark in its header; each hunk's mark in its
/// heading, a long one's again at its end, every viewed hunk folded; and the
/// threads on files the detail does not list after the files.
///
/// Keyed by the pull request, so a re-read re-renders the diff rather than
/// remounting it, and the layout, collapse and loaded files the reader has
/// stay as they are.
function PullRequestFiles({
    detail,
    reference,
    progress,
    reloadProgress,
    onFileRefused,
}: {
    detail: PullRequestDetail
    reference: PullRequestReference
    progress: ReviewProgress | null
    reloadProgress: () => Promise<ReviewProgress | null>
    onFileRefused: () => void
}) {
    const { headCommit: head, baseCommit: base } = detail
    const sideNames = useMemo(
        () => ({ old: detail.baseBranch, new: detail.headBranch }),
        [detail.baseBranch, detail.headBranch],
    )

    // With the commits the view rendered, so a file read since a push is
    // answered `changed` rather than shown against the wrong detail, and only
    // then does the view read the pull request again (*A withheld-file
    // request after a push is refused*). A load that could not complete says
    // why and leaves "Load diff" to try again (*A load that cannot complete
    // does not read the pull request again*). `DiffView` shows a rejection as
    // it is given, so it gets the words alone. A file that loads brings hunks
    // a file read may only now have keyed, so progress is read again (*A
    // loaded file gets its hunk marks*).
    const loadFile = useCallback(
        (file: DiffFile) =>
            getPullRequestFile(reference, fileKey(file), head, base).then(
                (outcome) => {
                    switch (outcome.kind) {
                        case "file":
                            void reloadProgress()
                            return outcome.file
                        case "changed":
                            onFileRefused()
                            throw FILE_CHANGED_TEXT
                        case "failed":
                            throw fileFailureText(
                                reference.provider,
                                outcome.reason,
                                outcome.untilUnix,
                            )
                    }
                },
                (err: unknown) => {
                    throw errorText(err)
                },
            ),
        [reference, head, base, onFileRefused, reloadProgress],
    )

    // The image files that link to their diff on the host once read: a
    // refused or undrawable version, or a read failed as `redirected` or
    // `unavailable`. They belong to the files they were read for, as
    // `DiffView`'s reads do.
    const [hostLinked, setHostLinked] = useState(() => ({
        files: detail.files,
        keys: NO_KEYS,
    }))
    const linked = hostLinked.files === detail.files ? hostLinked.keys : NO_KEYS
    const linkToHost = useCallback(
        (file: DiffFile) => {
            const files = detail.files
            setHostLinked((previous) => {
                const keys = previous.files === files ? previous.keys : NO_KEYS
                if (keys.has(fileKey(file))) return previous
                return { files, keys: new Set(keys).add(fileKey(file)) }
            })
        },
        [detail.files],
    )

    // An image file's versions, read when the reader activates "Show image",
    // with the commits the view rendered (*Pull-Request Image Reads*). Answered
    // as a file's load is: `changed` reads the pull request again, and a
    // failure says why beside "Show image". A read brings no hunks, so the
    // progress is not read again after one.
    const readImage = useCallback(
        (file: DiffFile) =>
            getPullRequestFileImage(reference, fileKey(file), head, base).then(
                (outcome): ImageVersions => {
                    if (imageReadLinksToHost(outcome)) linkToHost(file)
                    switch (outcome.kind) {
                        case "images":
                            return { old: outcome.old, new: outcome.new }
                        case "changed":
                            onFileRefused()
                            throw FILE_CHANGED_TEXT
                        case "failed":
                            throw fileFailureText(
                                reference.provider,
                                outcome.reason,
                                outcome.untilUnix,
                            )
                    }
                },
                (err: unknown) => {
                    throw errorText(err)
                },
            ),
        [reference, head, base, onFileRefused, linkToHost],
    )

    // Zoom opens the file's versions in a window of their own, which reads
    // them again as an image read of this detail's commits.
    const zoomImage = useCallback(
        (file: DiffFile) => {
            const source: ImageWindowSource = {
                kind: "pullRequest",
                reference,
                path: fileKey(file),
                head,
                base,
                sides: sideNames,
            }
            openImageWindow(imageWindowAddress(source), imageWindowTitle(source))
        },
        [reference, head, base, sideNames],
    )

    const split = useMemo(
        () => splitThreads(detail.threads, detail.files),
        [detail.threads, detail.files],
    )
    const byPath = useMemo(() => progressByPath(progress), [progress])
    // A mark the reader makes here that leaves its file viewed collapses that
    // file, once the progress after it says so (*Completing a file*).
    const diffRef = useRef<DiffViewHandle>(null)
    const completed = useCallback((file: DiffFile) => diffRef.current?.collapse(file), [])
    const { pending, hunkPending, includePending, failed, markFile, markHunk, includeFile } =
        useReviewMarks(reference, detail, reloadProgress, completed)

    // Each file a progress read shows skipped that the read before it did not
    // collapses, once, before it paints (`review-skip-patterns` design D10):
    // on opening, after the patterns change, and after a Skip, here or in
    // another view. Compared by path with the last read the files section
    // had, whichever detail each was for, so a file a push leaves skipped
    // keeps whatever the reader made of its section; and a failed read, which
    // leaves no progress, is no read to compare with. Nothing ever expands a
    // section here: the reader's own toggle does.
    const lastProgressRef = useRef<ReviewProgress | null>(null)
    useLayoutEffect(() => {
        if (progress === null) return
        const previous = lastProgressRef.current
        lastProgressRef.current = progress
        const paths = new Set(newlySkipped(previous, progress))
        if (paths.size === 0) return
        for (const file of detail.files) {
            if (paths.has(fileKey(file))) diffRef.current?.collapse(file)
        }
    }, [progress, detail.files])

    const renderFileHeaderExtra = useCallback(
        (file: DiffFile) => {
            const key = fileKey(file)
            const fileProgress = byPath.get(key)
            return (
                <ViewedToggle
                    path={key}
                    mark={viewedMark(fileProgress, hunkStates(progress, detail, key))}
                    skip={skipMark(fileProgress)}
                    pending={pending.get(key)}
                    including={includePending.get(key)}
                    error={failed.get(key)}
                    onChange={(viewed) => markFile(file, viewed)}
                    onInclude={(included) => includeFile(file, included)}
                />
            )
        },
        [byPath, progress, detail, pending, includePending, failed, markFile, includeFile],
    )

    // Each hunk's mark, for a file whose hunk states the progress gives for
    // this detail: a checkbox in its heading, a "Mark hunk viewed" row ending
    // a long unviewed one, and every viewed hunk folded. A mark in flight, of
    // the hunk or of its whole file, shows its new state at once.
    const hunkSlots = useCallback(
        (file: DiffFile): HunkSlots | undefined => {
            const key = fileKey(file)
            const states = hunkStates(progress, detail, key)
            if (states === null) return undefined
            const whole = pending.get(key)
            const viewedAt = (index: number) =>
                hunkPending.get(hunkMarkKey(key, index)) ?? whole ?? states[index] === true
            const folded = new Set(states.flatMap((_, index) => (viewedAt(index) ? [index] : [])))
            return {
                folded,
                heading: (index, hunk) =>
                    index < states.length && (
                        <HunkToggle
                            label={hunkMarkLabel(key, hunk)}
                            viewed={viewedAt(index)}
                            busy={hunkPending.has(hunkMarkKey(key, index)) || whole !== undefined}
                            onChange={(viewed) => markHunk(file, index, viewed)}
                        />
                    ),
                end: (index, hunk) =>
                    index < states.length &&
                    endRowFor(hunk.lines.length, viewedAt(index)) && (
                        <button
                            type="button"
                            className="diff-hunk-action"
                            aria-label={`Mark hunk viewed: ${key}, from ${hunkFirstLine(hunk)}`}
                            onClick={() => markHunk(file, index, true)}
                        >
                            Mark hunk viewed
                        </button>
                    ),
            }
        },
        [progress, detail, pending, hunkPending, markHunk],
    )

    // A file too large to preview links to its diff on the host, above its
    // threads (*A too-large file links to its diff on the host*), as does an
    // image file stored in Git LFS or one whose versions cannot all be shown.
    const page = detail.row.url
    const renderFilePreamble = useCallback(
        (file: DiffFile) => {
            const key = fileKey(file)
            const threads = split.byFile.get(key)
            const image = isImageFile(file) && (isLfsPointerFile(file) || linked.has(key))
            const host =
                file.content.kind === "tooLarge" || image
                    ? hostFileLink(reference.provider, page, file)
                    : null
            if (!threads && !host) return null
            return (
                <>
                    {host && (
                        <p className="pull-request-view-host-file">
                            <ContentLink href={host} reference={reference}>
                                View this file on {PROVIDER_NAMES[reference.provider]}
                            </ContentLink>
                        </p>
                    )}
                    {threads && <ReviewThreads threads={threads} reference={reference} />}
                </>
            )
        },
        [split, reference, page, linked],
    )

    const unlisted = unlistedFilesText(detail.unlistedFiles, reference.provider)
    const total = detail.files.length + detail.unlistedFiles

    return (
        <section className="pull-request-view-files">
            <h2 className="pull-request-view-heading">
                Files changed{" "}
                <span className="pull-request-view-count">{total}</span>
            </h2>
            {unlisted && <p className="pull-request-view-note">{unlisted}</p>}
            {detail.files.length === 0 ? (
                <p className="pull-request-view-empty">This pull request changes no files.</p>
            ) : (
                <DiffView
                    key={referenceKey(reference)}
                    files={detail.files}
                    sideNames={sideNames}
                    loadFile={loadFile}
                    renderFileHeaderExtra={renderFileHeaderExtra}
                    renderFilePreamble={renderFilePreamble}
                    hunkSlots={hunkSlots}
                    readImage={readImage}
                    imageReads="onRequest"
                    onImageUndrawable={linkToHost}
                    zoomImage={zoomImage}
                    ref={diffRef}
                />
            )}
            {split.unlisted.length > 0 && (
                <section className="pull-request-view-unlisted-threads">
                    <h3 className="pull-request-view-subheading">Comments on files not listed</h3>
                    <ReviewThreads threads={split.unlisted} reference={reference} />
                </section>
            )}
        </section>
    )
}

const NO_PENDING: ReadonlyMap<string, boolean> = new Map()
const NO_FAILURES: ReadonlyMap<string, string> = new Map()
const NO_KEYS: ReadonlySet<string> = new Set()

function without<V>(map: ReadonlyMap<string, V>, key: string): ReadonlyMap<string, V> {
    if (!map.has(key)) return map
    const next = new Map(map)
    next.delete(key)
    return next
}

/// A hunk's key among the marks in flight.
function hunkMarkKey(path: string, index: number): string {
    return `${path}\u0000${index}`
}

/// Marks and unmarks files, and hunks of them, viewed through the service,
/// and includes files the skip patterns match in the review or excludes them
/// again, naming the head and base commits of the detail the view rendered;
/// the service computes every key from its cached detail
/// (`pull-request-viewer`: *Review Progress*). A file or hunk being marked
/// shows its new state until the progress that follows the mark lands, and a
/// file being included shows its control busy. A refused mark or inclusion
/// says why in the file's header until the file or one of its hunks is marked
/// or included again or a new detail arrives, and asks for no read. When the
/// progress that follows a mark the reader made here shows its file viewed,
/// `completed` is told, and nothing else ever tells it: an inclusion never
/// completes a file.
function useReviewMarks(
    reference: PullRequestReference,
    detail: PullRequestDetail,
    reloadProgress: () => Promise<ReviewProgress | null>,
    completed: (file: DiffFile) => void,
) {
    const [pending, setPending] = useState(NO_PENDING)
    const [hunkPending, setHunkPending] = useState(NO_PENDING)
    const [includePending, setIncludePending] = useState(NO_PENDING)
    const [failed, setFailed] = useState(NO_FAILURES)

    useEffect(() => {
        setFailed(NO_FAILURES)
    }, [detail])

    const { headCommit: head, baseCommit: base } = detail
    // One mark's write, the progress read after it, and what follows: a
    // completed file collapses, a refusal says why, and the mark in flight
    // ends either way.
    const settle = useCallback(
        (file: DiffFile, viewed: boolean, write: Promise<void>, done: () => void) => {
            const key = fileKey(file)
            setFailed((current) => without(current, key))
            write
                .then(() => reloadProgress())
                .then(
                    (next) => {
                        const shown = { headCommit: head, baseCommit: base }
                        if (viewed && fileViewedIn(next, shown, key)) completed(file)
                    },
                    (err: unknown) => {
                        setFailed((current) =>
                            new Map(current).set(key, `Not saved: ${errorText(err)}`),
                        )
                    },
                )
                .finally(done)
        },
        [head, base, reloadProgress, completed],
    )

    const markFile = useCallback(
        (file: DiffFile, viewed: boolean) => {
            const key = fileKey(file)
            setPending((current) => new Map(current).set(key, viewed))
            settle(file, viewed, setFileViewed(reference, key, viewed, head, base), () =>
                setPending((current) => without(current, key)),
            )
        },
        [reference, head, base, settle],
    )

    const markHunk = useCallback(
        (file: DiffFile, index: number, viewed: boolean) => {
            const key = fileKey(file)
            const at = hunkMarkKey(key, index)
            setHunkPending((current) => new Map(current).set(at, viewed))
            settle(file, viewed, setHunkViewed(reference, key, index, viewed, head, base), () =>
                setHunkPending((current) => without(current, at)),
            )
        },
        [reference, head, base, settle],
    )

    // Review and Skip. Never `viewed`, so an inclusion completes no file;
    // a Skip's file collapses once the progress after it shows it skipped.
    const includeFile = useCallback(
        (file: DiffFile, included: boolean) => {
            const key = fileKey(file)
            setIncludePending((current) => new Map(current).set(key, included))
            settle(file, false, setFileIncluded(reference, key, included, head, base), () =>
                setIncludePending((current) => without(current, key)),
            )
        },
        [reference, head, base, settle],
    )

    return { pending, hunkPending, includePending, failed, markFile, markHunk, includeFile }
}

/// A hunk's mark, in the gutter of its heading row: a bare checkbox, named by
/// its file and its first line. Left out of a copy, as the row's controls are.
function HunkToggle({
    label,
    viewed,
    busy,
    onChange,
}: {
    label: string
    viewed: boolean
    /// A mark of the hunk, or of its whole file, is in flight.
    busy: boolean
    onChange: (viewed: boolean) => void
}) {
    return (
        <input
            type="checkbox"
            className="pull-request-view-hunk-mark"
            checked={viewed}
            aria-label={label}
            aria-busy={busy || undefined}
            title="Viewed"
            onChange={(event) => {
                if (!busy) onChange(event.currentTarget.checked)
            }}
        />
    )
}

/// A file's viewed mark, in its sticky header: the box, checked when the file
/// is viewed and mixed while some of its hunks are; and beside it how many
/// hunks are viewed, or that the file changed since viewed with how many to
/// review, and why when the reason is not its patch. For a file a skip
/// pattern matches, the pattern, with Review to include a skipped file or
/// Skip to exclude an included one. Left out of a copy, as the header's own
/// controls are.
function ViewedToggle({
    path,
    mark,
    skip,
    pending,
    including,
    error,
    onChange,
    onInclude,
}: {
    path: string
    mark: ViewedMark
    skip: SkipMark | null
    /// The state a mark in flight asks for.
    pending: boolean | undefined
    /// The inclusion an inclusion in flight asks for.
    including: boolean | undefined
    error: string | undefined
    onChange: (viewed: boolean) => void
    onInclude: (included: boolean) => void
}) {
    const settled = pending === undefined
    // A native checkbox's mixed state is a property, never an attribute, so
    // it is set on the element; activating a mixed box marks the file.
    const mixed = settled && mark.box === "mixed"
    const boxRef = useCallback(
        (input: HTMLInputElement | null) => {
            if (input) input.indeterminate = mixed
        },
        [mixed],
    )
    return (
        <span className="pull-request-view-mark" data-copy="skip">
            {settled && mark.note && (
                <span
                    className={
                        mark.changed
                            ? "diff-chip pull-request-view-changed"
                            : "diff-chip pull-request-view-partly"
                    }
                >
                    {mark.note}
                </span>
            )}
            {settled && mark.reason && (
                <span className="pull-request-view-changed-reason">{mark.reason}</span>
            )}
            {skip && (
                <SkipControl
                    path={path}
                    skip={skip}
                    busy={including !== undefined}
                    onInclude={onInclude}
                />
            )}
            <label className="pull-request-view-viewed">
                <input
                    ref={boxRef}
                    type="checkbox"
                    checked={pending ?? mark.box === "checked"}
                    aria-label={`Viewed: ${path}`}
                    aria-busy={!settled || undefined}
                    onChange={(event) => {
                        if (settled) onChange(event.currentTarget.checked)
                    }}
                />{" "}
                Viewed
            </label>
            {error && (
                <span className="diff-load-error" role="alert">
                    {error}
                </span>
            )}
        </span>
    )
}

/// What a file's header says of the skip pattern that matches it, and the
/// control that goes with it: "skipped · matches `<pattern>`" and Review,
/// which includes the file in the review without expanding its section, or
/// "matches `<pattern>`" and Skip, which excludes it again
/// (`pull-request-viewer`: *Changed Files in the Pull-Request View*). The
/// pattern is the reader's own, shown as written, its hidden characters
/// escaped as any untrusted text's are.
function SkipControl({
    path,
    skip,
    busy,
    onInclude,
}: {
    path: string
    skip: SkipMark
    /// An inclusion of the file is in flight.
    busy: boolean
    onInclude: (included: boolean) => void
}) {
    const label = skip.control === "review" ? "Review" : "Skip"
    return (
        <>
            <span className="pull-request-view-skip">
                {skip.words}{" "}
                <code className="pull-request-view-skip-pattern">
                    <EscapedText text={skip.pattern} />
                </code>
            </span>
            <button
                type="button"
                className="diff-hunk-action pull-request-view-include"
                aria-label={`${label}: ${path}`}
                aria-busy={busy || undefined}
                onClick={() => {
                    if (!busy) onInclude(skip.control === "review")
                }}
            >
                {label}
            </button>
        </>
    )
}

/// Review threads, each naming its file, its side and its line, with its
/// comments under it (`pull-request-viewer`: *A thread names its file, side
/// and line*). Above a file's diff in its preamble, across the section's full
/// width and the same in either layout; or after the files for a file the
/// detail does not list.
function ReviewThreads({
    threads,
    reference,
}: {
    threads: readonly ReviewThread[]
    reference: PullRequestReference
}) {
    return (
        <div className="pull-request-view-threads">
            {threads.map((thread) => {
                const place = threadPlace(thread)
                return (
                    <section
                        key={thread.id}
                        className={`pull-request-view-thread${thread.resolved ? " pull-request-view-thread--resolved" : ""}`}
                        aria-label={`Review thread on ${thread.path}, ${place}`}
                    >
                        <div className="pull-request-view-thread-anchor">
                            <span className="pull-request-view-thread-path">
                                <EscapedText text={thread.path} />
                            </span>
                            <span className="pull-request-view-thread-place">{place}</span>
                            {thread.outdated && <span className="diff-chip">outdated</span>}
                            {thread.resolved && <span className="diff-chip">resolved</span>}
                        </div>
                        <ol className="pull-request-view-entries">
                            {thread.comments.map((comment) => (
                                <li key={comment.id}>
                                    <CommentBlock
                                        comment={comment}
                                        review={null}
                                        reference={reference}
                                    />
                                </li>
                            ))}
                        </ol>
                    </section>
                )
            })}
        </div>
    )
}
