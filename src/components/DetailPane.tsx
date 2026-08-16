import { useCallback, useEffect, useReducer, useRef } from "react"
import type { RefObject } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import { onCacheUpdated, readArtifact } from "../api"
import {
    branchChipForWorktree,
    changeDirectoryName,
    identChipClass,
    isArchivedChangeId,
    type BranchChip,
} from "../changeIdentity"
import {
    effectiveTrigger,
    INITIAL,
    reduce,
    type LoadTrigger,
} from "../detail/refreshPolicy"
import { useCoalescedRefetch } from "../hooks/useCoalescedRefetch"
import { useTickingNow } from "../hooks/useTickingNow"
import { formatRelativeTime, RELATIVE_TIME_WIDEST } from "../relativeTime"
import type { ArtifactRenderTarget, WorkspaceView } from "../types"
import { CopyableIdentity } from "./CopyableIdentity"
import { EmptyState } from "./EmptyState"
import { MarkdownView } from "./MarkdownView"

export type ScrollAnchor =
    | { kind: "section"; index: number }
    | { kind: "task"; lineNumber: number }
    | null

interface DetailPaneProps {
    target: ArtifactRenderTarget | null
    scrollAnchor: ScrollAnchor
    /// Workspace views, used only to resolve the rendered artifact's branch
    /// and its owning workspace's palette colour for the identity header.
    /// Deliberately not folded into `target` — see `branchChipForWorktree`.
    ///
    /// Optional because the Archive reader renders through this same pane and
    /// has no views to give: an archived change never shows a branch, so it has
    /// no use for them. The suppression is enforced by `isArchivedChangeId`
    /// below rather than by the caller passing nothing, so an Archive reader
    /// that later gained views still would not sprout a branch chip.
    views?: WorkspaceView[]
}

