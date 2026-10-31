import { afterAll, beforeAll, beforeEach, describe, expect, test } from "bun:test"
import { act, useEffect } from "react"
import { installFakeDom, type FakeDocument } from "../testing/fakeDom"
import type { PullRequestActions } from "./usePullRequestActions"
import {
    useDraftItemsEditor,
    useReviewDraftEditor,
    type DraftItemsEditor,
    type ReviewDraftEditor,
} from "./useReviewDraftEditor"
import { MAX_BODY_BYTES, MAX_DRAFT_ITEMS } from "../reviewActions"
import type {
    DraftItem,
    DraftItemInput,
    PlacedInlineItem,
    PlacedReplyItem,
    PullRequestReference,
    ReviewActionState,
    ReviewCapability,
    ReviewDecision,
    ReviewDraft,
    ReviewThread,
    UnplacedInlineItem,
    UnplacedReplyItem,
} from "../types"

// `pull-request-actions`: *Draft Items and Decisions Are Explicit and Valid*,
// *Stale Commit Recovery Never Remaps an Inline Comment*. One draft editor's
// whole-list writes: create, add, edit, move/convert, delete, bounds and
// conflicts. The native shell is faked at Tauri's `invoke` boundary, so the
// real API wrappers and the real hook take part.

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

function draft(items: DraftItem[] = [], overrides: Partial<ReviewDraft> = {}): ReviewDraft {
    return {
        id: "draft-1",
        reference: REFERENCE,
        revision: 1,
        basis: { head: "head1", base: "base1" },
        decision: "comment",
        summary: "",
        items,
        ...overrides,
    }
}

function state(
    current: ReviewDraft | null,
    overrides: Partial<ReviewActionState> = {},
): ReviewActionState {
    return {
        reference: REFERENCE,
        draft: current,
        capabilities: CAPABILITIES,
        blockedReason: null,
        operation: null,
        staleBasis: false,
        ...overrides,
    }
}

function actions(
    current: ReviewActionState | null,
    overrides: Partial<PullRequestActions> = {},
): PullRequestActions {
    return {
        state: current,
        error: null,
        reload: () => Promise.resolve(current),
        resolveConflict: () => Promise.resolve(current),
        ...overrides,
    }
}

function placedInline(
    id: string,
    body: string,
    path = "src/a.ts",
    side: "old" | "new" = "new",
    line = 22,
): PlacedInlineItem {
    return { kind: "placedInline", id, body, path, side, line }
}

function unplacedInline(
    id: string,
    body: string,
    path = "src/old.ts",
    side: "old" | "new" = "new",
    line = 22,
): UnplacedInlineItem {
    return {
        kind: "unplacedInline",
        id,
        body,
        path,
        side,
        line,
        basis: { head: "old-head", base: "old-base" },
    }
}

function placedReply(id: string, body: string, target: string): PlacedReplyItem {
    return { kind: "placedReply", id, body, target }
}

function unplacedReply(id: string, body: string, target: string): UnplacedReplyItem {
    return { kind: "unplacedReply", id, body, target }
}

function thread(overrides: Partial<ReviewThread> = {}): ReviewThread {
    return {
        id: "t1",
        path: "src/a.ts",
        side: "new",
        line: 22,
        startSide: null,
        startLine: null,
        originalLine: null,
        originalStartLine: null,
        resolved: false,
        outdated: false,
        replyable: true,
        comments: [],
        ...overrides,
    }
}

// -------------------------------------------------------------------------
// The native shell
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

function installDesktop(
    answers: Record<string, (args: Record<string, unknown>) => unknown>,
): Desktop {
    const host = g.window as Record<string, unknown>
    const sent: Sent[] = []
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
    }
    return {
        sent,
        calls: (command) => sent.filter((call) => call.command === command),
        uninstall() {
            delete host.__TAURI_INTERNALS__
        },
    }
}

let nextId = 0

function materialize(input: DraftItemInput): DraftItem {
    if (input.kind === "carried") return input.item
    nextId += 1
    if (input.kind === "newInline") {
        return {
            kind: "placedInline",
            id: `new-${nextId}`,
            body: input.body,
            path: input.path,
            side: input.side,
            line: input.line,
        }
    }
    return {
        kind: "placedReply",
        id: `new-${nextId}`,
        body: input.body,
        target: input.target,
    }
}

/// What the service stores for one whole-list edit, built from the request.
function stored(args: Record<string, unknown>): ReviewDraft {
    return draft((args.items as DraftItemInput[]).map(materialize), {
        revision: Number(args.expectedRevision) + 1,
        decision: args.decision as ReviewDecision,
        summary: typeof args.summary === "string" ? args.summary : "",
    })
}

