import ReactMarkdown from "react-markdown"
import type { ExtraProps, Options as MarkdownOptions } from "react-markdown"
import remarkGfm from "remark-gfm"
import remarkMath from "remark-math"
import rehypeHighlight from "rehype-highlight"
import rehypeKatex from "rehype-katex"
import "katex/dist/katex.min.css"
import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react"
import type { ComponentProps, RefObject } from "react"
import type { Element, ElementContent, Root as HastRoot } from "hast"
import type { Root, RootContent } from "mdast"
import type { VFile } from "vfile"
import { MermaidBlock } from "./MermaidBlock"
import { SvgBlock } from "./SvgBlock"
import { Square, TaskCheckMark } from "./icons"
import { isWeb, openArtifactLink, openPullRequestLink } from "../api"
import { classifyHref, fragmentTarget } from "../links"
import { headingId, type HeadingEntry } from "../outline"
import {
    fenceSource,
    KATEX_OPTIONS,
    pullRequestComponents,
    REMARK_MATH_OPTIONS,
    remarkDropHtmlComments,
    sameMarkdownViewProps,
    textOf,
} from "../pullRequestMarkdown"
import type { PullRequestReference } from "../types"

// rehype-highlight runs before our component overrides do. Left alone it
// would shred a ```mermaid fence into hljs token spans before the source
// ever reaches MermaidBlock, so "mermaid" stays exempted here. A ```svg
// fence is deliberately NOT exempted: SvgBlock only intercepts a fence that
// parses as a valid, standalone SVG document (see its D3 gate), so a
// fence merely labelled `svg` that isn't one must still read as ordinary
// highlighted code (hljs aliases svg to its xml grammar) rather than
// unhighlighted plain text — fenceSource() reconstructs the source
// intact either way, since hljs's span-wrapping never alters the
// underlying text, only decorates ranges of it.
const HIGHLIGHT_OPTIONS = { plainText: ["mermaid"] }

/// A list of unified plugins, as react-markdown takes them.
type PluggableList = NonNullable<MarkdownOptions["rehypePlugins"]>

// KaTeX's and remark-math's options — `$$…$$` and ```math fences only, and
// KaTeX's non-trusting posture — live in `pullRequestMarkdown.tsx`, where the
// pull-request mode's tests read them too.

/**
 * remark-math parses a double-dollar expression sitting on a single line
 * ($$E=mc^2$$ alone in a paragraph) as *inline* math — only the multi-line
 * $$-fenced block form yields a display node. GitHub renders the standalone
 * single-line form as a block (it is the example its math docs use), so
 * promote a paragraph whose sole content is one double-dollar inlineMath
 * node to a display math block. $$…$$ embedded in surrounding prose stays
 * inline, matching GitHub.
 *
 * The source-offset check below outlived its original second purpose:
 * with `singleDollarTextMath: false` no single-dollar node reaches this
 * plugin, so the check can no longer *distinguish* one — it now simply
 * confirms what the parser already guaranteed. It stays because it is the
 * honest guard for what this function claims to promote, and because
 * flipping that option back would restore its discriminating role.
 */
