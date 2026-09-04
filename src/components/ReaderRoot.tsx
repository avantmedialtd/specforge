import { useEffect, useMemo } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { isTauri, setReaderWindowSize } from "../api"
import { useWorkspaces } from "../hooks/useWorkspaces"
import { useDocumentWidth } from "../hooks/useDocumentWidth"
import { readerTitle } from "../readerTitle"
import { decodeAddress } from "../routing/codec"
import { findViewByRoot, resolveAddress } from "../routing/resolve"
import type { WorkspaceView } from "../types"
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

/// Resolve the address into the one document this window shows.
function sourceForAddress(
    path: string,
    views: WorkspaceView[],
): { source: DocumentSource; label: string } | null {
    const address = decodeAddress(path)
    if (address.kind === "unresolvable") return null
    const result = resolveAddress(address, views)
    if (result.status !== "resolved" || result.view.kind !== "target") return null
    const target = result.view.target

    if (target.kind === "artifact") {
        const view = findViewByRoot(target.workspace, views)
        return {
            source: { kind: "artifact", target },
            label: labelFor(view, target.workspace),
        }
    }
    if (target.kind === "files" && target.selectedPath) {
        const view = findViewByRoot(target.root, views)
        return {
            source: { kind: "file", root: target.root, path: target.selectedPath },
            label: labelFor(view, target.root),
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
        () => sourceForAddress(addressPath, views),
        [addressPath, views],
    )

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
    if (loading) {
        return <div className="detail-pane-status">Loading…</div>
    }

    if (!resolved) {
        return (
            <EmptyState
                title="Document not found"
                body="This address doesn't name a document in any registered workspace. It may have been moved, or its workspace unregistered."
            />
        )
    }

    return (
        <DocumentView
            source={resolved.source}
            className="detail-pane reader-document"
            errorTitle="Couldn't load document"
            header={(status, headerRef) => (
                <div className="detail-identity" ref={headerRef}>
                    <div className="detail-identity-inner">
                        <CopyableIdentity
                            value={
                                resolved.source.kind === "file"
                                    ? resolved.source.path
                                    : resolved.source.target.changeId
                            }
                            noun={
                                resolved.source.kind === "file" ? "file path" : "change name"
                            }
                        />
                        {status.missing && <MissingDocumentLabel />}
                    </div>
                </div>
            )}
        />
    )
}
