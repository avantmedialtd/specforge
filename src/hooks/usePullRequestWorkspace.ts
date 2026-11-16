import { useCallback, useLayoutEffect, useRef, useState, type RefObject } from "react"
import {
    readWorkspace,
    restoredOffset,
    workspaceKey,
    workspaceScrollPort,
    writeWorkspace,
    type WorkspaceSurface,
    type WorkspaceTab,
    type WorkspacePlace,
} from "../pullRequestWorkspace"
import type { PullRequestDetail, PullRequestReference } from "../types"

/** The caller keys the owner by PR. No storage listener moves another window. */
export function usePullRequestWorkspace(
    reference: PullRequestReference,
    surface: WorkspaceSurface,
    root: RefObject<HTMLDivElement | null>,
    detail: PullRequestDetail | null,
) {
    const key = workspaceKey(reference, surface)
    const [initial] = useState(() => readWorkspace(key))
    const [tab, setTab] = useState(initial.tab)
    const [filesMounted, setFilesMounted] = useState(initial.tab === "files")
    const presentation = useRef({ ...initial })
    const scrolls = useRef<Partial<Record<WorkspaceTab, number>>>({})
    const latest = useRef({ tab, detail, key })
    latest.current = { tab, detail, key }
    const pendingRestore = useRef(initial.tab === "files" ? initial.place : null)
    const resumed = useRef<{ place: WorkspacePlace; top: number } | null>(null)
    const hasDetail = detail !== null

    const capture = useCallback(() => {
        const { tab, detail } = latest.current
        const port = workspaceScrollPort(root.current)
        if (!port) return
        // A cache-only paint may precede a newer network detail. Keep its
        // restoration eligible for a safe header fallback until the reader
        // actually moves away, not indefinitely through an active review.
        if (resumed.current && Math.abs(port.scrollTop - resumed.current.top) > 1) resumed.current = null
        scrolls.current[tab] = port.scrollTop
        if (tab === "files" && detail) {
            const sections = root.current?.querySelectorAll<HTMLElement>("[data-file-key]")
            const inset = root.current?.querySelector<HTMLElement>(".pull-request-view-header")?.offsetHeight ?? 0
            const top = port.getBoundingClientRect().top + inset
            let current: HTMLElement | undefined
            for (const section of sections ?? []) {
                if (!current || section.getBoundingClientRect().top <= top) current = section
            }
            presentation.current.place = current?.dataset.fileKey ? {
                path: current.dataset.fileKey,
                offset: Math.max(0, top - current.getBoundingClientRect().top),
                head: detail.headCommit,
                base: detail.baseCommit,
            } : null
        }
    }, [root])

    const choose = useCallback((next: WorkspaceTab) => {
        capture()
        pendingRestore.current = null
        resumed.current = null
        presentation.current = { ...presentation.current, tab: next }
        writeWorkspace(latest.current.key, presentation.current)
        if (next === "files") setFilesMounted(true)
        setTab(next)
    }, [capture])

    useLayoutEffect(() => {
        const port = workspaceScrollPort(root.current)
        if (!port) return
        port.scrollTop = scrolls.current[tab] ?? 0
    }, [tab, root])

    useLayoutEffect(() => {
        const port = workspaceScrollPort(root.current)
        if (!port) return
        const cached = resumed.current?.place
        const place = pendingRestore.current ??
            (cached && detail && (cached.head !== detail.headCommit || cached.base !== detail.baseCommit) ? cached : null)
        if (!place || !detail) return
        // Diff layout is measured in a child layout effect before it draws rows.
        const frame = requestAnimationFrame(() => {
            const sections = root.current?.querySelectorAll<HTMLElement>("[data-file-key]")
            const section = Array.from(sections ?? []).find((item) => item.dataset.fileKey === place.path)
            if (section) {
                port.scrollTop += section.getBoundingClientRect().top -
                    port.getBoundingClientRect().top +
                    restoredOffset(place, detail.headCommit, detail.baseCommit) -
                    (root.current?.querySelector<HTMLElement>(".pull-request-view-header")?.offsetHeight ?? 0)
            }
            resumed.current = section && place.head === detail.headCommit && place.base === detail.baseCommit
                ? { place, top: port.scrollTop } : null
            pendingRestore.current = null
        })
        return () => cancelAnimationFrame(frame)
    }, [tab, detail, root])

    useLayoutEffect(() => {
        const port = workspaceScrollPort(root.current)
        if (!port) return
        // Scroll only updates refs. Persist on navigation/unmount/pagehide,
        // never synchronously serialize localStorage on every scroll event.
        const persist = () => {
            capture()
            writeWorkspace(latest.current.key, presentation.current)
        }
        port.addEventListener("scroll", capture, { passive: true })
        window.addEventListener("pagehide", persist)
        return () => {
            persist()
            port.removeEventListener("scroll", capture)
            window.removeEventListener("pagehide", persist)
        }
    }, [capture, root, hasDetail])

    return { tab, choose, filesMounted }
}
