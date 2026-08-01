import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import rehypeHighlight from "rehype-highlight"
import { useEffect, useRef, useState } from "react"
import type { RefObject } from "react"
import type { Element, ElementContent } from "hast"
import { MermaidBlock } from "./MermaidBlock"
import { SvgBlock } from "./SvgBlock"
import { Square, TaskCheckMark } from "./icons"
import { isWeb, openArtifactLink } from "../api"

// rehype-highlight runs before our component overrides do. Left alone it
// would shred a ```mermaid fence into hljs token spans before the source
// ever reaches MermaidBlock, so "mermaid" stays exempted here. A ```svg
// fence is deliberately NOT exempted: SvgBlock only intercepts a fence that
// parses as a valid, standalone SVG document (see its D3 gate), so a
// fence merely labelled `svg` that isn't one must still read as ordinary
// highlighted code (hljs aliases svg to its xml grammar) rather than
// unhighlighted plain text — fenceSource() below reconstructs the source
// intact either way, since hljs's span-wrapping never alters the
// underlying text, only decorates ranges of it.
const HIGHLIGHT_OPTIONS = { plainText: ["mermaid"] }

function textOf(node: ElementContent): string {
    if (node.type === "text") return node.value
    if (node.type === "element") return node.children.map(textOf).join("")
    return ""
}

/** The raw source of a fence whose info string is `language` (e.g.
 * "mermaid", "svg"), or null if this <pre> isn't one. Walks the <code>
 * child's own children rather than reading `node`'s text directly, so it
 * reconstructs the original source intact even when rehype-highlight has
 * shredded it into `hljs-*` token spans (every language not exempted via
 * HIGHLIGHT_OPTIONS.plainText) — the span wrapping never alters the text
 * itself, only decorates ranges of it. */