/// The root-relative path of the artifact `target` points at — mirrors the
/// Rust side's `resolve_artifact_path` match exactly, so link hrefs resolve
/// against the same directory the backend would resolve an artifact read
/// from. `changeId` already carries the `archive/<dir>` prefix for an
/// archived change (see `ArchiveView`), so this doubles as the archive path
/// with no special-casing here.
function artifactBasePath(target: ArtifactRenderTarget): string {
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

/// A stable string identity for the artifact a target names. Used both as an
/// effect dependency (the target object itself is rebuilt every render) and to
/// drop a read whose artifact the user has already navigated away from.
function targetIdentity(target: ArtifactRenderTarget | null): string | null {
    if (!target) return null
    // NUL-joined: it cannot occur in a workspace path, change id, or
    // capability name, so no two distinct targets collide on one identity.
    return [
        target.workspace,
        target.changeId,
        target.artifactKind,
        target.capability ?? "",
    ].join("\u0000")
}

export function DetailPane({
    target,
    scrollAnchor,
    views = [],
}: DetailPaneProps) {
    const [state, dispatch] = useReducer(reduce, INITIAL)
    const containerRef = useRef<HTMLDivElement>(null)
    // The sticky identity header, measured (not assumed) by the scroll-anchor
    // effect below — the change name is rendered in full and may wrap, so the
    // header's height is not a constant.
    const headerRef = useRef<HTMLDivElement>(null)
    // The last anchor this pane actually scrolled to; see the anchor effect.
    const consumedAnchor = useRef<ScrollAnchor>(null)
    const { content, modifiedAt, error, loading } = state

    const identity = targetIdentity(target)
    // Monotonic token for the read that is allowed to land. Issuing a read
    // invalidates every earlier one, which an artifact-identity comparison
    // cannot do: two concurrent reads of the *same* artifact (navigate A → B →
    // A) both match the identity, so whichever settles last wins — and that is
    // not necessarily the one issued last. Same role as `artifact_gen` in the
    // terminal frontend.
    const loadSeq = useRef(0)
    // Trigger of the read currently outstanding, for `effectiveTrigger`.
    const pendingTrigger = useRef<LoadTrigger | null>(null)

    const workspace = target?.workspace
    const changeId = target?.changeId
    const artifactKind = target?.artifactKind
    const capability = target?.capability

    const load = useCallback(
        async (requested: LoadTrigger): Promise<void> => {
            if (!workspace || !changeId || !artifactKind) return
            const trigger = effectiveTrigger(requested, pendingTrigger.current)
            const seq = ++loadSeq.current
            pendingTrigger.current = trigger
            dispatch({ kind: trigger })
            try {
                const read = await readArtifact(
                    workspace,
                    changeId,
                    artifactKind,
                    capability,
                )
                if (seq !== loadSeq.current) return
                pendingTrigger.current = null
                dispatch({
                    kind: "resolved",
                    trigger,
                    content: read.body,
                    modifiedAt: read.modifiedAt,
                })
            } catch (err) {
                if (seq !== loadSeq.current) return
                pendingTrigger.current = null
                dispatch({ kind: "failed", trigger, error: String(err) })
            }
        },
        [workspace, changeId, artifactKind, capability],
    )

    // Fetch when the file identity changes — section / task clicks within the
    // same file leave this dep unchanged so no refetch.
    useEffect(() => {
        if (identity === null) {
            // Invalidate any in-flight read so it cannot repaint over a pane
            // that no longer has a target.
            loadSeq.current += 1
            pendingTrigger.current = null
            dispatch({ kind: "cleared" })
            return
        }
        void load("select")
    }, [identity, load])

    // Re-read the open artifact whenever the watcher reports that anything
    // changed (`spec-browser`: *Reactive Updates from Filesystem*). Coalesced
    // so one debounced backend batch produces a single read.
    //
    // Deliberately NOT filtered on the event payload's `workspace`: the
    // status-refresh paths in `WatcherManager` (`refresh_status_and_notify`,
    // `refresh_status_for`) emit `Updated` with whatever tracked workspace
    // happens to be first in the cached views, as a carrier for a "refetch
    // everything" nudge. Filtering on it would silently drop refreshes for a
    // pane whose workspace was not the carrier — invisible with one workspace
    // registered, routine across worktrees. Redundant reads are made free by
    // the equality guard in `reduce` instead.
    const scheduleRefresh = useCoalescedRefetch(() => load("watch"))

    useEffect(() => {
        let unlisten: UnlistenFn | undefined
        let cancelled = false
        void onCacheUpdated(() => scheduleRefresh())
            .then((off) => {
                if (cancelled) {
                    off()
                    return
                }
                unlisten = off
            })
            // Without this the rejection is silent: `unlisten` stays undefined,
            // the effect never retries (its only dep is stable), and the pane
            // is dead for the session while the tree keeps refreshing from its
            // own subscriptions — indistinguishable from "nothing changed".
            .catch((err) => {
                console.warn(
                    "detail pane: failed to subscribe to cache updates; " +
                        "the open artifact will not refresh on its own:",
                    err,
                )
            })
        return () => {
            cancelled = true
            unlisten?.()
        }
    }, [scheduleRefresh])

    // Scroll to the requested anchor once the markdown is in the DOM. We
    // walk up to find the scrollable ancestor and set its scrollTop directly
    // instead of relying on Element.scrollIntoView, which has produced
    // inconsistent results in WebKit when the doc was freshly mounted.
    //
    // The effect depends on `content` because it measures committed layout,
    // but content now also arrives from background refreshes. `consumedAnchor`
    // is what separates the two: an anchor scrolls once, when it is new. A
    // reader parked on a task while `tasks.md` changes underneath them must
    // not be yanked back to it on every batch (`spec-browser`: *Reading
    // position survives a refresh the user did not initiate*). `App` builds a
    // fresh anchor object per selection, so clicking the same node twice
    // still scrolls both times.
    useEffect(() => {
        if (!scrollAnchor || !content || !containerRef.current) return
        if (scrollAnchor === consumedAnchor.current) return
        // A `select` deliberately keeps the outgoing artifact rendered while
        // the next one loads, so without this the double-rAF below would
        // measure the *previous* document, scroll it, and mark the anchor
        // consumed — leaving the artifact the user actually clicked unscrolled.
        if (loading) return

        // Double-rAF: first frame waits for React to commit, second frame
        // waits for layout (rehype-highlight, font load, etc.) to settle so
        // getBoundingClientRect returns final positions.
        let raf2 = 0
        const raf1 = requestAnimationFrame(() => {
            raf2 = requestAnimationFrame(() => {
                const container = containerRef.current
                if (!container) return

                const scrollParent = findScrollableAncestor(container)
                if (!scrollParent) return

                const target: HTMLElement | null =
                    scrollAnchor.kind === "section"
                        ? (container.querySelectorAll<HTMLHeadingElement>(
                              "h2",
                          )[scrollAnchor.index] ?? null)
                        : container.querySelector<HTMLElement>(
                              `li[data-line="${scrollAnchor.lineNumber}"]`,
                          )
                if (!target) return

                const parentTop = scrollParent.getBoundingClientRect().top
                const targetTop = target.getBoundingClientRect().top
                const relative =
                    scrollParent.scrollTop + (targetTop - parentTop)

                // The sticky identity header covers the top of the scroll port,
                // so the box actually visible to the reader starts below it and
                // both offsets have to clear it — without this a section lands
                // *underneath* the header and reads as not having scrolled at
                // all (`spec-browser`: *Change Identity Header in the Detail
                // Pane*, "an anchored section is not obscured by the header").
                //
                // Measured, not read from a constant or a CSS variable: the
                // change name is rendered in full and wraps on a narrow pane,
                // so the header's height genuinely varies at runtime and any
                // fixed value would be wrong exactly when the name is longest.
                const headerH = headerRef.current?.offsetHeight ?? 0

                // Section: pin near the top with breathing room, below the
                // header. Task: centre within the box the header leaves.
                const offset =
                    scrollAnchor.kind === "section"
                        ? headerH + 16
                        : headerH +
                          (scrollParent.clientHeight -
                              headerH -
                              target.clientHeight) /
                              2

                // Marked here rather than at effect entry: a run cancelled
                // before this point never moved the reader, so the anchor is
                // still owed a scroll and the next run should honour it.
                consumedAnchor.current = scrollAnchor
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
    }, [scrollAnchor, content, loading])

    if (!target) {
        return (
            <EmptyState
                title="Nothing selected"
                body="Pick a Proposal, Design, Tasks, or capability spec from the tree."
            />
        )
    }

    if (loading && content == null) {
        return <div className="detail-pane-status">Loading…</div>
    }

    if (error) {
        return (
            <EmptyState
                title="Couldn't load artifact"
                body={<code className="detail-pane-error">{error}</code>}
            />
        )
    }

    if (content == null) {
        return null
    }

    return (
        // No `overflow` on this wrapper: `findScrollableAncestor` walks up from
        // the markdown container looking for the first scrollable ancestor, so
        // a wrapper that scrolled would capture every anchor before
        // `.split-pane-right` ever saw it.
        <div className="detail-pane">
            <ChangeIdentityHeader
                headerRef={headerRef}
                changeId={target.changeId}
                // An archived change is suppressed here, once, rather than
                // twice downstream: with no chip there is nothing to tint, so
                // an archived change cannot be painted in the colour of the
                // live workspace whose worktree its artifact happened to be
                // read from (`spec-browser`: *Change Identity Header in the
                // Detail Pane*, "an archived change shows no branch chip").
                chip={
                    isArchivedChangeId(target.changeId)
                        ? { branch: null, color: null }
                        : branchChipForWorktree(target.workspace, views)
                }
                // Deliberately NOT suppressed for an archived change, unlike
                // the chip above. A branch is suppressed because an archived
                // change genuinely has none; its file's modification time
                // exists and means exactly what it means for any other artifact
                // (`spec-browser`: *…* — "An archived artifact reports its
                // modification time like any other").
                modifiedAt={modifiedAt}
            />
            <MarkdownView
                content={content}
                containerRef={containerRef}
                root={target.workspace}
                basePath={artifactBasePath(target)}
            />
        </div>
    )
}

interface ChangeIdentityHeaderProps {
    headerRef: RefObject<HTMLDivElement>
    /// The render target's change id — carries the `archive/` prefix for an
    /// archived change, which `changeDirectoryName` strips.
    changeId: string
    /// What the branch chip should say and what colour to say it in. A null
    /// `branch` (flat workspace, detached HEAD, untracked path, archived
    /// change) renders no chip at all; a null `color` renders it neutral.
    chip: BranchChip
    /// When the rendered artifact's own file was last written, unix seconds, or
    /// null when the filesystem reported no usable time — in which case no
    /// label is rendered at all, rather than a date the application invented.
    modifiedAt: number | null
}

/// How long ago the artifact was last written, advancing on its own.
///
/// The words and the tick both come from the shared relative-time module, so
/// this label and the sidebar row naming the same change — visible at the same
/// time — cannot spell the same kind of value two different ways.
///
/// Advancing at all is affordable only because `MarkdownView` is memoized: each
/// tick re-renders `DetailPane`, and the document does not follow it. Without
/// that boundary a ticking label would re-run remark, rehype, mermaid and KaTeX
/// on a timer.
///
/// Formats once and uses the result for both the text and the title, so the two
/// cannot disagree at a tick boundary.
function LastChangedLabel({ modifiedAt }: { modifiedAt: number }) {
    const now = useTickingNow(modifiedAt)
    const text = formatRelativeTime(modifiedAt, now)
    return (
        // A plain span, matching the branch chip's treatment: informational, not
        // interactive, and therefore not a tab stop — the change name remains
        // the pane's single one. The `title` carries the fuller phrasing, since
        // "9 min ago" standing alone does not say what changed.
        //
        // The reserved width is inline rather than in the stylesheet because it
        // is a property of the formatter, not of the design: it is exactly as
        // wide as the widest label that formatter can emit, so rewording a label
        // moves the box with it and the change name never starts shifting on a
        // tick (`spec-browser`: *…* — "The advancing label never moves the
        // change name").
        <span
            className="identity-changed"
            style={{ minWidth: `${RELATIVE_TIME_WIDEST.length}ch` }}
            title={`Last changed ${text}`}
        >
            {text}
        </span>
    )
}

/// Names the change whose artifact the pane is rendering (`spec-browser`:
/// *Change Identity Header in the Detail Pane*).
///
/// The name is the change's DIRECTORY name, not its proposal title: the title
/// is what the tree already shows, while the directory name is the change's
/// filesystem identity and the token a user hands to external tooling. It is
/// rendered in full — the pane is wide enough, and a truncated identifier is
/// worse than useless when the point is to copy it.
///
/// The branch chip is a SIBLING of the name, never a child. The name carries
/// `user-select: all`, so a nested chip would be swept into the same atomic
/// selection and copied along with the name
/// (`archive/2026-08-16-add-change-identity-headers/design.md`, Decision 2).
///
/// The chip is tinted to the owning workspace's palette colour, built by the
/// same `identChipClass` the tree's chip uses — so where the tree ALSO renders
/// a chip, the two render identically. That is the sole-change-row case; a
/// change living in several worktrees renders its instances as plain labels
/// instead (`labelForInstance` in `WorkspaceTree`), so there is no chip there
/// to match and no equivalence to hold.
function ChangeIdentityHeader({
    headerRef,
    changeId,
    chip,
    modifiedAt,
}: ChangeIdentityHeaderProps) {
    // Two elements, not one: the outer bar carries the sticky positioning and
    // an opaque background spanning the full pane width, so scrolled content
    // cannot show through it; the inner element carries the prose column's
    // width bound and horizontal origin, so the identity sits directly above
    // the document's first line instead of floating left of it on a wide
    // window (`archive/2026-08-16-add-change-identity-headers/design.md`,
    // Decision 5). A single element cannot do both — `max-width`
    // would clip the background to the column.
    return (
        <div className="detail-identity" ref={headerRef}>
            <div className="detail-identity-inner">
                <CopyableIdentity
                    value={changeDirectoryName(changeId)}
                    noun="change name"
                />
                {chip.branch && (
                    <span className={identChipClass(chip.color, "identity-branch")}>
                        {chip.branch}
                    </span>
                )}
                {/* A SIBLING of the name, never a child — `.identity-name`
                    carries `user-select: all`, so a nested element would be
                    swept into the atomic selection and copied along with the
                    change name (`spec-browser`: *…* — "The copied value
                    excludes the last-changed label"). */}
                {modifiedAt !== null && (
                    <LastChangedLabel modifiedAt={modifiedAt} />
                )}
            </div>
        </div>
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
