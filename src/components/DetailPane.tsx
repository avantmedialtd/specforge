import { useCallback, useEffect, useReducer, useRef } from "react"
import type { RefObject } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import { onCacheUpdated, readArtifact } from "../api"
import {
    branchForWorktree,
    changeDirectoryName,
    isArchivedChangeId,
} from "../changeIdentity"
import {
    effectiveTrigger,
    INITIAL,
    reduce,
    type LoadTrigger,
} from "../detail/refreshPolicy"
import { useCoalescedRefetch } from "../hooks/useCoalescedRefetch"
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
    /// for the identity header. Deliberately not folded into `target` — see
    /// `branchForWorktree`.
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
    const { content, error, loading } = state

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
                const text = await readArtifact(
                    workspace,
                    changeId,
                    artifactKind,
                    capability,
                )
                if (seq !== loadSeq.current) return
                pendingTrigger.current = null
                dispatch({ kind: "resolved", trigger, content: text })
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
                branch={
                    isArchivedChangeId(target.changeId)
                        ? null
                        : branchForWorktree(target.workspace, views)
                }
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
    /// The owning worktree's branch, or null when there is none to name (flat
    /// workspace, detached HEAD, untracked path). Null renders no chip.
    branch: string | null
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
/// selection and copied along with the name (design.md D2).
function ChangeIdentityHeader({
    headerRef,
    changeId,
    branch,
}: ChangeIdentityHeaderProps) {
    // Two elements, not one: the outer bar carries the sticky positioning and
    // an opaque background spanning the full pane width, so scrolled content
    // cannot show through it; the inner element carries the prose column's
    // width bound and horizontal origin, so the identity sits directly above
    // the document's first line instead of floating left of it on a wide
    // window (design.md D5). A single element cannot do both — `max-width`
    // would clip the background to the column.
    return (
        <div className="detail-identity" ref={headerRef}>
            <div className="detail-identity-inner">
                <CopyableIdentity
                    value={changeDirectoryName(changeId)}
                    noun="change name"
                />
                {branch && <span className="identity-branch">{branch}</span>}
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