function Harness({
    current,
    onEditor,
}: {
    current: PullRequestActions
    onEditor: (editor: ReviewDraftEditor) => void
}) {
    const editor = useReviewDraftEditor(REFERENCE, current)
    useEffect(() => onEditor(editor))
    return null
}

async function mount(current: PullRequestActions) {
    let latest: ReviewDraftEditor | null = null
    const onEditor = (editor: ReviewDraftEditor) => {
        latest = editor
    }
    const container = dom.document.createElement("div")
    const root = createRoot(container as unknown as Element)
    await act(async () => root.render(<Harness current={current} onEditor={onEditor} />))
    return {
        get(): ReviewDraftEditor {
            if (!latest) throw new Error("the editor did not mount")
            return latest
        },
        async unmount() {
            await act(async () => root.unmount())
        },
    }
}

async function run<T>(work: () => Promise<T>): Promise<T> {
    let result!: T
    await act(async () => {
        result = await work()
    })
    return result
}

describe("the shared draft editor writes one whole list at a time", () => {
    test("adding an inline comment carries the draft and appends one newInline item", async () => {
        const existing = placedInline("i1", "Kept")
        const desktop = installDesktop({ edit_review_draft: stored })
        const mounted = await mount(
            actions(state(draft([existing], { revision: 3, summary: "Summary" }))),
        )
        try {
            const added = await run(() =>
                mounted.get().addInline(
                    { path: "src/a.ts", side: "new", line: 22 },
                    "New comment",
                ),
            )
            expect(added).toBe(true)
            expect(desktop.calls("create_review_draft")).toHaveLength(0)
            expect(desktop.calls("edit_review_draft")[0]?.args).toEqual({
                reference: REFERENCE,
                expectedRevision: 3,
                decision: "comment",
                summary: "Summary",
                items: [
                    { kind: "carried", item: existing },
                    {
                        kind: "newInline",
                        body: "New comment",
                        path: "src/a.ts",
                        side: "new",
                        line: 22,
                    },
                ],
            })
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("adding an inline comment creates the draft first when none exists", async () => {
        const created = draft([], { revision: 1 })
        const desktop = installDesktop({
            create_review_draft: () => created,
            edit_review_draft: stored,
        })
        const mounted = await mount(actions(state(null)))
        try {
            const added = await run(() =>
                mounted.get().addInline(
                    { path: "src/a.ts", side: "old", line: 21 },
                    "On the removed line",
                ),
            )
            expect(added).toBe(true)
            expect(desktop.calls("create_review_draft")).toHaveLength(1)
            expect(desktop.calls("edit_review_draft")[0]?.args).toEqual({
                reference: REFERENCE,
                expectedRevision: 1,
                decision: "comment",
                summary: null,
                items: [
                    {
                        kind: "newInline",
                        body: "On the removed line",
                        path: "src/a.ts",
                        side: "old",
                        line: 21,
                    },
                ],
            })
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("a reply requires a replyable thread and writes a newReply target", async () => {
        const target = thread({ id: "t1", replyable: true })
        const desktop = installDesktop({ edit_review_draft: stored })
        const mounted = await mount(actions(state(draft())))
        try {
            const added = await run(() => mounted.get().addReply(target, "A reply"))
            expect(added).toBe(true)
            expect(desktop.calls("edit_review_draft")[0]?.args.items).toEqual([
                { kind: "newReply", body: "A reply", target: "t1" },
            ])

            desktop.sent.length = 0
            const refused = await run(() =>
                mounted.get().addReply(thread({ id: "t2", replyable: false }), "No"),
            )
            expect(refused).toBe(false)
            expect(desktop.sent).toEqual([])
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("moving an unplaced comment converts it by id to the chosen current line", async () => {
        const former = unplacedInline("u1", "Former prose")
        const desktop = installDesktop({ edit_review_draft: stored })
        const mounted = await mount(actions(state(draft([former]))))
        try {
            const moved = await run(() =>
                mounted.get().moveInline(former, {
                    path: "src/a.ts",
                    side: "old",
                    line: 21,
                }),
            )
            expect(moved).toBe(true)
            expect(desktop.calls("edit_review_draft")[0]?.args.items).toEqual([
                {
                    kind: "carried",
                    item: {
                        kind: "placedInline",
                        id: "u1",
                        body: "Former prose",
                        path: "src/a.ts",
                        side: "old",
                        line: 21,
                    },
                },
            ])
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("moving a placed comment keeps its id and body and changes only its anchor", async () => {
        const placed = placedInline("p1", "Placed prose", "src/a.ts", "new", 22)
        const desktop = installDesktop({ edit_review_draft: stored })
        const mounted = await mount(actions(state(draft([placed]))))
        try {
            await run(() =>
                mounted.get().moveInline(placed, {
                    path: "src/a.ts",
                    side: "new",
                    line: 20,
                }),
            )
            expect(desktop.calls("edit_review_draft")[0]?.args.items).toEqual([
                {
                    kind: "carried",
                    item: {
                        kind: "placedInline",
                        id: "p1",
                        body: "Placed prose",
                        path: "src/a.ts",
                        side: "new",
                        line: 20,
                    },
                },
            ])
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("moving an unplaced reply converts it by id to a replyable thread", async () => {
        const former = unplacedReply("u2", "Former reply", "gone-thread")
        const desktop = installDesktop({ edit_review_draft: stored })
        const mounted = await mount(actions(state(draft([former]))))
        try {
            const moved = await run(() =>
                mounted.get().moveReply(former, thread({ id: "t1", replyable: true })),
            )
            expect(moved).toBe(true)
            expect(desktop.calls("edit_review_draft")[0]?.args.items).toEqual([
                {
                    kind: "carried",
                    item: {
                        kind: "placedReply",
                        id: "u2",
                        body: "Former reply",
                        target: "t1",
                    },
                },
            ])

            desktop.sent.length = 0
            const refused = await run(() =>
                mounted.get().moveReply(former, thread({ id: "t2", replyable: false })),
            )
            expect(refused).toBe(false)
            expect(desktop.sent).toEqual([])
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("editing and deleting write the whole remaining list", async () => {
        const first = placedInline("i1", "One")
        const second = placedReply("i2", "Two", "t1")
        const desktop = installDesktop({ edit_review_draft: stored })
        const mounted = await mount(actions(state(draft([first, second]))))
        try {
            const edited = await run(() => mounted.get().editItem(first, "Changed"))
            expect(edited).toBe(true)
            expect(desktop.calls("edit_review_draft")[0]?.args.items).toEqual([
                { kind: "carried", item: placedInline("i1", "Changed") },
                { kind: "carried", item: second },
            ])

            const removed = await run(() => mounted.get().removeItem(second))
            expect(removed).toBe(true)
            expect(desktop.calls("edit_review_draft")[1]?.args.items).toEqual([
                { kind: "carried", item: placedInline("i1", "Changed") },
            ])
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("empty, whitespace-only and over-30-KiB bodies never reach the service", async () => {
        const desktop = installDesktop({ edit_review_draft: stored })
        const mounted = await mount(actions(state(draft())))
        try {
            expect(await run(() => mounted.get().addInline({ path: "src/a.ts", side: "new", line: 22 }, ""))).toBe(false)
            expect(await run(() => mounted.get().addInline({ path: "src/a.ts", side: "new", line: 22 }, "   \n"))).toBe(false)
            expect(
                await run(() =>
                    mounted.get().addInline(
                        { path: "src/a.ts", side: "new", line: 22 },
                        "x".repeat(MAX_BODY_BYTES + 1),
                    ),
                ),
            ).toBe(false)
            // Two bytes per character: under 30 Ki characters, over 30 KiB.
            const multibyte = "é".repeat(Math.floor(MAX_BODY_BYTES / 2) + 1)
            expect(
                await run(() =>
                    mounted.get().addInline(
                        { path: "src/a.ts", side: "new", line: 22 },
                        multibyte,
                    ),
                ),
            ).toBe(false)
            expect(desktop.sent).toEqual([])
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("a draft holding 100 items refuses a 101st comment or reply without a write", async () => {
        const items = Array.from({ length: MAX_DRAFT_ITEMS }, (_, index) =>
            placedInline(`i${index}`, `Body ${index}`),
        )
        const desktop = installDesktop({ edit_review_draft: stored })
        const mounted = await mount(actions(state(draft(items))))
        try {
            expect(mounted.get().atItemLimit).toBe(true)
            expect(
                await run(() =>
                    mounted.get().addInline(
                        { path: "src/a.ts", side: "new", line: 22 },
                        "One more",
                    ),
                ),
            ).toBe(false)
            expect(
                await run(() => mounted.get().addReply(thread(), "One more")),
            ).toBe(false)
            expect(desktop.sent).toEqual([])
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("a blocked or capability-less state refuses item writes without a command", async () => {
        const desktop = installDesktop({ edit_review_draft: stored })
        const blocked = await mount(
            actions(state(draft(), { blockedReason: "credentialChanged" })),
        )
        try {
            expect(blocked.get().editable).toBe(false)
            expect(blocked.get().canInline).toBe(false)
            expect(blocked.get().canReply).toBe(false)
            expect(
                await run(() =>
                    blocked.get().addInline({ path: "src/a.ts", side: "new", line: 22 }, "No"),
                ),
            ).toBe(false)
            expect(await run(() => blocked.get().addReply(thread(), "No"))).toBe(false)
            expect(desktop.sent).toEqual([])
        } finally {
            await blocked.unmount()
        }

        const unavailable = CAPABILITIES.map((capability) =>
            capability.name === "inlineComment" || capability.name === "reply"
                ? { ...capability, available: false, reason: "not offered" }
                : capability,
        )
        const limited = await mount(
            actions(state(draft(), { capabilities: unavailable })),
        )
        try {
            expect(limited.get().canInline).toBe(false)
            expect(limited.get().canReply).toBe(false)
            expect(
                await run(() =>
                    limited.get().addInline({ path: "src/a.ts", side: "new", line: 22 }, "No"),
                ),
            ).toBe(false)
            expect(await run(() => limited.get().addReply(thread(), "No"))).toBe(false)
            expect(desktop.sent).toEqual([])
        } finally {
            await limited.unmount()
            desktop.uninstall()
        }
    })

    test("placing starts only for an item the draft still holds", async () => {
        const former = unplacedInline("u1", "Former")
        const mounted = await mount(actions(state(draft([former]))))
        try {
            await act(async () => {
                mounted.get().beginPlacing({ kind: "inline", itemId: "missing" })
            })
            expect(mounted.get().placing).toBeNull()

            await act(async () => {
                mounted.get().beginPlacing({ kind: "inline", itemId: "u1" })
            })
            expect(mounted.get().placing).toEqual({ kind: "inline", itemId: "u1" })

            await act(async () => {
                mounted.get().cancelPlacing()
            })
            expect(mounted.get().placing).toBeNull()
        } finally {
            await mounted.unmount()
        }
    })

    test("a revision conflict adopts the authoritative draft and asks for the rest of the state", async () => {
        const mine = draft([placedInline("i1", "Mine")], { revision: 1, summary: "mine" })
        const theirs = draft([], { revision: 2, summary: "theirs" })
        const resolved: ReviewDraft[] = []
        const desktop = installDesktop({
            edit_review_draft: () => {
                throw { kind: "staleRevision", current: theirs }
            },
        })
        const mounted = await mount(
            actions(state(mine), {
                resolveConflict: (current) => {
                    resolved.push(current)
                    return Promise.resolve(state(theirs))
                },
            }),
        )
        try {
            const item = mine.items[0]!
            const edited = await run(() => mounted.get().editItem(item, "Changed"))
            expect(edited).toBe(false)
            expect(resolved).toEqual([theirs])
            expect(mounted.get().summary).toBe("theirs")
            expect(mounted.get().notice?.text).toContain(
                "Another window changed this review.",
            )
            expect(mounted.get().itemError?.text).toBe("Another window changed this review.")
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })
})

function ItemsHarness({
    current,
    onRender,
}: {
    current: PullRequestActions
    onRender: (editor: ReviewDraftEditor, items: DraftItemsEditor) => void
}) {
    const editor = useReviewDraftEditor(REFERENCE, current)
    const items = useDraftItemsEditor(editor)
    useEffect(() => onRender(editor, items))
    return null
}

describe("the diff's item half of the editor", () => {
    test("keeps its identity across a summary keystroke and still writes through", async () => {
        const existing = placedInline("i1", "Kept")
        const desktop = installDesktop({ edit_review_draft: stored })
        const renders: { editor: ReviewDraftEditor; items: DraftItemsEditor }[] = []
        const container = dom.document.createElement("div")
        const root = createRoot(container as unknown as Element)
        const current = actions(state(draft([existing], { revision: 3, summary: "Summary" })))
        await act(async () =>
            root.render(
                <ItemsHarness
                    current={current}
                    onRender={(editor, items) => {
                        renders.push({ editor, items })
                    }}
                />,
            ),
        )
        try {
            const before = renders.at(-1)!
            await act(async () => before.editor.setSummary("Summary, typed further"))
            const typed = renders.at(-1)!
            expect(typed.editor.summary).toBe("Summary, typed further")
            expect(typed.editor).not.toBe(before.editor)
            // Every line slot, item card and reply affordance reads this half,
            // so a keystroke must leave it as it was.
            expect(typed.items).toBe(before.items)

            // Item state still reaches it.
            await act(async () => typed.items.beginPlacing({ kind: "inline", itemId: "i1" }))
            const placing = renders.at(-1)!
            expect(placing.items).not.toBe(typed.items)
            expect(placing.items.placing).toEqual({ kind: "inline", itemId: "i1" })
            await act(async () => placing.items.cancelPlacing())

            // A callback held from the first render calls the latest editor.
            const added = await run(() =>
                before.items.addInline({ path: "src/a.ts", side: "new", line: 22 }, "New"),
            )
            expect(added).toBe(true)
            expect(desktop.calls("edit_review_draft")).toHaveLength(1)
        } finally {
            await act(async () => root.unmount())
            desktop.uninstall()
        }
    })
})
