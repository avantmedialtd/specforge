import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import rehypeHighlight from "rehype-highlight"
import type { RefObject } from "react"
import type { Element, ElementContent } from "hast"
import { MermaidBlock } from "./MermaidBlock"
import { SvgBlock } from "./SvgBlock"
import { Square, TaskCheckMark } from "./icons"

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

interface MarkdownViewProps {
    content: string
    containerRef?: RefObject<HTMLDivElement>
}

export function MarkdownView({ content, containerRef }: MarkdownViewProps) {
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
                }}
            >
                {content}
            </ReactMarkdown>
        </div>
    )
}
