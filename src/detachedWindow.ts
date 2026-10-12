// What a detached window — a reader window, or a pull-request window — does on
// its own, whatever it shows: it closes on Escape and on the close-window
// shortcut, and it remembers its size for the next window of its kind
// (`reader-window`: *Dismissing a Reader Window Destroys It*;
// `pull-request-viewer`: *Pull-Request Window*, *Pull-Request Window
// Geometry*). `useDetachedWindow` wires it into each root; the decisions are
// here, as pure functions, so the pull-request window adopts the reader's
// behaviour under a test rather than by copy. Where each takes its address
// from (`readerAddressPath`) lives here too, so that neither root, each its
// own lazily loaded chunk, imports the other.

/// How long after the last resize event a detached window saves its size, so a
/// drag writes settings once rather than per frame.
export const SIZE_SAVE_DELAY_MS = 400

/// What a key press closes a window by. A DOM `KeyboardEvent` is one.
export interface CloseKeyEvent {
    key: string
    code: string
    metaKey: boolean
    ctrlKey: boolean
    altKey: boolean
    shiftKey: boolean
    defaultPrevented: boolean
}

/// Whether a key press closes a detached window:
///
/// - `shortcut` — Cmd-W or Ctrl-W, the close-window shortcut, read from the
///   key's position rather than its character, so it holds on any keyboard
///   layout. Its handler prevents the key's default as it closes;
/// - `escape` — Escape;
/// - `null` — any other key, and any key a control inside the window has
///   claimed by preventing its default first: a maximized figure takes the
///   first Escape, so one press returns to the content and a second closes
///   the window.
export function closeKeyOf(event: CloseKeyEvent): "shortcut" | "escape" | null {
    if (event.defaultPrevented) return null
    if (
        (event.metaKey || event.ctrlKey) &&
        !event.altKey &&
        !event.shiftKey &&
        event.code === "KeyW"
    ) {
        return "shortcut"
    }
    return event.key === "Escape" ? "escape" : null
}

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
///
/// The pull-request window takes its address the same way, under its own flag
/// (`pull-request-viewer`: *Pull-Request Window*).
export function readerAddressPath(search: string, pathname: string): string {
    const at = new URLSearchParams(search).get("at")
    return at && at.length > 0 ? at : pathname
}