function fenceSource(node: Element | undefined, language: string): string | null {
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

/** True if `node` is a checked GFM task-list checkbox. */
function isCheckedTaskCheckbox(node: ElementContent): boolean {
    return (
        node.type === "element" &&
        node.tagName === "input" &&
        node.properties?.type === "checkbox" &&
        node.properties?.checked === true
    )
}

/**
 * True if a checked task checkbox appears among `li`'s hast children — as a
 * direct child (tight lists) or nested one level into a `<p>` child (loose
 * lists, where remark-gfm wraps the checkbox in the item's first paragraph;
 * scanning every `p` is equivalent since only the first can hold one).
 * Drives the `task-list-item--done` class that dims a completed line in CSS.
 */
function liIsDone(li: Element | undefined): boolean {
    if (!li) return false
    return li.children.some(
        (child) =>
            isCheckedTaskCheckbox(child) ||
            (child.type === "element" &&
                child.tagName === "p" &&
                child.children.some(isCheckedTaskCheckbox)),
    )
}

type LinkClass = "external" | "file" | "inert"

const MARKDOWN_LINK_EXTENSIONS = new Set(["md", "markdown"])

/**
 * The URI scheme prefix of `href` (lowercased), or null for a scheme-less
 * relative reference. Mirrors RFC 3986's `scheme = ALPHA *(ALPHA / DIGIT /
 * "+" / "-" / ".")` grammar — the same shape `openspec-app::service::
 * href_scheme` implements in Rust — so a relative markdown link (which never
 * starts with `ALPHA ":"`) is never misread as a scheme by either side.
 */
function hrefScheme(href: string): string | null {
    const colon = href.indexOf(":")
    if (colon <= 0) return null
    const prefix = href.slice(0, colon)
    return /^[a-zA-Z][a-zA-Z0-9+\-.]*$/.test(prefix) ? prefix.toLowerCase() : null
}

/**
 * Classify a link href for AFFORDANCE ONLY — the cursor/class it renders with
 * and whether clicking it dispatches the open command at all. The service
 * re-classifies authoritatively (`open_artifact_link`), so a mismatch here
 * degrades to the command's own quiet-failure path rather than a security
 * gap. Mirrors `resolve_artifact_link`'s classification order: scheme, then
 * fragment/query-stripped extension.
 */
function classifyHref(href: string): LinkClass {
    const scheme = hrefScheme(href)
    if (scheme) {
        return scheme === "http" || scheme === "https" || scheme === "mailto" || scheme === "tel"
            ? "external"
            : "inert" // javascript:, file:, data:, ...
    }
    if (href === "" || href.startsWith("#")) return "inert"

    const withoutFragment = href.split("#")[0] ?? ""
    const pathPart = withoutFragment.split("?")[0] ?? ""
    const dot = pathPart.lastIndexOf(".")
    const ext = dot >= 0 ? pathPart.slice(dot + 1).toLowerCase() : ""
    return MARKDOWN_LINK_EXTENSIONS.has(ext) ? "inert" : "file"
}

interface MarkdownViewProps {
    content: string
    containerRef?: RefObject<HTMLDivElement>
    /// The authorized root for resolving/opening links in this content — the
    /// registered workspace for artifact views, or the browse root for
    /// file-browser previews. Passed straight through to `openArtifactLink`.
    root: string
    /// The root-relative path of the markdown file being viewed. Relative
    /// file hrefs resolve against its parent directory.
    basePath: string
}

/// How long the quiet open-failure indication stays visible — the same tone
/// (and a comparable order of magnitude) as the mermaid invalid-diagram
/// note, but transient since there's no fenced block here to permanently
/// replace.
const LINK_FAILURE_MS = 1600

export function MarkdownView({
    content,
    containerRef,
    root,
    basePath,
}: MarkdownViewProps) {
    // A quiet, transient indication that the last click couldn't be opened —
    // no blanking, no navigation, matching the invalid-mermaid tone. Keyed by
    // a bump counter (not a boolean) so a second failure while the first
    // toast is still showing restarts its timer instead of leaving a stale
    // one racing to clear it early.
    const [failureCount, setFailureCount] = useState(0)
    const failureTimer = useRef<number | undefined>(undefined)

    useEffect(() => () => window.clearTimeout(failureTimer.current), [])

    function attemptOpen(href: string) {
        openArtifactLink(root, basePath, href).catch(() => {
            window.clearTimeout(failureTimer.current)
            setFailureCount((n) => n + 1)
            failureTimer.current = window.setTimeout(
                () => setFailureCount(0),
                LINK_FAILURE_MS,
            )
        })
    }
    return (
        <div ref={containerRef} className="markdown-view">
            <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                rehypePlugins={[[rehypeHighlight, HIGHLIGHT_OPTIONS]]}
                components={{
                    // Carry the source-line number through to a data attribute
                    // so the detail pane can scroll to a specific task, and
                    // flag completed task items with task-list-item--done so
                    // CSS can dim the line (App.css).
                    li: ({ node, className, ...props }) => (
                        <li
                            data-line={node?.position?.start?.line}
                            className={
                                liIsDone(node)
                                    ? `${className ?? ""} task-list-item--done`.trim()
                                    : className
                            }
                            {...props}
                        />
                    ),
                    // Read-only viewer: render task checkboxes as inert status
                    // glyphs instead of a native control (WKWebView renders a
                    // disabled input washed-out gray, undersized against the
                    // 16px body text). Checkbox state still reaches assistive
                    // technology via role/aria-checked/aria-disabled on the
                    // wrapping span; no tabIndex, so the document doesn't
                    // grow a keyboard focus stop per task line.
                    input: ({ node: _node, ...props }) =>
                        props.type === "checkbox" ? (
                            <span
                                role="checkbox"
                                aria-checked={props.checked ?? false}
                                aria-disabled="true"
                                className="task-checkbox"
                            >
                                {props.checked ? (
                                    <TaskCheckMark width={16} height={16} />
                                ) : (
                                    <Square width={16} height={16} />
                                )}
                            </span>
                        ) : (
                            // Unreachable without rehype-raw (remark-gfm only
                            // ever emits checkboxes), but if it ever runs the
                            // hast `node` stays off the DOM element.
                            <input {...props} />
                        ),
                    // A ```mermaid fence becomes a diagram and a ```svg fence
                    // becomes an image; every other fence stays on the
                    // syntax-highlighted path. Intercepting at <pre> rather
                    // than <code> keeps both out of the code-well styling and
                    // avoids nesting an <img>/<svg> in a <pre>.
                    pre: ({ node, children, ...props }) => {
                        const mermaid = fenceSource(node, "mermaid")
                        if (mermaid !== null) return <MermaidBlock source={mermaid} />

                        const svg = fenceSource(node, "svg")
                        if (svg !== null) {
                            return (
                                <SvgBlock
                                    source={svg}
                                    fallback={<pre {...props}>{children}</pre>}
                                />
                            )
                        }

                        return <pre {...props}>{children}</pre>
                    },
                    // Every anchor click is intercepted — no href class is
                    // ever handed to the webview's navigator. `preventDefault`
                    // fires unconditionally; only external/file classes go on
                    // to dispatch the validated open command. Destructuring
                    // `className` (unused) out of the rest-spread means our
                    // own `className` below always wins regardless of spread
                    // order — mirrors the `li` override's pattern above.
                    a: ({
                        node: _node,
                        href,
                        children,
                        className: _className,
                        ...rest
                    }) => {
                        const raw = href ?? ""
                        const cls = classifyHref(raw)

                        if (cls === "inert") {
                            return (
                                <a
                                    {...rest}
                                    href={raw}
                                    className="markdown-link markdown-link--inert"
                                    onClick={(e) => e.preventDefault()}
                                >
                                    {children}
                                </a>
                            )
                        }

                        if (cls === "external") {
                            // Web transport: the browser handles target=_blank
                            // natively — no command exists on that surface.
                            if (isWeb()) {
                                return (
                                    <a
                                        {...rest}
                                        href={raw}
                                        target="_blank"
                                        rel="noopener noreferrer"
                                        className="markdown-link markdown-link--external"
                                    >
                                        {children}
                                    </a>
                                )
                            }
                            return (
                                <a
                                    {...rest}
                                    href={raw}
                                    className="markdown-link markdown-link--external"
                                    onClick={(e) => {
                                        e.preventDefault()
                                        attemptOpen(raw)
                                    }}
                                >
                                    {children}
                                </a>
                            )
                        }

                        // cls === "file": a workspace-relative link. On the
                        // web transport the target may not even exist on the
                        // viewer's machine, so it degrades to a non-navigating
                        // affordance that presents the path — no anchor, no
                        // href, so nothing can navigate by any interaction.
                        if (isWeb()) {
                            return (
                                <span
                                    className="markdown-link markdown-link--file markdown-link--unavailable"
                                    title={raw}
                                >
                                    {children}
                                </span>
                            )
                        }
                        return (
                            <a
                                {...rest}
                                href={raw}
                                className="markdown-link markdown-link--file"
                                onClick={(e) => {
                                    e.preventDefault()
                                    attemptOpen(raw)
                                }}
                            >
                                {children}
                            </a>
                        )
                    },
                }}
            >
                {content}
            </ReactMarkdown>
            {failureCount > 0 && (
                <div
                    key={failureCount}
                    className="markdown-link-failure"
                    role="status"
                >
                    Couldn’t open that link
                </div>
            )}
        </div>
    )
}
