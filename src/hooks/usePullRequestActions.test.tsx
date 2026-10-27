import { afterAll, beforeAll, describe, expect, test } from "bun:test"
import { act } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { installFakeDom, type FakeDocument } from "../testing/fakeDom"
import { settle } from "../testing/fakeTransport"
import type {
    ActionInvalidation,
    PullRequestReference,
    ReviewActionState,
    ReviewCapability,
    ReviewDraft,
} from "../types"
import {
    PullRequestActionsContext,
    usePullRequestActions,
    useReviewActions,
    type PullRequestActions,
} from "./usePullRequestActions"

// `pull-request-actions`: *Review Action State Synchronises Only Native
// Windows*, *Desktop-Only Pull-Request Reviews*. The real hook, rendered
// through `react-dom/client` on the fake DOM against a fake Tauri shell: its
// `invoke` answers `get_review_action_state`, and its `transformCallback`
// hands back the action broadcast's listener so a test can emit an
// invalidation exactly as the native shell forwards one.

const REFERENCE: PullRequestReference = {
    provider: "github",
    owner: "acme",
    repo: "api",
    number: 42,
}

const OTHER: PullRequestReference = { ...REFERENCE, number: 43 }

const CAPABILITIES: ReviewCapability[] = [
    { name: "comment", available: true, reason: null },
    { name: "approve", available: true, reason: null },
    { name: "requestChanges", available: true, reason: null },
    { name: "inlineComment", available: true, reason: null },
    { name: "reply", available: true, reason: null },
]

function draft(revision: number, overrides: Partial<ReviewDraft> = {}): ReviewDraft {
    return {
        id: "draft-1",
        reference: REFERENCE,
        revision,
        basis: { head: "head1", base: "base1" },
        decision: "comment",
        summary: "",
        items: [],
        ...overrides,
    }
}

function actionState(overrides: Partial<ReviewActionState> = {}): ReviewActionState {
    return {
        reference: REFERENCE,
        draft: draft(1),
        capabilities: CAPABILITIES,
        blockedReason: null,
        operation: null,
        staleBasis: false,
        ...overrides,
    }
}

let dom: { document: FakeDocument; uninstall: () => void }
let createRoot: typeof import("react-dom/client").createRoot

beforeAll(async () => {
    dom = installFakeDom()
    ;(globalThis as unknown as Record<string, unknown>).IS_REACT_ACT_ENVIRONMENT = true
    createRoot = (await import("react-dom/client")).createRoot
})

afterAll(() => {
    delete (globalThis as unknown as Record<string, unknown>).IS_REACT_ACT_ENVIRONMENT
    dom.uninstall()
})

/// The native host: Tauri's internals, recording each command sent and each
/// event callback registered, answering `get_review_action_state` from the
/// reply the test sets.
interface Desktop {
    sent: [string, Record<string, unknown>][]
    callbacks: ((event: { payload: unknown }) => void)[]
    reads: () => number
    listens: () => [string, Record<string, unknown>][]
    setReply: (next: () => unknown) => void
    uninstall: () => void
}

function installDesktop(first: () => unknown): Desktop {
    const g = globalThis as unknown as Record<string, unknown>
    const host = g.window as Record<string, unknown>
    const sent: [string, Record<string, unknown>][] = []
    const callbacks: ((event: { payload: unknown }) => void)[] = []
    let reply = first
    host.__TAURI_INTERNALS__ = {
        invoke: (command: string, args: Record<string, unknown>) => {
            sent.push([command, args])
            if (command === "get_review_action_state") {
                try {
                    return Promise.resolve(reply())
                } catch (err) {
                    return Promise.reject(err)
                }
            }
            return Promise.resolve(callbacks.length)
        },
        transformCallback: (callback: (event: { payload: unknown }) => void) => {
            callbacks.push(callback)
            return callbacks.length
        },
    }
    // Tauri's event plugin unregisters through this global on unlisten.
    host.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
    return {
        sent,
        callbacks,
        reads: () => sent.filter(([command]) => command === "get_review_action_state").length,
        listens: () =>
            sent.filter(([command]) => command === "plugin:event|listen") as [
                string,
                Record<string, unknown>,
            ][],
        setReply(next) {
            reply = next
        },
        uninstall() {
            delete host.__TAURI_INTERNALS__
            delete host.__TAURI_EVENT_PLUGIN_INTERNALS__
        },
    }
}

