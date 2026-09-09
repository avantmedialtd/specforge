import { useEffect, useMemo, useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { isTauri, listWorkspaceFileRows, setReaderWindowSize } from "../api"
import { copyWorktrees, defaultCopy, rowForPath } from "../fileCopies"
import { useWorkspaces } from "../hooks/useWorkspaces"
import { useDocumentWidth } from "../hooks/useDocumentWidth"
import { readerTitle } from "../readerTitle"
import { decodeAddress } from "../routing/codec"
import { findViewByRoot, resolveAddress } from "../routing/resolve"
import type { FileScope, WorkspaceView } from "../types"
import {
    DocumentView,
    MissingDocumentLabel,
    type DocumentSource,
} from "./DocumentView"
import { EmptyState } from "./EmptyState"
import { CopyableIdentity } from "./CopyableIdentity"

/// The chromeless document surface: one document and nothing that navigates.
///
/// No workspace tree, no commit rail, no footer, no quota pills, no Settings or
/// Archive entry point, and no control that could make this window show a
/// different document. The rendered markdown was already unable to navigate —
/// every anchor in `MarkdownView` opens through the OS or is inert — so this
/// component is the existing renderer with the chrome around it absent, not a
/// restricted variant of it (`reader-window`: *Reader Window Surface*).

/// Where the address comes from, per host.
///
/// The desktop shell has no URL routing — the asset protocol serves real
/// bundled files and has no `index.html` fallback for an unknown path — so a
/// reader is opened at `index.html?reader=1&at=<address>` and reads the address
/// out of `at`. The served UI puts the address in the path as it always does,
/// where it stays a real, shareable URL, and only the `reader` flag rides in
/// the query. Either way the flag is outside the path the codec reads, so
/// `encodeAddress`/`decodeAddress` are untouched by this feature
/// (`reader-window`: *Reader Presentation Is Not Part of the Address*).
export function readerAddressPath(search: string, pathname: string): string {
    const at = new URLSearchParams(search).get("at")
    return at && at.length > 0 ? at : pathname
}

/// Whether this document was loaded as a reader.
export function isReaderRequest(search: string): boolean {
    return new URLSearchParams(search).get("reader") === "1"
}

/// Close this window, whichever host it is.
function closeReaderWindow(): void {
    if (isTauri()) {
        // No `CloseRequested` handler is installed on a reader window, so the
        // request destroys it — the exact inverse of the main window, which
        // intercepts the same request and hides so the tray and watcher
        // survive (`reader-window`: *Dismissing a Reader Window Destroys It*).
        void getCurrentWindow().close()
        return
    }
    window.close()
}

/// What the address names. A repository-scoped file address names a repository
/// and a root-relative path and deliberately carries no worktree segment
/// (`view-routing`: *A file address carries no worktree segment*), so which
/// copy to read cannot be decided here — the pooled listing is registry and
/// filesystem data, and the codec and resolver see neither. Such an address
/// therefore arrives as `repoFile`, and the copy is resolved at load time.
type ReaderTarget =
    | { kind: "ready"; source: DocumentSource; label: string }
    | {
          kind: "repoFile"
          scope: FileScope
          /// The copy that opens when it holds the path, and the root a read
          /// falls back to when the listing cannot be had.
          mainWorktree: string
          path: string
          label: string
      }

/// Resolve the address into the one document this window shows.
function targetForAddress(
    path: string,
    views: WorkspaceView[],
): ReaderTarget | null {
    const address = decodeAddress(path)
    if (address.kind === "unresolvable") return null
    const result = resolveAddress(address, views)
    if (result.status !== "resolved" || result.view.kind !== "target") return null
    const target = result.view.target

    if (target.kind === "artifact") {
        const view = findViewByRoot(target.workspace, views)
        return {
            kind: "ready",
            source: { kind: "artifact", target },
            label: labelFor(view, target.workspace),
        }
    }
    if (target.kind === "files" && target.selectedPath) {
        const view = findViewByRoot(target.root, views)
        const label = labelFor(view, target.root)
        if (view && view.kind === "repo") {
            return {
                kind: "repoFile",
                scope: { kind: "repo", repoId: view.repoId },
                mainWorktree: view.mainWorktree,
                path: target.selectedPath,
                label,
            }
        }
        // A flat workspace's root IS a readable folder, so there is nothing to
        // resolve.
        return {
            kind: "ready",
            source: { kind: "file", root: target.root, path: target.selectedPath },
            label,
        }
    }
    // A `files` address with no file, the Dashboard, a commit: all resolve, and
    // none of them is a document. A reader shows a document or nothing.
    return null
}

function labelFor(view: WorkspaceView | null, fallback: string): string {
    if (!view) return fallback
    return view.kind === "repo"
        ? (view.displayName ?? view.name)
        : (view.displayName ?? view.workspace.name)
}

export function ReaderRoot() {
    const { views, loading } = useWorkspaces()

    // A reader window is its own React root and never passes through `App`, so
    // it needs its own reconciliation and its own listener. Without the
    // listener a reader left open would keep the width it launched with while
    // the main window re-laid out around it. The value itself is unused here —
    // the stamp on <body> is what the stylesheet reads.
    useDocumentWidth()
    const addressPath = useMemo(
        () => readerAddressPath(window.location.search, window.location.pathname),
        [],
    )
    const resolved = useMemo(
        () => targetForAddress(addressPath, views),
        [addressPath, views],
    )

    // A repository-scoped file address opens a DEFAULT COPY: the main
    // worktree's when it holds the path, otherwise the first copy that does
    // (`view-routing`: *A repository file address resolves to a default copy*,
    // *A repository file address resolves to the only worktree holding the
    // file*). That needs the pooled listing, which is a fetch — so the reader
    // resolves it here rather than in the pure resolver.
    const pending = resolved?.kind === "repoFile" ? resolved : null
    const pendingKey =
        pending && pending.scope.kind === "repo"
            ? `${pending.scope.repoId}\u0000${pending.path}`
            : null
    const [copyRoot, setCopyRoot] = useState<string | null>(null)
    const [copyPending, setCopyPending] = useState(false)
    useEffect(() => {
        if (!pending) {
            setCopyRoot(null)
            setCopyPending(false)
            return
        }
        let cancelled = false
        setCopyPending(true)
        listWorkspaceFileRows(pending.scope)
            .then((rows) => {
                if (cancelled) return
                const copies = copyWorktrees(rowForPath(rows, pending.path))
                // No copy holds it: fall back to the main worktree so the READ
                // reports not found, rather than the reader reporting an
                // unresolvable address for one that resolved perfectly well.
                setCopyRoot(defaultCopy(copies, pending.mainWorktree) ?? pending.mainWorktree)
                setCopyPending(false)
            })
            .catch(() => {
                if (cancelled) return
                setCopyRoot(pending.mainWorktree)
                setCopyPending(false)
            })
        return () => {
            cancelled = true
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps -- `pending` is
        // rebuilt every render; `pendingKey` is its value.
    }, [pendingKey])

    const source: DocumentSource | null =
        resolved?.kind === "ready"
            ? resolved.source
            : pending && copyRoot
              ? { kind: "file", root: copyRoot, path: pending.path }
              : null

    const title = useMemo(() => {
        const address = decodeAddress(addressPath)
        if (address.kind === "unresolvable" || !resolved) return "SpecForge"
        return readerTitle(address, resolved.label) || "SpecForge"
    }, [addressPath, resolved])

    // The browser host has no native titlebar to set, so the document title is
    // the window's name. The desktop host's title is set when the window is
    // built, from the same function, so the two agree.
    useEffect(() => {
        document.title = title
    }, [title])

    // Escape closes the window — but only when nothing inside it has claimed
    // the key first. A maximized figure consumes Escape and calls
    // `preventDefault`, so one press returns to the document and a second
    // closes the window, which is the same `defaultPrevented` contract
    // `FigureLightbox` and the Settings rename input already follow
    // (`reader-window`: *Dismissing a Reader Window Destroys It*).
    useEffect(() => {
        const onKeyDown = (e: KeyboardEvent) => {
            if (e.defaultPrevented) return
            // Cmd/Ctrl-W. On macOS the application menu's Close item already
            // binds this, but that menu is macOS-only — on Windows and Linux
            // the shell installs no menu at all, so without this the standard
            // close-window shortcut would simply do nothing in a reader. In the
            // browser host the shortcut belongs to the browser and never
            // reaches here.
            if ((e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey && e.code === "KeyW") {
                e.preventDefault()
                closeReaderWindow()
                return
            }
            if (e.key !== "Escape") return
            closeReaderWindow()
        }
        // Not capturing: a capturing listener would fire before the lightbox
        // could claim the key, and Escape would close the whole window instead
        // of the figure.
        window.addEventListener("keydown", onKeyDown)
        return () => window.removeEventListener("keydown", onKeyDown)
    }, [])

    // Remember the size for the next reader. One shared geometry, not one per
    // document — see `AppSettings::reader_window` for why. Debounced so a drag
    // writes settings once rather than per frame.
    useEffect(() => {
        if (!isTauri()) return
        let timer: ReturnType<typeof setTimeout> | undefined
        const onResize = () => {
            clearTimeout(timer)
            timer = setTimeout(() => {
                void setReaderWindowSize(window.innerWidth, window.innerHeight).catch(
                    () => {
                        // A failed write costs the next reader its size and
                        // nothing else; there is no user action to suggest.
                    },
                )
            }, 400)
        }
        window.addEventListener("resize", onResize)
        return () => {
            clearTimeout(timer)
            window.removeEventListener("resize", onResize)
        }
    }, [])

    // Cold load: `views` is empty until the first fetch lands, and resolving
    // against it would report "not found" for a perfectly good address. Same
    // reason the shell holds a deep address behind `loading`
    // (`view-routing`: *Cold-Load Address Resolution*).
    if (loading || copyPending) {
        return <div className="detail-pane-status">Loading…</div>
    }

    if (!resolved || !source) {
        return (
            <EmptyState
                title="Document not found"
                body="This address doesn't name a document in any registered workspace. It may have been moved, or its workspace unregistered."
            />
        )
    }

    return (
        <DocumentView
            source={source}
            className="detail-pane reader-document"
            errorTitle="Couldn't load document"
            header={(status, headerRef) => (
                <div className="detail-identity" ref={headerRef}>
                    <div className="detail-identity-inner">
                        <CopyableIdentity
                            value={
                                source.kind === "file"
                                    ? source.path
                                    : source.target.changeId
                            }
                            noun={source.kind === "file" ? "file path" : "change name"}
                        />
                        {status.missing && <MissingDocumentLabel />}
                    </div>
                </div>
            )}
        />
    )
}
