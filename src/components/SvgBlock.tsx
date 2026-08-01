import { useMemo, useState } from "react"
import type { ReactNode } from "react"
import { useDarkScheme } from "../hooks/useDarkScheme"
import { readToken } from "../theme"

const SVG_NS = "http://www.w3.org/2000/svg"
// WebKit (and, historically, Gecko) synthesize a <parsererror> element in
// this namespace on an XML parse failure. Scoping the gate to it means an
// author's own SVG-namespaced (or, pre-injection, null-namespaced) element
// literally named "parsererror" can never be mistaken for one.
const PARSER_ERROR_NS = "http://www.mozilla.org/newlayout/xml/parsererror.xml"

interface SvgRender {
    src: string
    alt: string
}

/** True for a concrete SVG length ("120", "120px", "10cm", "-4pt"): an
 * optional sign, a number, and an optional unit restricted to
 * px|cm|mm|in|pt|pc|em|ex|ch|rem. Rejects "auto", "", a percentage (no fixed
 * size in an image context), and garbage — none of those are a length at
 * all. */
const ABSOLUTE_LENGTH_RE =
    /^[+-]?(?:\d+\.?\d*|\.\d+)(?:px|cm|mm|in|pt|pc|em|ex|ch|rem)?$/

function isAbsoluteLength(value: string | null): boolean {
    if (value === null) return false
    return ABSOLUTE_LENGTH_RE.test(value.trim())
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
    return readToken("--text")
}

/** Skips a leading XML prolog before the root element — `<?...?>`
 * declarations/PIs, an `<!DOCTYPE ...>` (with a possibly bracketed internal
 * subset), `<!--...-->` comments, and whitespace, in any mixture or order —
 * and returns the index of the first non-trivia character. Returns null if
 * a `<?...?>`/`<!--...-->`/`<!DOCTYPE...>` block never closes, so the
 * caller can fall through rather than guess where the root starts. */
function skipProlog(source: string): number | null {
    let i = 0
    for (;;) {
        while (i < source.length && /\s/.test(source[i])) i++

        if (source.startsWith("<?", i)) {
            const end = source.indexOf("?>", i + 2)
            if (end === -1) return null
            i = end + 2
            continue
        }

        if (source.startsWith("<!--", i)) {
            const end = source.indexOf("-->", i + 4)
            if (end === -1) return null
            i = end + 3
            continue
        }

        if (source.startsWith("<!DOCTYPE", i)) {
            let depth = 0
            let j = i + "<!DOCTYPE".length
            for (; j < source.length; j++) {
                if (source[j] === "[") depth++
                else if (source[j] === "]") depth--
                else if (source[j] === ">" && depth <= 0) break
            }
            if (j >= source.length) return null
            i = j + 1
            continue
        }

        return i
    }
}

/** Finds the root element's start tag (`<svg ...>` or `<svg .../>`),
 * scanning past a leading prolog first and requiring the first real element
 * to be `<svg`. Quote-aware while scanning for the tag's end, so an
 * attribute value containing `>` or `/` can't close it early. Returns null
 * — a scan miss, meaning the caller falls through to the source fallback
 * rather than patch the wrong span — if the prolog never resolves, the
 * first real element isn't `<svg`, or the tag never closes. */
function findRootStartTag(source: string): { start: number; end: number } | null {
    const start = skipProlog(source)
    if (start === null || !/^<svg[\s/>]/.test(source.slice(start))) return null

    let quote: string | null = null
    for (let i = start + "<svg".length; i < source.length; i++) {
        const ch = source[i]
        if (quote) {
            if (ch === quote) quote = null
        } else if (ch === '"' || ch === "'") {
            quote = ch
        } else if (ch === ">") {
            return { start, end: i + 1 }
        }
    }
    return null
}

/** Patches the root `<svg>` start tag with an explicit `xmlns` declaring the
 * SVG namespace — replacing a textual `xmlns="..."` value if the tag
 * already has one (this is what a `rootNs === null` parse alongside an
 * explicit `xmlns=""` looks like textually), otherwise inserting the
 * declaration right after the tag name. Returns null on a scan miss (see
 * findRootStartTag) so the caller can fall through instead of patching some
 * `<svg` text that isn't really the root — e.g. one inside a leading
 * comment or doctype. */
