// How a link inside a rendered document is dispatched.
//
// Extracted from `MarkdownView.tsx` so the classification is reachable by
// `bun test`: the component imports KaTeX's stylesheet and the whole markdown
// pipeline, and JSX is not exercised by the test runner at all. The rule this
// module owns decides whether a click opens a file, opens a browser, scrolls
// the document, or does nothing — and one of those classes is new
// (`document-outline`: *Fragment Links Resolve Within the Document*).

/// What a click on a link does.
///
/// - `external` — an http/https/mailto/tel link, opened outside the app.
/// - `file` — a workspace-relative link to a non-markdown file, opened
///   through the validated open command.
/// - `fragment` — a fragment-only link naming a heading of THIS document:
///   scrolls to it, with no address change and no history entry.
/// - `danglingFragment` — a fragment-only link naming no heading: the quiet
///   "couldn't follow that link" indication, exactly as a missing file gets.
/// - `inert` — everything with no defined behaviour: a relative markdown
///   link (reserved for future in-app navigation), any other scheme, an empty
///   href.
export type LinkClass = "external" | "file" | "fragment" | "danglingFragment" | "inert"

const MARKDOWN_LINK_EXTENSIONS = new Set(["md", "markdown"])

/**
 * The URI scheme prefix of `href` (lowercased), or null for a scheme-less
 * relative reference. Mirrors RFC 3986's `scheme = ALPHA *(ALPHA / DIGIT /
 * "+" / "-" / ".")` grammar — the same shape `openspec-app::service::
 * href_scheme` implements in Rust — so a relative markdown link (which never
 * starts with `ALPHA ":"`) is never misread as a scheme by either side.
 */
export function hrefScheme(href: string): string | null {
    const colon = href.indexOf(":")
    if (colon <= 0) return null
    const prefix = href.slice(0, colon)
    return /^[a-zA-Z][a-zA-Z0-9+\-.]*$/.test(prefix) ? prefix.toLowerCase() : null
}

/// The identifier a fragment-only href names, percent-decoded, or `null` when
/// the href is not fragment-only. `#` alone names nothing.
export function fragmentTarget(href: string): string | null {
    if (!href.startsWith("#")) return null
    const raw = href.slice(1)
    if (raw === "") return null
    try {
        return decodeURIComponent(raw)
    } catch {
        // A malformed escape is not a heading name; fall back to the literal
        // text rather than throwing inside a click handler.
        return raw
    }
}

/**
 * Classify a link href for AFFORDANCE and dispatch — the cursor/class it
 * renders with and what clicking it does. The service re-classifies
 * authoritatively (`open_artifact_link`), so a mismatch on the `file` class
 * degrades to the command's own quiet-failure path rather than a security
 * gap. Mirrors `resolve_artifact_link`'s classification order: scheme, then
 * fragment, then fragment/query-stripped extension.
 *
 * `headingIds` is the set of identifiers the rendered document carries. A
 * fragment-only link is matched against it; a link carrying a fragment
 * TOGETHER WITH a path is unaffected and keeps its path's class.
 */
export function classifyHref(
    href: string,
    headingIds: ReadonlySet<string> = new Set(),
): LinkClass {
    const scheme = hrefScheme(href)
    if (scheme) {
        return scheme === "http" || scheme === "https" || scheme === "mailto" || scheme === "tel"
            ? "external"
            : "inert" // javascript:, file:, data:, ...
    }
    if (href === "") return "inert"

    const fragment = fragmentTarget(href)
    if (fragment !== null) {
        return headingIds.has(fragment) ? "fragment" : "danglingFragment"
    }
    // `#` alone, which names no heading and resolves to no path.
    if (href.startsWith("#")) return "danglingFragment"

    const withoutFragment = href.split("#")[0] ?? ""
    const pathPart = withoutFragment.split("?")[0] ?? ""
    const dot = pathPart.lastIndexOf(".")
    const ext = dot >= 0 ? pathPart.slice(dot + 1).toLowerCase() : ""
    return MARKDOWN_LINK_EXTENSIONS.has(ext) ? "inert" : "file"
}
