import { useMemo, useState } from "react"
import { useDarkScheme } from "../hooks/useDarkScheme"

const SVG_NS = "http://www.w3.org/2000/svg"

interface SvgRender {
    src: string
    alt: string
}

/** True for a concrete length ("120", "120px", "10cm") — not a percentage
 * (relative, no fixed size in an image context) and not empty/missing. */
function isAbsoluteLength(value: string | null): boolean {
    if (value === null) return false
    const trimmed = value.trim()
    return trimmed.length > 0 && !trimmed.endsWith("%")
}

/** True if `root` already declares its own `color`, as an attribute or
 * inline style — the only two places D4 checks before injecting the theme
 * token. An internal `<style>` rule targeting the root is deliberately not
 * consulted (out of scope: this stays a cheap, static check). */
function hasOwnColor(root: Element): boolean {
    if (root.hasAttribute("color")) return true
    const style = root.getAttribute("style")
    // Excludes "background-color:" etc — the char before "color" must be a
    // boundary (start, ";", or whitespace), not a hyphen.
    return style !== null && /(?:^|[;\s])color\s*:/i.test(style)
}

/** A root-level <title>'s text, or null. Namespace-agnostic tag-name match
 * (a namespace-less root's children are namespace-less too, per D3), so this
 * covers both the injected-xmlns and already-namespaced cases identically. */
function rootTitle(root: Element): string | null {
    for (const child of Array.from(root.children)) {
        if (child.localName === "title") {
            const text = child.textContent?.trim()
            if (text) return text
        }
    }
    return null
}

function textToken(): string {
    return getComputedStyle(document.documentElement)
        .getPropertyValue("--text")
        .trim()
}

/**
 * Parses and validates `source` as an SVG document, then applies the D4
 * rewrite pass and serializes to a data URI. Returns null for anything that
 * fails the gate — the caller falls back to raw source.
 *
 * The gate (D3): DOMParser accepts a namespace-less `<svg>` body just as
 * cleanly as a properly namespaced one — a missing xmlns is not a
 * well-formedness error — so parse success alone provides no SVG-ness
 * discrimination. Validity is parse success AND root identity: no
 * `parsererror` anywhere in the result, and the root's localName is `svg`
 * in either the SVG namespace or no namespace.
 */
function buildSvgRender(source: string): SvgRender | null {
    let doc = new DOMParser().parseFromString(source, "image/svg+xml")

    if (doc.getElementsByTagName("parsererror").length > 0) return null

    let root = doc.documentElement
    const rootNs = root.namespaceURI
    if (root.localName !== "svg" || (rootNs !== SVG_NS && rootNs !== null)) {
        return null
    }

    // 1. xmlns injection — mandatory for a standalone SVG document, routinely
    // omitted by authors who learned SVG inline in HTML. A parsed tree can't
    // be re-namespaced (setAttribute("xmlns") is a plain attribute, and the
    // serialized result still yields a namespace-less document that an <img>
    // "loads" but paints as nothing), so patch the source text and re-parse:
    // the second parse puts every element genuinely in the SVG namespace.
    if (rootNs === null) {
        const patched = source.replace(
            /<svg(?=[\s/>])/i,
            `<svg xmlns="${SVG_NS}"`,
        )
        doc = new DOMParser().parseFromString(patched, "image/svg+xml")
        root = doc.documentElement
        if (
            doc.getElementsByTagName("parsererror").length > 0 ||
            root.localName !== "svg" ||
            root.namespaceURI !== SVG_NS
        ) {
            return null
        }
    }

    // 2. Deterministic sizing — a viewBox gives ratio but not size in an
    // image context, so WKWebView would otherwise fall back to arbitrary
    // replaced-element defaults. Derive both from the viewBox extents at one
    // user unit per CSS pixel when either absolute dimension is missing. A
    // fence with neither viewBox nor dimensions is degenerate; browser
    // defaults are accepted rather than specified.
    if (
        !isAbsoluteLength(root.getAttribute("width")) ||
        !isAbsoluteLength(root.getAttribute("height"))
    ) {
        const viewBox = root.getAttribute("viewBox")
        if (viewBox !== null) {
            const extents = viewBox.trim().split(/[\s,]+/).map(Number)
            if (extents.length === 4 && extents.every((n) => Number.isFinite(n))) {
                const [, , vbWidth, vbHeight] = extents
                if (vbWidth > 0 && vbHeight > 0) {
                    root.setAttribute("width", String(vbWidth))
                    root.setAttribute("height", String(vbHeight))
                }
            }
        }
    }

    // 3. Theme colour — only when the author declared none. currentColor
    // then resolves to the live --text token by ordinary CSS inheritance
    // inside the image document; an author colour anywhere in the subtree
    // still wins, since descendants are never touched.
    if (!hasOwnColor(root)) {
        root.setAttribute("color", textToken())
    }

    const alt = rootTitle(root) ?? "Embedded SVG image"
    const serialized = new XMLSerializer().serializeToString(doc)
    return { src: `data:image/svg+xml,${encodeURIComponent(serialized)}`, alt }
}

interface SvgBlockProps {
    source: string
}

/**
 * Renders one ```svg fence as an inert image (D1): the source is parsed,
 * validated, rewritten, and serialized into a data URI set as an <img> src —
 * never injected into the live DOM — so scripts, event handlers, and
 * external references in the fence body can never execute or load. Unlike
 * MermaidBlock the whole pipeline is synchronous (D3): no external renderer
 * to await, so there is no pending state.
 */
export function SvgBlock({ source }: SvgBlockProps) {
    // Re-keys the memo below so the injected --text token (D4) follows the
    // scheme live, even though buildSvgRender re-reads the token itself
    // rather than branching on this value directly.
    const isDark = useDarkScheme()
    const rendered = useMemo(() => buildSvgRender(source), [source, isDark])

    // The <img> load is the second net (D3) for anything the parse gate
    // mispredicts (e.g. a well-formed non-SVG root named "svg"). Comparing
    // by identity against the current `rendered`: a source/scheme change
    // always produces a new object, so a stale failure clears with no extra
    // effect needed to reset it.
    const [erroredRender, setErroredRender] = useState<SvgRender | null>(null)

    if (rendered === null || rendered === erroredRender) {
        return (
            <div className="mermaid-block mermaid-block--error">
                <p className="mermaid-block__note">
                    Couldn’t render this image — showing its source.
                </p>
                <pre>
                    <code>{source}</code>
                </pre>
            </div>
        )
    }

    return (
        <div className="svg-block">
            <img
                src={rendered.src}
                alt={rendered.alt}
                onError={() => setErroredRender(rendered)}
            />
        </div>
    )
}
