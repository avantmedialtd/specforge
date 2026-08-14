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
export function usesMacTitlebarChrome(
    isTauriHost: boolean,
    userAgent: string,
): boolean {
    return isTauriHost && /Mac/i.test(userAgent)
}
