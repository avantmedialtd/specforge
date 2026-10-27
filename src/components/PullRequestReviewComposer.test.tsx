import { afterAll, beforeAll, beforeEach, describe, expect, test } from "bun:test"
import { act } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { FakeElement, FakeNode, installFakeDom, type FakeDocument } from "../testing/fakeDom"
import { installFakeTransport, settle } from "../testing/fakeTransport"
import type { PullRequestActions } from "../hooks/usePullRequestActions"
import { PullRequestActionsContext } from "../hooks/usePullRequestActions"
import type { PullRequestAddress } from "../routing/address"
import type {
    PullRequestDetail,
    PullRequestReference,
    PullRequestSummary,
    ReviewActionState,
    ReviewCapability,
    ReviewDraft,
    ReviewProgress,
} from "../types"
import { PullRequestReviewComposer } from "./PullRequestReviewComposer"
import { PullRequestAtAddress, type PaneNavigation } from "./PullRequestView"

// `pull-request-actions`: *Desktop-Only Pull-Request Reviews*, *Draft Items
// and Decisions Are Explicit and Valid*. The composer's rendered states, and
// the commands it sends when a reviewer edits or submits. The native shell is
// faked at Tauri's `invoke` boundary, so the real API wrappers, the real
// composer and the real provider take part.

const REFERENCE: PullRequestReference = {
    provider: "github",
    owner: "acme",
    repo: "api",
    number: 42,
}

const ADDRESS: PullRequestAddress = {
    kind: "pullRequest",
    provider: "github",
    owner: "acme",
    repo: "api",
    number: 42,
}

const CAPABILITIES: ReviewCapability[] = [
    { name: "comment", available: true, reason: null },
    { name: "approve", available: true, reason: null },
    { name: "requestChanges", available: true, reason: null },
    { name: "inlineComment", available: true, reason: null },
    { name: "reply", available: true, reason: null },
]

function draft(overrides: Partial<ReviewDraft> = {}): ReviewDraft {
    return {
        id: "draft-1",
        reference: REFERENCE,
        revision: 1,
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
        draft: draft(),
        capabilities: CAPABILITIES,
        blockedReason: null,
        operation: null,
        staleBasis: false,
        ...overrides,
    }
}

function row(overrides: Partial<PullRequestSummary> = {}): PullRequestSummary {
    return {
        id: 42,
        title: "Add rate limits",
        repoFullName: "acme/api",
        sourceRepoFullName: "acme/api",
        sourceBranch: "rate-limits",
        destinationBranch: "main",
        url: "https://github.com/acme/api/pull/42",
        draft: false,
        updatedAtUnix: 1_000,
        review: null,
        openTasks: 0,
        author: "ada",
        checks: null,
        conflicting: false,
        unresolvedThreads: 0,
        ...overrides,
    }
}

function detail(): PullRequestDetail {
    return {
        reference: REFERENCE,
        row: row(),
        headBranch: "rate-limits",
        baseBranch: "main",
        headCommit: "head1",
        baseCommit: "base1",
        author: "ada",
        description: "Adds rate limits.",
        conversation: [],
        checks: [],
        threads: [],
        files: [],
        unlistedFiles: 0,
        readAtUnix: 1_000,
        noLongerListed: false,
    }
}

function progress(): ReviewProgress {
    return {
        files: [],
        viewed: 0,
        changedSinceViewed: 0,
        skipped: 0,
        total: 0,
        lastMarkedHead: null,
        headCommit: "head1",
        baseCommit: "base1",
    }
}

function actions(
    state: ReviewActionState | null,
    overrides: Partial<PullRequestActions> = {},
): PullRequestActions {
    return {
        state,
        error: null,
        reload: () => Promise.resolve(state),
        resolveConflict: () => Promise.resolve(state),
        ...overrides,
    }
}

function renderComposer(state: ReviewActionState | null, overrides: Partial<PullRequestActions> = {}): string {
    return renderToStaticMarkup(
        <PullRequestActionsContext.Provider value={actions(state, overrides)}>
            <PullRequestReviewComposer reference={REFERENCE} />
        </PullRequestActionsContext.Provider>,
    )
}

