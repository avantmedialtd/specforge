// Pull-request content in the shared markdown renderer.
//
// Descriptions, conversation entries and review-thread comments are written by
// others, so they render in the renderer's pull-request mode, and that mode is
// the guard wherever they show: in the main window too, where no
// content-security policy stands behind it (`pull-request-viewer`:
// *Pull-Request Content Is Untrusted*, *Desktop Link Opener*; design D10). This
// module holds the mode's pieces: a remark plugin that drops template comments,
// and component overrides that make no `<img>`, draw no diagram and open links
// only by the rule below. Raw HTML needs nothing of its own: without
// `rehype-raw`, react-markdown renders it as text, so no element is ever made
// from it.
//
// It also holds what both modes share: the KaTeX and remark-math options, the
// fence helpers, and the renderer's memo comparison. It imports no stylesheet,
// so `bun test` can render these pieces through react-markdown, which is the
// only coverage they get: a frontend-only diff skips the mutation gate.

import { createContext, useContext } from "react"
import type { ComponentProps, ComponentType, ReactElement, ReactNode } from "react"
import type { Element, ElementContent } from "hast"
import type { Root, RootContent } from "mdast"
import type { Components, ExtraProps } from "react-markdown"
import { hrefScheme } from "./links"
import type { PullRequestReference } from "./types"

/** KaTeX's non-trusting posture: an invalid expression renders its source
 * in place instead of throwing (`throwOnError`), a command that would emit
 * a live link or fetch an external resource — `\href` and friends — renders
 * inert instead (`trust`), and `strict: "ignore"` quiets KaTeX's console
 * warnings for benign non-strict LaTeX. `as const` narrows `strict` to the
 * literal type KaTeX's own options expect. rehype-katex's own `Options`
 * type omits `throwOnError` — it forces its own throwOnError:true-then-
 * false catch/retry internally regardless of what's passed here — so that
 * field is an inert passenger on this object as far as rehype-katex is
 * concerned, kept anyway so the options read as one posture.
 *
 * `errorColor` is a live token reference, not a colour: KaTeX interpolates
 * it verbatim into an inline `style` attribute, where the CSS variable
 * resolves against the active scheme. Every error path carries it — the
 * whole-expression `.katex-error` span, rehype-katex's own fence fallback
 * (`settings.errorColor || '#cc0000'`), and the red in-place text KaTeX
 * emits for an undefined or untrusted command *inside* otherwise-valid
 * math, which no class-based CSS override could reach. */
export const KATEX_OPTIONS = {
    throwOnError: false,
    trust: false,
    strict: "ignore",
    errorColor: "var(--warn)",
} as const

/** Math is delimited by `$$…$$` and ```math fences ONLY. A single-dollar
 * span is not mathematics: it stays literal text, dollar signs included.
 *
 * The reason is prose safety, not parser taste. With single-dollar math on
 * (remark-math's default, and GitHub's behaviour), an ordinary sentence
 * that mentions two prices — "costs $50 per seat and $60 with add-ons" —
 * has everything between the dollars eaten and re-typeset as an italic
 * formula. A spec-reading tool silently corrupting prose is a worse
 * failure than diverging from GitHub on inline math, and the divergence
 * costs the author one extra character per side.
 *
 * Inline math is unaffected as a capability: `$$…$$` embedded in a
 * sentence still renders inline. */
export const REMARK_MATH_OPTIONS = { singleDollarTextMath: false } as const

// ---- Template comments ---------------------------------------------------

/// One HTML comment, as CommonMark reads one: `<!-->`, `<!--->`, or `<!--`,
/// then text not containing `-->`, then `-->`.
const HTML_COMMENT = /^<!--(?:-?>|(?:(?!-->)[\s\S])*-->)$/

/// Whether an `html` node's value is one comment and nothing else. Whitespace
/// around it passes, because a block keeps its indentation and trailing spaces
/// in its value; a comment with anything else beside it, a second comment
/// included, does not, and stays on screen as text.
export function isHtmlComment(value: string): boolean {
    return HTML_COMMENT.test(value.trim())
}

