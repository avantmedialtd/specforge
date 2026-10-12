import { afterAll, beforeEach, describe, expect, test } from "bun:test"
import { renderToStaticMarkup } from "react-dom/server"
import type { PullRequestReference } from "../types"
import { loadedMath, loadMath, mathReady, resetMathEngineForTest } from "../mathEngine"
import { pullRequestComponents } from "../pullRequestMarkdown"
import { MarkdownView, WORKSPACE_COMPONENTS, markdownComponents } from "./MarkdownView"

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

// `spec-browser`: *Rendered Blocks Survive Document Re-renders*. A server
// render cannot watch an element survive a second render, so what is pinned is
// the cause: the element types react-markdown is handed never change while
// the mode does not.
describe("stable overrides", () => {
    test("the workspace overrides are one frozen module constant", () => {
        expect(Object.isFrozen(WORKSPACE_COMPONENTS)).toBe(true)
        for (const tag of ["h1", "h2", "h3", "h4", "h5", "h6", "li", "input", "table", "pre", "a"]) {
            expect(typeof WORKSPACE_COMPONENTS[tag as keyof typeof WORKSPACE_COMPONENTS]).toBe(
                "function",
            )
        }
    })

    test("workspace markdown is handed the constant itself", () => {
        expect(markdownComponents(null)).toBe(WORKSPACE_COMPONENTS)
    })

    test("one set of pull-request overrides is merged once, and wins", () => {
        const overrides = pullRequestComponents({ web: true, open: () => {} })
        const merged = markdownComponents(overrides)
        expect(markdownComponents(overrides)).toBe(merged)
        expect(Object.isFrozen(merged)).toBe(true)
        expect(merged.a).toBe(overrides.a)
        expect(merged.img).toBe(overrides.img)
        expect(merged.pre).toBe(overrides.pre)
        // Its headings, list items, task boxes and tables are the workspace's,
        // so they stop remounting too.
        expect(merged.h1).toBe(WORKSPACE_COMPONENTS.h1)
        expect(merged.li).toBe(WORKSPACE_COMPONENTS.li)
        expect(merged.input).toBe(WORKSPACE_COMPONENTS.input)
        expect(merged.table).toBe(WORKSPACE_COMPONENTS.table)
        // Another pull request's overrides are another merge.
        expect(markdownComponents(pullRequestComponents({ web: true, open: () => {} }))).not.toBe(
            merged,
        )
    })

    test("workspace links still classify against the document's headings", () => {
        const html = workspace(
            [
                "# Intro",
                "[up](#intro) [later](#later) [gone](#missing) [site](https://example.com) [file](diagram.png)",
                "## Later",
                "- [x] done",
                "| a |\n| - |\n| 1 |",
            ].join("\n\n"),
        )
        expect(html).toContain('<h1 id="intro" data-line="1">Intro</h1>')
        expect(html).toContain('<h2 id="later" data-line="5">Later</h2>')
        // A fragment naming a heading after it resolves too.
        expect(html).toContain('<a href="#intro" class="markdown-link markdown-link--fragment">up</a>')
        expect(html).toContain('<a href="#later" class="markdown-link markdown-link--fragment">later</a>')
        expect(html).toContain('<a href="#missing" class="markdown-link markdown-link--inert">gone</a>')
        // The browser skin: an external link is an isolated tab, a file opens nothing.
        expect(html).toContain(
            '<a href="https://example.com" target="_blank" rel="noopener noreferrer" class="markdown-link markdown-link--external">site</a>',
        )
        expect(html).toContain(
            '<span class="markdown-link markdown-link--file markdown-link--unavailable" title="diagram.png">file</span>',
        )
        expect(html).toContain('class="task-list-item task-list-item--done"')
        expect(html).toContain('role="checkbox" aria-checked="true"')
        expect(html).toContain('<div class="table-scroll"><table>')
    })

    test("pull-request headings still carry no identifier, and no image is made", () => {
        const html = pullRequest("# Root\n\n![pixel](https://tracker.example/p.gif)\n\n- [ ] item")
        expect(html).not.toContain(" id=")
        expect(html).not.toContain("<img")
        expect(html).toContain('role="checkbox" aria-checked="false"')
    })
})

// `spec-browser`: *Surfaces and Rendering Engines Load on Demand*. The loaded
// flag is module state that every test file in the process shares, so each
// case starts from a reset engine.
describe("maths on demand", () => {
    beforeEach(() => resetMathEngineForTest())
    afterAll(() => resetMathEngineForTest())

    test("before the engine loads, a maths document never shows its source", () => {
        expect(loadedMath()).toBeNull()
        const html = workspace("$$x$$")
        expect(html).not.toContain("$$x$$")
        expect(html).not.toContain("katex")
        expect(html).toBe('<div class="markdown-view"></div>')
    })

    test("a document without maths does not wait for the engine", () => {
        expect(loadedMath()).toBeNull()
        expect(workspace("Costs $50 and $60.")).toContain("<p>Costs $50 and $60.</p>")
    })

    test("once the engine has loaded, the same document renders typeset", async () => {
        await loadMath()
        expect(loadedMath()).not.toBeNull()
        const html = workspace("$$x$$")
        expect(html).not.toContain("$$x$$")
        expect(html).toContain('class="katex')
    })

    test("a failed load ends the wait, and the next load tries again", async () => {
        let attempts = 0
        resetMathEngineForTest(() => {
            attempts += 1
            return Promise.reject(new Error("chunk unavailable"))
        })
        expect(mathReady("$$x$$", false)).toBe(false)
        await expect(loadMath()).rejects.toThrow("chunk unavailable")
        expect(loadedMath()).toBeNull()
        // The view that saw the failure renders without the engine.
        expect(mathReady("$$x$$", true)).toBe(true)
        await expect(loadMath()).rejects.toThrow("chunk unavailable")
        expect(attempts).toBe(2)
    })
})
