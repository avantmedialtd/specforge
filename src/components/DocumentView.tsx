import {
    Children,
    useCallback,
    useEffect,
    useLayoutEffect,
    useReducer,
    useRef,
    useState,
} from "react"
import type { ReactNode, RefObject } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import {
    onCacheUpdated,
    onDocumentChanged,
    readArtifact,
    readWorkspaceFile,
    unwatchDocument,
    watchDocument,
} from "../api"
import {
    effectiveTrigger,
    INITIAL,
    reduce,
    type LoadTrigger,
} from "../detail/refreshPolicy"
import { useCoalescedRefetch } from "../hooks/useCoalescedRefetch"
import {
    outlineEntries,
    sectionForHeading,
    sectionProgress,
    type HeadingEntry,
} from "../outline"
import type { ArtifactRenderTarget, Section } from "../types"
import { EmptyState } from "./EmptyState"
import { CompletionMark, OpenInWindow } from "./icons"
import { MarkdownView } from "./MarkdownView"

/// What a document surface is showing. The two shapes differ in how the bytes
/// are fetched and in nothing else — which is why every surface that renders
/// one markdown file goes through this component instead of reimplementing the
/// fetch, the freshness policy and the anchor scrolling three times.
export type DocumentSource =
    | { kind: "artifact"; target: ArtifactRenderTarget }
    /// One file beneath a browse root, root-relative and forward-slash
    /// separated — the file browser's selection, and a reader window opened on
    /// an arbitrary markdown file.
    | { kind: "file"; root: string; path: string }

/// Where a surface has been asked to scroll to, by SOURCE LINE — the one
/// mechanism the renderer already stamps on every block it emits (`data-line`).
///
/// One kind, since the tree stopped producing section and task anchors
/// (design D9): what remains is the document's own within-document
/// navigation — an outline entry and a fragment link — and both name a
/// heading, which is a line.
export type ScrollAnchor = { kind: "line"; line: number } | null

/// What a surface's own header needs from the document beneath it.
export interface DocumentStatus {
    /// When the file was last written, unix seconds, or null when there is no
    /// time to show. Always null for a `file` source: the workspace read
    /// returns bytes only.
    modifiedAt: number | null
    /// True once a refresh the user did not initiate failed to read the
    /// document — it was deleted, renamed away, or its change was archived.
    ///
    /// Deliberately NOT part of `refreshPolicy`'s state. That reducer's
    /// contract is that a failed background read is *unobservable* and returns
    /// the identical state object, which is what keeps a reader undisturbed;
    /// folding this in would have broken that guarantee for every surface in
    /// order to give one surface an indicator. The content stays exactly as the
    /// reducer left it — this flag only says the address stopped resolving
    /// (`reader-window`: *A Vanished Document Is Reported, Not Followed*).
    missing: boolean
}

/// Says the document is no longer at the address being shown.
///
/// The content stays on screen, and no surface follows the file anywhere — an
/// archived change's artifact still exists under `openspec/changes/archive/`,
/// but silently switching to it would make the surface stop showing the
/// address it was opened for (`reader-window`: *A Vanished Document Is
/// Reported, Not Followed*).
export function MissingDocumentLabel() {
    return (
        <span
            className="identity-missing"
            title="This document no longer exists at this address. The last version read is still shown."
        >
            no longer present
        </span>
    )
}

