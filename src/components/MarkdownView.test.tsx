import { describe, expect, test } from "bun:test"
import { renderToStaticMarkup } from "react-dom/server"
import type { PullRequestReference } from "../types"
import { MarkdownView } from "./MarkdownView"

// The shared renderer itself, server-rendered: which mode its props choose,
// and that pull-request content really renders in the pull-request mode
// (`pull-request-viewer`: *Pull-Request Content Is Untrusted*). The mode's own
// pieces are tested in `pullRequestMarkdown.test.tsx`; what is pinned here is
// that `MarkdownView` uses them, and only for a pull request. Nothing below
// reaches a diagram or an image block, which need a DOM — a `mermaid` fence in
// pull-request content included, which is the point.

const REFERENCE: PullRequestReference = {
    provider: "github",
    owner: "acme",
    repo: "api",
    number: 42,
}

/// A description holding what the mode exists to stop.
const DESCRIPTION = [
    "# Summary",
    "<!-- Describe your change -->",
    "![screenshot](https://img.example/s.png)",
    '<img src="https://tracker.example/p.gif">',
    "[guide](https://docs.example/guide) and [setup](docs/setup.md)",
].join("\n\n")

function workspace(content: string): string {
    return renderToStaticMarkup(
        <MarkdownView content={content} root="/ws" basePath="openspec/changes/x/proposal.md" />,
    )
}

function pullRequest(content: string): string {
    return renderToStaticMarkup(<MarkdownView content={content} pullRequest={REFERENCE} />)
}

describe("workspace markdown", () => {
    test("renders as it always has: images, heading identifiers, comments as text", () => {
        const html = workspace(DESCRIPTION)
        expect(html).toContain('<img src="https://img.example/s.png" alt="screenshot"/>')
        expect(html).toContain('<h1 id="summary" data-line="1">Summary</h1>')
        expect(html).toContain("&lt;!-- Describe your change --&gt;")
    })
})

describe("pull-request content", () => {
    test("renders in the pull-request mode", () => {
        const html = pullRequest(DESCRIPTION)
        expect(html).not.toContain("<img")
        expect(html).not.toContain("Describe your change")
        expect(html).toContain(
            '<span class="markdown-image">screenshot <span class="markdown-image-host">(img.example)</span></span>',
        )
        expect(html).toContain(
            '<span class="markdown-link markdown-link--unavailable" title="docs/setup.md">setup</span>',
        )
    })

    test("its headings carry no identifier of the author's choosing", () => {
        // `id="root"` would be the application's own root.
        const html = pullRequest("# Root\n\n## Summary")
        expect(html).toContain('<h1 data-line="1">Root</h1>')
        expect(html).not.toContain(" id=")
    })

    test("a mermaid fence shows its source and draws nothing", () => {
        expect(
            pullRequest('```mermaid\nflowchart TB\n  A@{ img: "https://tracker.example/p.png" }\n```'),
        ).toBe(
            '<div class="markdown-view"><pre><code class="language-mermaid">flowchart TB\n' +
                '  A@{ img: &quot;https://tracker.example/p.png&quot; }</code></pre></div>',
        )
    })

    test("the browser skin opens its links as opener-isolated tabs", () => {
        expect(pullRequest("[guide](https://docs.example/guide)")).toContain(
            '<a href="https://docs.example/guide" target="_blank" rel="noopener noreferrer" class="markdown-link markdown-link--external">guide</a>',
        )
    })

    test("on the desktop its links leave the page to the opener", () => {
        const g = globalThis as unknown as Record<string, unknown>
        const hadWindow = "window" in g
        const saved = g.window
        g.window = { __TAURI_INTERNALS__: {} }
        try {
            expect(pullRequest("[guide](https://docs.example/guide)")).toContain(
                '<a href="https://docs.example/guide" class="markdown-link markdown-link--external">guide</a>',
            )
        } finally {
            if (hadWindow) g.window = saved
            else delete g.window
        }
    })
})
