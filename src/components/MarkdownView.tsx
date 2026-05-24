import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import rehypeHighlight from "rehype-highlight"
import type { RefObject } from "react"

interface MarkdownViewProps {
    content: string
    containerRef?: RefObject<HTMLDivElement>
}

export function MarkdownView({ content, containerRef }: MarkdownViewProps) {
    return (
        <div ref={containerRef} className="markdown-view">
            <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                rehypePlugins={[rehypeHighlight]}
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
                }}
            >
                {content}
            </ReactMarkdown>
        </div>
    )
}