/// The identity row's TRAILING cluster: the elements that DESCRIBE the document
/// — the last-changed label, the open-in-reader control — as against the name
/// and branch chip that NAME it and pack at the leading edge.
///
/// It exists so that exactly ONE element on that row carries the auto margin
/// which collapses the gap between the two clusters. Flexbox distributes
/// positive free space EQUALLY among every auto main-axis margin on a line
/// (Flexbox 1 §9.5) — a second auto margin does not lose to the first, it
/// HALVES it. That is precisely how the last-changed label came to rest in
/// mid-row, ~206px short of the column's trailing edge, with the invisible
/// (`opacity: 0`) reader control silently holding the other half.
///
/// Membership is expressed by NESTING rather than by a rule naming each member,
/// so a third trailing element inherits the push by being placed here and
/// cannot reintroduce the split. The alternative — a selector standing one
/// member's margin down behind another's — would encode the invariant as DOM
/// adjacency and would fail silently the day the order changed.
///
/// BUILT here and PLACED by each surface, the same division
/// `MissingDocumentLabel` above is under: which trailing elements a surface has
/// is that surface's business, while what they do at the edge is not.
///
/// Renders NOTHING when it would be empty, rather than an empty wrapper: a
/// childless group is still a flex item and still costs the row one `gap`, so a
/// reader window — which has no trailing element at all — would otherwise gain
/// 8px of dead space for a node that draws nothing.
export function IdentityTrailing({ children }: { children?: ReactNode }) {
    // `Children.toArray` discards the `false`/`null` that a conditional child
    // renders to, so an absent label and an absent control both read as absent
    // here rather than as an empty-but-present group.
    if (Children.toArray(children).length === 0) {
        return null
    }
    return <div className="identity-trailing">{children}</div>
}

interface DocumentViewProps {
    /// The document to render, or null for "nothing selected".
    source: DocumentSource | null
    /// Where to scroll once the markdown is in the DOM.
    scrollAnchor?: ScrollAnchor
    /// The surface's own header, rendered above the document. Receives the
    /// document's status, a ref to attach to whatever element is sticky — the
    /// anchor scrolling measures it rather than assuming a height, because a
    /// wrapped change name genuinely varies at runtime — and the reader
    /// control to place, which is `null` when this surface offers none.
    ///
    /// The control is BUILT here and PLACED by the caller: the behaviour is
    /// the same wherever it appears, while the header's layout is the
    /// caller's business.
    header?: (
        status: DocumentStatus,
        headerRef: RefObject<HTMLDivElement | null>,
        readerControl: ReactNode,
    ) => ReactNode
    /// Open this document in its own reader window. Omitted by surfaces that
    /// should offer no such control — a reader window itself, above all: that
    /// document is already detached, and a control to detach it again would
    /// name an operation with nothing to do.
    onOpenReader?: () => void
    /// Shown when `source` is null.
    empty?: ReactNode
    /// Title of the error state for a failed user-initiated read.
    errorTitle?: string
    /// Class on the wrapper. Must NOT establish a scroll container: the anchor
    /// logic walks up for the first scrollable ancestor, and a wrapper that
    /// scrolled would capture every anchor before the surface's own scroll port
    /// ever saw it.
    className?: string
    /// The change's parsed sections, for the outline's per-section task
    /// counts. Passed by ONE caller, for ONE case — a live change's tasks
    /// artifact — so every other surface shows no counts by construction
    /// (`document-outline`: *Section Progress in a Tasks Outline*).
    sections?: Section[]
}

/// Separator for the composite identities below. A NUL cannot occur in a
/// workspace path, change id, capability name or relative path, so no two
/// distinct documents collide on one identity.
const SEP = "\u0000"

/// A stable string identity for the document a source names. Used both as an
/// effect dependency (the source object is rebuilt every render) and to drop a
/// read whose document the user has already navigated away from.
export function documentIdentity(source: DocumentSource | null): string | null {
    if (!source) return null
    if (source.kind === "file") {
        return ["file", source.root, source.path].join(SEP)
    }
    const { workspace, changeId, artifactKind, capability } = source.target
    return ["artifact", workspace, changeId, artifactKind, capability ?? ""].join(SEP)
}

/// The root-relative path of the file a source names — the base that markdown
/// links resolve against, and the path a document watch registers.
///
/// For an artifact this mirrors the Rust side's `resolve_artifact_path` match
/// exactly. `changeId` already carries the `archive/<dir>` prefix for an
/// archived change, so this doubles as the archive path with no special-casing.
export function documentPath(source: DocumentSource): string {
    if (source.kind === "file") return source.path
    const target = source.target
    const changeDir = `openspec/changes/${target.changeId}`
    switch (target.artifactKind) {
        case "proposal":
            return `${changeDir}/proposal.md`
        case "design":
            return `${changeDir}/design.md`
        case "tasks":
            return `${changeDir}/tasks.md`
        case "spec":
            return `${changeDir}/specs/${target.capability ?? ""}/spec.md`
    }
}