/// Drops each `html` node whose whole value is a comment: the boilerplate of
/// pull-request templates. It edits the parsed tree and never the source text,
/// so a removal cannot join what stood around a comment into a fence, a link or
/// an image (`[docs]<!-- -->(guide.md)` stays text), and a comment inside code,
/// which is a code node rather than an `html` one, stays visible.
export function remarkDropHtmlComments() {
    return (tree: Root) => {
        const drop = (parent: { children: RootContent[] }) => {
            for (let index = parent.children.length - 1; index >= 0; index -= 1) {
                const child = parent.children[index]
                if (child.type === "html" && isHtmlComment(child.value)) {
                    parent.children.splice(index, 1)
                } else if ("children" in child) {
                    drop(child)
                }
            }
        }
        drop(tree)
    }
}

// ---- Links ---------------------------------------------------------------

/// What a link in pull-request content does (`pull-request-viewer`: *Desktop
/// Link Opener*):
///
/// - `open` — an absolute `http` or `https` URL with a host. It opens outside
///   the app: through the desktop's opener, or as an opener-isolated new tab in
///   the browser skin. `url` is the URL as parsed, and `host` is what an
///   image's label shows;
/// - `relative` — a reference with no scheme (a path, a fragment, a
///   network-path `//host/…`). It shows its target and opens nothing;
/// - `inert` — every other scheme (`mailto`, `javascript`, `file`, `data`, an
///   application's own), an `http` or `https` reference that does not parse,
///   and an empty href. It opens nothing.
export type PullRequestHref =
    | { kind: "open"; url: string; host: string }
    | { kind: "relative"; target: string }
    | { kind: "inert" }

const INERT: PullRequestHref = { kind: "inert" }

export function pullRequestHref(href: string): PullRequestHref {
    const scheme = hrefScheme(href)
    if (scheme === null) return href === "" ? INERT : { kind: "relative", target: href }
    if (scheme !== "http" && scheme !== "https") return INERT
    // For these two schemes a URL parses only with a host.
    try {
        const url = new URL(href)
        return { kind: "open", url: url.href, host: url.host }
    } catch {
        return INERT
    }
}

// ---- Fences ----------------------------------------------------------------

/// The text a hast node holds, its descendants' included.
export function textOf(node: ElementContent): string {
    if (node.type === "text") return node.value
    if (node.type === "element") return node.children.map(textOf).join("")
    return ""
}

/** The raw source of a fence whose info string is `language` (e.g.
 * "mermaid", "svg"), or null if this <pre> isn't one. Walks the <code>
 * child's own children rather than reading `node`'s text directly, so it
 * reconstructs the original source intact even when rehype-highlight has
 * shredded it into `hljs-*` token spans (every language `MarkdownView` does
 * not exempt from highlighting) — the span wrapping never alters the text
 * itself, only decorates ranges of it. */
export function fenceSource(node: Element | undefined, language: string): string | null {
    const code = node?.children.find(
        (child): child is Element => child.type === "element",
    )
    if (code?.tagName !== "code") return null

    const className = code.properties?.className
    if (!Array.isArray(className) || !className.includes(`language-${language}`)) {
        return null
    }

    return code.children.map(textOf).join("").trimEnd()
}

// ---- Component overrides -------------------------------------------------

/// Where an openable link goes, per host.
export interface PullRequestLinkHost {
    /// The browser skin, where an openable link is a new, opener-isolated tab
    /// and nothing reaches the serving host.
    web: boolean
    /// The desktop's one way out of pull-request content:
    /// `openPullRequestLink(reference, href)`, never `openArtifactLink`.
    open: (href: string) => void
}

/// True beneath a link of the content, so an image there becomes part of that
/// link rather than a link of its own.
const InsideLink = createContext(false)

/// One link of the content, by the rule above, around `children`.
function contentLink(
    link: PullRequestHref,
    host: PullRequestLinkHost,
    children: ReactNode,
): ReactElement {
    const inner = <InsideLink.Provider value={true}>{children}</InsideLink.Provider>
    if (link.kind === "open") {
        return host.web ? (
            <a
                href={link.url}
                target="_blank"
                rel="noopener noreferrer"
                className="markdown-link markdown-link--external"
            >
                {inner}
            </a>
        ) : (
            <a
                href={link.url}
                className="markdown-link markdown-link--external"
                onClick={(event) => {
                    event.preventDefault()
                    host.open(link.url)
                }}
            >
                {inner}
            </a>
        )
    }
    // No anchor and no href, so nothing opens it by any interaction: not a
    // middle-click and not a context menu. The same dimmed span the browser
    // skin gives a workspace-file link, with a relative target in its tooltip.
    return (
        <span
            className="markdown-link markdown-link--unavailable"
            title={link.kind === "relative" ? link.target : undefined}
        >
            {inner}
        </span>
    )
}

