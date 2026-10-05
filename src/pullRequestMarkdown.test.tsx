import { describe, expect, test } from "bun:test"
import type { ReactElement } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import ReactMarkdown from "react-markdown"
import rehypeHighlight from "rehype-highlight"
import rehypeKatex from "rehype-katex"
import remarkGfm from "remark-gfm"
import remarkMath from "remark-math"
import type { Element } from "hast"
import {
    fenceSource,
    isHtmlComment,
    KATEX_OPTIONS,
    pullRequestComponents,
    pullRequestHref,
    REMARK_MATH_OPTIONS,
    remarkDropHtmlComments,
    sameMarkdownViewProps,
    type FenceRenderer,
    type PullRequestLinkHost,
} from "./pullRequestMarkdown"

// The pull-request mode, rendered as the shared renderer runs it: its remark
// and rehype plugins with the mode's plugin and overrides added. The
// highlighter runs without `MarkdownView`'s exemption for `mermaid`, so a
// mermaid fence is shown as plain source even when the highlighter has had
// it. Server rendering needs no DOM, and the markup is what a page would get.

const DESKTOP: PullRequestLinkHost = { web: false, open: () => {} }
const BROWSER: PullRequestLinkHost = { web: true, open: () => {} }

function render(markdown: string, host = DESKTOP, Fence?: FenceRenderer): string {
    return renderToStaticMarkup(
        <ReactMarkdown
            remarkPlugins={[remarkGfm, [remarkMath, REMARK_MATH_OPTIONS], remarkDropHtmlComments]}
            rehypePlugins={[rehypeHighlight, [rehypeKatex, KATEX_OPTIONS]]}
            components={pullRequestComponents(host, Fence)}
        >
            {markdown}
        </ReactMarkdown>,
    )
}

function count(haystack: string, needle: string): number {
    return haystack.split(needle).length - 1
}

/// An anchor element, and not KaTeX's MathML `<annotation>`.
const ANCHOR = /<a[\s>]/

describe("raw HTML", () => {
    test("raw HTML stays unrendered", () => {
        const html = render('<img src="https://tracker.example/p.gif">')
        expect(html).not.toContain("<img")
        // Shown as its source text, escaped, never as an element.
        expect(html).toContain("&lt;img src=")
    })

    test("raw HTML within a paragraph stays text too", () => {
        const html = render(
            'Before <img src="https://tracker.example/p.gif"> <a href="https://x.example">x</a> after',
        )
        expect(html).not.toContain("<img")
        expect(html).not.toMatch(ANCHOR)
    })
})

describe("template comments", () => {
    test("template comments disappear and code keeps them", () => {
        const html = render(
            "<!-- Describe your change -->\n\nText.\n\n```\n<!-- Describe your change -->\n```",
        )
        expect(count(html, "&lt;!-- Describe your change --&gt;")).toBe(1)
        expect(html).toBe("<p>Text.</p>\n<pre><code>&lt;!-- Describe your change --&gt;\n</code></pre>")
    })

    test("a comment disappears inside a paragraph, a list and a quote", () => {
        const html = render("text <!-- c --> more\n\n- item <!-- d -->\n- <!-- e -->\n\n> <!-- f -->")
        expect(html).not.toContain("&lt;!--")
        expect(html).toContain("text  more")
        expect(html).toContain("item")
    })

    test("inline code keeps a comment", () => {
        expect(render("`<!-- kept -->`")).toContain("<code>&lt;!-- kept --&gt;</code>")
    })

    test("a comment with anything beside it stays visible as text", () => {
        expect(render("<!-- a --> trailing")).toContain("&lt;!-- a --&gt; trailing")
    })

    test("removing a comment creates nothing", () => {
        const html = render("[docs]<!-- -->(guide.md)")
        expect(html).toBe("<p>[docs](guide.md)</p>")
        expect(html).not.toMatch(ANCHOR)
    })
})