/// The authorized root a source reads against — a workspace/worktree path for
/// an artifact, the browse root for a file.
export function documentRoot(source: DocumentSource): string {
    return source.kind === "file" ? source.root : source.target.workspace
}

/// Whether two heading lists describe the same document structure. The
/// renderer reports its headings after every commit, so the receiver must be
/// able to say "nothing changed" and hand back the previous array — otherwise
/// each report re-renders, which reports again.
function sameHeadings(a: HeadingEntry[], b: HeadingEntry[]): boolean {
    if (a.length !== b.length) return false
    return a.every((entry, index) => {
        const other = b[index]!
        return (
            entry.level === other.level &&
            entry.line === other.line &&
            entry.id === other.id &&
            entry.text === other.text
        )
    })
}

export function DocumentView({
    source,
    scrollAnchor = null,
    header,
    empty,
    errorTitle = "Couldn't load document",
    className = "detail-pane",
    onOpenReader,
    sections,
}: DocumentViewProps) {
    const [state, dispatch] = useReducer(reduce, INITIAL)
    const [missing, setMissing] = useState(false)
    const rootRef = useRef<HTMLDivElement>(null)
    const containerRef = useRef<HTMLDivElement>(null)
    const headerRef = useRef<HTMLDivElement>(null)
    // The last anchor this surface actually scrolled to; see the anchor effect.
    const consumedAnchor = useRef<ScrollAnchor>(null)
    const { content, modifiedAt, error, loading } = state

    // The rendered document's headings, reported by `MarkdownView` after each
    // commit. Deliberately NOT cleared when the document changes: the renderer
    // reports for whatever it is currently showing, and during a `select` that
    // is still the outgoing document — so the outline always describes what is
    // on screen. (Clearing here would also race: a child's effect runs before
    // its parent's, so the clear would land on the fresh list.)
    const [headings, setHeadings] = useState<HeadingEntry[]>([])
    const headingsRef = useRef(headings)
    headingsRef.current = headings
    const handleHeadings = useCallback((next: HeadingEntry[]) => {
        setHeadings((prev) => (sameHeadings(prev, next) ? prev : next.slice()))
    }, [])

    // Within-document navigation the document view owns: an outline entry or a
    // fragment link, both of which name a heading and scroll to its line. Held
    // here rather than pushed at the surface, because neither changes the
    // address (`document-outline`: *Outline Navigation* — "no history entry").
    const [ownAnchor, setOwnAnchor] = useState<ScrollAnchor>(null)
    const scrollToLine = useCallback((line: number) => {
        // A fresh object every time, so activating the same entry twice
        // scrolls twice — `consumedAnchor` compares by identity.
        setOwnAnchor({ kind: "line", line })
    }, [])
    const handleFragment = useCallback((id: string) => {
        const match = headingsRef.current.find((entry) => entry.id === id)
        if (match) setOwnAnchor({ kind: "line", line: match.line })
    }, [])
    // The heading whose line the outline marks as current.
    const [currentLine, setCurrentLine] = useState<number | null>(null)

    const identity = documentIdentity(source)
    // Monotonic token for the read that is allowed to land. Issuing a read
    // invalidates every earlier one, which a document-identity comparison
    // cannot do: two concurrent reads of the *same* document (navigate A → B →
    // A) both match the identity, so whichever settles last wins — and that is
    // not necessarily the one issued last.
    const loadSeq = useRef(0)
    // Trigger of the read currently outstanding, for `effectiveTrigger`.
    const pendingTrigger = useRef<LoadTrigger | null>(null)
    // The source the effects below act on, read through a ref so `load` can be
    // keyed on the identity string rather than on a fresh object every render.
    const sourceRef = useRef(source)
    sourceRef.current = source

    const load = useCallback(
        async (requested: LoadTrigger): Promise<void> => {
            const current = sourceRef.current
            if (!current) return
            const trigger = effectiveTrigger(requested, pendingTrigger.current)
            const seq = ++loadSeq.current
            pendingTrigger.current = trigger
            dispatch({ kind: trigger })
            try {
                const read =
                    current.kind === "artifact"
                        ? await readArtifact(
                              current.target.workspace,
                              current.target.changeId,
                              current.target.artifactKind,
                              current.target.capability,
                          )
                        : {
                              body: await readWorkspaceFile(current.root, current.path),
                              modifiedAt: null,
                          }
                if (seq !== loadSeq.current) return
                pendingTrigger.current = null
                setMissing(false)
                dispatch({
                    kind: "resolved",
                    trigger,
                    content: read.body,
                    modifiedAt: read.modifiedAt,
                })
            } catch (err) {
                if (seq !== loadSeq.current) return
                pendingTrigger.current = null
                // The document stopped resolving at the address this surface
                // was opened for. The reducer deliberately keeps the content;
                // this only lets a header say so.
                if (trigger === "watch") setMissing(true)
                dispatch({ kind: "failed", trigger, error: String(err) })
            }
        },
        // Keyed on the identity string, not the source object: a re-render that
        // rebuilds an equal source must not re-issue a read.
        // eslint-disable-next-line react-hooks/exhaustive-deps
        [identity],
    )

    // Fetch when the document identity changes — section / task clicks within
    // the same file leave this dep unchanged so no refetch.
    useEffect(() => {
        // Whatever was true of the PREVIOUS document says nothing about this
        // one. Without this the header renders the new document's identity
        // beside "no longer present" for the whole duration of its read —
        // `reduce`'s `select` branch deliberately keeps the outgoing content
        // on screen, so there is a real window in which both are shown.
        setMissing(false)
        if (identity === null) {
            // Invalidate any in-flight read so it cannot repaint over a surface
            // that no longer has a document.
            loadSeq.current += 1
            pendingTrigger.current = null
            dispatch({ kind: "cleared" })
            return
        }
        void load("select")
    }, [identity, load])

    // Register a document watch for as long as this surface shows this
    // document. Reference-counted in the shared layer, so several surfaces on
    // one document share a single filesystem watch and each releases its own
    // (`document-watch`: *Watch Registration Is Reference-Counted*).
    useEffect(() => {
        const current = sourceRef.current
        if (!current) return
        const root = documentRoot(current)
        const relPath = documentPath(current)
        void watchDocument(root, relPath).catch((err) => {
            // A refused registration leaves this surface on the cache-updated
            // subscription alone — stale for a document outside
            // `openspec/changes/`, which is worth a warning rather than silence.
            console.warn(
                "document view: failed to watch document; it may not refresh on its own:",
                err,
            )
        })
        return () => {
            void unwatchDocument(root, relPath).catch(() => {
                // Best-effort: the owner release on window/stream teardown is
                // the backstop that keeps a watch from being stranded.
            })
        }
    }, [identity])

    // Re-read on either notification. Coalesced so one debounced backend batch
    // — or both mechanisms reporting one edit to a document that lies inside
    // `openspec/changes/` — produces a single read (`document-watch`:
    // *Independent of the Workspace Watcher*).
    //
    // Deliberately NOT filtered on the event payload: `WatcherManager`'s
    // status-refresh paths emit `Updated` with whatever tracked workspace
    // happens to be first in the cached views, as a carrier for a "refetch
    // everything" nudge. Filtering would silently drop refreshes for a surface
    // whose workspace was not the carrier — invisible with one workspace
    // registered, routine across worktrees. Redundant reads are made free by
    // the equality guard in `reduce` instead.
    const scheduleRefresh = useCoalescedRefetch(() => load("watch"))

    useEffect(() => {
        let cancelled = false
        const offs: UnlistenFn[] = []
        const attach = (pending: Promise<UnlistenFn>, what: string) => {
            void pending
                .then((off) => {
                    if (cancelled) {
                        off()
                        return
                    }
                    offs.push(off)
                })
                // Without this the rejection is silent: the subscription stays
                // absent, the effect never retries (its only dep is stable),
                // and the surface is dead for the session while the tree keeps
                // refreshing from its own subscriptions — indistinguishable
                // from "nothing changed".
                .catch((err) => {
                    console.warn(
                        `document view: failed to subscribe to ${what}; ` +
                            "the open document will not refresh on its own:",
                        err,
                    )
                })
        }
        attach(onCacheUpdated(() => scheduleRefresh()), "cache updates")
        attach(
            onDocumentChanged((payload) => {
                // Unlike a cache event — whose `workspace` is a carrier for
                // "refetch everything" and must NOT be filtered on — a document
                // change names exactly one document, so every other surface can
                // ignore it. With a main window and several readers open, one
                // save would otherwise cost a read and a markdown re-parse in
                // every one of them.
                //
                // Matched on the relative path alone: the event echoes the root
                // the SERVICE canonicalised, which is not guaranteed to be
                // spelled the way this surface holds it. A same-named file in
                // another workspace therefore also refreshes — one extra read,
                // made free by the equality guard in `reduce`, which is the
                // safe direction to be wrong in.
                const current = sourceRef.current
                if (current && payload.relPath === documentPath(current)) {
                    scheduleRefresh()
                }
            }),
            "document changes",
        )
        return () => {
            cancelled = true
            for (const off of offs) off()
        }
    }, [scheduleRefresh])

    // Scroll to the requested anchor once the markdown is in the DOM. We walk
    // up to find the scrollable ancestor and set its scrollTop directly instead
    // of relying on Element.scrollIntoView, which has produced inconsistent
    // results in WebKit when the doc was freshly mounted.
    //
    // The effect depends on `content` because it measures committed layout, but
    // content also arrives from background refreshes. `consumedAnchor` is what
    // separates the two: an anchor scrolls once, when it is new. A reader parked
    // on a task while `tasks.md` changes underneath them must not be yanked back
    // to it on every batch (`spec-browser`: *Reading position survives a refresh
    // the user did not initiate*).
    // The surface's own anchor wins over the caller's while it stands; a new
    // caller anchor, or a new document, drops it.
    const anchor = ownAnchor ?? scrollAnchor
    useEffect(() => {
        setOwnAnchor(null)
    }, [identity, scrollAnchor])

    useEffect(() => {
        if (!anchor || !content || !containerRef.current) return
        if (anchor === consumedAnchor.current) return
        // A `select` deliberately keeps the outgoing document rendered while the
        // next one loads, so without this the double-rAF below would measure the
        // *previous* document, scroll it, and mark the anchor consumed — leaving
        // the document the user actually clicked unscrolled.
        if (loading) return

        // Double-rAF: first frame waits for React to commit, second frame waits
        // for layout (rehype-highlight, font load, etc.) to settle so
        // getBoundingClientRect returns final positions.
        let raf2 = 0
        const raf1 = requestAnimationFrame(() => {
            raf2 = requestAnimationFrame(() => {
                const container = containerRef.current
                if (!container) return

                const scrollParent = findScrollableAncestor(container)
                if (!scrollParent) return

                // By SOURCE LINE — the renderer stamps `data-line` on every
                // heading and every list item, so one query serves both.
                const target = container.querySelector<HTMLElement>(
                    `[data-line="${anchor.line}"]`,
                )
                if (!target) return

                const parentTop = scrollParent.getBoundingClientRect().top
                const targetTop = target.getBoundingClientRect().top
                const relative = scrollParent.scrollTop + (targetTop - parentTop)

                // A sticky header covers the top of the scroll port, so the box
                // actually visible starts below it and both offsets have to
                // clear it — without this a section lands *underneath* the
                // header and reads as not having scrolled at all.
                //
                // Measured, not read from a constant: a change name rendered in
                // full wraps on a narrow surface, so the header's height
                // genuinely varies at runtime and any fixed value would be wrong
                // exactly when the name is longest. Zero when the surface
                // attached no header, which is correct for one that has none.
                const headerH = headerRef.current?.offsetHeight ?? 0

                // Pin near the top with breathing room, below the header, so
                // the target comes to rest fully visible directly beneath it
                // (`document-outline`: *Outline Navigation*).
                const offset = headerH + 16

                // Marked here rather than at effect entry: a run cancelled
                // before this point never moved the reader, so the anchor is
                // still owed a scroll and the next run should honour it.
                consumedAnchor.current = anchor
                scrollParent.scrollTo({
                    top: Math.max(0, relative - offset),
                    behavior: "smooth",
                })
            })
        })

        return () => {
            cancelAnimationFrame(raf1)
            if (raf2) cancelAnimationFrame(raf2)
        }
    }, [anchor, content, loading])

    // Publish the rendered header's height so the outline can sit clear of it
    // in CSS. Written imperatively rather than held in state: the header's
    // height changes when the change name wraps or the macOS clearance
    // applies, and neither is worth a React render of the document.
    useLayoutEffect(() => {
        const root = rootRef.current
        const headerEl = headerRef.current
        if (!root) return
        const publish = () => {
            root.style.setProperty(
                "--doc-header-h",
                `${headerEl?.offsetHeight ?? 0}px`,
            )
        }
        publish()
        if (!headerEl || typeof ResizeObserver === "undefined") return
        const observer = new ResizeObserver(publish)
        observer.observe(headerEl)
        return () => observer.disconnect()
    }, [content, headings])

    // Track the outline's current entry — the heading that most recently
    // passed the top of the reading area. An `IntersectionObserver` rooted at
    // the scroll port wakes the recomputation; the answer itself is measured,
    // because "most recently passed" is an ordering question that a set of
    // intersection flags cannot answer on its own.
    //
    // Re-created when the headings or the content change, so no observer
    // outlives the document it was built for, and disconnected on unmount.
    useEffect(() => {
        const container = containerRef.current
        // Only the headings the outline actually LISTS: tracking a level-four
        // heading would make it "current" and leave no entry marked at all, so
        // scrolling into a scenario would silently clear the section above it.
        const entries = outlineEntries(headings)
        if (!container || entries.length === 0) {
            setCurrentLine(null)
            return
        }
        const targets = entries
            .map((entry) =>
                container.querySelector<HTMLElement>(`[data-line="${entry.line}"]`),
            )
            .filter((el): el is HTMLElement => el !== null)
        if (targets.length === 0) {
            setCurrentLine(null)
            return
        }
        const scrollParent = findScrollableAncestor(container)
        const recompute = () => {
            const portTop =
                (scrollParent?.getBoundingClientRect().top ?? 0) +
                (headerRef.current?.offsetHeight ?? 0)
            let current = Number(targets[0]!.dataset.line)
            for (const el of targets) {
                if (el.getBoundingClientRect().top - portTop > 1) break
                current = Number(el.dataset.line)
            }
            setCurrentLine(Number.isFinite(current) ? current : null)
        }
        const observer = new IntersectionObserver(recompute, {
            root: scrollParent,
            threshold: [0, 1],
        })
        for (const el of targets) observer.observe(el)
        recompute()
        return () => observer.disconnect()
    }, [headings, content])

    if (!source) {
        return <>{empty ?? null}</>
    }

    if (loading && content == null) {
        return <div className="detail-pane-status">Loading…</div>
    }

    if (error) {
        return (
            <EmptyState
                title={errorTitle}
                body={<code className="detail-pane-error">{error}</code>}
            />
        )
    }

    if (content == null) {
        return null
    }

    return (
        // No `overflow` on this wrapper: `findScrollableAncestor` walks up from
        // the markdown container looking for the first scrollable ancestor, so a
        // wrapper that scrolled would capture every anchor before the surface's
        // own scroll port ever saw it.
        <div className={className} ref={rootRef}>
            {header?.(
                { modifiedAt, missing },
                headerRef,
                onOpenReader ? <OpenReaderControl onClick={onOpenReader} /> : null,
            )}
            {/* A zero-height sticky rail, right after the header and before
                the prose: it contributes nothing to layout, so the column
                occupies the same position whether or not an outline is shown,
                and it is the query container the placement rules measure
                (design D8). The outline hangs off it, absolutely positioned in
                the column's trailing gutter. */}
            <div className="doc-outline-rail">
                <DocumentOutline
                    headings={headings}
                    currentLine={currentLine}
                    sections={sections}
                    onActivate={scrollToLine}
                />
            </div>
            <MarkdownView
                // Keyed on the document's identity so navigating to a DIFFERENT
                // document remounts the subtree. `react-markdown` does not key
                // its own children, so React would otherwise reconcile fence
                // components by position and reuse one that still held a
                // maximized figure, which would then display the newly loaded
                // document's diagram (`spec-browser`: *Maximized Figure View*).
                //
                // A same-document content edit keeps this key, so a
                // watcher-driven reparse still reuses the subtree and updates a
                // maximized figure in place instead of closing it.
                key={identity ?? undefined}
                content={content}
                containerRef={containerRef}
                root={documentRoot(source)}
                basePath={documentPath(source)}
                // Both are `useCallback([])`, which the memo on `MarkdownView`
                // depends on — see its closing note.
                onHeadings={handleHeadings}
                onFragment={handleFragment}
            />
        </div>
    )
}