function injectXmlns(source: string): string | null {
    const tag = findRootStartTag(source)
    if (tag === null) return null

    const startTag = source.slice(tag.start, tag.end)
    const existing = /\sxmlns\s*=\s*(?:"[^"]*"|'[^']*')/.exec(startTag)

    const patchedTag =
        existing !== null
            ? startTag.slice(0, existing.index) +
              ` xmlns="${SVG_NS}"` +
              startTag.slice(existing.index + existing[0].length)
            : `<svg xmlns="${SVG_NS}"` + startTag.slice("<svg".length)

    return source.slice(0, tag.start) + patchedTag + source.slice(tag.end)
}

/**
 * Parses and validates `source` as an SVG document, then applies the D4
 * rewrite pass and serializes to a data URI. Returns null for anything that
 * fails the gate — the caller falls back to the fence's highlighted source.
 *
 * The gate (D3): DOMParser accepts a namespace-less `<svg>` body just as
 * cleanly as a properly namespaced one — a missing xmlns is not a
 * well-formedness error — so parse success alone provides no SVG-ness
 * discrimination. Validity is parse success AND root identity: no
 * browser-synthesized `parsererror` (scoped to PARSER_ERROR_NS — see above)
 * anywhere in the result, and the root's localName is `svg` in either the
 * SVG namespace or no namespace.
 */
function buildSvgRender(source: string): SvgRender | null {
    let doc = new DOMParser().parseFromString(source, "image/svg+xml")

    if (doc.getElementsByTagNameNS(PARSER_ERROR_NS, "parsererror").length > 0) {
        return null
    }

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
        const patched = injectXmlns(source)
        if (patched === null) return null

        doc = new DOMParser().parseFromString(patched, "image/svg+xml")
        root = doc.documentElement
        if (
            doc.getElementsByTagNameNS(PARSER_ERROR_NS, "parsererror").length > 0 ||
            root.localName !== "svg" ||
            root.namespaceURI !== SVG_NS
        ) {
            return null
        }
    }

    // 2. Deterministic sizing — a viewBox gives ratio but not size in an
    // image context, so WKWebView would otherwise fall back to arbitrary
    // replaced-element defaults. Derive both from the viewBox extents at one
    // user unit per CSS pixel, but ONLY when BOTH absolute dimensions are
    // missing/unusable. If exactly one is authored, leave both attributes
    // untouched: the image context computes the missing one from the
    // viewBox ratio natively, so overwriting the authored one would silently
    // discard it. A fence with neither viewBox nor dimensions is degenerate;
    // browser defaults are accepted rather than specified.
    if (
        !isAbsoluteLength(root.getAttribute("width")) &&
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
    fallback: ReactNode
}

/**
 * Renders one ```svg fence as an inert image (D1): the source is parsed,
 * validated, rewritten, and serialized into a data URI set as an <img> src —
 * never injected into the live DOM — so scripts, event handlers, and
 * external references in the fence body can never execute or load. Unlike
 * MermaidBlock the whole pipeline is synchronous (D3): no external renderer
 * to await, so there is no pending state.
 *
 * `fallback` is the already syntax-highlighted <pre> MarkdownView would have
 * rendered for this fence had SvgBlock not intercepted it (D2) — shown,
 * under a quiet note, when the fence isn't a renderable SVG document or the
 * <img> itself fails to load.
 */
export function SvgBlock({ source, fallback }: SvgBlockProps) {
    // Re-keys the memo below so the injected --text token (D4) follows the
    // scheme live, even though buildSvgRender re-reads the token itself
    // rather than branching on this value directly.
    const isDark = useDarkScheme()
    const rendered = useMemo(() => buildSvgRender(source), [source, isDark])

    // The <img> load is the second net (D3) for anything the parse gate
    // mispredicts (e.g. a well-formed non-SVG root named "svg"). Latched on
    // the src STRING rather than the SvgRender object: useMemo above returns
    // a fresh object on every recompute — including a no-op scheme toggle,
    // e.g. when the author already declared their own `color` — so an
    // identity comparison would immediately un-latch and re-mount the same
    // failing <img>, flickering. Comparing the string means an unchanged src
    // stays latched, while a genuinely different one (content or injected
    // token changed) is free to retry.
    const [erroredSrc, setErroredSrc] = useState<string | null>(null)

    if (rendered === null || rendered.src === erroredSrc) {
        return (
            <div className="fence-block--error">
                <p className="fence-block__note">
                    Couldn’t render this image — showing its source.
                </p>
                {fallback}
            </div>
        )
    }

    return (
        <div className="svg-block">
            <img
                src={rendered.src}
                alt={rendered.alt}
                onError={() => setErroredSrc(rendered.src)}
            />
        </div>
    )
}
