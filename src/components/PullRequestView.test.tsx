import { afterAll, beforeAll, beforeEach, describe, expect, test } from "bun:test"
import { act } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { FakeElement, FakeNode, installFakeDom, type FakeDocument } from "../testing/fakeDom"
import { minimisedText } from "../pullRequestView"
import { MAX_BODY_BYTES } from "../reviewActions"
import { ReviewDraftEditorContext, type ReviewDraftEditor } from "../hooks/useReviewDraftEditor"
import { DIFF_LAYOUT_STORAGE_KEY } from "../diffLayout"
import type { PullRequestAddress } from "../routing/address"
import type { PullRequestResolution } from "../routing/resolve"
import type {
    ArtifactStatus,
    ChangeData,
    ChangeInstance,
    DiffFile,
    DraftItem,
    FileReviewProgress,
    Hunk,
    Line,
    PlacedInlineItem,
    PlacedReplyItem,
    PullRequestComment,
    PullRequestDetail,
    PullRequestLinks,
    PullRequestReference,
    PullRequestSummary,
    ReviewDraft,
    ReviewProgress,
    ReviewThread,
    UnplacedInlineItem,
    UnplacedReplyItem,
    WorkspaceView,
} from "../types"
import { PullRequestAtAddress, PullRequestFiles, type PaneNavigation } from "./PullRequestView"

// The view's presentations, server-rendered. Effects do not run on the server,
// so nothing here sends a command: what is pinned is what each outcome renders
// in the center pane and in the pull-request window before any read has
// answered (`pull-request-viewer`: *Pull-Request View*, *Pull-Request Window*,
// *Linked Change in the Pull-Request View*; `view-routing`: *Pull-Request
// Addresses*). The decisions themselves are tested in `pullRequestView.test.ts`.
// The browser skin is the host here, as `isWeb()` reads it without a window.

const ADDRESS: PullRequestAddress = {
    kind: "pullRequest",
    provider: "github",
    owner: "acme",
    repo: "api",
    number: 42,
}

const URL_42 = "https://github.com/acme/api/pull/42"

