import { expect, test } from "bun:test"
import { act, StrictMode } from "react"
import { FakeElement, installFakeDom } from "../testing/fakeDom"
import { FakeEventSource, installFakeTransport, settle } from "../testing/fakeTransport"
import { sha256Hex } from "../sha256"
import type { PullRequestContentImageOutcome, PullRequestReference } from "../types"
import { MarkdownView } from "./MarkdownView"
import { PullRequestLoadingProvider, PullRequestLoadingScope } from "./PullRequestLoadingContext"

class Observer {
    static all: Observer[] = []
    nodes = new Set<Element>()
    constructor(readonly callback: (entries: { target: Element; isIntersecting: boolean }[]) => void) {
        Observer.all.push(this)
    }
    observe(node: Element) { this.nodes.add(node) }
    unobserve(node: Element) { this.nodes.delete(node) }
    disconnect() { this.nodes.clear() }
    static near() {
        for (const observer of Observer.all) {
            observer.callback([...observer.nodes].map(target => ({ target, isIntersecting: true })))
        }
    }
}

const IMAGE: PullRequestContentImageOutcome = {
    kind: "image", mime: "image/png", width: 3, height: 2, data: "aW1hZ2U=", finalHost: "cdn.example",
}
const PR: PullRequestReference = { provider: "github", owner: "acme", repo: "api", number: 42 }

async function fixture(body: string, options: {
    outcome?: PullRequestContentImageOutcome
    visible?: boolean
    published?: boolean
    enabled?: boolean
} = {}) {
    const dom = installFakeDom()
    const g = globalThis as unknown as Record<string, unknown>
    const originalObserver = g.IntersectionObserver
    const bounds = Object.getOwnPropertyDescriptor(FakeElement.prototype, "getBoundingClientRect")!
    const originalCreate = Object.getOwnPropertyDescriptor(URL, "createObjectURL")!
    const originalRevoke = Object.getOwnPropertyDescriptor(URL, "revokeObjectURL")!
    const urls: string[] = [], revoked: string[] = []
    URL.createObjectURL = () => { const url = `blob:test-${urls.length}`; urls.push(url); return url }
    URL.revokeObjectURL = url => { revoked.push(url) }
    FakeElement.prototype.getBoundingClientRect = () => ({ x: 0, y: 10, top: 10, left: 0, right: 100, bottom: 40, width: 100, height: 30 })
    Observer.all = []
    g.IntersectionObserver = Observer
    ;(g.window as Record<string, unknown>).innerHeight = 800
    const requests: Record<string, unknown>[] = []
    const transport = installFakeTransport((command, args) => {
        if (command === "get_pr_content_loading") { return { autoShowImages: options.enabled ?? true, autoLoadLargeDiffs: false } }
        if (command === "get_pull_request_content_image") { requests.push(args); return options.outcome ?? IMAGE }
        return null
    })
    const { createRoot } = await import("react-dom/client")
    const container = dom.document.createElement("div")
    dom.document.body.appendChild(container)
    const root = createRoot(container as unknown as Element)
    await act(async () => root.render(<StrictMode><PullRequestLoadingProvider>
        <PullRequestLoadingScope visible={options.visible ?? true}>
            <MarkdownView content={body} pullRequest={PR} pullRequestPublished={options.published ?? true} />
        </PullRequestLoadingScope>
    </PullRequestLoadingProvider></StrictMode>))
    await settle(1)
    return {
        container, requests, urls, revoked,
        near: async () => { await act(async () => Observer.near()); await settle(1) },
        async close() {
            await act(async () => root.unmount())
            transport.uninstall()
            dom.uninstall()
            g.IntersectionObserver = originalObserver
            Object.defineProperty(FakeElement.prototype, "getBoundingClientRect", bounds)
            Object.defineProperty(URL, "createObjectURL", originalCreate)
            Object.defineProperty(URL, "revokeObjectURL", originalRevoke)
        },
    }
}

test("published duplicates share one authorised read, checked object URLs, labels and cleanup", async () => {
    const body = "![screen](https://images.example/a.png?sig=secret) ![again](https://images.example/a.png?sig=secret#part)"
    const view = await fixture(body)
    try {
        expect(view.requests).toHaveLength(0)
        await view.near()
        expect(view.requests).toEqual([{
            reference: PR, bodyDigest: sha256Hex(body), source: "https://images.example/a.png?sig=secret",
        }])
        expect(view.container.findAll("img")).toHaveLength(2)
        for (const image of view.container.findAll("img")) {
            expect(image.getAttribute("src")).toStartWith("blob:")
            expect(image.getAttribute("srcset")).toBeNull()
        }
        expect(view.container.textContent).toContain("images.example")
        expect(view.container.textContent).toContain("cdn.example")
        await act(async () => FakeEventSource.current!.emit("pr-content-loading-changed", {
            autoShowImages: false, autoLoadLargeDiffs: false,
        }))
        await view.near()
        expect(view.requests).toHaveLength(1)
        expect(view.container.findAll("img")).toHaveLength(2)
    } finally { await view.close() }
    expect(view.revoked).toEqual(view.urls)
})

test("linked-image retry is outside its single destination, with no nested control", async () => {
    const view = await fixture("[![screen](https://images.example/a)](https://example.com/details)", {
        outcome: { kind: "failed", reason: "deferred", untilUnix: 123 },
    })
    try {
        await view.near()
        const anchors = view.container.findAll("a")
        expect(anchors).toHaveLength(1)
        expect(anchors[0].getAttribute("href")).toBe("https://example.com/details")
        expect(anchors[0].findAll("button")).toHaveLength(0)
        expect(view.container.findAll("button").map(button => button.textContent)).toEqual(["Try again"])
        await view.near()
        expect(view.requests).toHaveLength(1)
    } finally { await view.close() }
})

test("off, hidden and local-draft previews never fetch, and unsafe/raw sources stay fallback", async () => {
    for (const options of [{ enabled: false }, { visible: false }, { published: false }]) {
        const view = await fixture("![screen](https://images.example/a)", options)
        try { await view.near(); expect(view.requests).toHaveLength(0) } finally { await view.close() }
    }
    const view = await fixture('![local](./a.png) ![ip](https://127.0.0.1/a)\n\n<img src="https://images.example/html">\n\n`![code](https://images.example/code)`')
    try {
        await view.near()
        expect(view.requests).toHaveLength(0)
        expect(view.container.findAll("img")).toHaveLength(0)
    } finally { await view.close() }
})

test("blocked image outcomes offer no retry or remote decoder source", async () => {
    const view = await fixture("![screen](https://images.example/a)", {
        outcome: { kind: "failed", reason: "unsafeAddress", untilUnix: null },
    })
    try {
        await view.near()
        expect(view.container.textContent).toContain("not permitted")
        expect(view.container.findAll("button")).toHaveLength(0)
        expect(view.container.findAll("img")).toHaveLength(0)
        expect(view.container.findAll("a")).toHaveLength(1)
    } finally { await view.close() }
})