describe("the review composer's states", () => {
    test("says it is reading before the first state answers", () => {
        const html = renderComposer(null)
        expect(html).toContain("Reading review state…")
        expect(html).not.toContain("Submit review")
        expect(html).not.toContain("<textarea")
    })

    test("reports a failed read with a way to try again", () => {
        const html = renderComposer(null, { error: "action store unavailable" })
        expect(html).toContain("Couldn&#x27;t read the review state — action store unavailable")
        expect(html).toContain("Try again")
    })

    test("shows a blocked draft read-only, with the service's reason", () => {
        const html = renderComposer(
            actionState({
                draft: draft({ summary: "Kept exactly", decision: "approve" }),
                blockedReason: "credentialChanged",
            }),
        )
        expect(html).toContain("The credential changed since this review was started.")
        expect(html).toContain("Kept exactly")
        expect(html).toContain("<textarea")
        expect(html).toContain("disabled")
        expect(html).toContain("Approve")
        expect(html).not.toContain('role="radio"')
        expect(html).toContain("Submit review")
    })

    test("lists an unavailable decision with its capability reason and offers only the rest", () => {
        const html = renderComposer(
            actionState({
                capabilities: [
                    { name: "comment", available: true, reason: null },
                    {
                        name: "approve",
                        available: false,
                        reason: "you authored this pull request",
                    },
                    {
                        name: "requestChanges",
                        available: false,
                        reason: "you authored this pull request",
                    },
                ],
            }),
        )
        expect(html).toContain("Approve is unavailable: you authored this pull request")
        expect(html).toContain("Request changes is unavailable: you authored this pull request")
        expect(html).toContain('role="radio"')
        expect(html).toContain("Comment")
        expect(html).not.toContain(">Approve</button>")
        expect(html).not.toContain(">Request changes</button>")
    })

    test("an empty approval may submit; an empty comment may not", () => {
        const approve = renderComposer(
            actionState({ draft: draft({ decision: "approve" }) }),
        )
        expect(approve).toContain("Submit review")
        expect(approve).not.toContain("Write a summary")
        expect(approve).not.toContain('disabled=""')

        const comment = renderComposer(actionState())
        expect(comment).toContain(
            "Write a summary or add an inline comment or reply before submitting a comment.",
        )
        expect(comment).toContain('disabled=""')
    })

    test("Request changes needs an explanation", () => {
        const html = renderComposer(
            actionState({ draft: draft({ decision: "requestChanges" }) }),
        )
        expect(html).toContain("Explain what needs to change before requesting changes.")
    })

    test("counts the summary in UTF-8 bytes", () => {
        const html = renderComposer(actionState({ draft: draft({ summary: "é" }) }))
        expect(html).toContain("2 / 30720 bytes")
    })

    test("says when no decision is available at all", () => {
        const html = renderComposer(actionState({ capabilities: [] }))
        expect(html).toContain("No decision is available for this pull request.")
        expect(html).not.toContain('role="radio"')
    })
})

// -------------------------------------------------------------------------
// The composer's commands, through the real API wrappers
// -------------------------------------------------------------------------

let dom: { document: FakeDocument; uninstall: () => void }
let createRoot: typeof import("react-dom/client").createRoot
const g = globalThis as unknown as Record<string, unknown>

beforeAll(async () => {
    dom = installFakeDom()
    g.IS_REACT_ACT_ENVIRONMENT = true
    createRoot = (await import("react-dom/client")).createRoot
})

afterAll(() => {
    delete g.IS_REACT_ACT_ENVIRONMENT
    dom.uninstall()
})

beforeEach(() => {
    delete (g.window as Record<string, unknown>).__TAURI_INTERNALS__
    delete (g.window as Record<string, unknown>).__TAURI_EVENT_PLUGIN_INTERNALS__
})

interface Sent {
    command: string
    args: Record<string, unknown>
}

interface Desktop {
    sent: Sent[]
    calls: (command: string) => Sent[]
    uninstall: () => void
}