function row(overrides: Partial<PullRequestSummary> = {}): PullRequestSummary {
    return {
        id: 42,
        title: "Add rate limits",
        repoFullName: "acme/api",
        sourceRepoFullName: "acme/api",
        sourceBranch: "rate-limits",
        destinationBranch: "main",
        url: URL_42,
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

function listed(pr: PullRequestSummary = row()): PullRequestResolution {
    return { status: "listed", row: pr, address: ADDRESS }
}

const PANE: PaneNavigation = { openWorktree: () => {}, openSettings: () => {} }

function render(
    resolution: PullRequestResolution,
    options: { pane?: PaneNavigation; views?: WorkspaceView[]; links?: PullRequestLinks } = {},
): string {
    return renderToStaticMarkup(
        <PullRequestAtAddress
            address={ADDRESS}
            resolution={resolution}
            views={options.views ?? []}
            links={options.links ?? null}
            pane={options.pane}
        />,
    )
}

describe("an address that resolves to no pull request", () => {
    test("pending shows Loading… and nothing else", () => {
        expect(render({ status: "pending" })).toBe('<div class="detail-pane-status">Loading…</div>')
    })

    test("the provider-off notice names its provider, with a way to Settings in the center pane", () => {
        const html = render({ status: "providerOff" }, { pane: PANE })
        expect(html).toContain("GitHub is switched off")
        expect(html).toContain("<button")
        expect(html).toContain("Open Settings › Integrations")
    })

    // *The window's notices do not link to Settings*.
    test("the window names Settings › Integrations as text, with no control that navigates", () => {
        for (const resolution of [
            { status: "providerOff" },
            { status: "unavailable", reason: "unauthenticated" },
        ] as PullRequestResolution[]) {
            const html = render(resolution)
            expect(html).toContain("Settings › Integrations")
            expect(html).not.toContain("<button")
            expect(html).not.toContain("<a")
        }
    })

    test("an unavailable list says so, and points nowhere", () => {
        const html = render({ status: "unavailable", reason: "unavailable" }, { pane: PANE })
        expect(html).toContain("GitHub is unavailable")
        expect(html).not.toContain("Settings")
    })
})

describe("a listed pull request, before its first read answers", () => {
    test("the header names it and its branches, and Loading… holds the detail's place", () => {
        const html = render(listed(), { pane: PANE })
        expect(html).toContain('<span class="pull-request-view-number">#42</span>')
        expect(html).toContain("Add rate limits")
        expect(html).toContain("rate-limits")
        expect(html).toContain('<div class="detail-pane-status">Loading…</div>')
    })

    // *The header carries the row's signals*.
    test("the header carries the row's failing checks and conflict marker", () => {
        const html = render(listed(row({ checks: "failing", conflicting: true })), { pane: PANE })
        expect(html).toContain("pull-request-checks--failing")
        expect(html).toContain("Conflicts")
    })

    // *Hidden characters in the header are visible*.
    test("a zero-width space in a branch name shows as a marked escape", () => {
        const html = render(listed(row({ sourceBranch: "rate​limits" })), { pane: PANE })
        expect(html).not.toContain("​")
        expect(html).toContain('data-code-point="200b"')
        expect(html).toContain("U+200B")
    })

    // *Only the center pane offers the pop-out control*.
    test("the center pane offers the pop-out control, and the window does not", () => {
        expect(render(listed(), { pane: PANE })).toContain('aria-label="Open in its own window"')
        expect(render(listed())).not.toContain("Open in its own window")
    })

    // *The provider's page in the browser skin*.
    test("both carry the provider's page, as an opener-isolated link in the browser skin", () => {
        for (const html of [render(listed(), { pane: PANE }), render(listed())]) {
            expect(html).toContain(
                `<a class="pull-request-view-action" href="${URL_42}" target="_blank" rel="noopener noreferrer">Open on GitHub</a>`,
            )
        }
    })

    // *Nothing can be posted*.
    test("no control comments, approves, requests changes or merges", () => {
        const html = render(listed(), { pane: PANE }).toLowerCase()
        for (const action of ["approve", "merge", "request changes", "reply", "comment"]) {
            expect(html).not.toContain(`>${action}`)
        }
        // The native composer is not part of the browser skin's markup.
        expect(html).not.toContain("pull-request-composer")
        expect(html).not.toContain("<textarea")
    })
})

describe("the linked change", () => {
    const REPO = "/code/api/.git"
    const WT = "/code/api-wt"
    const PROPOSAL_ONLY: ArtifactStatus = { proposal: true, design: false, tasks: false, specs: [] }
    const views = [
        {
            kind: "repo",
            repoId: REPO,
            mainWorktree: "/code/api",
            name: "api",
            defaultBranch: "main",
            active: [
                {
                    name: "add-rate-limits",
                    instances: [
                        {
                            worktreePath: WT,
                            branch: "rate-limits",
                            change: {
                                artifacts: PROPOSAL_ONLY,
                                completedTasks: 0,
                                totalTasks: 0,
                            } as unknown as ChangeData,
                            modifiedAt: 100,
                        } as unknown as ChangeInstance,
                    ],
                },
            ],
            displayName: null,
            color: null,
            worktrees: ["/code/api", WT],
        } as unknown as WorkspaceView,
    ]
    const links: PullRequestLinks = {
        worktrees: [],
        pullRequests: [
            { url: URL_42, worktrees: [{ repoId: REPO, worktreePath: WT, branch: "rate-limits" }] },
        ],
    }

    // *The center pane leads to the linked change*.
    test("is a control in the center pane", () => {
        expect(render(listed(), { pane: PANE, views, links })).toContain(
            '<button type="button" class="pull-request-view-linked-name pull-request-view-linked-name--control">add-rate-limits</button>',
        )
    })

    // *The window names the change passively*.
    test("is passive text in the window", () => {
        const html = render(listed(), { views, links })
        expect(html).toContain('<span class="pull-request-view-linked-name">add-rate-limits</span>')
        expect(html).not.toContain("pull-request-view-linked-name--control")
    })

    test("is named for no unlinked pull request", () => {
        expect(render(listed(), { pane: PANE, views })).not.toContain("pull-request-view-linked")
    })
})

// -------------------------------------------------------------------------
// Review thread placement, rendered
// -------------------------------------------------------------------------

// `pull-request-viewer`: *Changed Files in the Pull-Request View*; design D11.
// The files section is mounted for real, through `react-dom/client` on the
// fake DOM, so the slots `buildFileSlots` builds and `DiffView`'s line slots
// take part.

let dom: { document: FakeDocument; uninstall: () => void }
let createRoot: typeof import("react-dom/client").createRoot
const g = globalThis as unknown as Record<string, unknown>

/// The widths `measure` reads; the `ch` probe is 1000px wide, so one `ch` is
/// ten layout pixels and the sections column is 200ch.
const widths = { sections: 2000 }

class FakeResizeObserver {
    static readonly all: FakeResizeObserver[] = []
    constructor(readonly callback: () => void) {
        FakeResizeObserver.all.push(this)
    }
    observe() {}
    unobserve() {}
    disconnect() {}
    trigger() {
        this.callback()
    }
}

function widthOf(element: FakeElement): number {
    const className = element.getAttribute("class") ?? ""
    if (className.includes("diff-view-probe-ch")) return 1000
    if (className.includes("diff-view-probe-navigator")) return 300
    if (className.includes("diff-sections")) return widths.sections
    if (className.includes("diff-view-body")) return widths.sections + 300
    return widths.sections
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

function classesOf(element: FakeElement): string[] {
    return (element.getAttribute("class") ?? "").split(/\s+/).filter(Boolean)
}

function byClass(root: FakeElement, className: string): FakeElement[] {
    return allElements(root).filter((element) => classesOf(element).includes(className))
}

/// The attribute selectors the navigator uses; no tag, class or descendant
/// selector is needed by the view's mount path.
function matchesSelector(element: FakeElement, selector: string): boolean {
    return selector
        .split(/(?=\[)/)
        .filter((part) => part !== "")
        .every((part) => {
            const match = /^\[([^=\]]+)(?:="([^"]*)")?\]$/.exec(part)
            if (!match) return false
            const actual = element.getAttribute(match[1])
            return match[2] === undefined ? actual !== null : actual === match[2]
        })
}

const REFERENCE: PullRequestReference = {
    provider: "github",
    owner: "acme",
    repo: "api",
    number: 42,
}

function line(kind: Line["kind"], oldNo: number | null, newNo: number | null): Line {
    return { kind, oldNo, newNo, text: `${kind} line` }
}

/// A modified file: old 20/21 face new 20/22. The context line carries both
/// identities, the removed line only its old one, the added line only its new
/// one.
function hunk(): Hunk {
    return {
        oldStart: 20,
        oldLines: 2,
        newStart: 20,
        newLines: 2,
        section: null,
        lines: [
            line("context", 20, 20),
            line("removed", 21, null),
            line("added", null, 22),
        ],
    }
}

function changedFile(path: string, overrides: Partial<DiffFile> = {}): DiffFile {
    return {
        oldPath: path,
        newPath: path,
        oldMode: "100644",
        newMode: "100644",
        status: { kind: "modified" },
        additions: 1,
        deletions: 1,
        content: { kind: "hunks", hunks: [hunk()] },
        ...overrides,
    }
}

function comment(id: string, overrides: Partial<PullRequestComment> = {}): PullRequestComment {
    return {
        id,
        author: "ada",
        body: `Body of ${id}`,
        postedAtUnix: 1_000,
        url: null,
        minimizedReason: null,
        deleted: false,
        ...overrides,
    }
}

function thread(id: string, overrides: Partial<ReviewThread> = {}): ReviewThread {
    return {
        id,
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
        comments: [comment(`${id}-1`)],
        ...overrides,
    }
}

function draftWith(items: DraftItem[], overrides: Partial<ReviewDraft> = {}): ReviewDraft {
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

function placedInlineItem(
    id: string,
    path: string,
    side: "old" | "new",
    line: number,
): PlacedInlineItem {
    return { kind: "placedInline", id, body: `Draft ${id}`, path, side, line }
}

function unplacedInlineItem(
    id: string,
    path: string,
    side: "old" | "new",
    line: number,
): UnplacedInlineItem {
    return {
        kind: "unplacedInline",
        id,
        body: `Former ${id}`,
        path,
        side,
        line,
        basis: { head: "old-head", base: "old-base" },
    }
}

function placedReplyItem(id: string, target: string): PlacedReplyItem {
    return { kind: "placedReply", id, body: `Reply ${id}`, target }
}

function unplacedReplyItem(id: string, target: string): UnplacedReplyItem {
    return { kind: "unplacedReply", id, body: `Former reply ${id}`, target }
}

function fakeEditor(overrides: Partial<ReviewDraftEditor> = {}): ReviewDraftEditor {
    const refuse = () => Promise.resolve(false)
    return {
        state: null,
        error: null,
        reload: () => Promise.resolve(null),
        draft: null,
        summary: "",
        decision: "comment",
        setSummary: () => {},
        setDecision: () => {},
        saveForm: () => {},
        notice: null,
        saving: false,
        submitting: false,
        submit: () => {},
        editable: true,
        canInline: true,
        canReply: true,
        inlineUnavailable: null,
        replyUnavailable: null,
        atItemLimit: false,
        placing: null,
        beginPlacing: () => {},
        cancelPlacing: () => {},
        itemBusy: null,
        itemError: null,
        addInline: refuse,
        addReply: refuse,
        editItem: refuse,
        removeItem: refuse,
        moveInline: refuse,
        moveReply: refuse,
        actionBusy: null,
        recover: refuse,
        startOver: refuse,
        resume: refuse,
        abandon: refuse,
        ...overrides,
    }
}

function detailWith(
    threads: ReviewThread[],
    files: DiffFile[] = [changedFile("src/a.ts"), changedFile("src/b.ts")],
): PullRequestDetail {
    return {
        reference: REFERENCE,
        row: row(),
        headBranch: "rate-limits",
        baseBranch: "main",
        headCommit: "head1",
        baseCommit: "base1",
        author: "ada",
        description: "",
        conversation: [],
        checks: [],
        threads,
        files,
        unlistedFiles: 0,
        readAtUnix: 1_000,
        noLongerListed: false,
    }
}

function progressFor(files: FileReviewProgress[]): ReviewProgress {
    return {
        files,
        viewed: 0,
        changedSinceViewed: 0,
        skipped: 0,
        total: files.length,
        lastMarkedHead: null,
        headCommit: "head1",
        baseCommit: "base1",
    }
}

function viewedHunk(path: string): FileReviewProgress {
    return {
        path,
        state: "partlyViewed",
        keyedByHead: false,
        hunks: [true],
        matched: null,
        included: false,
    }
}

async function mountFiles(
    detail: PullRequestDetail,
    progress: ReviewProgress | null = null,
    editor: ReviewDraftEditor | null = null,
) {
    const container = dom.document.createElement("div")
    const root = createRoot(container as unknown as Element)
    const view = (nextDetail: PullRequestDetail, nextProgress: ReviewProgress | null) => {
        const files = (
            <PullRequestFiles
                detail={nextDetail}
                reference={REFERENCE}
                progress={nextProgress}
                reloadProgress={() => Promise.resolve(nextProgress)}
                onFileRefused={() => {}}
            />
        )
        return editor === null ? (
            files
        ) : (
            <ReviewDraftEditorContext.Provider value={editor}>
                {files}
            </ReviewDraftEditorContext.Provider>
        )
    }
    await act(async () => root.render(view(detail, progress)))
    return {
        container,
        async rerender(nextDetail: PullRequestDetail, nextProgress: ReviewProgress | null) {
            await act(async () => root.render(view(nextDetail, nextProgress)))
        },
        async unmount() {
            await act(async () => root.unmount())
        },
    }
}

function sectionOf(container: FakeElement, key: string): FakeElement {
    const found = allElements(container).find(
        (element) =>
            classesOf(element).includes("diff-file") &&
            element.getAttribute("data-file-key") === key,
    )
    if (found === undefined) throw new Error(`no section for ${key}`)
    return found
}

function slotLabels(section: FakeElement): string[] {
    return byClass(section, "diff-line-slot").map(
        (element) => element.getAttribute("aria-label") ?? "",
    )
}

function slotTexts(section: FakeElement): string[] {
    return byClass(section, "diff-line-slot").map((element) => element.textContent)
}

describe("review threads render where their line is", () => {
    beforeAll(async () => {
        dom = installFakeDom()
        g.IS_REACT_ACT_ENVIRONMENT = true
        createRoot = (await import("react-dom/client")).createRoot

        Object.defineProperty(FakeElement.prototype, "offsetWidth", {
            configurable: true,
            get(this: FakeElement) {
                return widthOf(this)
            },
        })
        Object.defineProperty(FakeElement.prototype, "clientWidth", {
            configurable: true,
            get(this: FakeElement) {
                return widthOf(this)
            },
        })
        Object.defineProperty(FakeElement.prototype, "dataset", {
            configurable: true,
            get(this: FakeElement) {
                return new Proxy(
                    {},
                    {
                        get: (_target, property: string) => {
                            const name = `data-${property.replace(
                                /[A-Z]/g,
                                (letter) => `-${letter.toLowerCase()}`,
                            )}`
                            return this.getAttribute(name) ?? undefined
                        },
                    },
                )
            },
        })
        const prototype = FakeElement.prototype as FakeElement & {
            toggleAttribute(name: string, force?: boolean): boolean
            querySelectorAll(selector: string): FakeElement[]
            querySelector(selector: string): FakeElement | null
        }
        prototype.toggleAttribute = function (name, force) {
            const next = force === undefined ? !this.hasAttribute(name) : force
            if (next) this.setAttribute(name, "")
            else this.removeAttribute(name)
            return next
        }
        prototype.querySelectorAll = function (selector) {
            return allElements(this).filter((element) => matchesSelector(element, selector))
        }
        prototype.querySelector = function (selector) {
            return this.querySelectorAll(selector)[0] ?? null
        }
        g.ResizeObserver = FakeResizeObserver
    })

    afterAll(() => {
        delete g.IS_REACT_ACT_ENVIRONMENT
        delete g.ResizeObserver
        delete g.localStorage
        for (const name of [
            "offsetWidth",
            "clientWidth",
            "dataset",
            "toggleAttribute",
            "querySelectorAll",
            "querySelector",
        ]) {
            Reflect.deleteProperty(FakeElement.prototype, name)
        }
        dom.uninstall()
    })

    beforeEach(() => {
        widths.sections = 2000
        FakeResizeObserver.all.length = 0
        g.localStorage = { getItem: () => null, setItem: () => {} }
    })

    test("a current thread and its reply sit on their line, each side apart", async () => {
        const parent = thread("parent", {
            comments: [comment("parent-1", { body: "This needs a guard." })],
        })
        const reply = thread("reply", {
            comments: [comment("reply-1", { body: "Added one." })],
        })
        const oldSide = thread("old", { side: "old", line: 21 })
        const mounted = await mountFiles(detailWith([parent, reply, oldSide]))
        try {
            const a = sectionOf(mounted.container, "src/a.ts")
            expect(slotLabels(a)).toEqual(["old line 21", "new line 22"])
            expect(byClass(a, "diff-file-preamble")).toHaveLength(0)
            const [oldSlot, newSlot] = slotTexts(a)
            expect(oldSlot).toContain("src/a.ts")
            expect(oldSlot).toContain("old line 21")
            expect(oldSlot).toContain("Body of old-1")
            expect(newSlot).toContain("new line 22")
            expect(newSlot).toContain("This needs a guard.")
            expect(newSlot).toContain("Added one.")
            expect(byClass(a, "pull-request-view-thread")).toHaveLength(3)
        } finally {
            await mounted.unmount()
        }
    })

    test("ranged, outdated and missing-line threads keep the file's preamble", async () => {
        const ranged = thread("ranged", { startSide: "new", startLine: 20 })
        const outdated = thread("outdated", { outdated: true, originalLine: 22 })
        const missing = thread("missing", { line: 99 })
        const mounted = await mountFiles(detailWith([ranged, outdated, missing]))
        try {
            const a = sectionOf(mounted.container, "src/a.ts")
            expect(byClass(a, "diff-line-slot")).toHaveLength(0)
            const preamble = byClass(a, "diff-file-preamble")[0]
            expect(preamble).toBeDefined()
            expect(preamble!.textContent).toContain("new lines 20–22")
            expect(preamble!.textContent).toContain("originally new line 22")
            expect(preamble!.textContent).toContain("new line 99")
            expect(byClass(preamble!, "pull-request-view-thread")).toHaveLength(3)
        } finally {
            await mounted.unmount()
        }
    })

    test("a folded hunk keeps its threads in the preamble", async () => {
        const onLine = thread("folded")
        const mounted = await mountFiles(
            detailWith([onLine]),
            progressFor([viewedHunk("src/a.ts")]),
        )
        try {
            const a = sectionOf(mounted.container, "src/a.ts")
            expect(byClass(a, "diff-hunk-fold")[0]?.textContent).toBe("Show 3 lines")
            expect(byClass(a, "diff-line-slot")).toHaveLength(0)
            expect(byClass(a, "diff-file-preamble")[0]?.textContent).toContain("new line 22")
        } finally {
            await mounted.unmount()
        }
    })

    test("a withheld file's thread waits in its preamble", async () => {
        const withheld = changedFile("src/big.ts", { content: { kind: "withheld" } })
        const waiting = thread("waiting", { path: "src/big.ts", line: 5 })
        const mounted = await mountFiles(detailWith([waiting], [withheld]))
        try {
            const big = sectionOf(mounted.container, "src/big.ts")
            expect(byClass(big, "diff-line-slot")).toHaveLength(0)
            expect(byClass(big, "diff-file-preamble")[0]?.textContent).toContain("new line 5")
        } finally {
            await mounted.unmount()
        }
    })

    test("a minimised comment stays collapsed on its line, in the preamble and after the files", async () => {
        const onLine = thread("line", {
            path: "src/b.ts",
            comments: [comment("line-1", { body: "On the line.", minimizedReason: "spam" })],
        })
        const inPreamble = thread("preamble", {
            startSide: "new",
            startLine: 20,
            comments: [
                comment("pre-1", { body: "In the preamble.", minimizedReason: "outdated" }),
            ],
        })
        const afterFiles = thread("unlisted", {
            path: "src/gone.ts",
            line: 3,
            comments: [comment("un-1", { body: "After the files.", minimizedReason: "" })],
        })
        const mounted = await mountFiles(detailWith([onLine, inPreamble, afterFiles]))
        try {
            const b = sectionOf(mounted.container, "src/b.ts")
            const onLineSlot = byClass(b, "diff-line-slot")[0]!
            const preamble = byClass(sectionOf(mounted.container, "src/a.ts"), "diff-file-preamble")[0]!
            const unlisted = byClass(mounted.container, "pull-request-view-unlisted-threads")[0]!
            const folded = [
                [onLineSlot, minimisedText("spam"), "On the line."],
                [preamble, minimisedText("outdated"), "In the preamble."],
                [unlisted, minimisedText(""), "After the files."],
            ] as const
            for (const [location, summary, body] of folded) {
                const details = byClass(location, "pull-request-view-minimised")
                expect(details).toHaveLength(1)
                expect(details[0]!.textContent).toContain(summary)
                expect(details[0]!.textContent).toContain(body)
                expect(details[0]!.hasAttribute("open")).toBe(false)
            }
        } finally {
            await mounted.unmount()
        }
    })

    test("a thread on a file not listed follows the diff", async () => {
        const unlisted = thread("gone", { path: "src/gone.ts", line: 3 })
        const mounted = await mountFiles(detailWith([unlisted]))
        try {
            const elements = allElements(mounted.container)
            const diff = elements.find((element) => classesOf(element).includes("diff-view"))
            const section = elements.find((element) =>
                classesOf(element).includes("pull-request-view-unlisted-threads"),
            )
            expect(diff).toBeDefined()
            expect(section).toBeDefined()
            expect(elements.indexOf(section!)).toBeGreaterThan(elements.indexOf(diff!))
            expect(section!.textContent).toContain("src/gone.ts")
            expect(section!.textContent).toContain("new line 3")
            expect(section!.textContent).toContain("Body of gone-1")
        } finally {
            await mounted.unmount()
        }
    })


// -------------------------------------------------------------------------
// Draft comments, replies and line affordances
// -------------------------------------------------------------------------

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

async function press(element: FakeElement): Promise<void> {
    const onClick = reactProps(element).onClick
    if (typeof onClick !== "function") throw new Error("the element has no click handler")
    await act(async () => {
        ;(onClick as (event: unknown) => void)({ currentTarget: element, target: element })
    })
}

async function type(element: FakeElement, value: string): Promise<void> {
    const onChange = reactProps(element).onChange
    if (typeof onChange !== "function") throw new Error("the element has no change handler")
    await act(async () => {
        ;(onChange as (event: unknown) => void)({ currentTarget: { value } })
    })
}

function named(root: FakeElement, label: string): FakeElement | undefined {
    return allElements(root).find(
        (element) => element.getAttribute("aria-label") === label,
    )
}

function slotCellIndex(section: FakeElement, label: string): number {
    const slot = byClass(section, "diff-line-slot").find(
        (element) => element.getAttribute("aria-label") === label,
    )
    if (slot === undefined) throw new Error(`no slot for ${label}`)
    const cell = slot.parentNode as FakeElement
    const row = cell.parentNode as FakeElement
    const cells = row.childNodes.filter(
        (node): node is FakeElement =>
            node instanceof FakeElement && classesOf(node).includes("diff-line-slot-cell"),
    )
    return cells.indexOf(cell)
}

function useSplitLayout(): void {
    g.localStorage = {
        getItem: (key: string) => (key === DIFF_LAYOUT_STORAGE_KEY ? "split" : null),
        setItem: () => {},
    }
}

describe("draft comments and replies render where their anchor is", () => {
    test("a placed draft comment sits in its new-side line slot, unified", async () => {
        const item = placedInlineItem("d1", "src/a.ts", "new", 22)
        const editor = fakeEditor({ draft: draftWith([item]), canInline: false })
        const mounted = await mountFiles(
            detailWith([], [changedFile("src/a.ts")]),
            null,
            editor,
        )
        try {
            const a = sectionOf(mounted.container, "src/a.ts")
            expect(slotLabels(a)).toEqual(["new line 22"])
            expect(slotTexts(a)[0]).toContain("Draft comment on new line 22 of src/a.ts")
            expect(slotTexts(a)[0]).toContain("Draft d1")
            expect(byClass(a, "pull-request-view-draft-item")).toHaveLength(1)
            expect(byClass(a, "diff-file-preamble")).toHaveLength(0)
        } finally {
            await mounted.unmount()
        }
    })

    test("a removed line's draft sits under the old half, a context line's sides each under theirs", async () => {
        useSplitLayout()
        const removed = placedInlineItem("removed", "src/a.ts", "old", 21)
        const oldContext = placedInlineItem("old-context", "src/a.ts", "old", 20)
        const newContext = placedInlineItem("new-context", "src/a.ts", "new", 20)
        const editor = fakeEditor({
            draft: draftWith([removed, oldContext, newContext]),
            canInline: false,
        })
        const mounted = await mountFiles(
            detailWith([], [changedFile("src/a.ts")]),
            null,
            editor,
        )
        try {
            expect(byClass(mounted.container, "diff-split")).toHaveLength(1)
            const a = sectionOf(mounted.container, "src/a.ts")
            expect(slotLabels(a)).toEqual(["old line 20", "new line 20", "old line 21"])
            expect(slotCellIndex(a, "old line 20")).toBe(0)
            expect(slotCellIndex(a, "new line 20")).toBe(1)
            expect(slotCellIndex(a, "old line 21")).toBe(0)
        } finally {
            await mounted.unmount()
        }
    })

    test("every rendered line carries an accessible comment affordance, unified and split", async () => {
        const editor = fakeEditor()
        const detail = detailWith([], [changedFile("src/a.ts")])
        const unified = await mountFiles(detail, null, editor)
        try {
            const a = sectionOf(unified.container, "src/a.ts")
            expect(
                byClass(a, "pull-request-composer-line-action").map(
                    (button) => button.getAttribute("aria-label"),
                ),
            ).toEqual([
                "Comment on old line 20 of src/a.ts",
                "Comment on new line 20 of src/a.ts",
                "Comment on old line 21 of src/a.ts",
                "Comment on new line 22 of src/a.ts",
            ])
        } finally {
            await unified.unmount()
        }

        useSplitLayout()
        const split = await mountFiles(detail, null, editor)
        try {
            const a = sectionOf(split.container, "src/a.ts")
            expect(byClass(a, "pull-request-composer-line-action")).toHaveLength(4)
            expect(slotCellIndex(a, "old line 21")).toBe(0)
            expect(slotCellIndex(a, "new line 22")).toBe(1)
        } finally {
            await split.unmount()
        }
    })

    test("a filler half gets no affordance, and neither does an absent line", async () => {
        useSplitLayout()
        const withFiller = changedFile("src/a.ts", {
            additions: 1,
            deletions: 0,
            content: {
                kind: "hunks",
                hunks: [
                    {
                        oldStart: 10,
                        oldLines: 1,
                        newStart: 10,
                        newLines: 2,
                        section: null,
                        lines: [line("context", 10, 10), line("added", null, 11)],
                    },
                ],
            },
        })
        const editor = fakeEditor()
        const mounted = await mountFiles(detailWith([], [withFiller]), null, editor)
        try {
            const a = sectionOf(mounted.container, "src/a.ts")
            expect(slotLabels(a)).toEqual(["old line 10", "new line 10", "new line 11"])
            expect(byClass(a, "pull-request-composer-line-action")).toHaveLength(3)
            expect(slotCellIndex(a, "new line 11")).toBe(1)
            expect(byClass(a, "diff-line-slot-cell--empty").length).toBeGreaterThan(0)
        } finally {
            await mounted.unmount()
        }
    })

    test("a reply renders with its target thread and only replyable threads offer one", async () => {
        const replyable = thread("t1")
        const closed = thread("t2", { replyable: false })
        const editor = fakeEditor({
            draft: draftWith([placedReplyItem("r1", "t1"), placedReplyItem("r2", "t2")]),
            canInline: false,
        })
        const mounted = await mountFiles(
            detailWith([replyable, closed], [changedFile("src/a.ts")]),
            null,
            editor,
        )
        try {
            const a = sectionOf(mounted.container, "src/a.ts")
            const text = slotTexts(a)[0]!
            expect(text).toContain("Your reply")
            expect(text).toContain("Reply r1")
            expect(text).toContain("Reply r2")
            const replies = allElements(a).filter(
                (element) =>
                    element.getAttribute("aria-label") ===
                    "Reply to thread on src/a.ts, new line 22",
            )
            expect(replies).toHaveLength(1)
        } finally {
            await mounted.unmount()
        }
    })

    test("an unplaced comment keeps its old locator in its former renamed file's preamble", async () => {
        const renamed = changedFile("src/new.ts", {
            oldPath: "src/old.ts",
            status: { kind: "renamed", similarity: null },
        })
        const former = unplacedInlineItem("u1", "src/old.ts", "new", 22)
        const editor = fakeEditor({ draft: draftWith([former]), canInline: false })
        const mounted = await mountFiles(detailWith([], [renamed]), null, editor)
        try {
            const file = sectionOf(mounted.container, "src/new.ts")
            expect(byClass(file, "diff-line-slot")).toHaveLength(0)
            const preamble = byClass(file, "diff-file-preamble")[0]!
            expect(preamble.textContent).toContain(
                "Former comment on new line 22 of src/old.ts",
            )
            expect(preamble.textContent).toContain("Former u1")
            expect(preamble.textContent).toContain("old-base…old-head")
            expect(named(preamble, "Former comment on new line 22 of src/old.ts")).toBeDefined()
            expect(preamble.textContent).toContain("Choose a current line")
        } finally {
            await mounted.unmount()
        }
    })

    test("unplaced inline and former reply prose with no listed former file follows the files", async () => {
        const formerComment = unplacedInlineItem("u1", "src/gone.ts", "old", 5)
        const formerReply = unplacedReplyItem("u2", "gone-thread")
        const editor = fakeEditor({
            draft: draftWith([formerComment, formerReply]),
            canInline: false,
        })
        const mounted = await mountFiles(
            detailWith([], [changedFile("src/a.ts")]),
            null,
            editor,
        )
        try {
            expect(byClass(mounted.container, "diff-line-slot")).toHaveLength(0)
            const after = byClass(mounted.container, "pull-request-view-detached-drafts")[0]!
            expect(after.textContent).toContain("Draft comments to place")
            expect(after.textContent).toContain("Former comment on old line 5 of src/gone.ts")
            expect(after.textContent).toContain("Former u1")
            expect(after.textContent).toContain("Draft replies without a thread")
            expect(after.textContent).toContain("Former reply to thread gone-thread")
            expect(after.textContent).toContain("Former reply u2")
            expect(after.textContent).toContain("Choose a replyable thread")
        } finally {
            await mounted.unmount()
        }
    })

    test("a placed reply whose thread has left this detail follows the files", async () => {
        const orphan = placedReplyItem("r1", "gone-thread")
        const editor = fakeEditor({ draft: draftWith([orphan]) })
        const mounted = await mountFiles(
            detailWith([], [changedFile("src/a.ts")]),
            null,
            editor,
        )
        try {
            const after = byClass(mounted.container, "pull-request-view-detached-drafts")[0]!
            expect(after.textContent).toContain("Draft replies whose thread is not here")
            expect(after.textContent).toContain("Reply r1")
            expect(after.textContent).toContain("gone-thread")
        } finally {
            await mounted.unmount()
        }
    })

    test("a placed comment on an absent line waits in the file's preamble", async () => {
        const absent = placedInlineItem("d1", "src/a.ts", "new", 99)
        const editor = fakeEditor({ draft: draftWith([absent]), canInline: false })
        const mounted = await mountFiles(
            detailWith([], [changedFile("src/a.ts")]),
            null,
            editor,
        )
        try {
            const a = sectionOf(mounted.container, "src/a.ts")
            expect(byClass(a, "diff-line-slot")).toHaveLength(0)
            expect(byClass(a, "diff-file-preamble")[0]?.textContent).toContain(
                "Draft comment on new line 99 of src/a.ts",
            )
        } finally {
            await mounted.unmount()
        }
    })

    test("a folded hunk keeps its draft comment in the file's preamble", async () => {
        const folded = placedInlineItem("d1", "src/a.ts", "new", 22)
        const editor = fakeEditor({ draft: draftWith([folded]), canInline: false })
        const mounted = await mountFiles(
            detailWith([], [changedFile("src/a.ts")]),
            progressFor([viewedHunk("src/a.ts")]),
            editor,
        )
        try {
            const a = sectionOf(mounted.container, "src/a.ts")
            expect(byClass(a, "diff-hunk-fold")[0]?.textContent).toBe("Show 3 lines")
            expect(byClass(a, "diff-line-slot")).toHaveLength(0)
            expect(byClass(a, "diff-file-preamble")[0]?.textContent).toContain(
                "Draft comment on new line 22 of src/a.ts",
            )
        } finally {
            await mounted.unmount()
        }
    })

    test("the line editor refuses an empty or over-30-KiB body before asking the service", async () => {
        const added: { anchor: unknown; body: string }[] = []
        const editor = fakeEditor({
            addInline: (anchor, body) => {
                added.push({ anchor, body })
                return Promise.resolve(true)
            },
        })
        const mounted = await mountFiles(
            detailWith([], [changedFile("src/a.ts")]),
            null,
            editor,
        )
        try {
            const a = sectionOf(mounted.container, "src/a.ts")
            const open = named(a, "Comment on new line 22 of src/a.ts")
            if (!open) throw new Error("the line affordance was not offered")
            await press(open)
            const field = a.findAll("textarea")[0]!
            const save = byClass(a, "pull-request-composer-item-save")[0]!
            expect(save.hasAttribute("disabled")).toBe(true)

            await type(field, "   ")
            expect(save.hasAttribute("disabled")).toBe(true)

            await type(field, "x".repeat(MAX_BODY_BYTES + 1))
            expect(save.hasAttribute("disabled")).toBe(true)
            await press(save)
            expect(added).toEqual([])

            await type(field, "A useful comment")
            expect(save.hasAttribute("disabled")).toBe(false)
            await press(save)
            expect(added).toEqual([
                { anchor: { path: "src/a.ts", side: "new", line: 22 }, body: "A useful comment" },
            ])
        } finally {
            await mounted.unmount()
        }
    })

    test("at the item limit no line or reply affordance renders", async () => {
        const editor = fakeEditor({
            draft: draftWith([placedReplyItem("r1", "t1")]),
            atItemLimit: true,
        })
        const mounted = await mountFiles(
            detailWith([thread("t1")], [changedFile("src/a.ts")]),
            null,
            editor,
        )
        try {
            const a = sectionOf(mounted.container, "src/a.ts")
            expect(byClass(a, "pull-request-composer-line-action")).toHaveLength(0)
            expect(byClass(a, "pull-request-composer-reply")).toHaveLength(0)
            expect(a.textContent).not.toContain("Reply to thread")
        } finally {
            await mounted.unmount()
        }
    })

    test("the native draft controls offer no multiline, suggestion, resolution, reopening, merge, close or decline action", async () => {
        const editor = fakeEditor({ draft: draftWith([placedInlineItem("d1", "src/a.ts", "new", 22)]) })
        const mounted = await mountFiles(
            detailWith([thread("t1")], [changedFile("src/a.ts")]),
            null,
            editor,
        )
        try {
            const labels = allElements(mounted.container)
                .filter((element) => element.localName === "button")
                .map((element) => `${element.textContent} ${element.getAttribute("aria-label") ?? ""}`)
                .join(" ")
            expect(labels).not.toMatch(
                /multiline|range|suggest|resolve|reopen|merge|close|decline/i,
            )
        } finally {
            await mounted.unmount()
        }
    })

    test("the browser skin renders the thread but no draft control", async () => {
        const mounted = await mountFiles(detailWith([thread("t1")]))
        try {
            const a = sectionOf(mounted.container, "src/a.ts")
            expect(a.textContent).toContain("Body of t1-1")
            expect(byClass(a, "pull-request-composer-line-action")).toHaveLength(0)
            expect(byClass(a, "pull-request-composer-reply")).toHaveLength(0)
            expect(byClass(a, "pull-request-view-draft-item")).toHaveLength(0)
        } finally {
            await mounted.unmount()
        }
    })
})
})