/// The browser skin: no Tauri internals, and a `fetch` that records anything
/// a command would post.
function installBrowser(): { posted: string[]; uninstall: () => void } {
    const g = globalThis as unknown as Record<string, unknown>
    const hadFetch = "fetch" in g
    const savedFetch = g.fetch
    const posted: string[] = []
    g.fetch = (_input: unknown, init?: { body?: string }) => {
        posted.push(String(init?.body))
        return Promise.reject(new Error("nothing may be sent"))
    }
    return {
        posted,
        uninstall() {
            if (hadFetch) g.fetch = savedFetch
            else delete g.fetch
        },
    }
}

function Probe({
    reference,
    seen,
}: {
    reference: PullRequestReference
    seen: PullRequestActions[]
}) {
    const actions = usePullRequestActions(reference)
    seen.push(actions)
    return null
}

async function mountProbe(reference: PullRequestReference = REFERENCE) {
    const seen: PullRequestActions[] = []
    const root = createRoot(dom.document.createElement("div") as unknown as Element)
    const view = (next: PullRequestReference) => (
        <Probe reference={next} seen={seen} />
    )
    await act(async () => root.render(view(reference)))
    return {
        seen,
        last: () => seen[seen.length - 1]!,
        async rerender(next: PullRequestReference) {
            await act(async () => root.render(view(next)))
        },
        async unmount() {
            await act(async () => root.unmount())
        },
    }
}

function emit(desktop: Desktop, payload: unknown): Promise<void> {
    return act(async () => {
        for (const callback of desktop.callbacks) callback({ payload })
    })
}

