import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import rehypeHighlight from "rehype-highlight"
import type { RefObject } from "react"
import type { Element, ElementContent } from "hast"
import { MermaidBlock } from "./MermaidBlock"

// rehype-highlight runs before our component overrides do. Left alone it would
// shred a ```mermaid fence into hljs token spans, and the diagram source would
// never reach MermaidBlock intact.
const HIGHLIGHT_OPTIONS = { plainText: ["mermaid"] }

function textOf(node: ElementContent): string {
    if (node.type === "text") return node.value
    if (node.type === "element") return node.children.map(textOf).join("")
    return ""
}

/** The raw source of a ```mermaid fence, or null if this <pre> isn't one. */
function mermaidSource(node: Element | undefined): string | null {
    const code = node?.children.find(
        (child): child is Element => child.type === "element",
    )
    if (code?.tagName !== "code") return null

    const className = code.properties?.className
    if (!Array.isArray(className) || !className.includes("language-mermaid")) {
        return null
    }

    return code.children.map(textOf).join("").trimEnd()
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
                    // so the detail pane can scroll to a specific task.
                    li: ({ node, ...props }) => (
                        <li
                            data-line={node?.position?.start?.line}
                            {...props}
                        />
                    ),
                    // Read-only viewer: render task checkboxes but keep them
                    // inert. Disabling stops both visual click feedback and
                    // any write-back attempts.
                    input: (props) =>
                        props.type === "checkbox" ? (
                            <input
                                {...props}
                                disabled
                                readOnly
                                onChange={() => {}}
                            />
                        ) : (
                            <input {...props} />
                        ),
                    // A ```mermaid fence becomes a diagram; every other fence
                    // stays on the syntax-highlighted path. Intercepting at
                    // <pre> rather than <code> keeps the diagram out of the
                    // code-well styling and avoids nesting an <svg> in a <pre>.
                    pre: ({ node, children, ...props }) => {
                        const source = mermaidSource(node)
                        return source === null ? (
                            <pre {...props}>{children}</pre>
                        ) : (
                            <MermaidBlock source={source} />
                        )
                    },
                }}
            >
                {content}
            </ReactMarkdown>
        </div>
    )
}
