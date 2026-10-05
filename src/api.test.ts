import { afterEach, beforeEach, describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import {
    getPullRequestDetail,
    getPullRequestFile,
    getReviewProgress,
    onPullRequestProviderChanged,
    onReviewProgressChanged,
    openPullRequestLink,
    openPullRequestWindow,
    setFileViewed,
    setPullRequestWindowSize,
} from "./api"
import { shortHash } from "./routing/slug"
import type { PullRequestReference } from "./types"

// The tree's collapse-state surface is asserted against the source text: what
// matters is that no wrapper EXISTS to be called, not what one would do if it
// did.
const API_SOURCE = readFileSync(
    fileURLToPath(new URL("./api.ts", import.meta.url)),
    "utf8",
)

describe("the tree's collapse-state wrappers are gone", () => {
    // Top-level disclosure is session-only (design D6), so a reveal has no
    // settings API left to call — which is what makes `view-routing`'s
    // *Navigation Reveal Is Transient* ("no settings write is performed as a
    // result of the reveal") true by construction rather than by review.
    //
    // The Rust handlers, the Tauri dispatch arms and the settings fields stay
    // in place on purpose, so an existing settings file still parses; this
    // pins only that the FRONTEND can no longer reach them.
    test.each([
        "getCollapsedTreeNodeIds",
        "setCollapsedTreeNodeIds",
        "getExpandedTreeNodeIds",
        "setExpandedTreeNodeIds",
    ])("%s is not exported from src/api.ts", (name) => {
        expect(API_SOURCE).not.toContain(`export async function ${name}`)
        expect(API_SOURCE).not.toContain(`export function ${name}`)
    })

    test.each([
        "get_collapsed_tree_node_ids",
        "set_collapsed_tree_node_ids",
        "get_expanded_tree_node_ids",
        "set_expanded_tree_node_ids",
    ])("the %s command is never invoked", (command) => {
        expect(API_SOURCE).not.toContain(`invokeLogged<string[]>("${command}")`)
        expect(API_SOURCE).not.toContain(`invokeLogged<void>("${command}"`)
    })
})

// The pull-request window, its links and its notices, on each host. There is
// no codegen, so these pin the command names and argument shapes the desktop
// handlers deserialise, and that the browser skin never sends the desktop-only
// commands at all (`pull-request-viewer`: *Pull-Request Window*, *Desktop Link
// Opener*, *Pull-Request Window Geometry*, *Provider Enabled Flags Stay
// Current*, *Review Progress*).
describe("the pull-request window, its links and its notices", () => {
    const g = globalThis as unknown as Record<string, unknown>
    let saved: { hadWindow: boolean; window: unknown; fetch: unknown }

    beforeEach(() => {
        saved = { hadWindow: "window" in g, window: g.window, fetch: g.fetch }
    })

    afterEach(() => {
        if (saved.hadWindow) g.window = saved.window
        else delete g.window
        g.fetch = saved.fetch
    })

    /// The desktop host: Tauri's internals, recording each command sent and
    /// each event callback registered.
    function desktop() {
        const sent: [string, unknown][] = []
        const callbacks: ((event: { payload: unknown }) => void)[] = []
        g.window = {
            __TAURI_INTERNALS__: {
                invoke: (command: string, args: unknown) => {
                    sent.push([command, args])
                    return Promise.resolve(callbacks.length)
                },
                transformCallback: (callback: (event: { payload: unknown }) => void) => {
                    callbacks.push(callback)
                    return callbacks.length
                },
            },
        }
        return { sent, callbacks }
    }

    /// The browser skin: a `window.open` that records the tab it opens and
    /// whether that tab was focused, and a transport that records anything
    /// sent.
    function browser() {
        const opened: string[] = []
        const focused: string[] = []
        const sent: string[] = []
        g.window = {
            open: (url: string, name: string) => {
                opened.push(`${url} ${name}`)
                return { focus: () => focused.push(name) }
            },
        }
        g.fetch = (_input: unknown, init?: { body?: string }) => {
            sent.push(String(init?.body))
            return Promise.reject(new Error("nothing may be sent"))
        }
        return { opened, focused, sent }
    }

    const PATH = "/pr/github/acme/api/42"
    const TITLE = "#42 Add rate limits — acme/api"
    const REFERENCE: PullRequestReference = {
        provider: "github",
        owner: "acme",
        repo: "api",
        number: 42,
    }

    test("the desktop opens the window through its command, inside the click", () => {
        const host = desktop()
        openPullRequestWindow(PATH, TITLE)
        expect(host.sent).toEqual([
            ["open_pull_request_window", { addressPath: PATH, title: TITLE }],
        ])
    })

    test("the browser skin opens the pull request's own tab, named from its path, and focuses it", () => {
        const host = browser()
        openPullRequestWindow(PATH, TITLE)
        const name = `specforge-pull-request:${shortHash(PATH)}`
        expect(host.opened).toEqual([`${PATH}?pullRequest=1 ${name}`])
        expect(host.focused).toEqual([name])
        expect(host.sent).toEqual([])
    })

    test("a link opens on the desktop through the reference and the href alone", async () => {
        const host = desktop()
        await openPullRequestLink(REFERENCE, "https://docs.example/guide")
        expect(host.sent).toEqual([
            [
                "open_pull_request_link",
                { reference: REFERENCE, href: "https://docs.example/guide" },
            ],
        ])
    })

    test("the browser skin never sends the link opener, and nothing opens", async () => {
        const host = browser()
        await expect(
            openPullRequestLink(REFERENCE, "https://docs.example/guide"),
        ).rejects.toThrow()
        expect(host.sent).toEqual([])
    })

    test("the window's size is saved on the desktop only", async () => {
        const host = desktop()
        await setPullRequestWindowSize(1280, 860)
        expect(host.sent).toEqual([
            ["set_pull_request_window_size", { width: 1280, height: 860 }],
        ])

        const skin = browser()
        await setPullRequestWindowSize(1280, 860)
        expect(skin.sent).toEqual([])
    })

    test("each notice is heard under its own name, with its payload", async () => {
        const host = desktop()
        const heard: unknown[] = []
        await onPullRequestProviderChanged((payload) => heard.push(payload))
        await onReviewProgressChanged((reference) => heard.push(reference))
        expect(
            host.sent.map(([command, args]) => [command, (args as { event: string }).event]),
        ).toEqual([
            ["plugin:event|listen", "pull-request-provider-changed"],
            ["plugin:event|listen", "review-progress-changed"],
        ])
        host.callbacks[0]!({ payload: { provider: "github", enabled: false } })
        host.callbacks[1]!({ payload: REFERENCE })
        expect(heard).toEqual([{ provider: "github", enabled: false }, REFERENCE])
    })
})

// The four pull-request commands both transports serve. There is no codegen,
// so these pin each command's name and the camelCase arguments its desktop
// handler and its web dispatch arm deserialise (`pull-request-viewer`: *Detail
// Reads Are Scoped to the Snapshot*, *Review Progress*).
describe("the pull-request commands both transports serve", () => {
    const g = globalThis as unknown as Record<string, unknown>
    let saved: { hadWindow: boolean; window: unknown; fetch: unknown }

    beforeEach(() => {
        saved = { hadWindow: "window" in g, window: g.window, fetch: g.fetch }
    })

    afterEach(() => {
        if (saved.hadWindow) g.window = saved.window
        else delete g.window
        g.fetch = saved.fetch
    })

    const REFERENCE: PullRequestReference = {
        provider: "bitbucket",
        owner: "acme",
        repo: "api",
        number: 7,
    }

    /// Each command once, as the view sends it.
    async function sendEach(): Promise<void> {
        await getPullRequestDetail(REFERENCE, true, false)
        await getPullRequestFile(REFERENCE, "src/api.ts", "head1", "base1")
        await getReviewProgress(REFERENCE)
        await setFileViewed(REFERENCE, "src/api.ts", true, "head1", "base1")
    }

    const SENT: [string, unknown][] = [
        ["get_pull_request_detail", { reference: REFERENCE, manual: true, cachedOnly: false }],
        [
            "get_pull_request_file",
            { reference: REFERENCE, path: "src/api.ts", head: "head1", base: "base1" },
        ],
        ["get_review_progress", { reference: REFERENCE }],
        [
            "set_file_viewed",
            { reference: REFERENCE, path: "src/api.ts", viewed: true, head: "head1", base: "base1" },
        ],
    ]

    test("the desktop invokes each command with its camelCase arguments", async () => {
        const sent: [string, unknown][] = []
        g.window = {
            __TAURI_INTERNALS__: {
                invoke: (command: string, args: unknown) => {
                    sent.push([command, args])
                    return Promise.resolve(null)
                },
            },
        }
        await sendEach()
        expect(sent).toEqual(SENT)
    })

    test("the browser skin posts the same commands and arguments to the web transport", async () => {
        const sent: unknown[] = []
        g.window = {}
        g.fetch = (_input: unknown, init?: { body?: string }) => {
            sent.push(JSON.parse(String(init?.body)))
            return Promise.resolve({ ok: true, json: () => Promise.resolve(null) })
        }
        await sendEach()
        expect(sent).toEqual(SENT.map(([command, args]) => ({ command, args })))
    })
})