describe("isHtmlComment", () => {
    test("one comment, alone or with the whitespace a block keeps", () => {
        for (const value of [
            "<!-- Describe your change -->",
            "   <!-- indented -->",
            "<!-- trailing -->   ",
            "<!-- multi\nline -->",
            "<!-- a -- b -->",
            "<!---->",
            "<!-->",
            "<!--->",
        ]) {
            expect(isHtmlComment(value)).toBe(true)
        }
    })

    test("anything else is not a comment", () => {
        for (const value of [
            "<!-- a --> <!-- b -->",
            "<!-- a --> text",
            "text <!-- a -->",
            "<!-- a -->x<!-- b -->",
            "<!-- unterminated",
            '<img src="x">',
            "<!-- a --!>",
            "",
        ]) {
            expect(isHtmlComment(value)).toBe(false)
        }
    })
})

describe("mermaid fences", () => {
    const fence = '```mermaid\nflowchart TB\n  A@{ img: "https://tracker.example/p.png" }\n```'

    test("a mermaid fence shows its source", () => {
        expect(render(fence)).toBe(
            '<pre><code class="language-mermaid">flowchart TB\n  A@{ img: &quot;https://tracker.example/p.png&quot; }</code></pre>',
        )
    })

    test("every other fence is the host's, so an svg fence keeps its own rendering", () => {
        const HostFence: FenceRenderer = ({ node: _node, children }) => (
            <pre data-fence="host">{children}</pre>
        )
        const html = render(`${fence}\n\n\`\`\`svg\n<svg/>\n\`\`\``, DESKTOP, HostFence)
        expect(count(html, 'data-fence="host"')).toBe(1)
        expect(html).toContain('<pre><code class="language-mermaid">')
    })
})

describe("images", () => {
    test("a standalone image becomes a labelled link", () => {
        const html = render("![screenshot](https://img.example/s.png)")
        expect(html).toBe(
            '<p><a href="https://img.example/s.png" class="markdown-link markdown-link--external">' +
                '<span class="markdown-image">screenshot <span class="markdown-image-host">(img.example)</span></span>' +
                "</a></p>",
        )
    })

    test("an image inside a link becomes part of that link", () => {
        const html = render("[![build](https://badge.example/b.svg)](https://ci.example/run/7)")
        expect(count(html, "<a ")).toBe(1)
        expect(html).toContain('<a href="https://ci.example/run/7"')
        expect(html).not.toContain('href="https://badge.example')
        expect(html).toContain(
            '<span class="markdown-image">build <span class="markdown-image-host">(badge.example)</span></span>',
        )
    })

    test("activating it opens only the link's destination", () => {
        const opened: string[] = []
        const { a } = pullRequestComponents({ web: false, open: (href) => opened.push(href) })
        const link = a({ href: "https://ci.example/run/7", children: "build" }) as ReactElement<{
            onClick: (event: { preventDefault(): void }) => void
        }>
        let prevented = false
        link.props.onClick({ preventDefault: () => (prevented = true) })
        expect(opened).toEqual(["https://ci.example/run/7"])
        expect(prevented).toBe(true)
    })

    test("an image with no alt text is labelled as one", () => {
        expect(render("![](https://img.example/s.png)")).toContain(
            '<span class="markdown-image">image <span class="markdown-image-host">(img.example)</span></span>',
        )
    })

    test("an image whose source opens nothing becomes no link either", () => {
        const relative = render("![diagram](docs/flow.png)")
        expect(relative).not.toMatch(ANCHOR)
        expect(relative).toContain('title="docs/flow.png"')
        expect(relative).toContain('<span class="markdown-image">diagram</span>')

        const inline = render("![pixel](data:image/gif;base64,R0lGODlhAQABAAAAACw=)")
        expect(inline).not.toMatch(ANCHOR)
        expect(inline).not.toContain("data:")
    })
})

describe("maths", () => {
    test("maths cannot carry a live link", () => {
        for (const markdown of [
            "$$\n\\href{https://evil.example}{click}\n$$",
            "$$\\href{https://evil.example}{click}$$",
            "```math\n\\href{https://evil.example}{click}\n```",
        ]) {
            const html = render(markdown)
            expect(html).toContain('class="katex')
            expect(html).not.toMatch(ANCHOR)
        }
    })
})