function remarkPromoteStandaloneDisplayMath() {
    return (tree: Root, file: VFile) => {
        const source = String(file.value)
        const promote = (parent: { children: RootContent[] }) => {
            parent.children.forEach((child, index) => {
                if (child.type === "paragraph" && child.children.length === 1) {
                    const only = child.children[0]
                    const offset = only?.position?.start.offset
                    if (
                        only?.type === "inlineMath" &&
                        typeof offset === "number" &&
                        source.startsWith("$$", offset)
                    ) {
                        // The `data` recipe mirrors what mdast-util-math puts
                        // on a parsed flow-math node — without it, remark-
                        // rehype has no handler for the bare node type and
                        // degrades it to plain text.
                        parent.children[index] = {
                            type: "math",
                            value: only.value,
                            position: child.position,
                            data: {
                                hName: "pre",
                                hChildren: [
                                    {
                                        type: "element",
                                        tagName: "code",
                                        properties: {
                                            className: [
                                                "language-math",
                                                "math-display",
                                            ],
                                        },
                                        children: [
                                            { type: "text", value: only.value },
                                        ],
                                    },
                                ],
                            },
                        }
                        return
                    }
                }
                if ("children" in child) promote(child)
            })
        }
        promote(tree)
    }
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

/**
 * Stamp every heading with its identifier, and collect the document's heading
 * structure into `sink`, in document order.
 *
 * A rehype pass rather than work inside the heading COMPONENTS, because the
 * anchors need the complete identifier set to classify a fragment link, and a
 * link can precede the heading it names. unified builds the whole hast tree
 * before `toJsxRuntime` renders any of it, so running here means the set is
 * complete before the first `<a>` is asked what class it is
 * (`document-outline`: *Fragment Links Resolve Within the Document*).
 *
 * The derivation itself is `headingId`, which is pure and unit-tested; this
 * only walks the tree and mutates `properties.id`.
 */
function rehypeHeadingIds(sink: HeadingEntry[]) {
    return (tree: HastRoot) => {
        sink.length = 0
        const seen = new Map<string, number>()
        const walk = (node: HastRoot | Element) => {
            for (const child of node.children) {
                if (child.type !== "element") continue
                if (/^h[1-6]$/.test(child.tagName)) {
                    const text = textOf(child).trim()
                    const id = headingId(text, seen)
                    if (id !== "") {
                        child.properties = { ...child.properties, id }
                    }
                    sink.push({
                        level: Number(child.tagName.slice(1)),
                        text,
                        line: child.position?.start?.line ?? 0,
                        id,
                    })
                    continue
                }
                walk(child)
            }
        }
        walk(tree)
    }
}

/// The identifier `rehypeHeadingIds` stamped on this heading, or undefined for
/// a heading whose text carries none — an empty `id` attribute names nothing
/// and is better not rendered at all.
function headingIdOf(node: Element | undefined): string | undefined {
    const id = node?.properties?.id
    return typeof id === "string" && id !== "" ? id : undefined
}

/// How a fence is drawn. A ```mermaid fence becomes a diagram and a ```svg
/// fence becomes an image; every other fence stays on the syntax-highlighted
/// path. Intercepting at <pre> rather than <code> keeps both out of the
/// code-well styling and avoids nesting an <img>/<svg> in a <pre>. A ```math
/// fence is deliberately NOT handled here: rehypeKatex (in rehypePlugins
/// below) matches any <pre><code class="language-math"> at the hast stage —
/// the same way it matches $…$/$$…$$ — and splices the whole <pre> out before
/// this component ever runs, replacing it with rendered display math or, for
/// invalid input, its own .katex-error span. A component-level interception
/// here would never run.
///
/// The pull-request mode hands it every fence but a `mermaid` one, which it
/// shows as source and never draws, so an `svg` fence keeps its inert image
/// there too (`pull-request-viewer`: *Pull-Request Content Is Untrusted*).
function WorkspaceFence({ node, children, ...props }: ComponentProps<"pre"> & ExtraProps) {
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
}

interface MarkdownViewProps {
    content: string
    containerRef?: RefObject<HTMLDivElement | null>
    /// Called after each render with the document's headings, in document
    /// order — what the surrounding document view builds its outline from.
    /// MUST be referentially stable (see the memo note at the bottom).
    onHeadings?: (headings: HeadingEntry[]) => void
    /// Called when a fragment-only link naming one of this document's headings
    /// is activated. The scroll itself belongs to the document view, which is
    /// what measures the sticky header. MUST be referentially stable.
    onFragment?: (id: string) => void
}

/// Whose markdown this is, which decides what its links may open: a
/// workspace's, by its root and path, or a pull request's, by its reference.
type MarkdownSource =
    | {
          /// The authorized root for resolving/opening links in this content —
          /// the registered workspace for artifact views, or the browse root
          /// for file-browser previews. Passed straight through to
          /// `openArtifactLink`.
          root: string
          /// The root-relative path of the markdown file being viewed.
          /// Relative file hrefs resolve against its parent directory.
          basePath: string
          pullRequest?: undefined
      }
    | {
          /// The pull request whose content this is — its description, a
          /// conversation entry, a review-thread comment — written by others,
          /// so rendered in the pull-request mode, which is the guard wherever
          /// it shows, the main window included (`pull-request-viewer`:
          /// *Pull-Request Content Is Untrusted*, *Desktop Link Opener*). Its
          /// links open through this reference alone: `openPullRequestLink` on
          /// the desktop, never `openArtifactLink`.
          pullRequest: PullRequestReference
          root?: undefined
          basePath?: undefined
      }

/// How long the quiet open-failure indication stays visible — the same tone
/// (and a comparable order of magnitude) as the mermaid invalid-diagram
/// note, but transient since there's no fenced block here to permanently
/// replace.
const LINK_FAILURE_MS = 1600

function MarkdownViewImpl({
    content,
    containerRef,
    root,
    basePath,
    pullRequest,
    onHeadings,
    onFragment,
}: MarkdownViewProps & MarkdownSource) {
    // A quiet, transient indication that the last click couldn't be opened —
    // no blanking, no navigation, matching the invalid-mermaid tone. Keyed by
    // a bump counter (not a boolean) so a second failure while the first
    // toast is still showing restarts its timer instead of leaving a stale
    // one racing to clear it early.
    const [failureCount, setFailureCount] = useState(0)
    const failureTimer = useRef<number | undefined>(undefined)

    useEffect(() => () => window.clearTimeout(failureTimer.current), [])

    function reportFailure() {
        window.clearTimeout(failureTimer.current)
        setFailureCount((n) => n + 1)
        failureTimer.current = window.setTimeout(
            () => setFailureCount(0),
            LINK_FAILURE_MS,
        )
    }

    function attemptOpen(href: string) {
        // Workspace markdown always has both. Pull-request content never comes
        // here: the mode's own links replace the workspace's.
        if (root === undefined || basePath === undefined) return
        openArtifactLink(root, basePath, href).catch(reportFailure)
    }

    // The pull-request mode's links, images and fences, in place of the
    // workspace's: links by the mode's rule, through `openPullRequestLink` on
    // the desktop; images that never become `<img>`; and `mermaid` fences as
    // their source, every other fence going to `WorkspaceFence`. Built once
    // per pull request rather than per render: every call makes fresh
    // components, and a fresh component is a new element type, which would
    // remount each link, image and fence on every render. The `reportFailure`
    // it captures serves as well as any later render's, since it touches only
    // a ref and a state setter.
    const pullRequestOverrides = useMemo(
        () =>
            pullRequest
                ? pullRequestComponents(
                      {
                          web: isWeb(),
                          open: (href) => {
                              openPullRequestLink(pullRequest, href).catch(reportFailure)
                          },
                      },
                      WorkspaceFence,
                  )
                : null,
        // eslint-disable-next-line react-hooks/exhaustive-deps -- the
        // reference's fields, not the object a caller may rebuild per render.
        [pullRequest?.provider, pullRequest?.owner, pullRequest?.repo, pullRequest?.number],
    )

    // Filled by `rehypeHeadingIds` while `<ReactMarkdown>` below renders, so
    // it is complete before any `<a>` is classified and before the effect at
    // the end of this render reports it upward. A fresh array per render: a
    // discarded render's array is discarded with it.
    const headings: HeadingEntry[] = []
    // Built once per render, on first use — which is necessarily after the
    // rehype pass has filled `headings`.
    let headingIds: Set<string> | null = null
    const knownHeadingIds = () =>
        (headingIds ??= new Set(headings.map((h) => h.id).filter((id) => id !== "")))

    // The first rehype pass, so heading text is read before rehype-katex or
    // rehype-highlight can rewrite anything inside a heading.
    //
    // Registered as a `[plugin, options]` tuple, NOT as
    // `rehypeHeadingIds(headings)`: unified calls the plugin (the attacher) at
    // freeze time with its options and keeps what it RETURNS as the
    // transformer. Passing the transformer itself makes unified call it with
    // the options (`undefined`) as the tree, which reads `.children` of
    // `undefined` and unmounts the whole app.
    //
    // Workspace markdown only. Pull-request content's headings carry no
    // identifier: a stranger's heading would stamp an `id` of its choosing into
    // the page, where `id="root"` names the application's own root and is
    // styled as it, and the mode reads none, since it builds no outline and its
    // fragment links open nothing.
    const headingIdPass: PluggableList = pullRequestOverrides
        ? []
        : [[rehypeHeadingIds, headings]]

    // No dependency array: this runs after EVERY commit, which is the only
    // point at which `headings` is known to be filled. The receiver bails when
    // the list is unchanged, so reporting cannot loop.
    //
    // A LAYOUT effect, not a passive one: the receiver's `setState` then lands
    // in the same commit as the document, so the outline can never describe a
    // document other than the one on screen — not even for the frame or two a
    // passive effect may wait for the scheduler (or far longer in a hidden
    // tab, where the outline was observed lagging a whole navigation behind).
    // The work is one comparator run per commit, which is cheap.
    useLayoutEffect(() => {
        onHeadings?.(headings)
    })

    return (
        <div ref={containerRef} className="markdown-view">
            <ReactMarkdown
                remarkPlugins={[
                    remarkGfm,
                    [remarkMath, REMARK_MATH_OPTIONS],
                    remarkPromoteStandaloneDisplayMath,
                    // Pull-request content drops its template comments, last,
                    // so every pass above sees the tree as its author wrote it.
                    ...(pullRequestOverrides ? [remarkDropHtmlComments] : []),
                ]}
                rehypePlugins={[
                    // First: see `headingIdPass`.
                    ...headingIdPass,
                    [rehypeHighlight, HIGHLIGHT_OPTIONS],
                    [rehypeKatex, KATEX_OPTIONS],
                ]}
                components={{
                    // Headings carry their derived identifier (so a fragment
                    // link resolves to them) and their source line (so the
                    // outline scrolls to them through the same `data-line`
                    // path a task line uses).
                    h1: ({ node, children, ...props }) => (
                        <h1
                            {...props}
                            id={headingIdOf(node)}
                            data-line={node?.position?.start?.line}
                        >
                            {children}
                        </h1>
                    ),
                    h2: ({ node, children, ...props }) => (
                        <h2
                            {...props}
                            id={headingIdOf(node)}
                            data-line={node?.position?.start?.line}
                        >
                            {children}
                        </h2>
                    ),
                    h3: ({ node, children, ...props }) => (
                        <h3
                            {...props}
                            id={headingIdOf(node)}
                            data-line={node?.position?.start?.line}
                        >
                            {children}
                        </h3>
                    ),
                    h4: ({ node, children, ...props }) => (
                        <h4
                            {...props}
                            id={headingIdOf(node)}
                            data-line={node?.position?.start?.line}
                        >
                            {children}
                        </h4>
                    ),
                    h5: ({ node, children, ...props }) => (
                        <h5
                            {...props}
                            id={headingIdOf(node)}
                            data-line={node?.position?.start?.line}
                        >
                            {children}
                        </h5>
                    ),
                    h6: ({ node, children, ...props }) => (
                        <h6
                            {...props}
                            id={headingIdOf(node)}
                            data-line={node?.position?.start?.line}
                        >
                            {children}
                        </h6>
                    ),
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
                    // A table is wrapped in its own scroll container so a
                    // table too wide for the content column scrolls within
                    // its own bounds instead of widening the document or
                    // panning the whole pane (spec-browser: *Wide Block
                    // Containment*). Wrapping rather than styling the table
                    // itself with `display: block; overflow: auto` is
                    // deliberate: that recipe costs the element its table
                    // semantics — the accessibility tree, caption behaviour,
                    // and column-width negotiation all key off the table
                    // display types. The wrapper carries the block margin
                    // (App.css) so the scrollbar hugs the table.
                    table: ({ node: _node, ...props }) => (
                        <div className="table-scroll">
                            <table {...props} />
                        </div>
                    ),
                    // Diagrams, images and highlighted code: `WorkspaceFence`.
                    // Through an arrow, as every override here is, so a fence
                    // mounts and remounts on the same renders as its siblings.
                    pre: (props) => <WorkspaceFence {...props} />,
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
                        const cls = classifyHref(raw, knownHeadingIds())

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

                        // A fragment naming a heading of THIS document scrolls
                        // to it — no navigation, no address change, no history
                        // entry. It is not inert and must not wear the dead-
                        // link affordance.
                        if (cls === "fragment") {
                            return (
                                <a
                                    {...rest}
                                    href={raw}
                                    className="markdown-link markdown-link--fragment"
                                    onClick={(e) => {
                                        e.preventDefault()
                                        const id = fragmentTarget(raw)
                                        if (id) onFragment?.(id)
                                    }}
                                >
                                    {children}
                                </a>
                            )
                        }

                        // A fragment naming no heading falls into the same
                        // quiet indication a missing file gets: it could not be
                        // followed, and the document stays fully usable.
                        if (cls === "danglingFragment") {
                            return (
                                <a
                                    {...rest}
                                    href={raw}
                                    className="markdown-link markdown-link--inert"
                                    onClick={(e) => {
                                        e.preventDefault()
                                        reportFailure()
                                    }}
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
                    // Pull-request content: the mode's own links, images and
                    // fences, over the workspace's.
                    ...pullRequestOverrides,
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

/// Memoized on its props, which is a **correctness prerequisite** for the detail
/// pane's equality guard rather than a spare optimization.
///
/// `refreshPolicy`'s `reduce` compares content AND modification time, so a file
/// rewritten with identical bytes produces a new state object carrying a
/// referentially-equal `content`. Without this boundary that new object would
/// re-run the whole pipeline below — remark, rehype, every `MermaidBlock`,
/// KaTeX, the SVG gate — to move a text label in the header. With it, the
/// shallow comparison sees the same string and skips.
///
/// That reducer path is the whole justification. The header's *ticking* label is
/// not: its timer state sits in a leaf component, so a tick re-renders only
/// itself and never reaches here regardless of this memo.
///
/// **The constraint this depends on, which no type enforces:** every prop must
/// stay a primitive or a stably-identified ref. Today `content`, `root` and
/// `basePath` are strings, `containerRef` is a `useRef`, and `onHeadings` /
/// `onFragment` are `useCallback`s with empty dependency arrays in the one
/// caller (`DocumentView`) that passes them. Adding an inline object, array, or
/// callback prop would defeat the shallow comparison silently, with no error
/// and no test failure — only a document that repaints on every tick.
///
/// The one object prop, the pull-request mode's `pullRequest` reference, is
/// the exception the comparison makes: `sameMarkdownViewProps` compares it by
/// its fields and every other prop by identity, exactly as the default does.
export const MarkdownView = memo(MarkdownViewImpl, sameMarkdownViewProps)