/// The document's outline: one entry per level-two and level-three heading, in
/// document order, level-three entries subordinate (`document-outline`:
/// *Document Outline Surface*).
///
/// Renders nothing — not an empty shell — when the document has no heading to
/// list, so no space is reserved for one. Whether it is SHOWN at all when it
/// does have entries is a CSS question, decided by a container query against
/// the surface's own width (design D8); a layout decision expressed in
/// JavaScript would reflow a frame late and need a second implementation for
/// reader windows.
function DocumentOutline({
    headings,
    currentLine,
    sections,
    onActivate,
}: {
    headings: HeadingEntry[]
    currentLine: number | null
    sections?: Section[]
    onActivate: (line: number) => void
}) {
    const entries = outlineEntries(headings)
    if (entries.length === 0) return null
    return (
        <nav className="doc-outline" aria-label="Outline">
            <ol className="doc-outline-list">
                {entries.map((entry) => {
                    const section = sections
                        ? sectionForHeading(entry.text, sections)
                        : undefined
                    const progress = section ? sectionProgress(section) : null
                    return (
                        <li
                            key={`${entry.line}:${entry.id}`}
                            className={`doc-outline-item doc-outline-item--l${entry.level}`}
                        >
                            <button
                                type="button"
                                className="doc-outline-link"
                                // Activation SCROLLS — no address change, no
                                // history entry, no re-read.
                                onClick={() => onActivate(entry.line)}
                                aria-current={
                                    entry.line === currentLine ? "true" : undefined
                                }
                            >
                                <span className="doc-outline-text">{entry.text}</span>
                                {progress && progress.total > 0 && (
                                    <span className="doc-outline-progress">
                                        {progress.completed >= progress.total ? (
                                            <CompletionMark />
                                        ) : (
                                            `${progress.completed}/${progress.total}`
                                        )}
                                    </span>
                                )}
                            </button>
                        </li>
                    )
                })}
            </ol>
        </nav>
    )
}

