// Which root a page renders, and what its `head` carries before anything
// renders (`pull-request-viewer`: *Pull-Request Window*, *Pull-Request Window
// Permissions*, *Pull-Request Content Is Untrusted*; design D3 and D10).
//
// One bundle serves three roots. A reader window and a pull-request window
// load the application's own document with a flag in the query, outside the
// path the codec reads, so one path names one thing in every presentation;
// `main.tsx` reads the flag once, here, before any router runs.
//
// The policies are `<meta>` elements, and engines honour a policy `<meta>`
// only inside `head`, and only for fetches that start after it. So they are
// installed once, before the root renders: never at module scope, since
// `main.tsx` imports every root statically, and never in an effect, which
// runs only after the children have mounted. No stylesheet is imported here,
// so `bun test` can load this module as it is.

export type WindowKind = "application" | "reader" | "pullRequest" | "imageWindow"

/// The root a page's query (`location.search`) asks for: `reader=1` a reader
/// window, `pullRequest=1` a pull-request window, `imageWindow=1` an image
/// file's zoom window (`diff-view`: *Image Comparison*, Zooming), and anything
/// else the application. A page carrying more than one flag takes the first of
/// those, in that order.
export function windowKind(search: string): WindowKind {
    const params = new URLSearchParams(search)
    if (params.get("reader") === "1") return "reader"
    if (params.get("pullRequest") === "1") return "pullRequest"
    if (params.get("imageWindow") === "1") return "imageWindow"
    return "application"
}

/// The pull-request window's content-security policy: no image, font or media
/// from anywhere but the app itself, and no plugin content, so an element that
/// pull-request text smuggled past the renderer still fetches nothing. The
/// window's own bundled fonts and icons are `'self'` or inline, so they load.
export const PULL_REQUEST_CONTENT_SECURITY_POLICY =
    "img-src 'self' data: blob:; font-src 'self' data:; media-src 'none'; object-src 'none'"

/// What the installer needs of a document. `main.tsx` passes `document`, and a
/// test passes a stand-in, since `bun test` runs without a DOM.
export type HeadDocument = Pick<Document, "createElement" | "head">

/// Install `kind`'s `head` policies into `doc`. Every root turns DNS
/// prefetching off, so no host is resolved merely because a link to it is on
/// screen. The pull-request window also gets the content-security policy, and
/// so does the zoom window, which may show a pull request's versions; never
/// the application or a reader, whose documents load what they always have.
export function installHeadPolicies(kind: WindowKind, doc: HeadDocument): void {
    appendMeta(doc, "x-dns-prefetch-control", "off")
    if (kind === "pullRequest" || kind === "imageWindow") {
        appendMeta(doc, "Content-Security-Policy", PULL_REQUEST_CONTENT_SECURITY_POLICY)
    }
}

function appendMeta(doc: HeadDocument, httpEquiv: string, content: string): void {
    const meta = doc.createElement("meta")
    meta.setAttribute("http-equiv", httpEquiv)
    meta.setAttribute("content", content)
    doc.head.appendChild(meta)
}
