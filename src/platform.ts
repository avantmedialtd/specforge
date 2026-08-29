/// Whether the frontend should present the macOS hidden-inset titlebar
/// layout — the traffic-light safe area in the side panes and the drag
/// region across the top of the window.
///
/// That layout is a property of the *native* macOS window, so the host check
/// comes first and the user-agent only distinguishes platforms within it.
/// A user-agent test alone is not enough: every Apple user-agent carries a
/// "Mac" token. iPadOS Safari reports `Macintosh; Intel Mac OS X` by default
/// — byte-for-byte the desktop string — and both the iPad "Request Mobile
/// Website" and iPhone strings end `like Mac OS X`. Matching any of those in
/// a browser reserves space for window controls that do not exist and lays a
/// pointer-capturing drag region over content that needs the taps.
/// `isReaderWindow` excludes reader windows, which carry a NATIVE titlebar
/// rather than the main window's overlay one. They are native macOS windows in
/// the native host, so the first two tests both pass for them — and reserving
/// the traffic-light safe area under a real titlebar would leave a band of
/// empty space at the top of the document, and lay a drag region over the
/// header. The window kind is the only thing that distinguishes the two cases,
/// so it has to be an input here rather than inferred
/// (`reader-window`: *Reader Window Title and Titlebar*).
export function usesMacTitlebarChrome(
    isTauriHost: boolean,
    userAgent: string,
    isReaderWindow = false,
): boolean {
    return isTauriHost && !isReaderWindow && /Mac/i.test(userAgent)
}