function findScrollableAncestor(el: HTMLElement): HTMLElement | null {
    let parent: HTMLElement | null = el.parentElement
    while (parent) {
        const style = getComputedStyle(parent)
        if (
            (style.overflowY === "auto" || style.overflowY === "scroll") &&
            parent.scrollHeight > parent.clientHeight
        ) {
            return parent
        }
        parent = parent.parentElement
    }
    return null
}

/// The visible way to open the current document in its own window.
///
/// Cmd/Ctrl-click on a row does the same thing, but a modifier chord is
/// invisible — and on a touch device there is no modifier key at all, which
/// would leave reader windows unreachable rather than merely undiscovered.
/// So this control follows the same contract the figure-maximize affordance
/// does: a real button (hence keyboard-operable), revealed on hover where
/// hover exists, rendered at rest where it does not, and given an enlarged
/// hit area on a coarse pointer — see the *Essential Controls Are
/// Discoverable Without Hover* and *Interactive Targets Meet a Minimum Size
/// on Coarse Pointers* requirements in the `touch-input` capability.
function OpenReaderControl({ onClick }: { onClick: () => void }) {
    return (
        <button
            type="button"
            className="identity-open-reader"
            onClick={onClick}
            aria-label="Open in its own window"
            title="Open in its own window"
        >
            <OpenInWindow width={13} height={13} />
        </button>
    )
}