describe("links", () => {
    test("on the desktop a link goes to the opener and never to the page", () => {
        const html = render("[guide](https://docs.example/guide)")
        expect(html).toBe(
            '<p><a href="https://docs.example/guide" class="markdown-link markdown-link--external">guide</a></p>',
        )
    })

    test("on the desktop every openable link is routed through the opener", () => {
        const opened: string[] = []
        const { a } = pullRequestComponents({ web: false, open: (href) => opened.push(href) })
        for (const href of ["https://docs.example/guide", "http://plain.example/"]) {
            const link = a({ href, children: "x" }) as ReactElement<{
                onClick?: (event: { preventDefault(): void }) => void
            }>
            link.props.onClick?.({ preventDefault: () => {} })
        }
        expect(opened).toEqual(["https://docs.example/guide", "http://plain.example/"])
    })

    test("the browser skin opens links in an isolated tab", () => {
        expect(render("[guide](https://docs.example/guide)", BROWSER)).toBe(
            '<p><a href="https://docs.example/guide" target="_blank" rel="noopener noreferrer" class="markdown-link markdown-link--external">guide</a></p>',
        )
    })

    test("an autolink follows the same rule", () => {
        expect(render("<https://auto.example/x> and https://literal.example/y", BROWSER)).toBe(
            '<p><a href="https://auto.example/x" target="_blank" rel="noopener noreferrer" class="markdown-link markdown-link--external">https://auto.example/x</a>' +
                ' and <a href="https://literal.example/y" target="_blank" rel="noopener noreferrer" class="markdown-link markdown-link--external">https://literal.example/y</a></p>',
        )
    })

    test("a mailto link and a relative link open nothing", () => {
        for (const host of [DESKTOP, BROWSER]) {
            const html = render("[mail](mailto:a@b.example) and [setup](docs/setup.md)", host)
            expect(html).not.toMatch(ANCHOR)
            expect(html).not.toContain("mailto:")
            // The relative link shows its target.
            expect(html).toContain(
                '<span class="markdown-link markdown-link--unavailable" title="docs/setup.md">setup</span>',
            )
        }
    })

    test("pull-request content follows its own link rules", () => {
        // In the desktop application: the https link through the pull-request
        // opener, the mailto link nowhere.
        const opened: string[] = []
        const { a } = pullRequestComponents({ web: false, open: (href) => opened.push(href) })
        for (const href of ["https://docs.example/guide", "mailto:a@b.example"]) {
            const link = a({ href, children: "x" }) as ReactElement<{
                onClick?: (event: { preventDefault(): void }) => void
            }>
            link.props.onClick?.({ preventDefault: () => {} })
        }
        expect(opened).toEqual(["https://docs.example/guide"])
    })

    test("a script, file or custom-scheme link opens nothing", () => {
        const html = render(
            "[js](javascript:alert(1)) [file](file:///etc/passwd) [app](vscode://open?x) [data](data:text/html,x)",
        )
        expect(html).not.toMatch(ANCHOR)
        for (const scheme of ["javascript:", "file:", "vscode:", "data:"]) {
            expect(html).not.toContain(scheme)
        }
    })
})

describe("fenceSource", () => {
    /// A highlighted fence as rehype-highlight leaves it: its text split into
    /// token spans.
    const highlighted: Element = {
        type: "element",
        tagName: "pre",
        properties: {},
        children: [
            {
                type: "element",
                tagName: "code",
                properties: { className: ["hljs", "language-svg"] },
                children: [
                    {
                        type: "element",
                        tagName: "span",
                        properties: { className: ["hljs-tag"] },
                        children: [{ type: "text", value: "<svg/>" }],
                    },
                    { type: "text", value: "\n" },
                ],
            },
        ],
    }

    test("reads a fence's source back whole through its token spans", () => {
        expect(fenceSource(highlighted, "svg")).toBe("<svg/>")
    })

    test("a fence of another language, or no fence, has no source", () => {
        expect(fenceSource(highlighted, "mermaid")).toBeNull()
        expect(fenceSource(undefined, "svg")).toBeNull()
    })
})