/// The native shell: Tauri's `invoke`, answering each command from `answers`
/// and recording what was sent. A handler that throws rejects with that value,
/// as a structured refusal crosses the real boundary.
function installDesktop(
    answers: Record<string, (args: Record<string, unknown>) => unknown>,
): Desktop {
    const host = g.window as Record<string, unknown>
    const sent: Sent[] = []
    let callbackId = 0
    // Tauri's event plugin unregisters listeners through this global.
    host.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
    host.__TAURI_INTERNALS__ = {
        invoke: (command: string, args?: Record<string, unknown>) => {
            sent.push({ command, args: args ?? {} })
            const answer = answers[command]
            if (!answer) return Promise.resolve(command === "plugin:event|listen" ? 1 : undefined)
            try {
                return Promise.resolve(answer(args ?? {}))
            } catch (err) {
                return Promise.reject(err)
            }
        },
        transformCallback: () => {
            callbackId += 1
            return callbackId
        },
    }
    return {
        sent,
        calls: (command) => sent.filter((call) => call.command === command),
        uninstall() {
            delete host.__TAURI_INTERNALS__
            delete host.__TAURI_EVENT_PLUGIN_INTERNALS__
        },
    }
}

function allElements(root: FakeElement): FakeElement[] {
    const found: FakeElement[] = []
    const walk = (node: FakeNode) => {
        for (const child of node.childNodes) {
            if (child instanceof FakeElement) {
                found.push(child)
                walk(child)
            }
        }
    }
    walk(root)
    return found
}

function byClass(root: FakeElement, className: string): FakeElement[] {
    return allElements(root).filter((element) =>
        (element.getAttribute("class") ?? "").split(/\s+/).includes(className),
    )
}

function reactProps(element: FakeElement): Record<string, unknown> {
    const holder = element as unknown as Record<string, unknown>
    for (const key of Reflect.ownKeys(element)) {
        if (typeof key === "string" && key.startsWith("__reactProps$")) {
            return holder[key] as Record<string, unknown>
        }
    }
    for (const key of Reflect.ownKeys(element)) {
        if (typeof key !== "string" || !key.startsWith("__reactFiber$")) continue
        const fiber = holder[key] as { memoizedProps?: Record<string, unknown> }
        if (fiber.memoizedProps !== undefined) return fiber.memoizedProps
    }
    throw new Error("the element carries no React props")
}

async function fire(element: FakeElement, prop: string, event: unknown): Promise<void> {
    const handler = reactProps(element)[prop]
    if (typeof handler !== "function") throw new Error(`the element has no ${prop} handler`)
    await act(async () => {
        ;(handler as (event: unknown) => void)(event)
    })
}

function textareaOf(container: FakeElement): FakeElement {
    const found = container.findAll("textarea")
    if (found.length !== 1) throw new Error(`expected one textarea, found ${found.length}`)
    return found[0]!
}

function valueOf(element: FakeElement): string {
    return (element as unknown as { value?: string }).value ?? ""
}

function radioNamed(container: FakeElement, name: string): FakeElement | undefined {
    return allElements(container).find(
        (element) =>
            element.getAttribute("role") === "radio" && element.textContent === name,
    )
}

function submitButton(container: FakeElement): FakeElement {
    const found = byClass(container, "pull-request-composer-submit")
    if (found.length !== 1) throw new Error(`expected one submit button, found ${found.length}`)
    return found[0]!
}

async function mountComposer(initial: PullRequestActions) {
    const container = dom.document.createElement("div") as unknown as FakeElement
    const root = createRoot(container as unknown as Element)
    const view = (next: PullRequestActions) => (
        <PullRequestActionsContext.Provider value={next}>
            <PullRequestReviewComposer reference={REFERENCE} />
        </PullRequestActionsContext.Provider>
    )
    await act(async () => root.render(view(initial)))
    return {
        container,
        async rerender(next: PullRequestActions) {
            await act(async () => root.render(view(next)))
        },
        async unmount() {
            await act(async () => root.unmount())
        },
    }
}