/// What an image shows instead of itself: its alt text, or "image" when it has
/// none, and the host its source names, when it names one.
function imageLabel(alt: string, host: string | null): ReactElement {
    return (
        <span className="markdown-image">
            {alt === "" ? "image" : alt}
            {host !== null && " "}
            {host !== null && <span className="markdown-image-host">({host})</span>}
        </span>
    )
}

/// An image, which never becomes an `<img>`: a remote image is a tracking pixel
/// that tells its author when, and from where, the pull request was read. On
/// its own it becomes a link to its source, by the rule any link follows;
/// inside a link it becomes part of that link's label, with no link or handler
/// of its own, so one activation opens one destination.
function ContentImage({
    src,
    alt,
    host,
}: {
    src: string
    alt: string
    host: PullRequestLinkHost
}) {
    const insideLink = useContext(InsideLink)
    const link = pullRequestHref(src)
    const label = imageLabel(alt, link.kind === "open" ? link.host : null)
    return insideLink ? label : contentLink(link, host, label)
}

/// How the host draws a fence this mode leaves alone: the workspace renderer's
/// own `pre`, so an `svg` fence keeps its inert image rendering (`spec-browser`:
/// *SVG Fence Rendering*).
export type FenceRenderer = ComponentType<ComponentProps<"pre"> & ExtraProps>

function PlainFence({ node: _node, children, ...props }: ComponentProps<"pre"> & ExtraProps) {
    return <pre {...props}>{children}</pre>
}

/// The pull-request mode's overrides for react-markdown's `components`: links
/// by the rule above, images that never become `<img>`, and every `mermaid`
/// fence shown as its plain, unhighlighted source. A diagram is never drawn,
/// because the diagram engine fetches any remote image a diagram declares
/// while laying it out, whatever its security level. Every other fence goes to
/// `Fence`, a plain `<pre>` unless the host gives its own.
export function pullRequestComponents(
    host: PullRequestLinkHost,
    Fence: FenceRenderer = PlainFence,
) {
    return {
        a: ({ href, children }: ComponentProps<"a"> & ExtraProps) =>
            contentLink(pullRequestHref(href ?? ""), host, children),
        img: ({ src, alt }: ComponentProps<"img"> & ExtraProps) => (
            <ContentImage src={typeof src === "string" ? src : ""} alt={alt ?? ""} host={host} />
        ),
        pre: (props: ComponentProps<"pre"> & ExtraProps) => {
            const mermaid = fenceSource(props.node, "mermaid")
            if (mermaid === null) return <Fence {...props} />
            return (
                <pre>
                    <code className="language-mermaid">{mermaid}</code>
                </pre>
            )
        },
    } satisfies Components
}

// ---- The renderer's memo ---------------------------------------------------

/// Whether `MarkdownView` may skip a render: every prop unchanged by identity,
/// as React's own comparison has it, except the pull-request mode's
/// `pullRequest` reference, compared by its fields. Every other prop is a
/// primitive, a ref or a stable callback; the reference is a value, and a view
/// that builds an equal one on each of its renders would otherwise re-run
/// remark, rehype and KaTeX over every comment it shows, each time.
export function sameMarkdownViewProps(prev: object, next: object): boolean {
    const before = prev as Record<string, unknown>
    const after = next as Record<string, unknown>
    const keys = new Set([...Object.keys(before), ...Object.keys(after)])
    keys.delete("pullRequest")
    for (const key of keys) {
        if (!Object.is(before[key], after[key])) return false
    }
    return sameReferenceFields(
        before.pullRequest as PullRequestReference | undefined,
        after.pullRequest as PullRequestReference | undefined,
    )
}

/// Field by field and exactly, so a reference respelt in another case renders
/// again and its links open under the spelling now shown.
function sameReferenceFields(
    a: PullRequestReference | undefined,
    b: PullRequestReference | undefined,
): boolean {
    if (a === undefined || b === undefined) return a === b
    return (
        a.provider === b.provider &&
        a.owner === b.owner &&
        a.repo === b.repo &&
        a.number === b.number
    )
}
