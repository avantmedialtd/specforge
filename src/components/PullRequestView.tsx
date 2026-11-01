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
    isTauri,
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
    fileHunkStates,
    fileSlotSignature,
    fileViewedIn,
    endRowFor,
    hostFileLink,
    hunkMarkKey,
    hunkMarkLabel,
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
    type FileSlotInputs,
    type LinkedChange,
    type NoticeWords,
    type ReadTrigger,
    type SkipMark,
    type ViewedMark,
    type ViewRead,
} from "../pullRequestView"
import { referenceOf, sameReference, type PullRequestAddress } from "../routing/address"
import {
    anchorDraftItems,
    anchorInlineDrafts,
    anchorThreads,
    commentableLines,
    renderedLines,
    type DraftAnchors,
} from "../reviewAnchors"
import { bodyBytes, MAX_BODY_BYTES } from "../reviewActions"
import { createSlotCache, lineSlotLookup, type LineSlotEntry } from "../slotCache"
import { shareStructure } from "../structuralShare"
import { encodeAddress } from "../routing/codec"
import type { PullRequestResolution } from "../routing/resolve"
import type {
    DiffFile,
    DiffSide,
    DraftItem,
    ImageVersions,
    PlacedInlineItem,
    PlacedReplyItem,
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
    UnplacedInlineItem,
    UnplacedReplyItem,
    WorkspaceView,
} from "../types"
import { useLatest } from "../hooks/useLatest"
import { PullRequestActionsContext, usePullRequestActions } from "../hooks/usePullRequestActions"
import {
    ReviewDraftEditorProvider,
    useDraftItemsEditorContext,
} from "../hooks/useReviewDraftEditor"
import {
    DiffView,
    EscapedText,
    type DiffViewHandle,
    type FileSlots,
    type HunkSlots,
    type LineAction,
    type LineSlot,
} from "./DiffView"
import { IdentityTrailing } from "./DocumentView"
import { CommentIcon } from "./icons"
import { EmptyState } from "./EmptyState"
import { prettifyError } from "./errors"
import { MarkdownView } from "./MarkdownView"
import { OpenReaderControl } from "./OpenReaderControl"
import { PullRequestMarkers, PullRequestSignalCells } from "./PullRequestPanel"
import { PullRequestReviewComposer } from "./PullRequestReviewComposer"
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
        // The address's fields, not the object a caller may rebuild per render.
        // eslint-disable-next-line react-hooks/exhaustive-deps
        [address.provider, address.owner, address.repo, address.number],
    )
    const { read, reading, refresh, fileRefused } = usePullRequestRead(reference, row)
    const { detail, notice } = read
    const [progress, reloadProgress] = useReviewProgress(reference, detail)
    // One action state for this pull request's presentations: the center pane
    // and the detached window render this component, and its composer reads
    // the value from here (`pull-request-actions`: *Review Action State
    // Synchronises Only Native Windows*). A browser tab reads and subscribes
    // to nothing.
    const actions = usePullRequestActions(reference)

    // The action read can beat the detail read on opening and answer that a
    // current detail is required. Once the detail arrives — or a refresh moves
    // its commits — read again under that detail's basis.
    const reloadActions = actions.reload
    const actionBasisRef = useRef(detail ? `${detail.headCommit}:${detail.baseCommit}` : null)
    useEffect(() => {
        if (!detail) return
        const basis = `${detail.headCommit}:${detail.baseCommit}`
        if (actionBasisRef.current === basis) return
        actionBasisRef.current = basis
        void reloadActions()
    }, [detail, reloadActions])

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
            <PullRequestActionsContext.Provider value={actions}>
                <div className="detail-pane pull-request-view" ref={rootRef}>
                    {placeholder}
                </div>
            </PullRequestActionsContext.Provider>
        )
    }

    const page = providerPage(row, detail)
    const popOut = pane ? popOutAddress(provider, row, detail) : null
    const reviewed = progressWords(progress)
    const head = detail?.headBranch ?? headed.sourceBranch
    const base = detail?.baseBranch ?? headed.destinationBranch
    const author = detail?.author ?? headed.author

    const view = (
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
                        {/* Desktop-only (`pull-request-actions`: *Desktop-Only
                            Pull-Request Reviews*): a browser tab has no action
                            state to read and renders no composer. */}
                        {isTauri() && <PullRequestReviewComposer reference={reference} />}
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

    return (
        <PullRequestActionsContext.Provider value={actions}>
            {isTauri() ? (
                <ReviewDraftEditorProvider reference={reference} actions={actions}>
                    {view}
                </ReviewDraftEditorProvider>
            ) : (
                view
            )}
        </PullRequestActionsContext.Provider>
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
                // Shared with the progress held, so an unchanged file's entry,
                // and its hunk states, keep their identity across a reload:
                // what keeps that file's slots, and its section, as they were
                // (`fileSlotSignature`).
                if (asked === askedRef.current) setProgress((prev) => shareStructure(prev, next))
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
/// branches, in the layout commit detail uses; each file's current
/// single-line review threads on their lines, the rest in its preamble, and
/// its viewed mark in its header; each hunk's mark in its heading, a long
/// one's again at its end, every viewed hunk folded; and the threads on files
/// the detail does not list after the files.
///
/// Keyed by the pull request, so a re-read re-renders the diff rather than
/// remounting it, and the layout, collapse and loaded files the reader has
/// stay as they are. Exported for the render tests in
/// `PullRequestView.test.tsx`.
export function PullRequestFiles({
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
    // The draft's own items, grouped once for every file's slots and for the
    // items no listed file can hold (`pull-request-viewer`: *Changed Files in
    // the Pull-Request View*). A browser tab has no editor, so it has no
    // items and renders no draft control.
    const editor = useDraftItemsEditorContext()
    const draftItems = editor?.draft?.items ?? NO_DRAFT_ITEMS
    const draftAnchors = useMemo(
        () => anchorDraftItems(draftItems, detail.files, detail.threads),
        [draftItems, detail.files, detail.threads],
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

    // Each file's slots, built through a cache keyed on that file's own
    // inputs (`fileSlotSignature`), so a mark, its time in flight and the
    // progress read after it rebuild only the marked file's slots, and every
    // other file's section skips its render (`pull-request-viewer`: *Review
    // Marks Re-render Only the Marked File*; design D5). The handlers go
    // through a latest-ref, so a cached slot never calls a stale one.
    const page = detail.row.url
    const inputs: FileSlotInputs = useMemo(
        () => ({
            progress,
            byPath,
            head,
            base,
            page,
            reference,
            pending,
            hunkPending,
            includePending,
            failed,
            threads: split.byFile,
            repliesByThread: draftAnchors.repliesByThread,
            linked,
            inlineAffordances:
                editor !== null &&
                (editor.placing?.kind === "inline" ||
                    (editor.placing === null &&
                        editor.editable &&
                        editor.canInline &&
                        !editor.atItemLimit)),
            composing: editor?.composing ?? null,
            inlineDrafts: draftAnchors.inlineByFile,
            unplacedInline: draftAnchors.unplacedInlineByFile,
        }),
        [
            progress,
            byPath,
            head,
            base,
            page,
            reference,
            pending,
            hunkPending,
            includePending,
            failed,
            split.byFile,
            linked,
            editor,
            draftAnchors,
        ],
    )
    const handlers = useLatest<SlotHandlers>({ markFile, markHunk, includeFile })
    const [slotCache] = useState(() => createSlotCache<FileSlots>())
    useEffect(() => {
        slotCache.retain(detail.files.map(fileKey))
    }, [slotCache, detail.files])
    const fileSlots = useCallback(
        (file: DiffFile) =>
            slotCache.get(fileKey(file), fileSlotSignature(file, inputs), () =>
                buildFileSlots(file, inputs, handlers),
            ),
        [slotCache, inputs, handlers],
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
                    fileSlots={fileSlots}
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
                    <ReviewThreads
                        threads={split.unlisted}
                        reference={reference}
                        repliesByThread={draftAnchors.repliesByThread}
                    />
                </section>
            )}
            <DetachedDrafts anchors={draftAnchors} reference={reference} />
        </section>
    )
}

/// What a file's slots do, read at the time they do it.
interface SlotHandlers {
    markFile: (file: DiffFile, viewed: boolean) => void
    markHunk: (file: DiffFile, index: number, viewed: boolean) => void
    includeFile: (file: DiffFile, included: boolean) => void
}

/// One file's slots (`DiffView`'s `FileSlots`):
///
/// - its header extra: the viewed mark, and Skip or Review;
/// - its hunk slots, for a file whose hunk states the progress gives for this
///   detail: a checkbox in each hunk's heading, a "Mark hunk viewed" row
///   ending a long unviewed one, and every viewed hunk folded. A mark in
///   flight, of the hunk or of its whole file, shows its new state at once;
/// - its preamble: a link to its diff on the host for a file too large to
///   preview, an image file stored in Git LFS or one whose versions cannot
///   all be shown (*A too-large file links to its diff on the host*), above
///   its review threads that its diff cannot render on their line — outdated,
///   ranged, without a line, or on a line the diff does not render;
/// - its line slots: every other review thread, on its side-qualified line,
///   the reviewer's placed draft comments, and the line's comment affordance
///   while inline comments are offered (`pull-request-viewer`: *Changed Files
///   in the Pull-Request View*).
///
/// THE INVARIANT: every value read here for `file`, apart from `handlers`,
/// appears in `fileSlotSignature` (`src/pullRequestView.ts`), which is what
/// the cache keys these slots on. A new input here goes into that signature in
/// the same change, or the file's slots go stale. The handlers are read
/// through their ref when a control is used, never when the slots are built.
/// One side-qualified line's combined thread, draft and affordance content.
interface LineSlotParts {
    side: DiffSide
    line: number
    threads: ReactNode[]
    items: PlacedInlineItem[]
}

function buildFileSlots(
    file: DiffFile,
    inputs: FileSlotInputs,
    handlers: { readonly current: SlotHandlers },
): FileSlots {
    const key = fileKey(file)
    const { reference } = inputs
    const fileProgress = inputs.byPath.get(key)
    const states = fileHunkStates(inputs, key)

    const headerExtra = (
        <ViewedToggle
            path={key}
            mark={viewedMark(fileProgress, states)}
            skip={skipMark(fileProgress)}
            pending={inputs.pending.get(key)}
            including={inputs.includePending.get(key)}
            error={inputs.failed.get(key)}
            onChange={(viewed) => handlers.current.markFile(file, viewed)}
            onInclude={(included) => handlers.current.includeFile(file, included)}
        />
    )

    let hunks: HunkSlots | undefined
    if (states !== null) {
        const whole = inputs.pending.get(key)
        const viewedAt = (index: number) =>
            inputs.hunkPending.get(hunkMarkKey(key, index)) ?? whole ?? states[index] === true
        hunks = {
            folded: new Set(states.flatMap((_, index) => (viewedAt(index) ? [index] : []))),
            heading: (index, hunk) =>
                index < states.length && (
                    <HunkToggle
                        label={hunkMarkLabel(key, hunk)}
                        viewed={viewedAt(index)}
                        busy={inputs.hunkPending.has(hunkMarkKey(key, index)) || whole !== undefined}
                        onChange={(viewed) => handlers.current.markHunk(file, index, viewed)}
                    />
                ),
            end: (index, hunk) =>
                index < states.length &&
                endRowFor(hunk.lines.length, viewedAt(index)) && (
                    <button
                        type="button"
                        className="diff-hunk-action"
                        aria-label={`Mark hunk viewed: ${key}, from ${hunkFirstLine(hunk)}`}
                        onClick={() => handlers.current.markHunk(file, index, true)}
                    >
                        Mark hunk viewed
                    </button>
                ),
        }
    }

    const threads = inputs.threads.get(key)
    // A thread or draft comment the diff cannot render on its line keeps the
    // preamble, where its anchor is stated, rather than disappearing inside
    // the diff.
    const rendered = renderedLines(file, hunks?.folded)
    const anchors = anchorThreads(threads ?? [], file, hunks?.folded, rendered)
    const drafts = anchorInlineDrafts(
        inputs.inlineDrafts.get(key) ?? [],
        file,
        hunks?.folded,
        rendered,
    )
    const unplaced = inputs.unplacedInline.get(key) ?? []
    const image = isImageFile(file) && (isLfsPointerFile(file) || inputs.linked.has(key))
    const host =
        file.content.kind === "tooLarge" || image
            ? hostFileLink(reference.provider, inputs.page, file)
            : null
    const preamble =
        host ||
        drafts.preamble.length > 0 ||
        unplaced.length > 0 ||
        anchors.preamble.length > 0 ? (
            <>
                {host && (
                    <p className="pull-request-view-host-file">
                        <ContentLink href={host} reference={reference}>
                            View this file on {PROVIDER_NAMES[reference.provider]}
                        </ContentLink>
                    </p>
                )}
                {(drafts.preamble.length > 0 || unplaced.length > 0) && (
                    <div className="pull-request-view-draft-items">
                        {drafts.preamble.map((item) => (
                            <DraftItemCard key={item.id} item={item} reference={reference} />
                        ))}
                        {unplaced.map((item) => (
                            <UnplacedInlineDraft key={item.id} item={item} reference={reference} />
                        ))}
                    </div>
                )}
                {anchors.preamble.length > 0 && (
                    <ReviewThreads
                        threads={anchors.preamble}
                        reference={reference}
                        repliesByThread={inputs.repliesByThread}
                    />
                )}
            </>
        ) : null
    let line: LineSlot | undefined
    let lineAction: LineAction | undefined
    if (anchors.slots.length > 0 || drafts.slots.length > 0 || inputs.inlineAffordances) {
        const entries: LineSlotEntry<ReactNode>[] = []
        const byPlace = new Map<string, LineSlotParts>()
        for (const slot of anchors.slots) {
            byPlace.set(`${slot.side}\u0000${slot.line}`, {
                side: slot.side,
                line: slot.line,
                threads: [
                    <ReviewThreads
                        key="threads"
                        threads={slot.threads}
                        reference={reference}
                        repliesByThread={inputs.repliesByThread}
                    />,
                ],
                items: [],
            })
        }
        for (const slot of drafts.slots) {
            const place = `${slot.side}\u0000${slot.line}`
            const sitting = byPlace.get(place)
            if (sitting) sitting.items = [...slot.items]
            else
                byPlace.set(place, {
                    side: slot.side,
                    line: slot.line,
                    threads: [],
                    items: [...slot.items],
                })
        }
        // The lines that offer a comment: one per row, on a context line's
        // new side. They offer it as a gutter action on hover, never as a
        // row of their own; a line gets a slot only for content — a thread,
        // a draft, or the one editor that is open.
        const offered = commentableLines(file, hunks?.folded)
        if (inputs.inlineAffordances) {
            lineAction = (side, lineNo) =>
                offered[side].has(lineNo) ? (
                    <LineCommentAction path={key} side={side} line={lineNo} />
                ) : null
        }
        const composing =
            inputs.composing?.path === key && offered[inputs.composing.side].has(inputs.composing.line)
                ? inputs.composing
                : null
        if (composing) {
            const place = `${composing.side}\u0000${composing.line}`
            if (!byPlace.has(place)) {
                byPlace.set(place, {
                    side: composing.side,
                    line: composing.line,
                    threads: [],
                    items: [],
                })
            }
        }
        for (const slot of byPlace.values()) {
            const open = composing?.side === slot.side && composing.line === slot.line
            entries.push([
                slot.side,
                slot.line,
                <>
                    {slot.items.length > 0 && (
                        <div className="pull-request-view-draft-items">
                            {slot.items.map((item) => (
                                <DraftItemCard key={item.id} item={item} reference={reference} />
                            ))}
                        </div>
                    )}
                    {open && <LineComposer path={key} side={slot.side} line={slot.line} />}
                    {slot.threads}
                </>,
            ])
        }
        line = lineSlotLookup(entries)
    }

    return { headerExtra, preamble, hunks, line, lineAction }
}

const NO_DRAFT_ITEMS: readonly DraftItem[] = []

/// One side-qualified line's gutter action, drawn by the diff on hover or
/// focus: a comment icon that opens the line's editor, or, while an unplaced
/// or moved comment is choosing a line, the icon that places it here. Its
/// accessible name says which, and names the side and line.
function LineCommentAction({
    path,
    side,
    line,
}: {
    path: string
    side: DiffSide
    line: number
}) {
    const editor = useDraftItemsEditorContext()
    if (!editor) return null
    const place = editor.placing
    if (place !== null && place.kind !== "inline") return null
    if (place?.kind === "inline") {
        const item = editor.draft?.items.find((candidate) => candidate.id === place.itemId)
        if (!item || !isInlineItem(item)) return null
        const label = `Place comment on ${side} line ${line} of ${path}`
        return (
            <button
                type="button"
                className="pull-request-line-comment pull-request-line-comment--place"
                aria-label={label}
                title="Place comment here"
                disabled={editor.itemBusy !== null}
                onClick={() => void editor.moveInline(item, { path, side, line })}
            >
                <CommentIcon width={14} height={14} />
            </button>
        )
    }
    if (!editor.editable || !editor.canInline || editor.atItemLimit) return null
    return (
        <button
            type="button"
            className="pull-request-line-comment"
            aria-label={`Comment on ${side} line ${line} of ${path}`}
            title="Comment on this line"
            onClick={() => editor.beginComposing({ path, side, line })}
        >
            <CommentIcon width={14} height={14} />
        </button>
    )
}

/// The open new-comment editor beneath the one line it was opened on. It
/// lives in the draft editor's state, not here, so it survives this slot
/// re-rendering or the affordances turning off while the reviewer types.
function LineComposer({ path, side, line }: { path: string; side: DiffSide; line: number }) {
    const editor = useDraftItemsEditorContext()
    if (!editor) return null
    return (
        <div className="pull-request-composer-line">
            <DraftItemEditor
                label={`Comment on ${side} line ${line} of ${path}`}
                submitLabel="Add comment"
                errorId="new"
                onSubmit={(body) => editor.addInline({ path, side, line }, body)}
                onCancel={editor.cancelComposing}
            />
        </div>
    )
}

/// One draft item's prose, anchor and controls. A placed item states the line
/// or thread it is on; an unplaced one states its former anchor and offers the
/// explicit conversion the stale-recovery rule requires. Nothing here offers a
/// range, a structured suggestion, resolution, reopening, merge, close or
/// decline.
function DraftItemCard({
    item,
    reference,
    anchor,
}: {
    item: DraftItem
    reference: PullRequestReference
    anchor?: string
}) {
    const editor = useDraftItemsEditorContext()
    const [editing, setEditing] = useState(false)
    if (!editor) return null
    const unplaced = item.kind === "unplacedInline" || item.kind === "unplacedReply"
    const busy = editor.itemBusy === item.id
    const error = editor.itemError?.itemId === item.id ? editor.itemError.text : null
    const placing = editor.placing?.itemId === item.id ? editor.placing : null
    const place =
        item.kind === "placedInline" || item.kind === "unplacedInline"
            ? `${sideWord(item.side)} line ${item.line} of ${item.path}`
            : "a reply"
    const label =
        anchor ??
        (item.kind === "unplacedInline"
            ? `Former comment on ${place}`
            : item.kind === "unplacedReply"
              ? `Former reply to thread ${item.target}`
              : item.kind === "placedReply"
                ? "Draft reply"
                : `Draft comment on ${place}`)
    return (
        <section
            className={`pull-request-view-draft-item${unplaced ? " pull-request-view-draft-item--unplaced" : ""}`}
            aria-label={label}
        >
            <div className="pull-request-view-draft-anchor">
                <span>{label}</span>
                {item.kind === "unplacedInline" && (
                    <span className="pull-request-view-draft-basis">
                        {item.basis.base}…{item.basis.head}
                    </span>
                )}
            </div>
            {editing ? (
                <DraftItemEditor
                    label={`Edit ${label}`}
                    initial={item.body}
                    submitLabel="Save comment"
                    errorId={item.id}
                    onSubmit={(body) => editor.editItem(item, body)}
                    onCancel={() => setEditing(false)}
                />
            ) : (
                <MarkdownView content={item.body} pullRequest={reference} />
            )}
            {!editing && editor.editable && (
                <div className="pull-request-view-draft-actions">
                    <button
                        type="button"
                        className="pull-request-view-action"
                        disabled={busy}
                        onClick={() => setEditing(true)}
                    >
                        Edit
                    </button>
                    <button
                        type="button"
                        className="pull-request-view-action"
                        disabled={busy}
                        onClick={() => void editor.removeItem(item)}
                    >
                        Delete
                    </button>
                    {!placing && (
                        <button
                            type="button"
                            className="pull-request-view-action"
                            disabled={busy}
                            onClick={() =>
                                editor.beginPlacing({
                                    kind:
                                        item.kind === "placedReply" ||
                                        item.kind === "unplacedReply"
                                            ? "reply"
                                            : "inline",
                                    itemId: item.id,
                                })
                            }
                        >
                            {unplaced
                                ? item.kind === "unplacedReply"
                                    ? "Choose a replyable thread"
                                    : "Choose a current line"
                                : "Move"}
                        </button>
                    )}
                    {placing && (
                        <>
                            <span className="pull-request-view-note" role="status">
                                {placing.kind === "inline"
                                    ? "Choose a line in the diff."
                                    : "Choose a replyable thread."}
                            </span>
                            <button
                                type="button"
                                className="pull-request-view-action"
                                onClick={editor.cancelPlacing}
                            >
                                Cancel
                            </button>
                        </>
                    )}
                </div>
            )}
            {error && (
                <p className="pull-request-composer-item-error" role="alert">
                    {error}
                </p>
            )}
        </section>
    )
}

/// The textarea one draft item is written in, shared by new comments, new
/// replies and edits. It refuses an empty body and one over 30 KiB before
/// asking the service, and words a service refusal beside the field.
function DraftItemEditor({
    label,
    initial = "",
    submitLabel,
    errorId,
    onSubmit,
    onCancel,
}: {
    label: string
    initial?: string
    submitLabel: string
    errorId: string
    onSubmit: (body: string) => Promise<boolean>
    onCancel: () => void
}) {
    const editor = useDraftItemsEditorContext()
    const [body, setBody] = useState(initial)
    const [working, setWorking] = useState(false)
    const bytes = bodyBytes(body)
    const tooLarge = bytes > MAX_BODY_BYTES
    const empty = body.trim() === ""
    const busy = editor?.itemBusy !== null && editor?.itemBusy !== undefined
    const error = editor?.itemError?.itemId === errorId ? editor.itemError.text : null
    const submit = async () => {
        if (empty || tooLarge || busy || working) return
        setWorking(true)
        try {
            if (await onSubmit(body)) onCancel()
        } finally {
            setWorking(false)
        }
    }
    return (
        <div className="pull-request-composer-item-editor" role="group" aria-label={label}>
            <textarea
                className="settings-text-input pull-request-composer-item-body"
                aria-label={label}
                rows={3}
                value={body}
                onChange={(event) => setBody(event.currentTarget.value)}
            />
            <div className="pull-request-composer-item-meta">
                <span className="pull-request-composer-count">
                    {bytes} / {MAX_BODY_BYTES} bytes
                </span>
                {tooLarge && (
                    <span className="pull-request-composer-item-error" role="alert">
                        A comment can be at most 30 KiB.
                    </span>
                )}
                {error && (
                    <span className="pull-request-composer-item-error" role="alert">
                        {error}
                    </span>
                )}
            </div>
            <div className="pull-request-composer-item-actions">
                <button
                    type="button"
                    className="btn-primary pull-request-composer-item-save"
                    disabled={empty || tooLarge || busy || working}
                    onClick={() => void submit()}
                >
                    {submitLabel}
                </button>
                <button type="button" className="pull-request-view-action" onClick={onCancel}>
                    Cancel
                </button>
            </div>
        </div>
    )
}

/// An unplaced inline comment in its former file's preamble: its prose and
/// former anchor, with an explicit current-line choice. It is never moved by
/// path, line number or text.
function UnplacedInlineDraft({
    item,
    reference,
}: {
    item: UnplacedInlineItem
    reference: PullRequestReference
}) {
    return <DraftItemCard item={item} reference={reference} />
}

/// Draft items no listed file can hold: an unplaced comment whose former path
/// is not listed, a placed comment whose file has left the detail, a reply
/// whose former target is gone, and a reply whose target thread is not in this
/// detail. They follow the files, each naming its former anchor or target.
function DetachedDrafts({
    anchors,
    reference,
}: {
    anchors: DraftAnchors
    reference: PullRequestReference
}) {
    const inline = anchors.inlineAfter
    const replies = anchors.unplacedReplies
    const orphans = anchors.orphanReplies
    if (inline.length === 0 && replies.length === 0 && orphans.length === 0) return null
    return (
        <section
            className="pull-request-view-detached-drafts"
            aria-label="Draft comments and replies not placed"
        >
            {inline.length > 0 && (
                <div className="pull-request-view-draft-after">
                    <h3 className="pull-request-view-subheading">Draft comments to place</h3>
                    {inline.map((item) => (
                        <DraftItemCard key={item.id} item={item} reference={reference} />
                    ))}
                </div>
            )}
            {replies.length > 0 && (
                <div className="pull-request-view-draft-after pull-request-view-draft-replies">
                    <h3 className="pull-request-view-subheading">Draft replies without a thread</h3>
                    {replies.map((item) => (
                        <DraftItemCard key={item.id} item={item} reference={reference} />
                    ))}
                </div>
            )}
            {orphans.length > 0 && (
                <div className="pull-request-view-draft-after pull-request-view-draft-orphan-replies">
                    <h3 className="pull-request-view-subheading">
                        Draft replies whose thread is not here
                    </h3>
                    {orphans.map((item) => (
                        <DraftItemCard
                            key={item.id}
                            item={item}
                            reference={reference}
                            anchor={`Draft reply to thread ${item.target}`}
                        />
                    ))}
                </div>
            )}
        </section>
    )
}

/// The reply affordance beside one thread, and the editor it opens. A reply is
/// offered only where the detail says the thread is replyable; while an
/// unplaced reply is choosing a target, each replyable thread offers "Place
/// reply here" instead.
function ThreadReply({ thread }: { thread: ReviewThread }) {
    const editor = useDraftItemsEditorContext()
    const [open, setOpen] = useState(false)
    if (!editor) return null
    if (editor.placing !== null && editor.placing.kind !== "reply") return null
    const placing = editor.placing?.kind === "reply" ? editor.placing : null
    if (placing) {
        if (!thread.replyable) return null
        const item = editor.draft?.items.find((candidate) => candidate.id === placing.itemId)
        if (!item || !isReplyItem(item)) return null
        return (
            <button
                type="button"
                className="pull-request-composer-place"
                aria-label={`Place reply on thread on ${thread.path}, ${threadPlace(thread)}`}
                disabled={editor.itemBusy !== null}
                onClick={() => void editor.moveReply(item, thread)}
            >
                Place reply here
            </button>
        )
    }
    if (!editor.canReply || editor.atItemLimit || !thread.replyable) return null
    return (
        <div className="pull-request-composer-reply">
            {!open && (
                <button
                    type="button"
                    className="pull-request-view-action"
                    aria-label={`Reply to thread on ${thread.path}, ${threadPlace(thread)}`}
                    onClick={() => setOpen(true)}
                >
                    Reply
                </button>
            )}
            {open && (
                <DraftItemEditor
                    label={`Reply to thread on ${thread.path}, ${threadPlace(thread)}`}
                    submitLabel="Add reply"
                    errorId="new"
                    onSubmit={(body) => editor.addReply(thread, body)}
                    onCancel={() => setOpen(false)}
                />
            )}
        </div>
    )
}

function sideWord(side: DiffSide): string {
    return side === "old" ? "old" : "new"
}

function isInlineItem(item: DraftItem): item is PlacedInlineItem | UnplacedInlineItem {
    return item.kind === "placedInline" || item.kind === "unplacedInline"
}

function isReplyItem(item: DraftItem): item is PlacedReplyItem | UnplacedReplyItem {
    return item.kind === "placedReply" || item.kind === "unplacedReply"
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
    repliesByThread,
}: {
    threads: readonly ReviewThread[]
    reference: PullRequestReference
    repliesByThread?: ReadonlyMap<string, readonly PlacedReplyItem[]>
}) {
    return (
        <div className="pull-request-view-threads">
            {threads.map((thread) => {
                const place = threadPlace(thread)
                const replies = repliesByThread?.get(thread.id) ?? []
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
                            {replies.map((reply) => (
                                <li key={reply.id}>
                                    <DraftItemCard
                                        item={reply}
                                        reference={reference}
                                        anchor="Your reply"
                                    />
                                </li>
                            ))}
                        </ol>
                        <ThreadReply thread={thread} />
                    </section>
                )
            })}
        </div>
    )
}
