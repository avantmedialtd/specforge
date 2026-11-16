import { afterAll, beforeAll, beforeEach, expect, test } from "bun:test"
import { act, useState } from "react"
import {
    ReviewDraftEditorContext, type ReviewDraftEditor,
} from "../hooks/useReviewDraftEditor"
import { FakeElement, installFakeDom, type FakeDocument } from "../testing/fakeDom"
import type { ReviewDraft, ReviewActionState } from "../types"
import { PullRequestReviewSheet } from "./PullRequestReviewSheet"

const reference = { provider: "github" as const, owner: "acme", repo: "app", number: 7 }
const draft: ReviewDraft = {
    id: "draft-1", reference, revision: 1, basis: { head: "h", base: "b" },
    decision: "comment", summary: "Existing summary",
    items: [{ kind: "unplacedInline", id: "item-1", path: "removed.ts", side: "new",
        line: 2, body: "Keep this thought", basis: { head: "old", base: "b" } }],
}
const state: ReviewActionState = {
    reference, draft, blockedReason: null, operation: null, staleBasis: false,
    capabilities: [{ name: "comment", available: true, reason: null }],
}
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
function props(element: FakeElement): Record<string, unknown> {
    const key = Object.keys(element).find((key) => key.startsWith("__reactProps$"))!
    return (element as unknown as Record<string, Record<string, unknown>>)[key]
}
async function fire(element: FakeElement, name: string, event: unknown = {}) {
    await act(async () => {
        (props(element)[name] as (event: unknown) => void)(event)
    })
}
function button(root: FakeElement, label: string) {
    const found = root.findAll("button").find((element) => element.textContent.includes(label))
    if (!found) throw Error(`Missing button: ${label}`)
    return found
}
function baseEditor(): ReviewDraftEditor {
    return {
        state, draft, error: null, reload: async () => state,
        summary: draft.summary, decision: "comment", setSummary() {}, setDecision() {}, saveForm() {},
        notice: null, saving: false, submitting: false, submit() {}, editable: true,
        canInline: true, canReply: true, inlineUnavailable: null, replyUnavailable: null, atItemLimit: false,
        placing: null, beginPlacing() {}, cancelPlacing() {}, composing: null,
        beginComposing() {}, cancelComposing() {}, itemBusy: null, itemError: null,
        addInline: async () => true, addReply: async () => true, editItem: async () => true,
        removeItem: async () => true, moveInline: async () => true, moveReply: async () => true,
        actionBusy: null, recover: async () => true, startOver: async () => true,
        resume: async () => true, abandon: async () => true,
    }
}

test("the browser renders no review sheet control even with an editor in context", async () => {
    const container = dom.document.createElement("div")
    const root = createRoot(container as unknown as Element)
    try {
        await act(async () => root.render(
            <ReviewDraftEditorContext.Provider value={baseEditor()}>
                <PullRequestReviewSheet reference={reference} detail={null} progress={null} onJump={() => {}} />
            </ReviewDraftEditorContext.Provider>,
        ))
        expect(container.findAll("button")).toHaveLength(0)
        expect(dom.document.body.findAll("dialog")).toHaveLength(0)
    } finally {
        await act(async () => root.unmount())
    }
})

test("dismissal saves the shared editor and retains failed edits", async () => {
    ;(g.window as Record<string, unknown>).__TAURI_INTERNALS__ = {}
    let saves = 0
    let submissions = 0
    const container = dom.document.createElement("div")
    const root = createRoot(container as unknown as Element)
    function Host() {
        const [summary, setSummary] = useState(draft.summary)
        const [notice, setNotice] = useState<ReviewDraftEditor["notice"]>(null)
        const editor = {
            ...baseEditor(), summary, setSummary, notice,
            saveForm: () => { saves++; setNotice({ text: "Couldn't save — offline", failed: true }) },
            submit: () => { submissions++ },
        }
        return <ReviewDraftEditorContext.Provider value={editor}>
            <PullRequestReviewSheet reference={reference} detail={null} progress={null} onJump={() => {}} />
        </ReviewDraftEditorContext.Provider>
    }
    try {
        await act(async () => root.render(<Host />))
        expect(dom.document.body.findAll("dialog")).toHaveLength(0)
        await fire(button(container, "Finish review (1)"), "onClick")
        const dialog = dom.document.body.findAll("dialog")[0]!
        expect(dialog.open).toBe(true)
        expect(dialog.getAttribute("aria-labelledby")).toBe("pr-review-sheet-title")
        expect(dialog.findAll("textarea")[0]!.value).toBe(draft.summary)
        await fire(dialog.findAll("textarea")[0]!, "onChange", { currentTarget: { value: "Unsaved thought" } })
        await fire(button(dialog, "Close"), "onClick")
        expect(saves).toBe(1)
        expect(dom.document.body.findAll("dialog")).toHaveLength(0)
        expect(container.textContent).toContain("Save needs attention")
        await fire(button(container, "Finish review"), "onClick")
        const reopened = dom.document.body.findAll("dialog")[0]!
        expect(reopened.findAll("textarea")[0]!.value).toBe("Unsaved thought")
        expect(submissions).toBe(0) // An unplaced item still blocks eligibility.
        expect(button(reopened, "Submit review").hasAttribute("disabled")).toBe(true)
        await fire(reopened, "onKeyDown", { key: "Escape", preventDefault() {}, stopPropagation() {} })
        expect(saves).toBe(2)
    } finally {
        await act(async () => root.unmount())
        delete (g.window as Record<string, unknown>).__TAURI_INTERNALS__
    }
})