describe("sameMarkdownViewProps", () => {
    const reference = { provider: "github" as const, owner: "acme", repo: "api", number: 42 }
    const onHeadings = () => {}

    test("unchanged props skip the render, as React's own comparison would", () => {
        const props = { content: "x", root: "/ws", basePath: "a.md", onHeadings }
        expect(sameMarkdownViewProps(props, { ...props })).toBe(true)
    })

    test("any other prop changed by identity renders", () => {
        const props = { content: "x", root: "/ws", basePath: "a.md", onHeadings }
        expect(sameMarkdownViewProps(props, { ...props, content: "y" })).toBe(false)
        expect(sameMarkdownViewProps(props, { ...props, basePath: "b.md" })).toBe(false)
        expect(sameMarkdownViewProps(props, { ...props, onHeadings: () => {} })).toBe(false)
        expect(sameMarkdownViewProps(props, { content: "x", root: "/ws", basePath: "a.md" })).toBe(
            false,
        )
    })

    test("an equal reference rebuilt by the caller skips the render", () => {
        expect(
            sameMarkdownViewProps(
                { content: "x", pullRequest: reference },
                { content: "x", pullRequest: { ...reference } },
            ),
        ).toBe(true)
    })

    test("another pull request, or the same one respelt, renders", () => {
        const props = { content: "x", pullRequest: reference }
        for (const other of [
            { ...reference, number: 43 },
            { ...reference, provider: "bitbucket" as const },
            { ...reference, owner: "Acme" },
            { ...reference, repo: "web" },
        ]) {
            expect(sameMarkdownViewProps(props, { ...props, pullRequest: other })).toBe(false)
        }
    })

    test("switching between workspace markdown and a pull request's renders", () => {
        expect(
            sameMarkdownViewProps(
                { content: "x", pullRequest: reference },
                { content: "x", root: "/ws", basePath: "a.md" },
            ),
        ).toBe(false)
        expect(sameMarkdownViewProps({ content: "x" }, { content: "x", pullRequest: reference })).toBe(
            false,
        )
    })
})

describe("pullRequestHref", () => {
    test("an absolute http or https URL with a host opens", () => {
        expect(pullRequestHref("https://ci.example/run/7")).toEqual({
            kind: "open",
            url: "https://ci.example/run/7",
            host: "ci.example",
        })
        expect(pullRequestHref("HTTP://Example.COM:8080/a")).toEqual({
            kind: "open",
            url: "http://example.com:8080/a",
            host: "example.com:8080",
        })
    })

    test("a reference without a scheme is relative", () => {
        for (const href of ["docs/setup.md", "./a.png", "#section", "?q=1", "//evil.example/x"]) {
            expect(pullRequestHref(href)).toEqual({ kind: "relative", target: href })
        }
    })

    test("every other scheme, an unparseable URL and an empty href are inert", () => {
        for (const href of [
            "mailto:a@b.example",
            "tel:+15550100",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "data:text/html,x",
            "vscode://open",
            "https://",
            "http://exa mple.example/",
            "",
        ]) {
            expect(pullRequestHref(href)).toEqual({ kind: "inert" })
        }
    })
})

/// The samples the spec gives (`pull-request-viewer`: *Pull-Request Content Is
/// Untrusted*, *Desktop Link Opener*), with the remote image variants above.
const SAMPLES = [
    '<img src="https://tracker.example/p.gif">',
    "<!-- Describe your change -->\n\n```\n<!-- Describe your change -->\n```",
    "[docs]<!-- -->(guide.md)",
    '```mermaid\nflowchart TB\n  A@{ img: "https://tracker.example/p.png" }\n```',
    "![screenshot](https://img.example/s.png)",
    "[![build](https://badge.example/b.svg)](https://ci.example/run/7)",
    "$$\n\\href{https://evil.example}{click}\n$$",
    "[mail](mailto:a@b.example) and [setup](docs/setup.md)",
    "![](https://img.example/s.png) ![diagram](docs/flow.png)",
    "![pixel](data:image/gif;base64,R0lGODlhAQABAAAAACw=)",
]

describe("every sample", () => {
    test("the main window is guarded by the mode alone", () => {
        // A description holding a remote image, a mermaid fence and raw HTML
        // renders nothing that would fetch: no image, no diagram, no element.
        const html = render(
            [
                "![screenshot](https://img.example/s.png)",
                '```mermaid\nflowchart TB\n  A@{ img: "https://tracker.example/p.png" }\n```',
                '<img src="https://tracker.example/p.gif">',
            ].join("\n\n"),
        )
        expect(html).not.toContain("<img")
        expect(html).not.toContain("<svg")
        expect(html).not.toContain('src="')
    })

    // `<img` also matches an SVG `<image>`, which would fetch just the same.
    test("no rendered output contains <img", () => {
        for (const host of [DESKTOP, BROWSER]) {
            for (const sample of SAMPLES) {
                expect(render(sample, host)).not.toContain("<img")
            }
        }
    })
})
