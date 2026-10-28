import { afterAll, beforeAll, beforeEach, describe, expect, test } from "bun:test"
import { act } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { FakeElement, installFakeDom, type FakeDocument } from "../testing/fakeDom"
import { settle } from "../testing/fakeTransport"
import type { PullRequestActions } from "../hooks/usePullRequestActions"
import { PullRequestActionsContext } from "../hooks/usePullRequestActions"
import {
    ReviewDraftEditorProvider,
    useReviewDraftEditorContext,
} from "../hooks/useReviewDraftEditor"
import type {
    LedgerEntry,
    LedgerOutcome,
    LedgerStep,
    PullRequestReference,
    ReviewActionState,
    ReviewCapability,
    ReviewDraft,
    ReviewOperationState,
} from "../types"
import { PullRequestReviewComposer } from "./PullRequestReviewComposer"
import { PullRequestReviewStatus } from "./PullRequestReviewStatus"

// `pull-request-actions`: *Current State Authorises Every Draft and Submission
// Boundary*, *Interrupted Operations Reconcile Without Automatic Mutation*,
// *Review Operations Respect Provider Failure Boundaries*. The status block's
// blocked reasons, explicit stale recovery, live and paused operations,
// receipts, and the commands its Resume, Abandon and Start over controls send.