describe("the review composer's commands", () => {
    test("a summary is saved exactly as typed when the field loses focus", async () => {
        const stored = draft({ revision: 2, summary: "old" })
        const desktop = installDesktop({
            edit_review_draft: (args) =>
                draft({
                    revision: 3,
                    summary: typeof args.summary === "string" ? args.summary : "",
                    decision: args.decision as ReviewDraft["decision"],
                }),
        })
        const mounted = await mountComposer(actions(actionState({ draft: stored })))
        try {
            const field = textareaOf(mounted.container)
            await fire(field, "onChange", { currentTarget: { value: "  exact prose  " } })
            await fire(field, "onBlur", {})
            await settle()
            const [edit] = desktop.calls("edit_review_draft")
            expect(edit?.args).toEqual({
                reference: REFERENCE,
                expectedRevision: 2,
                decision: "comment",
                summary: "  exact prose  ",
                items: [],
            })
            expect(valueOf(textareaOf(mounted.container))).toBe("  exact prose  ")
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("a whitespace-only summary is stored as absent, not as spaces", async () => {
        const stored = draft({ revision: 1, summary: "old" })
        const desktop = installDesktop({
            edit_review_draft: (args) =>
                draft({
                    revision: 2,
                    summary: typeof args.summary === "string" ? args.summary : "",
                }),
        })
        const mounted = await mountComposer(actions(actionState({ draft: stored })))
        try {
            const field = textareaOf(mounted.container)
            await fire(field, "onChange", { currentTarget: { value: "   \n " } })
            await fire(field, "onBlur", {})
            await settle()
            expect(desktop.calls("edit_review_draft")[0]?.args.summary).toBeNull()
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("choosing a decision saves it, creating the draft first when none exists", async () => {
        const created = draft({ revision: 1, decision: "comment" })
        const desktop = installDesktop({
            create_review_draft: () => created,
            edit_review_draft: (args) =>
                draft({
                    revision: 2,
                    decision: args.decision as ReviewDraft["decision"],
                    summary: typeof args.summary === "string" ? args.summary : "",
                }),
        })
        const mounted = await mountComposer(actions(actionState({ draft: null })))
        try {
            const approve = radioNamed(mounted.container, "Approve")
            if (!approve) throw new Error("Approve was not offered")
            await fire(approve, "onClick", {})
            await settle()
            expect(desktop.calls("create_review_draft")).toHaveLength(1)
            const [edit] = desktop.calls("edit_review_draft")
            expect(edit?.args).toEqual({
                reference: REFERENCE,
                expectedRevision: 1,
                decision: "approve",
                summary: null,
                items: [],
            })
            expect(radioNamed(mounted.container, "Approve")?.getAttribute("aria-checked")).toBe(
                "true",
            )
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("a revision conflict adopts the authoritative draft and asks for the rest of the state", async () => {
        const mine = draft({ revision: 1, summary: "mine" })
        const theirs = draft({ revision: 2, summary: "theirs" })
        const resolved: ReviewDraft[] = []
        const desktop = installDesktop({
            edit_review_draft: () => {
                throw { kind: "staleRevision", current: theirs }
            },
        })
        const mounted = await mountComposer(
            actions(actionState({ draft: mine }), {
                resolveConflict: (current) => {
                    resolved.push(current)
                    return Promise.resolve(actionState({ draft: theirs }))
                },
            }),
        )
        try {
            const field = textareaOf(mounted.container)
            await fire(field, "onChange", { currentTarget: { value: "my edit" } })
            await fire(field, "onBlur", {})
            await settle()
            expect(resolved).toEqual([theirs])
            expect(valueOf(textareaOf(mounted.container))).toBe("theirs")
            expect(byClass(mounted.container, "pull-request-composer-notice")[0]?.textContent).toContain(
                "Another window changed this review.",
            )
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("an edit made during a save is saved in a second round against the new revision", async () => {
        const stored = draft({ revision: 1, summary: "one" })
        const pending: {
            args: Record<string, unknown>
            resolve: (next: ReviewDraft) => void
        }[] = []
        const desktop = installDesktop({
            edit_review_draft: (args) =>
                new Promise<ReviewDraft>((resolve) => {
                    pending.push({ args, resolve })
                }),
        })
        const mounted = await mountComposer(actions(actionState({ draft: stored })))
        try {
            const field = textareaOf(mounted.container)
            await fire(field, "onChange", { currentTarget: { value: "first" } })
            await fire(field, "onBlur", {})
            await settle()
            expect(pending).toHaveLength(1)
            expect(pending[0]!.args.expectedRevision).toBe(1)

            // A second edit lands while the first save is still in flight.
            await fire(field, "onChange", { currentTarget: { value: "second" } })
            await act(async () => {
                pending[0]!.resolve(draft({ revision: 2, summary: "first" }))
            })
            await settle()
            expect(pending).toHaveLength(2)
            expect(pending[1]!.args).toEqual({
                reference: REFERENCE,
                expectedRevision: 2,
                decision: "comment",
                summary: "second",
                items: [],
            })

            await act(async () => {
                pending[1]!.resolve(draft({ revision: 3, summary: "second" }))
            })
            await settle()
            expect(valueOf(textareaOf(mounted.container))).toBe("second")
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("submitting without leaving the summary field saves the typed summary first", async () => {
        let reloads = 0
        const stored = draft({ revision: 1, summary: "" })
        const desktop = installDesktop({
            edit_review_draft: (args) =>
                draft({
                    revision: 2,
                    summary: typeof args.summary === "string" ? args.summary : "",
                    decision: args.decision as ReviewDraft["decision"],
                }),
            submit_review: () => "cancelled",
        })
        const mounted = await mountComposer(
            actions(actionState({ draft: stored }), {
                reload: () => {
                    reloads += 1
                    return Promise.resolve(actionState({ draft: stored }))
                },
            }),
        )
        try {
            const field = textareaOf(mounted.container)
            await fire(field, "onChange", { currentTarget: { value: "Typed, not blurred" } })
            await fire(submitButton(mounted.container), "onClick", {})
            await settle()
            expect(desktop.calls("edit_review_draft")[0]?.args.summary).toBe(
                "Typed, not blurred",
            )
            expect(desktop.calls("submit_review")[0]?.args.expectedRevision).toBe(2)
            expect(reloads).toBe(1)
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("submitting freezes the stored revision; a cancel re-reads nothing", async () => {
        let reloads = 0
        const stored = draft({ revision: 5, summary: "Ready" })
        const desktop = installDesktop({ submit_review: () => "cancelled" })
        const mounted = await mountComposer(
            actions(actionState({ draft: stored }), {
                reload: () => {
                    reloads += 1
                    return Promise.resolve(actionState({ draft: stored }))
                },
            }),
        )
        try {
            await fire(submitButton(mounted.container), "onClick", {})
            await settle()
            expect(desktop.calls("submit_review")[0]?.args).toEqual({
                reference: REFERENCE,
                expectedRevision: 5,
            })
            expect(desktop.calls("edit_review_draft")).toHaveLength(0)
            expect(reloads).toBe(0)
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("an accepted submission re-reads the state; an empty approval needs no content", async () => {
        let reloads = 0
        const stored = draft({ revision: 3, decision: "approve" })
        const desktop = installDesktop({ submit_review: () => "accepted" })
        const mounted = await mountComposer(
            actions(actionState({ draft: stored }), {
                reload: () => {
                    reloads += 1
                    return Promise.resolve(actionState({ draft: stored }))
                },
            }),
        )
        try {
            const submit = submitButton(mounted.container)
            expect(submit.hasAttribute("disabled")).toBe(false)
            await fire(submit, "onClick", {})
            await settle()
            expect(desktop.calls("submit_review")[0]?.args.expectedRevision).toBe(3)
            expect(reloads).toBe(1)
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("a blocked draft cannot be edited or submitted", async () => {
        const desktop = installDesktop({})
        const mounted = await mountComposer(
            actions(
                actionState({
                    draft: draft({ summary: "Kept" }),
                    blockedReason: "actorChanged",
                }),
            ),
        )
        try {
            expect(textareaOf(mounted.container).hasAttribute("disabled")).toBe(true)
            expect(radioNamed(mounted.container, "Comment")).toBeUndefined()
            expect(submitButton(mounted.container).hasAttribute("disabled")).toBe(true)
            expect(mounted.container.textContent).toContain(
                "A different account is signed in since this review was started.",
            )
            expect(desktop.sent).toEqual([])
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })
})

// -------------------------------------------------------------------------
// Both native presentations, and the browser's absence
// -------------------------------------------------------------------------

const PANE: PaneNavigation = { openWorktree: () => {}, openSettings: () => {} }

function listed() {
    return { status: "listed" as const, row: row(), address: ADDRESS }
}

async function mountView(options: {
    pane?: PaneNavigation
    desktop: Desktop
}): Promise<{ container: FakeElement; unmount: () => Promise<void> }> {
    const container = dom.document.createElement("div") as unknown as FakeElement
    const root = createRoot(container as unknown as Element)
    await act(async () =>
        root.render(
            <PullRequestAtAddress
                address={ADDRESS}
                resolution={listed()}
                views={[]}
                links={null}
                pane={options.pane}
            />,
        ),
    )
    await settle()
    return {
        container,
        async unmount() {
            await act(async () => root.unmount())
        },
    }
}

function nativeAnswers(state: ReviewActionState): Record<string, (args: Record<string, unknown>) => unknown> {
    return {
        get_pull_request_detail: () => ({ kind: "detail", detail: detail() }),
        get_review_progress: () => progress(),
        get_review_action_state: () => state,
    }
}

describe("the composer in both native presentations", () => {
    test("the center pane and the detached window show the same review controls", async () => {
        const state = actionState({ draft: draft({ summary: "Draft summary" }) })
        const desktop = installDesktop(nativeAnswers(state))
        const center = await mountView({ pane: PANE, desktop })
        const detached = await mountView({ desktop })
        try {
            for (const container of [center.container, detached.container]) {
                expect(container.textContent).toContain("Adds rate limits.")
                const review = byClass(container, "pull-request-composer")
                expect(review).toHaveLength(1)
                expect(review[0]!.textContent).toContain("Review")
                expect(valueOf(textareaOf(container))).toBe("Draft summary")
                expect(submitButton(container)).toBeDefined()
            }
            // Each presentation read its own current state, and both heard the
            // action broadcast's event name.
            expect(desktop.calls("get_review_action_state").length).toBeGreaterThanOrEqual(2)
            const listened = desktop
                .calls("plugin:event|listen")
                .map((call) => call.args.event)
            expect(listened.filter((event) => event === "review-action-changed")).toHaveLength(2)
        } finally {
            await center.unmount()
            await detached.unmount()
            desktop.uninstall()
        }
    })

    test("a browser tab renders the pull request but no composer or write command", async () => {
        const transport = installFakeTransport((command) => {
            if (command === "get_pull_request_detail") {
                return { kind: "detail", detail: detail() }
            }
            if (command === "get_review_progress") return progress()
            throw new Error(`a browser sent ${command}`)
        })
        const container = dom.document.createElement("div") as unknown as FakeElement
        const root = createRoot(container as unknown as Element)
        try {
            await act(async () =>
                root.render(
                    <PullRequestAtAddress
                        address={ADDRESS}
                        resolution={listed()}
                        views={[]}
                        links={null}
                        pane={PANE}
                    />,
                ),
            )
            await settle()
            expect(container.textContent).toContain("Adds rate limits.")
            expect(byClass(container, "pull-request-composer")).toHaveLength(0)
            expect(container.findAll("textarea")).toHaveLength(0)
            expect(container.textContent).not.toContain("Submit review")
            for (const write of [
                "get_review_action_state",
                "create_review_draft",
                "edit_review_draft",
                "submit_review",
                "recover_review_draft",
                "start_review_draft_over",
            ]) {
                expect(transport.calls).not.toContain(write)
            }
        } finally {
            await act(async () => root.unmount())
            transport.uninstall()
        }
    })
})
