import { useCallback, useEffect, useReducer, useRef } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import { onCacheUpdated, readArtifact } from "../api"
import { INITIAL, reduce, type LoadTrigger } from "../detail/refreshPolicy"
import { useCoalescedRefetch } from "../hooks/useCoalescedRefetch"
import type { ArtifactRenderTarget } from "../types"
import { EmptyState } from "./EmptyState"
import { MarkdownView } from "./MarkdownView"

export type ScrollAnchor =
    | { kind: "section"; index: number }
    | { kind: "task"; lineNumber: number }
    | null

interface DetailPaneProps {
    target: ArtifactRenderTarget | null
    scrollAnchor: ScrollAnchor
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

export function DetailPane({ target, scrollAnchor }: DetailPaneProps) {
    const [state, dispatch] = useReducer(reduce, INITIAL)
    const containerRef = useRef<HTMLDivElement>(null)
    // The last anchor this pane actually scrolled to; see the anchor effect.
    const consumedAnchor = useRef<ScrollAnchor>(null)
    const { content, error, loading } = state

    const identity = targetIdentity(target)
    // The artifact the pane is currently pointed at. A read that settles after
    // the user moved on compares unequal here and is discarded.
    const activeIdentity = useRef<string | null>(null)

    const workspace = target?.workspace
    const changeId = target?.changeId
    const artifactKind = target?.artifactKind
    const capability = target?.capability

    const load = useCallback(
        async (trigger: LoadTrigger): Promise<void> => {
            if (!workspace || !changeId || !artifactKind) return
            const issuedFor = identity
            dispatch({ kind: trigger })
            try {
                const text = await readArtifact(
                    workspace,
                    changeId,
                    artifactKind,
                    capability,
                )
                if (activeIdentity.current !== issuedFor) return
                dispatch({ kind: "resolved", trigger, content: text })
            } catch (err) {
                if (activeIdentity.current !== issuedFor) return
                dispatch({ kind: "failed", trigger, error: String(err) })
            }
        },
        [identity, workspace, changeId, artifactKind, capability],
    )

    // Fetch when the file identity changes — section / task clicks within the
    // same file leave this dep unchanged so no refetch.
    useEffect(() => {
        activeIdentity.current = identity
        if (identity === null) {
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
        void onCacheUpdated(() => scheduleRefresh()).then((off) => {
            if (cancelled) {
                off()
                return
            }
            unlisten = off
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

                // Section: pin near top with breathing room. Task: centre.
                const offset =
                    scrollAnchor.kind === "section"
                        ? 16
                        : (scrollParent.clientHeight - target.clientHeight) / 2

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
    }, [scrollAnchor, content])

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
        <MarkdownView
            content={content}
            containerRef={containerRef}
            root={target.workspace}
            basePath={artifactBasePath(target)}
        />
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