const REFERENCE: PullRequestReference = {
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

function operation(overrides: Partial<ReviewOperationState> = {}): ReviewOperationState {
    return {
        id: "operation-1",
        phase: "paused",
        ledger: [],
        partial: { succeeded: [], unresolved: [], failed: [] },
        receipt: null,
        ...overrides,
    }
}

function entry(step: LedgerStep, outcome: LedgerOutcome): LedgerEntry {
    return { step, outcome }
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

function renderStatus(state: ReviewActionState | null): string {
    const next = actions(state)
    return renderToStaticMarkup(
        <PullRequestActionsContext.Provider value={next}>
            <ReviewDraftEditorProvider reference={REFERENCE} actions={next}>
                <PullRequestReviewStatus />
            </ReviewDraftEditorProvider>
        </PullRequestActionsContext.Provider>,
    )
}

function confirmed(id: string | null): LedgerOutcome {
    return { kind: "confirmed", providerId: id }
}

describe("the review status's states", () => {
    test("names a corrupt action store as blocked with no action to take", () => {
        const html = renderStatus(actionState({ blockedReason: "actionStoreCorrupt" }))
        expect(html).toContain("The review action store is damaged.")
        expect(html).toContain("blocked")
        expect(html).not.toContain("Start over")
        expect(html).not.toContain("Resume review")
        expect(html).not.toContain("Abandon review")
    })

    test("offers explicit stale recovery and says no line moves automatically", () => {
        const html = renderStatus(
            actionState({ staleBasis: true, blockedReason: "staleCommits" }),
        )
        expect(html).toContain("The branch has new commits since this review was started.")
        expect(html).toContain("Recover onto current commits")
        expect(html).toContain("every inline comment becomes unplaced")
        expect(html).toContain("Nothing is moved automatically")
    })

    test("a live operation in another process offers no local control", () => {
        const html = renderStatus(
            actionState({ operation: operation({ phase: "reconciling" }) }),
        )
        expect(html).toContain("another SpecForge process")
        expect(html).not.toContain("Resume review")
        expect(html).not.toContain("Abandon review")
        expect(html).not.toContain("Start over")
    })

    test("a paused operation lists every confirmed, unresolved and failed write", () => {
        const ledger = [
            entry({ kind: "inlineComment", item: "i1" }, confirmed("comment-7")),
            entry({ kind: "reply", item: "r1" }, { kind: "uncertain" }),
            entry({ kind: "summary" }, { kind: "failed", reason: "rate limited" }),
        ]
        const html = renderStatus(
            actionState({
                operation: operation({
                    ledger,
                    partial: {
                        succeeded: [ledger[0]!],
                        unresolved: [ledger[1]!],
                        failed: [ledger[2]!],
                    },
                }),
            }),
        )
        expect(html).toContain("Inline comment i1")
        expect(html).toContain("confirmed (comment-7)")
        expect(html).toContain("Reply r1")
        expect(html).toContain("unresolved — the provider may or may not hold it")
        expect(html).toContain("Summary")
        expect(html).toContain("failed — rate limited")
        expect(html).toContain("Resume review")
        expect(html).toContain('disabled=""')
        expect(html).toContain("Resume waits until the unresolved step is reconciled")
        expect(html).toContain("Abandon review")
    })

    test("Resume is enabled only when every unresolved step is eligible", () => {
        const step = entry({ kind: "summary" }, confirmed(null))
        const paused = renderStatus(
            actionState({
                operation: operation({
                    ledger: [step],
                    partial: { succeeded: [step], unresolved: [], failed: [] },
                }),
            }),
        )
        expect(paused).toContain("Resume review")
        expect(paused).not.toContain('disabled=""')

        const failed = entry({ kind: "decision" }, { kind: "failed", reason: "forbidden" })
        const blocked = renderStatus(
            actionState({
                operation: operation({
                    ledger: [failed],
                    partial: { succeeded: [], unresolved: [], failed: [failed] },
                }),
            }),
        )
        expect(blocked).toContain("The first unresolved step failed and is not retried")
        expect(blocked).toContain('disabled=""')
    })

    test("a submitted operation shows the provider receipt and no Resume or Abandon", () => {
        const html = renderStatus(
            actionState({
                operation: operation({
                    phase: "submitted",
                    receipt: {
                        reviewId: "review-9",
                        decision: "approve",
                        remainingPendingReview: null,
                    },
                }),
            }),
        )
        expect(html).toContain("Review submitted as review-9 (Approve).")
        expect(html).not.toContain("Resume review")
        expect(html).not.toContain("Abandon review")
        expect(html).toContain("Start over")
    })

    test("an abandoned operation keeps its effects and names a pending review for manual handling", () => {
        const html = renderStatus(
            actionState({
                operation: operation({
                    phase: "abandoned",
                    receipt: {
                        reviewId: "review-9",
                        decision: null,
                        remainingPendingReview: "pending-4",
                    },
                }),
            }),
        )
        expect(html).toContain("Review abandoned.")
        expect(html).toContain("Provider review review-9 may remain; nothing was rolled back.")
        expect(html).toContain(
            "A pending review may remain at the provider (pending-4). Remove it there if needed.",
        )
        expect(html).not.toContain("Resume review")
        expect(html).not.toContain("Abandon review")
    })

    test("Start over is offered only with a draft and no unfinished operation", () => {
        expect(renderStatus(actionState())).toContain("Start over")
        expect(renderStatus(actionState({ draft: null }))).not.toContain("Start over")
        expect(
            renderStatus(actionState({ operation: operation({ phase: "paused" }) })),
        ).not.toContain("Start over")
        expect(
            renderStatus(
                actionState({
                    operation: operation({ phase: "submitted", receipt: null }),
                }),
            ),
        ).toContain("Start over")
    })
})

// -------------------------------------------------------------------------
// The status's commands, through the real API wrappers
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
/// and recording what was sent.
function installDesktop(
    answers: Record<string, (args: Record<string, unknown>) => unknown>,
): Desktop {
    const host = g.window as Record<string, unknown>
    const sent: Sent[] = []
    let callbackId = 0
    host.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
    host.__TAURI_INTERNALS__ = {
        invoke: (command: string, args?: Record<string, unknown>) => {
            sent.push({ command, args: args ?? {} })
            const answer = answers[command]
            if (!answer) return Promise.resolve(undefined)
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

function buttonNamed(container: FakeElement, name: string): FakeElement {
    const found = container
        .findAll("button")
        .filter((button) => button.textContent === name)
    if (found.length !== 1) {
        throw new Error(`expected one ${name} button, found ${found.length}`)
    }
    return found[0]!
}

function textareaOf(container: FakeElement): FakeElement {
    const found = container.findAll("textarea")
    if (found.length !== 1) throw new Error(`expected one textarea, found ${found.length}`)
    return found[0]!
}

function valueOf(element: FakeElement): string {
    return (element as unknown as { value?: string }).value ?? ""
}

/// Reads the shared editor's local form and draft, so a test sees what the
/// service's answer was adopted into rather than only what the composer paints.
function EditorProbe() {
    const editor = useReviewDraftEditorContext()
    const items =
        editor?.draft?.items
            .map((item) => `${item.kind}:${item.id}:${item.body}`)
            .join(",") ?? ""
    return (
        <span className="probe">
            {editor?.summary ?? ""}::{items}
        </span>
    )
}

async function mountComposer(initial: PullRequestActions) {
    const container = dom.document.createElement("div") as unknown as FakeElement
    const root = createRoot(container as unknown as Element)
    const view = (next: PullRequestActions) => (
        <PullRequestActionsContext.Provider value={next}>
            <ReviewDraftEditorProvider reference={REFERENCE} actions={next}>
                <PullRequestReviewComposer reference={REFERENCE} />
                <EditorProbe />
            </ReviewDraftEditorProvider>
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

describe("the review status's commands", () => {
    test("stale recovery is one explicit click that adopts the recovered draft", async () => {
        const before = draft({
            revision: 1,
            summary: "Kept",
            items: [
                {
                    kind: "placedInline",
                    id: "i1",
                    body: "Former comment",
                    path: "src/a.ts",
                    side: "new",
                    line: 22,
                },
            ],
        })
        const recovered = draft({
            revision: 2,
            summary: "Kept",
            items: [
                { kind: "placedReply", id: "r1", body: "Valid reply", target: "t1" },
                {
                    kind: "unplacedInline",
                    id: "i1",
                    body: "Former comment",
                    path: "src/a.ts",
                    side: "new",
                    line: 22,
                    basis: { head: "old-head", base: "old-base" },
                },
            ],
        })
        const after = actionState({ draft: recovered })
        const desktop = installDesktop({
            recover_review_draft: () => recovered,
        })
        const initial = actions(
            actionState({
                draft: before,
                blockedReason: "staleCommits",
                staleBasis: true,
            }),
            { reload: () => Promise.resolve(after) },
        )
        const mounted = await mountComposer(initial)
        try {
            // Nothing remaps until the reviewer asks.
            expect(desktop.calls("recover_review_draft")).toHaveLength(0)

            await fire(buttonNamed(mounted.container, "Recover onto current commits"), "onClick", {})
            await settle()
            expect(desktop.calls("recover_review_draft")[0]?.args).toEqual({
                reference: REFERENCE,
                expectedRevision: 1,
            })
            expect(valueOf(textareaOf(mounted.container))).toBe("Kept")

            await mounted.rerender(actions(after, { reload: () => Promise.resolve(after) }))
            const probe = mounted.container.findAll("span").find((span) =>
                (span.getAttribute("class") ?? "").includes("probe"),
            )
            expect(probe?.textContent).toContain("Kept::")
            expect(probe?.textContent).toContain("placedReply:r1:Valid reply")
            expect(probe?.textContent).toContain("unplacedInline:i1:Former comment")

            // Recovery is a draft move, never a provider write or a new draft.
            expect(desktop.calls("edit_review_draft")).toHaveLength(0)
            expect(desktop.calls("create_review_draft")).toHaveLength(0)
            expect(desktop.calls("submit_review")).toHaveLength(0)
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("Resume sends only the operation command and shows the submitted receipt", async () => {
        const step = entry({ kind: "summary" }, confirmed(null))
        const paused = operation({
            ledger: [step],
            partial: { succeeded: [step], unresolved: [], failed: [] },
        })
        const submitted = operation({
            phase: "submitted",
            ledger: [step],
            partial: null,
            receipt: {
                reviewId: "review-3",
                decision: "comment",
                remainingPendingReview: null,
            },
        })
        const after = actionState({ operation: submitted })
        const desktop = installDesktop({ resume_review_operation: () => undefined })
        const mounted = await mountComposer(
            actions(actionState({ operation: paused }), {
                reload: () => Promise.resolve(after),
            }),
        )
        try {
            await fire(buttonNamed(mounted.container, "Resume review"), "onClick", {})
            await settle()
            expect(desktop.calls("resume_review_operation")[0]?.args).toEqual({
                reference: REFERENCE,
            })
            expect(desktop.calls("edit_review_draft")).toHaveLength(0)
            expect(desktop.calls("create_review_draft")).toHaveLength(0)

            await mounted.rerender(
                actions(after, { reload: () => Promise.resolve(after) }),
            )
            expect(mounted.container.textContent).toContain(
                "Review submitted as review-3 (Comment).",
            )
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("a fenced paused operation can be abandoned locally, with no transfer to the new credential", async () => {
        const failed = entry(
            { kind: "decision" },
            { kind: "failed", reason: "the token lacks review-write permission" },
        )
        const paused = operation({
            ledger: [failed],
            partial: { succeeded: [], unresolved: [], failed: [failed] },
        })
        const abandoned = operation({
            phase: "abandoned",
            ledger: [failed],
            partial: null,
            receipt: {
                reviewId: null,
                decision: null,
                remainingPendingReview: "pending-8",
            },
        })
        const after = actionState({
            operation: abandoned,
            blockedReason: "credentialChanged",
        })
        const desktop = installDesktop({ abandon_review_operation: () => undefined })
        const mounted = await mountComposer(
            actions(
                actionState({
                    operation: paused,
                    blockedReason: "credentialChanged",
                }),
                { reload: () => Promise.resolve(after) },
            ),
        )
        try {
            expect(mounted.container.textContent).toContain(
                "The credential changed since this review was started.",
            )
            expect(mounted.container.textContent).not.toContain("Start over")
            expect(buttonNamed(mounted.container, "Resume review").hasAttribute("disabled")).toBe(
                true,
            )

            await fire(buttonNamed(mounted.container, "Abandon review"), "onClick", {})
            await settle()
            expect(desktop.calls("abandon_review_operation")[0]?.args).toEqual({
                reference: REFERENCE,
            })
            // Abandonment is local: no frozen content is edited, replaced or resubmitted.
            expect(desktop.calls("edit_review_draft")).toHaveLength(0)
            expect(desktop.calls("create_review_draft")).toHaveLength(0)
            expect(desktop.calls("start_review_draft_over")).toHaveLength(0)
            expect(desktop.calls("submit_review")).toHaveLength(0)
            expect(desktop.calls("resume_review_operation")).toHaveLength(0)

            await mounted.rerender(
                actions(after, { reload: () => Promise.resolve(after) }),
            )
            expect(mounted.container.textContent).toContain(
                "A pending review may remain at the provider (pending-8). Remove it there if needed.",
            )
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("Start over replaces the draft with a visibly empty one and sends nothing else", async () => {
        const before = draft({
            revision: 4,
            summary: "Old prose",
            decision: "requestChanges",
            items: [
                {
                    kind: "placedInline",
                    id: "i1",
                    body: "Old comment",
                    path: "src/a.ts",
                    side: "new",
                    line: 22,
                },
            ],
        })
        const fresh = draft({
            id: "draft-2",
            revision: 1,
            summary: "",
            decision: "comment",
            items: [],
        })
        const after = actionState({ draft: fresh })
        const desktop = installDesktop({ start_review_draft_over: () => fresh })
        const mounted = await mountComposer(
            actions(actionState({ draft: before }), {
                reload: () => Promise.resolve(after),
            }),
        )
        try {
            await fire(buttonNamed(mounted.container, "Start over"), "onClick", {})
            await settle()
            expect(desktop.calls("start_review_draft_over")[0]?.args).toEqual({
                reference: REFERENCE,
            })
            expect(valueOf(textareaOf(mounted.container))).toBe("")
            expect(mounted.container.textContent).not.toContain("Old prose")

            await mounted.rerender(
                actions(after, { reload: () => Promise.resolve(after) }),
            )
            const probe = mounted.container.findAll("span").find((span) =>
                (span.getAttribute("class") ?? "").includes("probe"),
            )
            expect(probe?.textContent).toBe("::")

            expect(desktop.calls("edit_review_draft")).toHaveLength(0)
            expect(desktop.calls("create_review_draft")).toHaveLength(0)
            expect(desktop.calls("submit_review")).toHaveLength(0)
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })
})
