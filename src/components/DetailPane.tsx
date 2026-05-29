import { useEffect, useRef, useState } from "react"
import { readArtifact } from "../api"
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

export function DetailPane({ target, scrollAnchor }: DetailPaneProps) {
    const [content, setContent] = useState<string | null>(null)
    const [error, setError] = useState<string | null>(null)
    const [loading, setLoading] = useState(false)
    const containerRef = useRef<HTMLDivElement>(null)

    // Fetch only when the file identity changes — section / task clicks
    // within the same file leave these deps unchanged so no refetch.
    useEffect(() => {
        if (!target) {
            setContent(null)
            setError(null)
            return
        }

        let cancelled = false
        setLoading(true)
        setError(null)
        readArtifact(
            target.workspace,
            target.changeId,
            target.artifactKind,
            target.capability,
        )
            .then((text) => {
                if (cancelled) return
                setContent(text)
                setLoading(false)
            })
            .catch((err) => {
                if (cancelled) return
                setError(String(err))
                setContent(null)
                setLoading(false)
            })

        return () => {
            cancelled = true
        }
    }, [
        target?.workspace,
        target?.changeId,
        target?.artifactKind,
        target?.capability,
    ])

    // Scroll to the requested anchor once the markdown is in the DOM. We
    // walk up to find the scrollable ancestor and set its scrollTop directly
    // instead of relying on Element.scrollIntoView, which has produced
    // inconsistent results in WebKit when the doc was freshly mounted.
    useEffect(() => {
        if (!scrollAnchor || !content || !containerRef.current) return

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

    return <MarkdownView content={content} containerRef={containerRef} />
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
