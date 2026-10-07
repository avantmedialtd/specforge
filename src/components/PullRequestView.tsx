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
    getReviewProgress,
    isWeb,
    onReviewProgressChanged,
    openPullRequest as openProviderPage,
    openPullRequestLink,
    openPullRequestWindow,
    setFileViewed,
} from "../api"
import { fileKey } from "../diffFiles"
import { pullRequestTitle } from "../pullRequestOpen"
import {
    answeredRead,
    askedRead,
    checkLink,
    checkStateLabel,
    FILE_CHANGED_TEXT,
    fileFailureText,
    hostFileLink,
    linkedChange,
    minimisedText,
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
    splitThreads,
    threadPlace,
    unlistedFilesText,
    viewedMark,
    type LinkedChange,
    type NoticeWords,
    type ReadTrigger,
    type ViewedMark,
    type ViewRead,
} from "../pullRequestView"
import { referenceOf, sameReference, type PullRequestAddress } from "../routing/address"
import { encodeAddress } from "../routing/codec"
import type { PullRequestResolution } from "../routing/resolve"
import type {
    DiffFile,
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
import { DiffView, EscapedText } from "./DiffView"
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
/// case (`pull-request-viewer`: *Review Progress*). Null until read, and when
/// it cannot be. The reload resolves once its answer is applied.
function useReviewProgress(
    reference: PullRequestReference,
    detail: PullRequestDetail | null,
): [ReviewProgress | null, () => Promise<void>] {
    const [progress, setProgress] = useState<ReviewProgress | null>(null)
    const referenceRef = useRef(reference)
    referenceRef.current = reference
    const askedRef = useRef(0)

    const reload = useCallback((): Promise<void> => {
        const asked = ++askedRef.current
        return getReviewProgress(referenceRef.current).then(
            (next) => {
                if (asked === askedRef.current) setProgress(next)
            },
            () => {
                if (asked === askedRef.current) setProgress(null)
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
/// its preamble and its viewed mark in its header; and the threads on files the
/// detail does not list after the files.
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
    reloadProgress: () => Promise<void>
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
    // it is given, so it gets the words alone.
    const loadFile = useCallback(
        (file: DiffFile) =>
            getPullRequestFile(reference, fileKey(file), head, base).then(
                (outcome) => {
                    switch (outcome.kind) {
                        case "file":
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
        [reference, head, base, onFileRefused],
    )

    const split = useMemo(
        () => splitThreads(detail.threads, detail.files),
        [detail.threads, detail.files],
    )
    const byPath = useMemo(() => progressByPath(progress), [progress])
    const { pending, failed, mark } = useViewedMarks(reference, detail, reloadProgress)

    const renderFileHeaderExtra = useCallback(
        (file: DiffFile) => {
            const key = fileKey(file)
            return (
                <ViewedToggle
                    path={key}
                    mark={viewedMark(byPath.get(key))}
                    pending={pending.get(key)}
                    error={failed.get(key)}
                    onChange={(viewed) => mark(file, viewed)}
                />
            )
        },
        [byPath, pending, failed, mark],
    )

    // A file too large to preview links to its diff on the host, above its
    // threads (*A too-large file links to its diff on the host*).
    const page = detail.row.url
    const renderFilePreamble = useCallback(
        (file: DiffFile) => {
            const threads = split.byFile.get(fileKey(file))
            const host =
                file.content.kind === "tooLarge"
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
        [split, reference, page],
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

function without<V>(map: ReadonlyMap<string, V>, key: string): ReadonlyMap<string, V> {
    if (!map.has(key)) return map
    const next = new Map(map)
    next.delete(key)
    return next
}

/// Marks and unmarks files viewed through the service, naming the head and
/// base commits of the detail the view rendered; the service computes each
/// key from its cached detail (`pull-request-viewer`: *Review Progress*). A
/// file being marked shows its new state until the progress that follows the
/// mark lands; one whose mark was refused says so in its header until it is
/// marked again or a new detail arrives. A refused mark asks for no read.
function useViewedMarks(
    reference: PullRequestReference,
    detail: PullRequestDetail,
    reloadProgress: () => Promise<void>,
) {
    const [pending, setPending] = useState(NO_PENDING)
    const [failed, setFailed] = useState(NO_FAILURES)

    useEffect(() => {
        setFailed(NO_FAILURES)
    }, [detail])

    const { headCommit: head, baseCommit: base } = detail
    const mark = useCallback(
        (file: DiffFile, viewed: boolean) => {
            const key = fileKey(file)
            setPending((current) => new Map(current).set(key, viewed))
            setFailed((current) => without(current, key))
            setFileViewed(reference, key, viewed, head, base)
                .then(reloadProgress, (err: unknown) => {
                    setFailed((current) => new Map(current).set(key, `Not saved: ${errorText(err)}`))
                })
                .finally(() => setPending((current) => without(current, key)))
        },
        [reference, head, base, reloadProgress],
    )

    return { pending, failed, mark }
}

/// A file's viewed mark, in its sticky header: the toggle, and when the file
/// changed since it was viewed, that flag with its reason when the reason is
/// not its patch. Left out of a copy, as the header's own controls are.
function ViewedToggle({
    path,
    mark,
    pending,
    error,
    onChange,
}: {
    path: string
    mark: ViewedMark
    /// The state a mark in flight asks for.
    pending: boolean | undefined
    error: string | undefined
    onChange: (viewed: boolean) => void
}) {
    const settled = pending === undefined
    return (
        <span className="pull-request-view-mark" data-copy="skip">
            {settled && mark.changed && (
                <span className="diff-chip pull-request-view-changed">changed since viewed</span>
            )}
            {settled && mark.reason && (
                <span className="pull-request-view-changed-reason">{mark.reason}</span>
            )}
            <label className="pull-request-view-viewed">
                <input
                    type="checkbox"
                    checked={pending ?? mark.viewed}
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