describe("usePullRequestActions", () => {
    test("a native mount reads the state and subscribes to the action broadcast", async () => {
        const state = actionState()
        const desktop = installDesktop(() => state)
        const mounted = await mountProbe()
        try {
            await settle()
            expect(desktop.reads()).toBe(1)
            expect(desktop.listens()).toHaveLength(1)
            expect(desktop.listens()[0]![1].event).toBe("review-action-changed")
            expect(mounted.last().state).toBe(state)
            expect(mounted.last().error).toBeNull()
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("a matching invalidation re-reads and shows the new revision", async () => {
        const first = actionState()
        const desktop = installDesktop(() => first)
        const mounted = await mountProbe()
        try {
            await settle()
            const second = actionState({ draft: draft(2, { summary: "newer" }) })
            desktop.setReply(() => second)
            const invalidation: ActionInvalidation = {
                kind: "draft",
                reference: REFERENCE,
                revision: 2,
            }
            await emit(desktop, invalidation)
            await settle()
            expect(desktop.reads()).toBe(2)
            expect(mounted.last().state).toEqual(second)
            expect(mounted.last().state?.draft?.revision).toBe(2)
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("another pull request's invalidation, or none at all, is ignored", async () => {
        const first = actionState()
        const desktop = installDesktop(() => first)
        const mounted = await mountProbe()
        try {
            await settle()
            await emit(desktop, { kind: "operation", reference: OTHER, phase: "paused" })
            await emit(desktop, undefined)
            await settle()
            expect(desktop.reads()).toBe(1)
            expect(mounted.last().state).toBe(first)
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("an equal re-read keeps the state it already had", async () => {
        const first = actionState()
        const desktop = installDesktop(() => first)
        const mounted = await mountProbe()
        try {
            await settle()
            desktop.setReply(() => actionState())
            await emit(desktop, { kind: "draft", reference: REFERENCE, revision: 1 })
            await settle()
            expect(desktop.reads()).toBe(2)
            expect(mounted.last().state).toBe(first)
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("a failed read reports its error, keeps the state and is cleared by the next answer", async () => {
        const first = actionState()
        const desktop = installDesktop(() => first)
        const mounted = await mountProbe()
        try {
            await settle()
            desktop.setReply(() => {
                throw new Error("action store corrupt")
            })
            await emit(desktop, { kind: "operation", reference: REFERENCE, phase: "paused" })
            await settle()
            expect(mounted.last().error).toContain("action store corrupt")
            expect(mounted.last().state).toBe(first)

            const second = actionState({ draft: draft(3) })
            desktop.setReply(() => second)
            await emit(desktop, { kind: "draft", reference: REFERENCE, revision: 3 })
            await settle()
            expect(mounted.last().error).toBeNull()
            expect(mounted.last().state).toEqual(second)
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("resolveConflict adopts the authoritative draft before the re-read lands", async () => {
        const first = actionState({ draft: draft(1, { summary: "mine" }) })
        const desktop = installDesktop(() => first)
        const mounted = await mountProbe()
        try {
            await settle()
            const authoritative = draft(2, { summary: "theirs" })
            const reread = actionState({
                draft: authoritative,
                blockedReason: "staleCommits",
                staleBasis: true,
            })
            let answer!: (state: ReviewActionState) => void
            desktop.setReply(
                () =>
                    new Promise<ReviewActionState>((resolve) => {
                        answer = resolve
                    }),
            )
            let resolving!: Promise<ReviewActionState | null>
            await act(async () => {
                resolving = mounted.last().resolveConflict(authoritative)
            })
            // The revision the service holds is shown at once, so the next
            // edit names it; the rest of the state arrives with the re-read.
            expect(mounted.last().state?.draft).toBe(authoritative)
            expect(mounted.last().state?.draft?.revision).toBe(2)
            expect(mounted.last().state?.blockedReason).toBeNull()

            await act(async () => {
                answer(reread)
                await resolving
            })
            await settle()
            expect(desktop.reads()).toBe(2)
            expect(mounted.last().state).toEqual(reread)
            expect(mounted.last().state?.draft).toBe(authoritative)
            expect(mounted.last().state?.staleBasis).toBe(true)
            expect(mounted.last().state?.blockedReason).toBe("staleCommits")
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("two native windows showing one pull request converge on its revision", async () => {
        const first = actionState()
        const desktop = installDesktop(() => first)
        const main = await mountProbe()
        const detached = await mountProbe()
        try {
            await settle()
            // Each window made its own mount read and heard the broadcast.
            expect(desktop.reads()).toBe(2)
            expect(desktop.listens()).toHaveLength(2)

            const second = actionState({ draft: draft(2) })
            desktop.setReply(() => second)
            await emit(desktop, { kind: "draft", reference: REFERENCE, revision: 2 })
            await settle()
            expect(desktop.reads()).toBe(4)
            expect(main.last().state?.draft?.revision).toBe(2)
            expect(detached.last().state?.draft?.revision).toBe(2)
            expect(main.last().state).toEqual(detached.last().state)
        } finally {
            await main.unmount()
            await detached.unmount()
            desktop.uninstall()
        }
    })

    test("rebuilding an equal reference neither re-reads nor re-subscribes", async () => {
        const first = actionState()
        const desktop = installDesktop(() => first)
        const mounted = await mountProbe()
        try {
            await settle()
            await mounted.rerender({ ...REFERENCE })
            await settle()
            expect(desktop.reads()).toBe(1)
            expect(desktop.listens()).toHaveLength(1)
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("a read still in flight when the view lets go lands nowhere", async () => {
        let answer!: (state: ReviewActionState) => void
        const desktop = installDesktop(
            () =>
                new Promise<ReviewActionState>((resolve) => {
                    answer = resolve
                }),
        )
        const mounted = await mountProbe()
        await settle()
        const renders = mounted.seen.length
        await mounted.unmount()
        await act(async () => {
            answer(actionState())
        })
        await settle()
        expect(desktop.reads()).toBe(1)
        expect(mounted.seen).toHaveLength(renders)
        desktop.uninstall()
    })

    test("the browser reads and subscribes to nothing", async () => {
        const browser = installBrowser()
        const mounted = await mountProbe()
        try {
            await settle()
            expect(browser.posted).toEqual([])
            expect(mounted.last().state).toBeNull()
            expect(mounted.last().error).toBeNull()

            let reloaded: ReviewActionState | null = null
            await act(async () => {
                reloaded = await mounted.last().reload()
            })
            expect(reloaded).toBeNull()
            expect(browser.posted).toEqual([])
            expect(mounted.seen).toHaveLength(1)
        } finally {
            await mounted.unmount()
            browser.uninstall()
        }
    })
})

describe("useReviewActions", () => {
    test("a consumer reads the action state its pull-request view provides", async () => {
        const provided: PullRequestActions = {
            state: actionState(),
            error: null,
            reload: () => Promise.resolve(null),
            resolveConflict: () => Promise.resolve(null),
        }
        const seen: PullRequestActions[] = []
        function Consumer() {
            seen.push(useReviewActions())
            return null
        }
        const root = createRoot(dom.document.createElement("div") as unknown as Element)
        try {
            await act(async () =>
                root.render(
                    <PullRequestActionsContext.Provider value={provided}>
                        <Consumer />
                    </PullRequestActionsContext.Provider>,
                ),
            )
            expect(seen[0]).toBe(provided)
        } finally {
            await act(async () => root.unmount())
        }
    })

    test("a surface outside a pull-request view has no action state", () => {
        function Consumer() {
            useReviewActions()
            return null
        }
        expect(() => renderToStaticMarkup(<Consumer />)).toThrow(
            "review actions are read only inside a pull-request view",
        )
    })
})
