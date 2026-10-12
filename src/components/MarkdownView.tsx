import ReactMarkdown from "react-markdown"
import type { Components, ExtraProps, Options as MarkdownOptions } from "react-markdown"
import remarkGfm from "remark-gfm"
import remarkMath from "remark-math"
import rehypeHighlight from "rehype-highlight"
import {
    createContext,
    memo,
    useCallback,
    useContext,
    useEffect,
    useImperativeHandle,
    useLayoutEffect,
    useMemo,
    useRef,
    useState,
} from "react"
import type { ComponentProps, Ref, RefObject } from "react"
import type { Element, ElementContent, Root as HastRoot } from "hast"
import type { Root, RootContent } from "mdast"
import type { VFile } from "vfile"
import { MermaidBlock } from "./MermaidBlock"
import { SvgBlock } from "./SvgBlock"
import { Square, TaskCheckMark } from "./icons"
import { isWeb, openArtifactLink, openPullRequestLink } from "../api"
import { classifyHref, fragmentTarget } from "../links"
import { loadMath, loadedMath, mathReady } from "../mathEngine"
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
/// below, once `mathEngine.ts` has loaded it) matches any <pre><code class="language-math"> at the hast stage —
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

// ---- The workspace overrides ----------------------------------------------
//
// Module-level components, never arrows built in a render: react-markdown
// takes each override as an element TYPE, and a type that is new on every
// render makes React unmount and remount every heading, list item, table,
// link and fence beneath it — every `MermaidBlock` falling back to its pending
// placeholder and drawing again on each live edit (`spec-browser`: *Rendered
// Blocks Survive Document Re-renders*; design D1). What a render must tell
// them reaches them through `MarkdownLinkContext` instead.

type HeadingTag = "h1" | "h2" | "h3" | "h4" | "h5" | "h6"

/// A heading override: the derived identifier (so a fragment link resolves to
/// it) and the source line (so the outline scrolls to it through the same
/// `data-line` path a task line uses). Pull-request headings come here too,
/// and carry no identifier, because their pass stamps none (`headingIdPass`).
function headingOverride(Tag: HeadingTag) {
    function Heading({ node, children, ...props }: ComponentProps<HeadingTag> & ExtraProps) {
        return (
            <Tag {...props} id={headingIdOf(node)} data-line={node?.position?.start?.line}>
                {children}
            </Tag>
        )
    }
    Heading.displayName = `Markdown${Tag.toUpperCase()}`
    return Heading
}

/// Carries the source-line number through to a data attribute so the detail
/// pane can scroll to a specific task, and flags completed task items with
/// task-list-item--done so CSS can dim the line (App.css).
function ListItem({ node, className, ...props }: ComponentProps<"li"> & ExtraProps) {
    return (
        <li
            data-line={node?.position?.start?.line}
            className={liIsDone(node) ? `${className ?? ""} task-list-item--done`.trim() : className}
            {...props}
        />
    )
}

/// Read-only viewer: task checkboxes render as inert status glyphs instead of
/// a native control (WKWebView renders a disabled input washed-out gray,
/// undersized against the 16px body text). Checkbox state still reaches
/// assistive technology via role/aria-checked/aria-disabled on the wrapping
/// span; no tabIndex, so the document doesn't grow a keyboard focus stop per
/// task line.
function TaskCheckbox({ node: _node, ...props }: ComponentProps<"input"> & ExtraProps) {
    if (props.type !== "checkbox") {
        // Unreachable without rehype-raw (remark-gfm only ever emits
        // checkboxes), but if it ever runs the hast `node` stays off the DOM
        // element.
        return <input {...props} />
    }
    return (
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
    )
}

/// A table is wrapped in its own scroll container so a table too wide for the
/// content column scrolls within its own bounds instead of widening the
/// document or panning the whole pane (spec-browser: *Wide Block
/// Containment*). Wrapping rather than styling the table itself with
/// `display: block; overflow: auto` is deliberate: that recipe costs the
/// element its table semantics — the accessibility tree, caption behaviour,
/// and column-width negotiation all key off the table display types. The
/// wrapper carries the block margin (App.css) so the scrollbar hugs the table.
function ScrollingTable({ node: _node, ...props }: ComponentProps<"table"> & ExtraProps) {
    return (
        <div className="table-scroll">
            <table {...props} />
        </div>
    )
}

/// What a workspace link needs of the render that shows it. Provided afresh
/// by every render of `MarkdownViewImpl`, so each link re-renders (never
/// remounts) with its document and classifies itself against THAT render's
/// headings (`document-outline`: *Fragment Links Resolve Within the
/// Document*).
interface MarkdownLinks {
    /// The identifiers of this render's headings. A function, built on first
    /// use: `rehypeHeadingIds` fills the render's heading list before any
    /// `<a>` renders, so the first call sees the complete set.
    knownHeadingIds: () => ReadonlySet<string>
    onFragment: ((id: string) => void) | undefined
    /// Opens a workspace link through `openArtifactLink`, reporting a failure.
    attemptOpen: (href: string) => void
    /// Shows the quiet "could not open" indication.
    reportFailure: () => void
    /// The browser skin, where external links are new tabs and workspace
    /// files open nothing.
    web: boolean
}

