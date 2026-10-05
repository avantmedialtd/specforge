import { useEffect } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { isTauri } from "../api"
import { closeKeyOf, SIZE_SAVE_DELAY_MS } from "../detachedWindow"

/// Close this window, whichever host it is.
function closeDetachedWindow(): void {
    if (isTauri()) {
        // No `CloseRequested` handler is installed on a detached window, so
        // the request destroys it — the exact inverse of the main window,
        // which intercepts the same request and hides so the tray and watcher
        // survive (`reader-window`: *Dismissing a Reader Window Destroys It*;
        // `pull-request-viewer`: *Pull-Request Window*).
        void getCurrentWindow().close()
        return
    }
    window.close()
}

/// What a detached window's root does besides rendering: a reader window's,
/// and a pull-request window's. It titles the window, closes it on Escape and
/// on Cmd/Ctrl-W, and saves its size, through `saveSize`, once a resize has
/// ended, so the next window of its kind adopts it. One geometry per kind, not
/// one per document or pull request — see `AppSettings::reader_window` for why.
///
/// `saveSize` is the kind's own desktop command, `setReaderWindowSize` or
/// `setPullRequestWindowSize`; a module function, so it never changes between
/// renders.
export function useDetachedWindow(
    title: string,
    saveSize: (width: number, height: number) => Promise<void>,
): void {
    // The browser host has no native titlebar to set, so the document title is
    // the window's name. The desktop host's title is set when the window is
    // built, from the same function, so the two agree; a pull-request window's
    // then follows this one, sanitised again by the shell (`pull-request-viewer`:
    // *Pull-Request Window Title*).
    useEffect(() => {
        document.title = title
    }, [title])

    // Escape closes the window — but only when nothing inside it has claimed
    // the key first. A maximized figure consumes Escape and calls
    // `preventDefault`, so one press returns to the content and a second
    // closes the window, which is the same `defaultPrevented` contract
    // `FigureLightbox` and the Settings rename input already follow
    // (`reader-window`: *Dismissing a Reader Window Destroys It*).
    //
    // Cmd/Ctrl-W too. On macOS the application menu's Close item already
    // binds it, but that menu is macOS-only — on Windows and Linux the shell
    // installs no menu at all, so without this the standard close-window
    // shortcut would simply do nothing in a detached window. In the browser
    // host the shortcut belongs to the browser and never reaches here.
    useEffect(() => {
        const onKeyDown = (e: KeyboardEvent) => {
            const close = closeKeyOf(e)
            if (close === null) return
            if (close === "shortcut") e.preventDefault()
            closeDetachedWindow()
        }
        // Not capturing: a capturing listener would fire before the lightbox
        // could claim the key, and Escape would close the whole window instead
        // of the figure.
        window.addEventListener("keydown", onKeyDown)
        return () => window.removeEventListener("keydown", onKeyDown)
    }, [])

    // Remember the size for the next window of this kind. Debounced so a drag
    // writes settings once rather than per frame.
    useEffect(() => {
        if (!isTauri()) return
        let timer: ReturnType<typeof setTimeout> | undefined
        const onResize = () => {
            clearTimeout(timer)
            timer = setTimeout(() => {
                void saveSize(window.innerWidth, window.innerHeight).catch(() => {
                    // A failed write costs the next window its size and
                    // nothing else; there is no user action to suggest.
                })
            }, SIZE_SAVE_DELAY_MS)
        }
        window.addEventListener("resize", onResize)
        return () => {
            clearTimeout(timer)
            window.removeEventListener("resize", onResize)
        }
    }, [saveSize])
}
