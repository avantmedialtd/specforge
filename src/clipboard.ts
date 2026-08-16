// Copying a value out of the app.
//
// The same bundle runs on origins with different powers, so there are two
// paths and the choice between them is not a preference:
//
//   - the asynchronous Clipboard API, available in the desktop WebView and on
//     any secure origin (which includes loopback, so `specforge-serve` on
//     `localhost` has it);
//   - a synchronous `document.execCommand("copy")` over a live selection,
//     for origins that are not secure contexts. `specforge-serve --bind
//     <non-loopback>` serves plain HTTP from a non-localhost origin, where
//     `navigator.clipboard` is `undefined` — verified in a browser against
//     such a bind, not assumed.
//
// The two paths MUST NOT be chained. `document.execCommand` is only permitted
// while the browser still considers itself inside the user gesture that
// triggered it, and an `await` on a rejected `writeText` ends that gesture — so
// "try async, fall back to sync on failure" silently degrades to "no copy at
// all" exactly where the fallback was supposed to help. The strategy is chosen
// up front from what the origin exposes, and each path runs alone.

/// Which copy path this origin can use. Split out from the DOM work so the
/// decision is testable without a document.
export type ClipboardStrategy = "async" | "selection"

/// `async` when the origin exposes the asynchronous Clipboard API, `selection`
/// otherwise. Takes the navigator so a test can pass a stub; defaults to the
/// real one.
export function clipboardStrategy(
    nav: { clipboard?: { writeText?: unknown } } | undefined = typeof navigator ===
    "undefined"
        ? undefined
        : navigator,
): ClipboardStrategy {
    return typeof nav?.clipboard?.writeText === "function" ? "async" : "selection"
}

/// Select an element's entire contents, replacing any existing selection.
///
/// Done before either copy path, for two reasons: it is the visible
/// confirmation of exactly what was copied, and it is the thing the selection
/// path copies. A pointer click on a `user-select: all` element has already
/// produced this selection; a keyboard activation has not, so it is made
/// explicitly rather than relied upon.
export function selectContents(el: HTMLElement): boolean {
    const selection = window.getSelection()
    if (!selection) return false
    const range = document.createRange()
    range.selectNodeContents(el)
    selection.removeAllRanges()
    selection.addRange(range)
    return true
}

/// Copy `el`'s text, leaving it selected. Resolves to whether the clipboard
/// actually received it.
///
/// On failure the selection is deliberately left in place: the user can still
/// press the platform's own copy shortcut, so a refused clipboard write
/// degrades to the manual gesture rather than to nothing. Callers surface that
/// distinction rather than claiming success.
export async function copyElement(el: HTMLElement): Promise<boolean> {
    const text = el.textContent ?? ""
    if (text.length === 0) return false
    selectContents(el)

    if (clipboardStrategy() === "async") {
        try {
            await navigator.clipboard.writeText(text)
            return true
        } catch {
            // The gesture is spent; the selection path is no longer permitted.
            // The text stays selected so the manual shortcut still works.
            return false
        }
    }

    try {
        return document.execCommand("copy")
    } catch {
        return false
    }
}