test("eligible submission delegates to the existing editor confirmation path", async () => {
    ;(g.window as Record<string, unknown>).__TAURI_INTERNALS__ = {}
    const container = dom.document.createElement("div")
    const root = createRoot(container as unknown as Element)
    const clean = { ...draft, items: [] }
    let submissions = 0
    try {
        const editor = { ...baseEditor(), draft: clean, state: { ...state, draft: clean },
            submit: () => { submissions++ } }
        await act(async () => root.render(
            <ReviewDraftEditorContext.Provider value={editor}>
                <PullRequestReviewSheet reference={reference} detail={null} progress={null} onJump={() => {}} />
            </ReviewDraftEditorContext.Provider>,
        ))
        await fire(button(container, "Finish review"), "onClick")
        const dialog = dom.document.body.findAll("dialog")[0]!
        expect(button(dialog, "Submit review").hasAttribute("disabled")).toBe(false)
        await fire(button(dialog, "Submit review"), "onClick")
        expect(submissions).toBe(1)
    } finally {
        await act(async () => root.unmount())
        delete (g.window as Record<string, unknown>).__TAURI_INTERNALS__
    }
})

test("unplaced item navigation closes the sheet and preserves the explicit placement destination", async () => {
    ;(g.window as Record<string, unknown>).__TAURI_INTERNALS__ = {}
    const jumps: unknown[] = []
    const container = dom.document.createElement("div")
    const root = createRoot(container as unknown as Element)
    let saves = 0
    try {
        await act(async () => root.render(
            <ReviewDraftEditorContext.Provider value={{ ...baseEditor(), saveForm: () => { saves++ } }}>
                <PullRequestReviewSheet reference={reference} detail={null} progress={null} onJump={(jump) => jumps.push(jump)} />
            </ReviewDraftEditorContext.Provider>,
        ))
        await fire(button(container, "Finish review"), "onClick")
        await fire(button(dom.document.body, "Choose placement"), "onClick")
        expect(jumps).toEqual([{ kind: "draft", id: "item-1", path: "removed.ts" }])
        expect(saves).toBe(1)
        expect(dom.document.body.findAll("dialog")).toHaveLength(0)
    } finally {
        await act(async () => root.unmount())
        delete (g.window as Record<string, unknown>).__TAURI_INTERNALS__
    }
})

test("Tab wraps between actual tab stops, excluding unselected roving radio choices", async () => {
    ;(g.window as Record<string, unknown>).__TAURI_INTERNALS__ = {}
    const container = dom.document.createElement("div")
    const root = createRoot(container as unknown as Element)
    try {
        await act(async () => root.render(
            <ReviewDraftEditorContext.Provider value={baseEditor()}>
                <PullRequestReviewSheet reference={reference} detail={null} progress={null} onJump={() => {}} />
            </ReviewDraftEditorContext.Provider>,
        ))
        await fire(button(container, "Finish review"), "onClick")
        const dialog = dom.document.body.findAll("dialog")[0]!
        const first = Object.assign(dom.document.createElement("button"), { tabIndex: 0, getClientRects: () => [1] })
        const last = Object.assign(dom.document.createElement("button"), { tabIndex: 0, getClientRects: () => [1] })
        const radio = Object.assign(dom.document.createElement("button"), { tabIndex: -1, getClientRects: () => [1] })
        const currentTarget = { querySelectorAll: () => [first, last, radio] }
        let prevented = 0
        last.focus()
        await fire(dialog, "onKeyDown", {
            key: "Tab", currentTarget, preventDefault() { prevented++ },
        })
        expect(dom.document.activeElement).toBe(first)
        await fire(dialog, "onKeyDown", {
            key: "Tab", shiftKey: true, currentTarget, preventDefault() { prevented++ },
        })
        expect(dom.document.activeElement).toBe(last)
        expect(prevented).toBe(2)
    } finally {
        await act(async () => root.unmount())
        delete (g.window as Record<string, unknown>).__TAURI_INTERNALS__
    }
})