const NO_HEADING_IDS: ReadonlySet<string> = new Set()

const MarkdownLinkContext = createContext<MarkdownLinks>({
    knownHeadingIds: () => NO_HEADING_IDS,
    onFragment: undefined,
    attemptOpen: () => {},
    reportFailure: () => {},
    web: false,
})

/// Every anchor click is intercepted — no href class is ever handed to the
/// webview's navigator. `preventDefault` fires unconditionally; only
/// external/file classes go on to dispatch the validated open command.
/// Destructuring `className` (unused) out of the rest-spread means our own
/// `className` below always wins regardless of spread order — mirrors the
/// `li` override's pattern above.
function WorkspaceLink({
    node: _node,
    href,
    children,
    className: _className,
    ...rest
}: ComponentProps<"a"> & ExtraProps) {
    const { knownHeadingIds, onFragment, attemptOpen, reportFailure, web } =
        useContext(MarkdownLinkContext)
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

    // A fragment naming a heading of THIS document scrolls to it — no
    // navigation, no address change, no history entry. It is not inert and
    // must not wear the dead-link affordance.
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

    // A fragment naming no heading falls into the same quiet indication a
    // missing file gets: it could not be followed, and the document stays
    // fully usable.
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
        // Web transport: the browser handles target=_blank natively — no
        // command exists on that surface.
        if (web) {
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

    // cls === "file": a workspace-relative link. On the web transport the
    // target may not even exist on the viewer's machine, so it degrades to a
    // non-navigating affordance that presents the path — no anchor, no href,
    // so nothing can navigate by any interaction.
    if (web) {
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
}

/// The workspace mode's overrides, one module constant, so every render hands
/// react-markdown the same element types. Fences go straight to
/// `WorkspaceFence`, so an unchanged diagram stays mounted — and drawn —
/// across a render of its document.
export const WORKSPACE_COMPONENTS: Readonly<Components> = Object.freeze({
    h1: headingOverride("h1"),
    h2: headingOverride("h2"),
    h3: headingOverride("h3"),
    h4: headingOverride("h4"),
    h5: headingOverride("h5"),
    h6: headingOverride("h6"),
    li: ListItem,
    input: TaskCheckbox,
    table: ScrollingTable,
    pre: WorkspaceFence,
    a: WorkspaceLink,
})

/// The merged overrides for each pull-request mode's set, made once per set.
const mergedComponents = new WeakMap<object, Readonly<Components>>()

/// The overrides a render hands react-markdown: the workspace's alone, or, for
/// pull-request content, the workspace's with the mode's links, images and
/// fences over them (PR overrides last, so they win). The same object for the
/// same `overrides`, so a document's element types never change while its
/// mode does not.
export function markdownComponents(overrides: Partial<Components> | null): Readonly<Components> {
    if (overrides === null) return WORKSPACE_COMPONENTS
    let merged = mergedComponents.get(overrides)
    if (merged === undefined) {
        merged = Object.freeze({ ...WORKSPACE_COMPONENTS, ...overrides })
        mergedComponents.set(overrides, merged)
    }
    return merged
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

/// What `MarkdownViewImpl` asks of its failure indication.
interface LinkFailureHandle {
    /// Shows the indication, or restarts it while it shows.
    report: () => void
}

/// A quiet, transient indication that the last click couldn't be opened — no
/// blanking, no navigation, matching the invalid-mermaid tone. Its own
/// component, owning its count and its timer, so showing and clearing it
/// renders this alone and never reruns the document's markdown pipeline
/// (`spec-browser`: *Rendered Blocks Survive Document Re-renders*; design D2).
///
/// Keyed by a bump counter (not a boolean) so a second failure while the first
/// toast is still showing restarts its timer, and its animation, instead of
/// leaving a stale one racing to clear it early.
function LinkFailureToast({ ref }: { ref: Ref<LinkFailureHandle> }) {
    const [failureCount, setFailureCount] = useState(0)
    const failureTimer = useRef<number | undefined>(undefined)

    useEffect(() => () => window.clearTimeout(failureTimer.current), [])

    useImperativeHandle(
        ref,
        () => ({
            report() {
                window.clearTimeout(failureTimer.current)
                setFailureCount((n) => n + 1)
                failureTimer.current = window.setTimeout(
                    () => setFailureCount(0),
                    LINK_FAILURE_MS,
                )
            },
        }),
        [],
    )

    if (failureCount === 0) return null
    return (
        <div key={failureCount} className="markdown-link-failure" role="status">
            Couldn’t open that link
        </div>
    )
}

function MarkdownViewImpl({
    content,
    containerRef,
    root,
    basePath,
    pullRequest,
    onHeadings,
    onFragment,
}: MarkdownViewProps & MarkdownSource) {
    // Stable for the view's life: it reads the toast through a ref, so the
    // pull-request overrides built once below, and every link, report into
    // the toast without this view rendering again.
    const toastRef = useRef<LinkFailureHandle>(null)
    const reportFailure = useCallback(() => toastRef.current?.report(), [])

    // The pull-request mode's links, images and fences, in place of the
    // workspace's: links by the mode's rule, through `openPullRequestLink` on
    // the desktop; images that never become `<img>`; and `mermaid` fences as
    // their source, every other fence going to `WorkspaceFence`. Built once
    // per pull request rather than per render: every call makes fresh
    // components, and a fresh component is a new element type, which would
    // remount each link, image and fence on every render. The `reportFailure`
    // it captures is the stable one above.
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
        // The reference's fields, not the object a caller may rebuild per render.
        // eslint-disable-next-line react-hooks/exhaustive-deps
        [pullRequest?.provider, pullRequest?.owner, pullRequest?.repo, pullRequest?.number],
    )

    // The same object while the mode does not change: see `markdownComponents`.
    const components = useMemo(
        () => markdownComponents(pullRequestOverrides),
        [pullRequestOverrides],
    )

    // Maths needs its engine, which loads on demand (`mathEngine.ts`). A
    // document that may hold maths while the engine is not loaded keeps the
    // last content this view fully rendered, or shows nothing when it has
    // rendered none, and renders again once the engine is ready: maths never
    // shows as its source (`spec-browser`: *Surfaces and Rendering Engines
    // Load on Demand*). Never a Suspense throw, which would blank a document
    // the reader is looking at for a refresh they did not start. A load that
    // fails ends the wait: the document then renders with its maths as code,
    // and keeps following edits, rather than stay blank or frozen.
    const katex = loadedMath()
    const [mathFailed, setMathFailed] = useState(false)
    const ready = mathReady(content, mathFailed)
    const lastRenderedRef = useRef<string | null>(null)
    const shown = ready ? content : lastRenderedRef.current
    useLayoutEffect(() => {
        if (shown !== null) lastRenderedRef.current = shown
    }, [shown])
    const [, setMathLoaded] = useState(false)
    useEffect(() => {
        if (ready) return
        let current = true
        loadMath().then(
            () => {
                if (current) setMathLoaded(true)
            },
            (err: unknown) => {
                console.warn("failed to load the mathematics engine:", err)
                if (current) setMathFailed(true)
            },
        )
        return () => {
            current = false
        }
    }, [ready])

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

    // What this render tells its links, made per render so each link
    // classifies itself against this render's headings. A new value
    // re-renders the links; it never remounts them.
    const links: MarkdownLinks = {
        knownHeadingIds,
        onFragment,
        attemptOpen: (href) => {
            // Workspace markdown always has both. Pull-request content never
            // comes here: the mode's own links replace the workspace's.
            if (root === undefined || basePath === undefined) return
            openArtifactLink(root, basePath, href).catch(reportFailure)
        },
        reportFailure,
        web: isWeb(),
    }

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
    // Last, once loaded: see `ready` above.
    const mathPass: PluggableList = katex ? [[katex, KATEX_OPTIONS]] : []

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
            <MarkdownLinkContext.Provider value={links}>
                <ReactMarkdown
                    remarkPlugins={[
                        remarkGfm,
                        [remarkMath, REMARK_MATH_OPTIONS],
                        remarkPromoteStandaloneDisplayMath,
                        // Pull-request content drops its template comments,
                        // last, so every pass above sees the tree as its
                        // author wrote it.
                        ...(pullRequestOverrides ? [remarkDropHtmlComments] : []),
                    ]}
                    rehypePlugins={[
                        // First: see `headingIdPass`.
                        ...headingIdPass,
                        [rehypeHighlight, HIGHLIGHT_OPTIONS],
                        ...mathPass,
                    ]}
                    components={components}
                >
                    {shown ?? ""}
                </ReactMarkdown>
            </MarkdownLinkContext.Provider>
            <LinkFailureToast ref={toastRef} />
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
///
/// A render this memo lets through (new content) is cheap where it used to be
/// costly: the overrides are module-level components (`WORKSPACE_COMPONENTS`,
/// merged once per pull request by `markdownComponents`), so unchanged blocks
/// keep their elements and an unchanged diagram stays drawn. The link-failure
/// indication renders on its own (`LinkFailureToast`) and never brings this
/// view through again.
export const MarkdownView = memo(MarkdownViewImpl, sameMarkdownViewProps)
