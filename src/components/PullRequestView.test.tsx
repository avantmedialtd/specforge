import { afterAll, beforeAll, beforeEach, describe, expect, test } from "bun:test"
import { act } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { FakeElement, FakeNode, installFakeDom, type FakeDocument } from "../testing/fakeDom"
import { minimisedText } from "../pullRequestView"
import type { PullRequestAddress } from "../routing/address"
import type { PullRequestResolution } from "../routing/resolve"
import type {
    ArtifactStatus,
    ChangeData,
    ChangeInstance,
    DiffFile,
    FileReviewProgress,
    Hunk,
    Line,
    PullRequestComment,
    PullRequestDetail,
    PullRequestLinks,
    PullRequestReference,
    PullRequestSummary,
    ReviewProgress,
    ReviewThread,
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

async function mountFiles(detail: PullRequestDetail, progress: ReviewProgress | null = null) {
    const container = dom.document.createElement("div")
    const root = createRoot(container as unknown as Element)
    const view = (nextDetail: PullRequestDetail, nextProgress: ReviewProgress | null) => (
        <PullRequestFiles
            detail={nextDetail}
            reference={REFERENCE}
            progress={nextProgress}
            reloadProgress={() => Promise.resolve(nextProgress)}
            onFileRefused={() => {}}
        />
    )
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
})
