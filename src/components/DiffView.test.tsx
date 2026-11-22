import { afterAll, beforeAll, beforeEach, describe, expect, test } from "bun:test"
import { act, type ReactNode } from "react"
import { FakeElement, FakeNode, installFakeDom, type FakeDocument } from "../testing/fakeDom"
import { fileKey, type NavigatorReviewBadge } from "../diffFiles"
import { DIFF_LAYOUT_STORAGE_KEY } from "../diffLayout"
import { createSlotCache, lineSlotLookup } from "../slotCache"
import type { DiffFile, Hunk, Line, ImageVersions } from "../types"
import { PrContentLoading } from "../prContentLoading"
import { DiffView, type DiffNavigation, type FileSlots } from "./DiffView"

// `diff-view`: *Provider-Neutral Diff Line Slots* and *Diff View Hosts*. The
// view is mounted for real, through `react-dom/client` on the fake DOM, so the
// measured layout, the fold state and the host's slots take part. Effects do
// not run in a server render, and the view renders no section until it has
// measured its width.

let dom: { document: FakeDocument; uninstall: () => void }
let createRoot: typeof import("react-dom/client").createRoot
const g = globalThis as unknown as Record<string, unknown>

/// The widths `measure` reads. The `ch` probe is 1000px wide, so one `ch` is
/// ten layout pixels and the sections column's 2000px is 200ch: wide enough
/// for side by side's 104ch entry threshold.
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
    if (className.includes("diff-view-probe-ch")) { return 1000 }
    if (className.includes("diff-view-probe-navigator")) { return 300 }
    if (className.includes("diff-sections")) { return widths.sections }
    if (className.includes("diff-view-body")) { return widths.sections + 300 }
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
            if (!match) { return false }
            const actual = element.getAttribute(match[1])
            return match[2] === undefined ? actual !== null : actual === match[2]
        })
}

function closestWithAttribute(element: FakeElement, name: string): FakeElement | null {
    for (let node: FakeNode | null = element; node; node = node.parentNode) {
        if (node instanceof FakeElement && node.hasAttribute(name)) { return node }
    }
    return null
}

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
        if (next) { this.setAttribute(name, "") }
        else { this.removeAttribute(name) }
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

let layout: "unified" | "split" = "unified"

beforeEach(() => {
    layout = "unified"
    widths.sections = 2000
    FakeResizeObserver.all.length = 0
    g.localStorage = {
        getItem: (key: string) => (key === DIFF_LAYOUT_STORAGE_KEY ? layout : null),
        setItem: (_key: string, value: string) => {
            layout = value === "split" ? "split" : "unified"
        },
    }
})

function line(
    kind: Line["kind"],
    oldNo: number | null,
    newNo: number | null,
    text: string,
): Line {
    return { kind, oldNo, newNo, text }
}

/// A modified file with one hunk: a context line, a removed line and an
/// added line. Old 20/21 face new 20/22; the context line carries both.
function hunk(): Hunk {
    return {
        oldStart: 20,
        oldLines: 2,
        newStart: 20,
        newLines: 2,
        section: "function handler()",
        lines: [
            line("context", 20, 20, "const before = true"),
            line("removed", 21, null, "const answer = 41"),
            line("added", null, 22, "const answer = 42"),
        ],
    }
}

/// Two removed lines and one added: the second side-by-side row pairs old 22
/// with a filler, and new 21 faces old 21.
function fillerHunk(): Hunk {
    return {
        oldStart: 21,
        oldLines: 2,
        newStart: 21,
        newLines: 1,
        section: null,
        lines: [
            line("removed", 21, null, "first removed"),
            line("removed", 22, null, "second removed"),
            line("added", null, 21, "only added"),
        ],
    }
}

function file(overrides: Partial<DiffFile> = {}): DiffFile {
    return {
        oldPath: "src/api.ts",
        newPath: "src/api.ts",
        oldMode: "100644",
        newMode: "100644",
        status: { kind: "modified" },
        additions: 1,
        deletions: 1,
        content: { kind: "hunks", hunks: [hunk()] },
        ...overrides,
    }
}

type Lookup = (side: "old" | "new", line: number) => ReactNode

function slotsFor(byFile: Record<string, Lookup>): (file: DiffFile) => FileSlots | undefined {
    return (candidate) => {
        const lookup = byFile[candidate.newPath ?? candidate.oldPath ?? ""]
        return lookup === undefined ? undefined : { line: lookup }
    }
}

function slotOnly(content: string, side?: "old" | "new", line?: number): Lookup {
    return (candidateSide, candidateLine) =>
        (side === undefined || candidateSide === side) &&
        (line === undefined || candidateLine === line)
            ? content
            : undefined
}

interface MountOptions {
    visible?: boolean
    navigation?: DiffNavigation
    loadFile?: (file: DiffFile) => Promise<DiffFile>
    readImage?: () => Promise<ImageVersions>
    imageReads?: "nearView" | "onRequest"
    automaticReads?: PrContentLoading
    zoomImage?: (file: DiffFile) => void
}

interface Mounted {
    container: FakeElement
    rerender: (
        files: DiffFile[],
        slots: ((file: DiffFile) => FileSlots | undefined) | undefined,
    ) => Promise<void>
    remeasure: () => Promise<void>
    unmount: () => Promise<void>
}

async function mount(
    files: DiffFile[],
    slots: ((file: DiffFile) => FileSlots | undefined) | undefined,
    options: MountOptions = {},
): Promise<Mounted> {
    const container = dom.document.createElement("div")
    if (options.automaticReads) { dom.document.body.appendChild(container) }
    const root = createRoot(container as unknown as Element)
    // Stable, as both hosts keep theirs: `FileSection` compares it by identity.
    const sideNames = { old: "base", new: "head" }
    const render = (
        nextFiles: DiffFile[],
        nextSlots: ((file: DiffFile) => FileSlots | undefined) | undefined,
    ) =>
        root.render(
            <DiffView
                visible={options.visible}
                navigation={options.navigation}
                files={nextFiles}
                sideNames={sideNames}
                loadFile={options.loadFile ?? ((candidate) => Promise.resolve(candidate))}
                fileSlots={nextSlots}
                readImage={options.readImage}
                imageReads={options.imageReads}
                automaticReads={options.automaticReads}
                zoomImage={options.zoomImage}
            />,
        )
    await act(async () => render(files, slots))
    return {
        container,
        async rerender(nextFiles, nextSlots) {
            await act(async () => render(nextFiles, nextSlots))
        },
        async remeasure() {
            await act(async () => {
                for (const observer of FakeResizeObserver.all) { observer.trigger() }
            })
        },
        async unmount() {
            await act(async () => root.unmount())
            container.parentNode?.removeChild(container)
        },
    }
}

describe("PR automatic reads through the shared renderer", () => {
    class NearObserver {
        static all: NearObserver[] = []
        nodes = new Set<Element>()
        constructor(readonly callback: (entries: { target: Element; isIntersecting: boolean }[]) => void) { NearObserver.all.push(this) }
        observe(node: Element) { this.nodes.add(node) }
        unobserve(node: Element) { this.nodes.delete(node) }
        disconnect() { this.nodes.clear() }
        static enter() {
            for (const observer of NearObserver.all) {
                observer.callback([...observer.nodes].map(target => ({ target, isIntersecting: true })))
            }
        }
    }
    for (const images of [false, true]) {
        for (const diffs of [false, true]) {
            test(`independent preferences: images=${images}, diffs=${diffs}`, async () => {
                const observer = g.IntersectionObserver
                const bounds = Object.getOwnPropertyDescriptor(FakeElement.prototype, "getBoundingClientRect")!
                FakeElement.prototype.getBoundingClientRect = () => ({ x: 0, y: 20, top: 20, left: 0, right: 100, bottom: 50, width: 100, height: 30 })
                g.IntersectionObserver = NearObserver
                ;(g.window as Record<string, unknown>).innerHeight = 800
                NearObserver.all = []
                const controller = new PrContentLoading()
                controller.policy({ autoShowImages: images, autoLoadLargeDiffs: diffs }, true)
                let texts = 0, pictures = 0
                const files = [
                    file({ content: { kind: "withheld" } }),
                    file({ oldPath: "a.png", newPath: "a.png", content: { kind: "binary" } }),
                    file({ oldPath: "huge.txt", newPath: "huge.txt", content: { kind: "tooLarge" } }),
                ]
                const view = await mount(files, undefined, {
                    automaticReads: controller,
                    loadFile: async candidate => { texts++; return { ...candidate, content: { kind: "hunks", hunks: [hunk()] } } },
                    readImage: async () => { pictures++; return { old: { kind: "absent" }, new: { kind: "absent" } } },
                })
                try {
                    await act(async () => NearObserver.enter())
                    expect(texts).toBe(Number(diffs))
                    expect(pictures).toBe(Number(images))
                    controller.policy({ autoShowImages: false, autoLoadLargeDiffs: false }, true)
                    controller.policy({ autoShowImages: images, autoLoadLargeDiffs: diffs }, true)
                    await act(async () => NearObserver.enter())
                    expect(texts).toBe(Number(diffs))
                    expect(pictures).toBe(Number(images))
                    if (diffs) { expect(view.container.textContent).toContain("const answer = 42") }
                } finally {
                    await view.unmount(); controller.dispose()
                    g.IntersectionObserver = observer
                    Object.defineProperty(FakeElement.prototype, "getBoundingClientRect", bounds)
                }
            })
        }
    }
    function measuring() {
        const observer = g.IntersectionObserver
        const bounds = Object.getOwnPropertyDescriptor(FakeElement.prototype, "getBoundingClientRect")!
        FakeElement.prototype.getBoundingClientRect = () => ({ x: 0, y: 20, top: 20, left: 0, right: 100, bottom: 50, width: 100, height: 30 })
        g.IntersectionObserver = NearObserver
        ;(g.window as Record<string, unknown>).innerHeight = 800
        NearObserver.all = []
        return () => {
            g.IntersectionObserver = observer
            Object.defineProperty(FakeElement.prototype, "getBoundingClientRect", bounds)
        }
    }

    test("an automatically queued image keeps its manual control and shares its flight", async () => {
        const restore = measuring()
        const controller = new PrContentLoading()
        controller.policy({ autoShowImages: true, autoLoadLargeDiffs: true }, true)
        const finishText: (() => void)[] = []
        let finishImage!: (value: ImageVersions) => void
        let reads = 0
        const files = [
            file({ oldPath: "a.ts", newPath: "a.ts", content: { kind: "withheld" } }),
            file({ oldPath: "b.ts", newPath: "b.ts", content: { kind: "withheld" } }),
            file({ oldPath: "a.png", newPath: "a.png", content: { kind: "binary" } }),
        ]
        const view = await mount(files, undefined, {
            automaticReads: controller, imageReads: "nearView",
            loadFile: candidate => new Promise<DiffFile>(resolve => {
                finishText.push(() => resolve({ ...candidate, content: { kind: "hunks", hunks: [hunk()] } }))
            }),
            readImage: () => { reads++; return new Promise<ImageVersions>(resolve => { finishImage = resolve }) },
        })
        try {
            await act(async () => NearObserver.enter())
            expect(finishText).toHaveLength(2)
            expect(reads).toBe(0)
            const manual = view.container.findAll("button").find(button => button.textContent === "Show image")
            expect(manual).toBeDefined()
            await press(manual!)
            expect(reads).toBe(1)
            await press(manual!)
            expect(reads).toBe(1)
            await act(async () => { for (const finish of finishText) { finish() } })
            await act(async () => NearObserver.enter())
            expect(reads).toBe(1)
            await act(async () => finishImage({ old: { kind: "absent" }, new: { kind: "absent" } }))
        } finally { await view.unmount(); controller.dispose(); restore() }
    })

    test("hidden and collapsed files wait; text results are classified before a subsequent image read", async () => {
        const restore = measuring()
        const controller = new PrContentLoading()
        controller.policy({ autoShowImages: false, autoLoadLargeDiffs: true }, true)
        let texts = 0, pictures = 0
        const files = [file({ oldPath: "new.png", newPath: "new.png", content: { kind: "withheld" } })]
        const options: MountOptions = {
            visible: false, automaticReads: controller,
            loadFile: async candidate => { texts++; return { ...candidate, content: { kind: "binary" } } },
            readImage: async () => { pictures++; return { old: { kind: "absent" }, new: { kind: "absent" } } },
        }
        const view = await mount(files, undefined, options)
        try {
            await act(async () => NearObserver.enter())
            expect(texts).toBe(0)
            options.visible = true
            await view.rerender(files, undefined)
            await act(async () => { (reactProps(byClass(view.container, "diff-file-toggle")[0]).onClick as () => void)() })
            await act(async () => NearObserver.enter())
            expect(texts).toBe(0)
            await act(async () => { (reactProps(byClass(view.container, "diff-file-toggle")[0]).onClick as () => void)() })
            await act(async () => NearObserver.enter())
            expect(texts).toBe(1)
            expect(pictures).toBe(0)
            controller.policy({ autoShowImages: true, autoLoadLargeDiffs: true }, true)
            await act(async () => NearObserver.enter())
            expect(pictures).toBe(1)
        } finally { await view.unmount(); controller.dispose(); restore() }
    })

    for (const chosen of ["unified", "split"] as const) {
        test(`automatic insertion preserves a live editor and old model results are ignored (${chosen})`, async () => {
            layout = chosen
            const restore = measuring()
            const controller = new PrContentLoading()
            controller.policy({ autoShowImages: true, autoLoadLargeDiffs: true }, true)
            let answer!: (value: DiffFile) => void
            let attempts = 0
            const pending = file({ content: { kind: "withheld" } })
            const stable = file({ oldPath: "draft.ts", newPath: "draft.ts" })
            const files = [pending, stable]
            const draft = <textarea defaultValue="unsent draft" />
            const slots = (candidate: DiffFile) => candidate === stable ? { preamble: draft } : undefined
            const view = await mount(files, slots, {
                automaticReads: controller,
                loadFile: () => { attempts++; return new Promise<DiffFile>(resolve => { answer = resolve }) },
            })
            try {
                const editor = view.container.findAll("textarea")[0]
                editor.focus()
                await act(async () => NearObserver.enter())
                expect(attempts).toBe(1)
                expect(dom.document.activeElement).toBe(editor)
                await act(async () => answer({ ...pending, content: { kind: "hunks", hunks: [hunk()] } }))
                expect(view.container.findAll("textarea")[0]).toBe(editor)
                expect(editor.value).toBe("unsent draft")
                expect(dom.document.activeElement).toBe(editor)
                const replacement = [file({ content: { kind: "withheld" } }), stable]
                await view.rerender(replacement, slots)
                await act(async () => NearObserver.enter())
                expect(attempts).toBe(2)
                await view.rerender([file({ content: { kind: "binary" } }), stable], slots)
                await act(async () => answer({ ...pending, content: { kind: "hunks", hunks: [hunk()] } }))
                expect(byClass(view.container, "diff-file")[0].textContent).not.toContain("const answer = 42")
            } finally { await view.unmount(); controller.dispose(); restore() }
        })
    }
})

function lineSlots(container: FakeElement): FakeElement[] {
    return byClass(container, "diff-line-slot")
}

function navigationFor(files: readonly DiffFile[], overrides: Partial<DiffNavigation> = {}): DiffNavigation {
    return {
        controls: {
            search: <label>Find file<input type="search" /></label>,
            filter: <label>Show files<select><option>All</option></select></label>,
        },
        candidates: new Set(files.map(fileKey)),
        remaining: new Set(files.map(fileKey)),
        badges: new Map(files.map((candidate) => [fileKey(candidate), {
            reviewState: "unviewed", discussionCount: 0, label: "Unreviewed",
        }])),
        progressKnown: true,
        ...overrides,
    }
}

describe("compact PR navigator rows", () => {
    test("all six review icons are shape-distinct, passive and fully described", async () => {
        const badges: NavigatorReviewBadge[] = [
            { reviewState: "unviewed", discussionCount: 0, label: "Unreviewed" },
            { reviewState: "viewed", discussionCount: 0, label: "Viewed" },
            { reviewState: "partlyViewed", discussionCount: 0, label: "Partly viewed" },
            { reviewState: "changedSinceViewed", discussionCount: 0, label: "Changed since review" },
            { reviewState: "skipped", discussionCount: 0, label: "Skipped" },
            { reviewState: null, discussionCount: 0, label: "Review progress unknown" },
        ]
        const files = badges.map((_, index) => fileAt(`src/${index}.ts`))
        const mounted = await mount(files, undefined, {
            navigation: navigationFor(files, {
                badges: new Map(files.map((candidate, index) => [fileKey(candidate), badges[index]])),
            }),
        })
        try {
            const rows = byClass(mounted.container, "diff-nav-row--file")
            expect(rows).toHaveLength(6)
            const shapes = new Set<string>()
            rows.forEach((row, index) => {
                const badge = badges[index]
                const label = `src/${index}.ts; modified; +1 added; −1 removed; ${badge.label}`
                expect(row.getAttribute("aria-label")).toBe(label)
                expect(row.getAttribute("title")).toBe(label)
                expect(row.textContent).not.toContain(badge.label)
                const marker = byClass(row, "pr-file-review")[0]!
                expect(marker.getAttribute("data-review-state")).toBe(badge.reviewState ?? "unknown")
                expect(marker.getAttribute("aria-hidden")).toBe("true")
                expect(marker.hasAttribute("tabindex")).toBe(false)
                expect(reactProps(marker).onClick).toBeUndefined()
                const svg = marker.findAll("svg")[0]!
                expect(svg.getAttribute("aria-hidden")).toBe("true")
                expect(svg.getAttribute("fill")).toBe("none")
                shapes.add(JSON.stringify(svg.children.map((child) => [child.localName, [...child.attributes]])))
                expect(marker.findAll("button")).toHaveLength(0)
                expect(marker.findAll("input")).toHaveLength(0)
            })
            expect(shapes.size).toBe(6)
            expect(byClass(mounted.container, "pr-file-badge")).toHaveLength(0)
            expect(byClass(mounted.container, "pr-file-discussion")).toHaveLength(0)
        } finally {
            await mounted.unmount()
        }
    })

    test("discussion presence adds no control and missing counts never become zeroes", async () => {
        const files = [
            fileAt("src/a/index.ts", { additions: 1234, deletions: 5678 }),
            fileAt("src/b/index.ts", { additions: null, deletions: 0 }),
            fileAt("src/c/\u200bindex.ts", { additions: null, deletions: null }),
        ]
        const navigation = navigationFor(files)
        navigation.badges = new Map([
            [fileKey(files[0]), { reviewState: "viewed", discussionCount: 3, label: "Viewed · 3 discussions" }],
            [fileKey(files[1]), { reviewState: null, discussionCount: 0, label: "Review progress unknown" }],
            [fileKey(files[2]), { reviewState: "skipped", discussionCount: 0, label: "Skipped" }],
        ])
        const mounted = await mount(files, undefined, { navigation })
        try {
            const rows = byClass(mounted.container, "diff-nav-row--file")
            expect(rows[0].getAttribute("aria-label"))
                .toBe("src/a/index.ts; modified; +1234 added; −5678 removed; Viewed · 3 discussions")
            expect(rows[1].getAttribute("aria-label"))
                .toBe("src/b/index.ts; modified; −0 removed; Review progress unknown")
            expect(rows[2].getAttribute("aria-label"))
                .toBe("src/c/U+200Bindex.ts; modified; Skipped")
            expect(byClass(rows[1], "diff-count--added")).toHaveLength(0)
            expect(byClass(rows[1], "diff-count--removed")[0].textContent).toBe("−0")
            expect(byClass(rows[2], "diff-counts")).toHaveLength(0)
            expect(byClass(rows[0], "pr-file-discussion")).toHaveLength(1)
            expect(byClass(rows[1], "pr-file-discussion")).toHaveLength(0)
            const discussion = byClass(rows[0], "pr-file-discussion")[0]!
            expect(discussion.getAttribute("aria-hidden")).toBe("true")
            expect(reactProps(discussion).onClick).toBeUndefined()
            for (const row of rows) {
                expect(allElements(row).filter((element) =>
                    element.hasAttribute("tabindex") || element.hasAttribute("tabIndex"),
                )).toHaveLength(0)
            }
        } finally {
            await mounted.unmount()
        }
    })

    test("full-depth PR indentation stays visible and leaves non-PR rows unchanged", async () => {
        const folders = ["root", "one", "two", "three", "four", "five", "six", "seven", "eight"]
        const paths = folders.map((_, index) => `${folders.slice(0, index + 1).join("/")}/a.ts`)
        const files = paths.map((path) => fileAt(path))
        const pr = await mount(files, undefined, { navigation: navigationFor(files) })
        const commit = await mount(files, undefined)
        try {
            const prRows = byClass(pr.container, "diff-nav-row--file")
            const commitRows = byClass(commit.container, "diff-nav-row--file")
            expect(prRows.map((row) => row.getAttribute("aria-level")))
                .toEqual(folders.map((_, index) => String(index + 2)))
            expect(commitRows.map((row) => row.getAttribute("aria-level")))
                .toEqual(prRows.map((row) => row.getAttribute("aria-level")))
            const indents = folders.map((_, index) => `${(index + 1) * 12 + 4}px`)
            expect(prRows.map((row) => row.style.paddingLeft)).toEqual(indents)
            expect(commitRows.map((row) => row.style.paddingLeft)).toEqual(indents)
            expect(byClass(pr.container, "diff-nav-row--directory").map((row) => row.style.paddingLeft))
                .toEqual(folders.map((_, index) => `${index * 12 + 4}px`))
            expect(commitRows.map((row) => row.textContent)).toEqual(prRows.map((row) => row.textContent))
            expect(commitRows.every((row) => row.getAttribute("aria-label") === null)).toBe(true)
            expect(byClass(commit.container, "pr-file-controls")).toHaveLength(0)
            expect(byClass(commit.container, "pr-file-review")).toHaveLength(0)
            expect(byClass(commit.container, "pr-file-tree")).toHaveLength(0)
            const tree = byClass(pr.container, "pr-file-tree")[0]!
            const content = byClass(pr.container, "pr-file-tree-content")[0]!
            expect(tree.getAttribute("role")).toBe("tree")
            expect(content.parentNode).toBe(tree)
            expect(byClass(pr.container, "pr-file-controls")[0].parentNode).toBe(tree.parentNode)
            expect(byClass(commit.container, "diff-counts").length).toBeGreaterThanOrEqual(files.length)
        } finally {
            await pr.unmount()
            await commit.unmount()
        }
    })

    test("focus reveals a deep filename by scrolling only the tree, including at enlarged zoom", async () => {
        const files = [fileAt("root/a.ts")]
        const mounted = await mount(files, undefined, { navigation: navigationFor(files) })
        try {
            const tree = byClass(mounted.container, "pr-file-tree")[0]!
            const row = byClass(tree, "diff-nav-row--file")[0]!
            const name = byClass(row, "diff-nav-name")[0]!
            const status = byClass(row, "diff-nav-status")[0]!
            const outer = Object.assign(mounted.container, { scrollLeft: 7, scrollTop: 8 })
            const port = Object.assign(tree, { scrollLeft: 0, scrollTop: 0, clientHeight: 200 })
            Object.defineProperty(tree, "offsetWidth", { configurable: true, value: 240 })
            Object.defineProperty(tree, "clientWidth", { configurable: true, value: 240 })
            Object.assign(row, {
                closest: () => row,
                querySelector: (selector: string) => selector === ".diff-nav-name" ? name : status,
            })
            for (const scale of [1, 1.5]) {
                const rect = (left: number, top: number, width: number, height: number) => ({
                    x: left * scale, y: top * scale, left: left * scale, top: top * scale,
                    right: (left + width) * scale, bottom: (top + height) * scale,
                    width: width * scale, height: height * scale,
                })
                port.scrollLeft = 0
                port.scrollTop = 0
                tree.getBoundingClientRect = () => rect(10, 20, 240, 200)
                row.getBoundingClientRect = () => rect(10, 400, 400, 25)
                name.getBoundingClientRect = () => rect(350, 400, 50, 25)
                status.getBoundingClientRect = () => rect(334, 400, 12, 25)
                await act(async () => {
                    ;(reactProps(tree).onFocus as (event: unknown) => void)({ target: row })
                })
                expect(port.scrollLeft).toBe(150)
                expect(port.scrollTop).toBe(205)
                expect(outer.scrollLeft).toBe(7)
                expect(outer.scrollTop).toBe(8)
            }
        } finally {
            await mounted.unmount()
        }
    })

    test("progress-only changes retain navigator rows, sections, draft values and render isolation", async () => {
        const files = [fileAt("src/a.ts"), fileAt("src/b.ts")]
        let lineCalls = 0
        const slots: FileSlots = {
            line: (side, number) => {
                lineCalls++
                return side === "new" && number === 22 ? <textarea defaultValue="Saved draft" /> : undefined
            },
        }
        const fileSlots = () => slots
        const options: MountOptions = { navigation: navigationFor(files) }
        const mounted = await mount(files, fileSlots, options)
        try {
            const rows = byClass(mounted.container, "diff-nav-row--file")
            const sections = byClass(mounted.container, "diff-file")
            const editors = mounted.container.findAll("textarea")
            editors[0].value = "Keep my unsaved draft"
            const calls = lineCalls
            options.navigation = navigationFor(files, {
                badges: new Map(files.map((candidate) => [fileKey(candidate), {
                    reviewState: "viewed", discussionCount: 0, label: "Viewed",
                }])),
            })
            await mounted.rerender(files, fileSlots)
            expectSameElements(byClass(mounted.container, "diff-nav-row--file"), rows)
            expectSameElements(byClass(mounted.container, "diff-file"), sections)
            expectSameElements(mounted.container.findAll("textarea"), editors)
            expect(editors[0].value).toBe("Keep my unsaved draft")
            expect(lineCalls).toBe(calls)
            expect(rows[0].getAttribute("aria-label")).toEndWith("; Viewed")
        } finally {
            await mounted.unmount()
        }
    })

    test("keyboard navigation keeps one roving row and only activates its file", async () => {
        const files = [fileAt("src/a.ts"), fileAt("src/b.ts")]
        const navigation = navigationFor(files)
        const mounted = await mount(files, undefined, { navigation })
        try {
            const tree = allElements(mounted.container).find((element) => element.getAttribute("role") === "tree")!
            const rows = allElements(tree).filter((element) => element.getAttribute("role") === "treeitem")
            mounted.container.style.overflowY = "auto"
            Object.assign(mounted.container, { scrollTop: 0 })
            rows.forEach((row, index) => {
                // The fake DOM does not reflect tabindex or dispatch focus.
                Object.defineProperty(row, "tabIndex", {
                    configurable: true,
                    get: () => Number(row.getAttribute("tabindex")),
                    set: (value: number) => row.setAttribute("tabindex", String(value)),
                })
                row.setAttribute("tabindex", index === 0 ? "0" : "-1")
                Object.assign(row, {
                    closest: () => row,
                    scrollIntoView() {},
                    click: () => (reactProps(row).onClick as () => void)(),
                })
            })
            const key = async (target: FakeElement, value: string) => {
                await act(async () => {
                    ;(reactProps(tree).onKeyDown as (event: unknown) => void)({
                        target, key: value, preventDefault() {},
                    })
                    if (rows.includes(dom.document.activeElement!)) {
                        ;(reactProps(tree).onFocus as (event: unknown) => void)({
                            target: dom.document.activeElement,
                        })
                    }
                })
            }
            await key(rows[0], "ArrowRight")
            expect(dom.document.activeElement).toBe(rows[1])
            expect(rows.filter((row) => (row as FakeElement & { tabIndex: number }).tabIndex === 0)).toHaveLength(1)
            await key(rows[1], "Enter")
            expect(rows[1].getAttribute("aria-current")).toBe("true")
            expect(navigation.badges.get("src/a.ts")?.reviewState).toBe("unviewed")
            await key(rows[1], "ArrowLeft")
            expect(dom.document.activeElement).toBe(rows[0])
            await key(rows[2], " ")
            expect(rows[2].getAttribute("aria-current")).toBe("true")
            expect(navigation.badges.get("src/b.ts")?.reviewState).toBe("unviewed")
        } finally {
            await mounted.unmount()
        }
    })
})

describe("optional workspace navigation", () => {
    test("named controls keep search, toolbar, clear and notices in reading order", async () => {
        const files = [file()]
        const options: MountOptions = {
            navigation: navigationFor(files, {
                controls: {
                    search: <label className="test-search">Find file<input type="search" /></label>,
                    filter: <label className="test-filter">Show files<select><option>All</option></select></label>,
                    clear: <button type="button">Clear filters</button>,
                },
            }),
        }
        const mounted = await mount(files, undefined, options)
        try {
            const controls = byClass(mounted.container, "pr-file-controls")[0]!
            const toolbar = byClass(controls, "pr-file-toolbar")[0]!
            expect(controls.children[0].getAttribute("class")).toBe("test-search")
            expect(controls.children[1]).toBe(toolbar)
            expect(controls.children[2].textContent).toBe("Clear filters")
            expect(toolbar.children[0].getAttribute("class")).toBe("test-filter")
            expect(toolbar.children[1].getAttribute("title")).toBe("1 matching file")
            expect(toolbar.children[1].children[0].getAttribute("aria-hidden")).toBe("true")
            expect(toolbar.children[1].children[0].textContent).toBe("1")
            expect(toolbar.children[1].children[1].textContent).toBe("1 matching file")
            const next = toolbar.children[2]
            expect(next.textContent).toBe("Next →")
            expect(next.getAttribute("aria-label")).toBe("Next unreviewed")
            expect(reactProps(next).disabled).toBe(false)

            options.navigation = { ...options.navigation!, progressKnown: false }
            await mounted.rerender(files, undefined)
            expect(reactProps(next).disabled).toBe(true)
            expect(next.getAttribute("title")).toBe("Review progress unavailable.")
            const hint = allElements(controls).find((element) =>
                element.getAttribute("id") === next.getAttribute("aria-describedby"),
            )!
            expect(hint.textContent).toBe("Review progress unavailable.")
            expect(byClass(controls, "pull-request-view-note")[0].textContent).toBe("Review progress unavailable.")

            options.navigation = { ...options.navigation!, progressKnown: true, candidates: new Set(), remaining: new Set() }
            await mounted.rerender(files, undefined)
            expect(byClass(controls, "pr-file-matches")[0].getAttribute("title")).toBe("0 matching files")
            expect(controls.children[2].textContent).toBe("Clear filters")
            expect(controls.children.find((element) => element.getAttribute("role") === "status")?.textContent)
                .toBe("No matching files.")
            expect(next.getAttribute("title")).toBe("No other matching unreviewed files.")
            expect(reactProps(next).disabled).toBe(true)
        } finally {
            await mounted.unmount()
        }
    })

    test("a current file remains identifiable after it stops matching", async () => {
        const files = [fileAt("src/a.ts"), fileAt("src/b.ts")]
        const options: MountOptions = { navigation: navigationFor(files) }
        const mounted = await mount(files, undefined, options)
        try {
            mounted.container.style.overflowY = "auto"
            Object.assign(mounted.container, { scrollTop: 0 })
            const row = byClass(mounted.container, "diff-nav-row--file")[0]
            await press(row)
            options.navigation = {
                ...options.navigation!, candidates: new Set(["src/b.ts"]), remaining: new Set(["src/b.ts"]),
            }
            await mounted.rerender(files, undefined)
            expect(row.getAttribute("aria-current")).toBe("true")
            expect(byClass(mounted.container, "diff-nav-row--file")).toContain(row)
            const controls = byClass(mounted.container, "pr-file-controls")[0]
            expect(controls.textContent).toContain("Current file is outside this filter: src/a.ts")
            expect(byClass(mounted.container, "diff-file")).toHaveLength(2)
        } finally {
            await mounted.unmount()
        }
    })

    test("filtering the navigator retains every file section and loaded content", async () => {
        const files = [file(), file({ newPath: "docs/notes.md", oldPath: "docs/notes.md" })]
        const options: MountOptions = {
            navigation: {
                controls: {
                    search: <label>Find file<input type="search" /></label>,
                    filter: <label>Show files<select><option>All</option></select></label>,
                },
                candidates: new Set(["docs/notes.md"]),
                remaining: new Set(["docs/notes.md"]),
                badges: new Map([["docs/notes.md", {
                    reviewState: "unviewed", discussionCount: 0, label: "Unreviewed",
                }]]),
                progressKnown: true,
            },
        }
        const mounted = await mount(files, undefined, options)
        try {
            expect(byClass(mounted.container, "diff-file")).toHaveLength(2)
            const navigator = byClass(mounted.container, "diff-navigator")[0]!
            expect(navigator.textContent).toContain("notes.md")
            expect(navigator.textContent).not.toContain("api.ts")
            const sections = byClass(mounted.container, "diff-file")
            options.navigation = { ...options.navigation!, candidates: new Set() }
            await mounted.rerender(files, undefined)
            expect(byClass(mounted.container, "diff-file")).toEqual(sections)
            expect(navigator.textContent).toContain("No matching files.")
            options.navigation = undefined
            await mounted.rerender(files, undefined)
            expect(navigator.textContent).toContain("api.ts")
            expect(navigator.textContent).not.toContain("Find file")
        } finally {
            await mounted.unmount()
        }
    })

    test("a hidden retained diff ignores zero-width measurements and measures again when visible", async () => {
        layout = "split"
        const files = [file()]
        const options: MountOptions = { visible: true }
        const mounted = await mount(files, undefined, options)
        try {
            expect(byClass(mounted.container, "diff-split")).toHaveLength(1)
            options.visible = false
            widths.sections = 0
            await mounted.rerender(files, undefined)
            await mounted.remeasure()
            expect(byClass(mounted.container, "diff-split")).toHaveLength(1)
            expect(layout).toBe("split")
            widths.sections = 500
            options.visible = true
            await mounted.rerender(files, undefined)
            expect(byClass(mounted.container, "diff-unified")).toHaveLength(1)
            expect(layout).toBe("split")
        } finally {
            await mounted.unmount()
        }
    })
})

function slotLabels(container: FakeElement): string[] {
    return lineSlots(container).map((element) => element.getAttribute("aria-label") ?? "")
}

function slotTexts(container: FakeElement): string[] {
    return lineSlots(container).map((element) => element.textContent)
}

function diffRows(container: FakeElement): FakeElement[] {
    return allElements(container).filter((element) => {
        const classes = classesOf(element)
        return classes.includes("diff-row") || classes.includes("diff-split-row")
    })
}

/// What a copy of the diff's own rows would carry: the model's identities and
/// the code cells' text, never a slot's.
function rowSnapshot(container: FakeElement): unknown[] {
    return diffRows(container).map((row) => ({
        old: row.getAttribute("data-old-line"),
        new: row.getAttribute("data-new-line"),
        code: byClass(row, "diff-code").map((cell) => cell.textContent),
        text: row.textContent,
    }))
}

function slotCells(container: FakeElement): FakeElement[] {
    return byClass(container, "diff-line-slot-cell")
}

/// The fake DOM keeps the prop's spelling where a browser reflects it as
/// `colspan`.
function colSpanOf(cell: FakeElement): string | null {
    return cell.getAttribute("colspan") ?? cell.getAttribute("colSpan")
}

describe("split source scrolls without moving review context", () => {
    test("each code viewport excludes its marker and slots and shares one keyboard scrollbar", async () => {
        layout = "split"
        const source = hunk()
        source.lines[1] = { ...source.lines[1]!, text: "                                const answer = 41" }
        const mounted = await mount(
            [file({ content: { kind: "hunks", hunks: [source] } })],
            slotsFor({ "src/api.ts": slotOnly("Keep this discussion fixed", "new", 22) }),
        )
        try {
            const cells = byClass(mounted.container, "diff-code")
            expect(cells).toHaveLength(4)
            for (const cell of cells) {
                const port = byClass(cell, "diff-code-line")[0]!
                const marker = byClass(cell, "diff-marker")[0]!
                expect(port.hasAttribute("data-code-scroll")).toBe(true)
                expect(port.getAttribute("tabindex") ?? port.getAttribute("tabIndex")).toBe("-1")
                expect(byClass(port, "diff-marker")).toHaveLength(0)
                expect(byClass(port, "diff-line-slot")).toHaveLength(0)
                expect(marker.parentNode).toBe(port.parentNode)
            }
            expect(byClass(cells[2]!, "diff-text")[0]!.textContent).toBe(source.lines[1]!.text)
            const bar = byClass(mounted.container, "diff-horizontal-scroll")
            expect(bar).toHaveLength(1)
            expect(bar[0]!.getAttribute("tabindex") ?? bar[0]!.getAttribute("tabIndex")).toBe("0")
            expect(bar[0]!.getAttribute("aria-label")).toContain("src/api.ts")
            expect(bar[0]!.getAttribute("data-copy")).toBe("skip")
            expect(lineSlots(mounted.container)[0]!.textContent).toBe("Keep this discussion fixed")
            expect(closestWithAttribute(lineSlots(mounted.container)[0]!, "data-code-scroll")).toBeNull()
        } finally {
            await mounted.unmount()
        }
    })

    test("unified retains its existing whole-section scroller without split-only controls", async () => {
        const mounted = await mount([file()], undefined)
        try {
            expect(byClass(mounted.container, "diff-unified")).toHaveLength(1)
            expect(byClass(mounted.container, "diff-code-line")).toHaveLength(0)
            expect(byClass(mounted.container, "diff-horizontal-scroll")).toHaveLength(0)
            expect(byClass(mounted.container, "diff-marker")).toHaveLength(3)
        } finally {
            await mounted.unmount()
        }
    })
})

describe("a host's line action sits in its line's gutter", () => {
    /// A host action on every line `answers` allows, labelled by its side and
    /// line.
    function actionsFor(
        answers: (side: "old" | "new", line: number) => boolean,
    ): (file: DiffFile) => FileSlots | undefined {
        const lineAction = (side: "old" | "new", line: number) =>
            answers(side, line) ? (
                <button type="button" className="host-action" aria-label={`${side} ${line}`} />
            ) : null
        return () => ({ lineAction })
    }

    function actionLabels(root: FakeElement): string[] {
        return allElements(root)
            .filter((element) => classesOf(element).includes("host-action"))
            .map((element) => element.getAttribute("aria-label") ?? "")
    }

    test("unified offers one per row — a context line's new side first — and makes no slot row", async () => {
        const mounted = await mount([file()], actionsFor(() => true))
        try {
            expect(actionLabels(mounted.container)).toEqual(["new 20", "old 21", "new 22"])
            expect(slotLabels(mounted.container)).toEqual([])
            for (const box of allElements(mounted.container).filter((element) =>
                classesOf(element).includes("diff-line-action"),
            )) {
                expect(box.getAttribute("data-copy")).toBe("skip")
            }
        } finally {
            await mounted.unmount()
        }

        // A context line whose new side answers nothing offers its old side's.
        const oldOnly = await mount(
            [file()],
            actionsFor((side, line) => !(side === "new" && line === 20)),
        )
        try {
            expect(actionLabels(oldOnly.container)).toEqual(["old 20", "old 21", "new 22"])
        } finally {
            await oldOnly.unmount()
        }
    })

    test("side by side offers one per half, in that half's own gutter", async () => {
        layout = "split"
        const mounted = await mount([file()], actionsFor(() => true))
        try {
            expect(actionLabels(mounted.container)).toEqual(["old 20", "new 20", "old 21", "new 22"])
            const oldGutters = allElements(mounted.container).filter((element) => {
                const classes = classesOf(element)
                return classes.includes("diff-num") && classes.includes("diff-cell--old")
            })
            expect(oldGutters.flatMap(actionLabels)).toEqual(["old 20", "old 21"])
            expect(slotLabels(mounted.container)).toEqual([])
        } finally {
            await mounted.unmount()
        }
    })

    test("an action changes no copied text", async () => {
        const plain = await mount([file()], undefined)
        const copiedPlain = rowSnapshot(plain.container)
        await plain.unmount()
        const withActions = await mount([file()], actionsFor(() => true))
        try {
            expect(rowSnapshot(withActions.container)).toEqual(copiedPlain)
        } finally {
            await withActions.unmount()
        }
    })
})

describe("a line slot follows its side-qualified line", () => {
    test("unified renders it immediately after its line, across the section", async () => {
        const mounted = await mount(
            [file()],
            slotsFor({ "src/api.ts": slotOnly("Looks right", "new", 22) }),
        )
        try {
            expect(slotLabels(mounted.container)).toEqual(["new line 22"])
            expect(slotTexts(mounted.container)).toEqual(["Looks right"])
            const row = diffRows(mounted.container).find(
                (element) => element.getAttribute("data-new-line") === "22",
            )
            expect(row).toBeDefined()
            expect(row!.nextSibling).toBe(lineSlots(mounted.container)[0]!)
            expect(lineSlots(mounted.container)[0]!.getAttribute("role")).toBe("group")
        } finally {
            await mounted.unmount()
        }
    })

    test("both identities of a context line are independent", async () => {
        const oldOnly = await mount(
            [file()],
            slotsFor({ "src/api.ts": slotOnly("On the old side", "old", 20) }),
        )
        try {
            expect(slotLabels(oldOnly.container)).toEqual(["old line 20"])
            expect(slotTexts(oldOnly.container)).toEqual(["On the old side"])
        } finally {
            await oldOnly.unmount()
        }

        const both = await mount(
            [file()],
            slotsFor({
                "src/api.ts": (side, number) =>
                    number === 20 ? (side === "old" ? "old context" : "new context") : undefined,
            }),
        )
        try {
            expect(slotLabels(both.container)).toEqual(["old line 20", "new line 20"])
            expect(slotTexts(both.container)).toEqual(["old context", "new context"])
        } finally {
            await both.unmount()
        }
    })

    test("a paired row carries a slot under each half", async () => {
        layout = "split"
        const mounted = await mount(
            [file()],
            slotsFor({
                "src/api.ts": (side, number) =>
                    (side === "old" && number === 21) || (side === "new" && number === 22)
                        ? `${side} content`
                        : undefined,
            }),
        )
        try {
            const cells = slotCells(mounted.container)
            expect(cells).toHaveLength(2)
            expect(cells.map(colSpanOf)).toEqual(["2", "2"])
            expect(cells.map((cell) => cell.textContent)).toEqual(["old content", "new content"])
            expect(cells.map((cell) => lineSlots(cell)[0]!.getAttribute("aria-label"))).toEqual([
                "old line 21",
                "new line 22",
            ])
        } finally {
            await mounted.unmount()
        }
    })

    test("a context line's one populated half leaves the other half empty", async () => {
        layout = "split"
        const mounted = await mount(
            [file()],
            slotsFor({ "src/api.ts": slotOnly("old context", "old", 20) }),
        )
        try {
            const cells = slotCells(mounted.container)
            expect(cells).toHaveLength(2)
            expect(cells[0]!.textContent).toBe("old context")
            expect(cells[1]!.textContent).toBe("")
            expect(classesOf(cells[1]!)).toContain("diff-line-slot-cell--empty")
            expect(cells[1]!.hasAttribute("aria-hidden")).toBe(true)
        } finally {
            await mounted.unmount()
        }
    })

    test("a one-sided file gives its side's slot the full width", async () => {
        layout = "split"
        const added = await mount(
            [
                file({
                    oldPath: null,
                    newPath: "src/new.ts",
                    status: { kind: "added" },
                    content: {
                        kind: "hunks",
                        hunks: [
                            {
                                oldStart: 0,
                                oldLines: 0,
                                newStart: 1,
                                newLines: 2,
                                section: null,
                                lines: [
                                    line("added", null, 1, "first"),
                                    line("added", null, 2, "second"),
                                ],
                            },
                        ],
                    },
                }),
            ],
            slotsFor({
                "src/new.ts": (side, number) =>
                    side === "new" && number === 1 ? "new one" : undefined,
            }),
        )
        try {
            const cells = slotCells(added.container)
            expect(cells).toHaveLength(1)
            expect(colSpanOf(cells[0]!)).toBe("2")
            expect(cells[0]!.textContent).toBe("new one")
            expect(lineSlots(cells[0]!)[0]!.getAttribute("aria-label")).toBe("new line 1")
        } finally {
            await added.unmount()
        }

        const deleted = await mount(
            [
                file({
                    oldPath: "src/gone.ts",
                    newPath: null,
                    status: { kind: "deleted" },
                    content: {
                        kind: "hunks",
                        hunks: [
                            {
                                oldStart: 1,
                                oldLines: 1,
                                newStart: 0,
                                newLines: 0,
                                section: null,
                                lines: [line("removed", 1, null, "gone")],
                            },
                        ],
                    },
                }),
            ],
            slotsFor({ "src/gone.ts": slotOnly("old one", "old", 1) }),
        )
        try {
            const cells = slotCells(deleted.container)
            expect(cells).toHaveLength(1)
            expect(colSpanOf(cells[0]!)).toBe("2")
            expect(cells[0]!.textContent).toBe("old one")
            expect(lineSlots(cells[0]!)[0]!.getAttribute("aria-label")).toBe("old line 1")
        } finally {
            await deleted.unmount()
        }
    })
})

describe("file identity is the view's own", () => {
    test("a renamed file keeps its new path, while old and new sides stay distinct", async () => {
        const renamed = file({
            oldPath: "src/old-api.ts",
            newPath: "src/api.ts",
            status: { kind: "renamed", similarity: 80 },
        })
        const mounted = await mount(
            [renamed],
            slotsFor({
                "src/api.ts": (side, number) =>
                    (side === "old" && number === 21) || (side === "new" && number === 22)
                        ? `${side} side`
                        : undefined,
            }),
        )
        try {
            expect(slotLabels(mounted.container)).toEqual(["old line 21", "new line 22"])
            expect(slotTexts(mounted.container)).toEqual(["old side", "new side"])
        } finally {
            await mounted.unmount()
        }

        const wrongIdentity = await mount(
            [renamed],
            slotsFor({ "src/old-api.ts": slotOnly("wrong file") }),
        )
        try {
            expect(lineSlots(wrongIdentity.container)).toHaveLength(0)
        } finally {
            await wrongIdentity.unmount()
        }
    })

    test("a deleted file keeps its old path", async () => {
        const deleted = file({
            oldPath: "src/gone.ts",
            newPath: null,
            status: { kind: "deleted" },
            content: {
                kind: "hunks",
                hunks: [
                    {
                        oldStart: 7,
                        oldLines: 1,
                        newStart: 0,
                        newLines: 0,
                        section: null,
                        lines: [line("removed", 7, null, "removed")],
                    },
                ],
            },
        })
        const mounted = await mount(
            [deleted],
            slotsFor({ "src/gone.ts": slotOnly("old line seven", "old", 7) }),
        )
        try {
            expect(slotLabels(mounted.container)).toEqual(["old line 7"])
            expect(slotTexts(mounted.container)).toEqual(["old line seven"])
        } finally {
            await mounted.unmount()
        }
    })
})

describe("nothing is invented for a filler or an absent line", () => {
    test("a filler takes no slot and a missing line renders none", async () => {
        layout = "split"
        const calls: string[] = []
        const mounted = await mount(
            [
                file({
                    content: { kind: "hunks", hunks: [fillerHunk()] },
                }),
            ],
            slotsFor({
                "src/api.ts": (side, number) => {
                    calls.push(`${side}:${number}`)
                    if (side === "old" && number === 22) { return "under the removed line" }
                    if (side === "new" && number === 23) { return "not in the model" }
                    return undefined
                },
            }),
        )
        try {
            expect(slotLabels(mounted.container)).toEqual(["old line 22"])
            expect(slotTexts(mounted.container)).toEqual(["under the removed line"])
            // The filler half of the second row was never asked, and the
            // absent new line 23 was never asked or moved to a neighbour.
            expect(calls).not.toContain("new:22")
            expect(calls).not.toContain("new:23")
            expect(calls).toContain("old:22")
            const cells = slotCells(mounted.container)
            expect(cells).toHaveLength(2)
            expect(cells[0]!.textContent).toBe("under the removed line")
            expect(classesOf(cells[1]!)).toContain("diff-line-slot-cell--empty")
        } finally {
            await mounted.unmount()
        }
    })

    test("a slot keyed by another file renders nothing", async () => {
        const mounted = await mount(
            [file()],
            slotsFor({ "src/elsewhere.ts": slotOnly("wrong file") }),
        )
        try {
            expect(lineSlots(mounted.container)).toHaveLength(0)
        } finally {
            await mounted.unmount()
        }
    })
})

describe("slots stay outside the diff and its copy", () => {
    test("slot content changes no row, number or code cell", async () => {
        const bare = await mount([file()], undefined)
        let before: unknown[]
        try {
            before = rowSnapshot(bare.container)
            expect(bare.container.textContent).not.toContain("Looks right")
        } finally {
            await bare.unmount()
        }

        const mounted = await mount(
            [file()],
            slotsFor({
                "src/api.ts": (side, number) =>
                    number === 22 ? "Looks right" : side === "old" ? "old note" : undefined,
            }),
        )
        try {
            expect(rowSnapshot(mounted.container)).toEqual(before)
            for (const slot of lineSlots(mounted.container)) {
                expect(closestWithAttribute(slot, "data-copy-line")).toBeNull()
                expect(slot.hasAttribute("data-hunk-index")).toBe(false)
                expect(slot.hasAttribute("data-line-index")).toBe(false)
            }
            // The code cells carry the model's text, never the slot's.
            expect(byClass(mounted.container, "diff-code").map((cell) => cell.textContent)).toEqual(
                [" const before = true", "−const answer = 41", "+const answer = 42"],
            )
        } finally {
            await mounted.unmount()
        }
    })

    test("a host with no line slots renders no slot row", async () => {
        const none = await mount([file()], undefined)
        let bare: unknown[]
        try {
            bare = rowSnapshot(none.container)
            expect(lineSlots(none.container)).toHaveLength(0)
        } finally {
            await none.unmount()
        }

        const emptyLookup = await mount([file()], slotsFor({ "src/api.ts": () => undefined }))
        try {
            expect(rowSnapshot(emptyLookup.container)).toEqual(bare)
            expect(lineSlots(emptyLookup.container)).toHaveLength(0)
            expect(slotCells(emptyLookup.container)).toHaveLength(0)
        } finally {
            await emptyLookup.unmount()
        }

        const otherSlots = await mount([file()], () => ({ headerExtra: "viewed" }))
        try {
            expect(lineSlots(otherSlots.container)).toHaveLength(0)
            expect(otherSlots.container.textContent).toContain("viewed")
        } finally {
            await otherSlots.unmount()
        }
    })
})

describe("switching layout or folding keeps the same content on the same line", () => {
    test("a layout switch moves each slot to its side's half and back", async () => {
        layout = "split"
        widths.sections = 500 // 50ch: side by side is chosen but too narrow.
        let loads = 0
        let reads = 0
        let zooms = 0
        const slots = slotsFor({
            "src/api.ts": (side, number) =>
                (side === "old" && number === 21) || (side === "new" && number === 22)
                    ? `${side} content`
                    : undefined,
        })
        const mounted = await mount([file()], slots, {
            loadFile: (candidate) => {
                loads += 1
                return Promise.resolve(candidate)
            },
            readImage: () => {
                reads += 1
                return Promise.reject(new Error("no image may be read"))
            },
            zoomImage: () => {
                zooms += 1
            },
        })
        try {
            // Unified in effect: two full-width divs after their rows.
            expect(byClass(mounted.container, "diff-unified")).toHaveLength(1)
            expect(byClass(mounted.container, "diff-split")).toHaveLength(0)
            expect(slotLabels(mounted.container)).toEqual(["old line 21", "new line 22"])
            expect(slotTexts(mounted.container)).toEqual(["old content", "new content"])

            widths.sections = 2000 // 200ch: side by side comes into effect.
            await mounted.remeasure()
            expect(byClass(mounted.container, "diff-split")).toHaveLength(1)
            expect(byClass(mounted.container, "diff-unified")).toHaveLength(0)
            expect(slotLabels(mounted.container)).toEqual(["old line 21", "new line 22"])
            expect(slotTexts(mounted.container)).toEqual(["old content", "new content"])
            const cells = slotCells(mounted.container)
            expect(cells).toHaveLength(2)
            expect(cells[0]!.textContent).toBe("old content")
            expect(cells[1]!.textContent).toBe("new content")

            widths.sections = 500 // 50ch: below the 96ch leave threshold.
            await mounted.remeasure()
            expect(byClass(mounted.container, "diff-unified")).toHaveLength(1)
            expect(slotLabels(mounted.container)).toEqual(["old line 21", "new line 22"])
            expect(slotTexts(mounted.container)).toEqual(["old content", "new content"])

            expect(loads).toBe(0)
            expect(reads).toBe(0)
            expect(zooms).toBe(0)
        } finally {
            await mounted.unmount()
        }
    })

    test("folding hides its lines' slots, and unfolding brings the same content back", async () => {
        const fold = slotsFor({
            "src/api.ts": (side, number) => {
                if (side === "old" && number === 21) { return "old content" }
                if (side === "new" && number === 22) { return "new content" }
                return undefined
            },
        })
        const withHunkSlots = (folded: boolean) => {
            return (candidate: DiffFile): FileSlots => ({
                ...fold(candidate),
                hunks: {
                    folded: folded ? new Set([0]) : new Set<number>(),
                    heading: () => null,
                    end: () => null,
                },
            })
        }

        const mounted = await mount([file()], withHunkSlots(true))
        try {
            expect(lineSlots(mounted.container)).toHaveLength(0)
            expect(mounted.container.textContent).toContain("Show 3 lines")

            await mounted.rerender([file()], withHunkSlots(false))
            expect(slotLabels(mounted.container)).toEqual(["old line 21", "new line 22"])
            expect(slotTexts(mounted.container)).toEqual(["old content", "new content"])

            await mounted.rerender([file()], withHunkSlots(true))
            expect(lineSlots(mounted.container)).toHaveLength(0)

            await mounted.rerender([file()], withHunkSlots(false))
            expect(slotLabels(mounted.container)).toEqual(["old line 21", "new line 22"])
            expect(slotTexts(mounted.container)).toEqual(["old content", "new content"])
        } finally {
            await mounted.unmount()
        }
    })
})

// -------------------------------------------------------------------------
// Per-file slot updates
// -------------------------------------------------------------------------

// `diff-view`: *Host Slot Updates Re-render Only Their File*. Each file's
// whole `FileSlots` — header, preamble, folds and line lookup — is cached by
// its own inputs, as `PullRequestView` caches them. The line lookup is the
// render probe: the view calls it while a section renders, so a file whose
// count stays put did not render.

interface HostFileState {
    header: string
    preamble: string
    lines: ReadonlyArray<readonly [string, number, string]>
    folded: boolean
}

class SlotHost {
    private readonly cache = createSlotCache<FileSlots>()
    readonly builds = new Map<string, number>()
    readonly lineCalls = new Map<string, number>()

    constructor(readonly state: Record<string, HostFileState>) {}

    /// A fresh function per host render, as `PullRequestView`'s callback has
    /// when its inputs change.
    slots(): (file: DiffFile) => FileSlots | undefined {
        return (file) => {
            const key = fileKey(file)
            const state = this.state[key]
            if (state === undefined) { return undefined }
            return this.cache.get(
                key,
                [file, state.header, state.preamble, state.lines, state.folded],
                () => {
                    this.builds.set(key, (this.builds.get(key) ?? 0) + 1)
                    const lookup = lineSlotLookup(state.lines)
                    return {
                        headerExtra: <span className="host-header">{state.header}</span>,
                        preamble: <p className="host-preamble">{state.preamble}</p>,
                        hunks: {
                            folded: state.folded ? new Set([0]) : new Set<number>(),
                            heading: () => null,
                            end: () => null,
                        },
                        line: (side, line) => {
                            this.lineCalls.set(key, (this.lineCalls.get(key) ?? 0) + 1)
                            return lookup(side, line)
                        },
                    }
                },
            )
        }
    }

    resetCalls(): void {
        this.lineCalls.clear()
    }

    calls(key: string): number {
        return this.lineCalls.get(key) ?? 0
    }
}

function fileAt(path: string, overrides: Partial<DiffFile> = {}): DiffFile {
    return file({ oldPath: path, newPath: path, ...overrides })
}

function sectionOf(container: FakeElement, key: string): FakeElement {
    const found = allElements(container).find(
        (element) =>
            classesOf(element).includes("diff-file") &&
            element.getAttribute("data-file-key") === key,
    )
    if (found === undefined) { throw new Error(`no section for ${key}`) }
    return found
}

/// The props React keeps on a host element, so a test can press a control the
/// way a reader would without a browser event system.
function reactProps(element: FakeElement): Record<string, unknown> {
    const holder = element as unknown as Record<string, unknown>
    for (const key of Reflect.ownKeys(element)) {
        if (typeof key === "string" && key.startsWith("__reactProps$")) {
            return holder[key] as Record<string, unknown>
        }
    }
    for (const key of Reflect.ownKeys(element)) {
        if (typeof key !== "string" || !key.startsWith("__reactFiber$")) { continue }
        const fiber = holder[key] as { memoizedProps?: Record<string, unknown> }
        if (fiber.memoizedProps !== undefined) { return fiber.memoizedProps }
    }
    throw new Error("the element carries no React props")
}

async function press(element: FakeElement): Promise<void> {
    const onClick = reactProps(element).onClick
    if (typeof onClick !== "function") { throw new Error("the element has no click handler") }
    await act(async () => {
        ;(onClick as (event: unknown) => void)({
            currentTarget: element,
            target: element,
            preventDefault() {},
            stopPropagation() {},
        })
    })
}

function hostOf(files: readonly DiffFile[], folded = false): SlotHost {
    const state: Record<string, HostFileState> = {}
    for (const file of files) {
        const key = fileKey(file)
        state[key] = {
            header: `${key} header`,
            preamble: `${key} preamble`,
            lines: [["new", 22, `${key} note`]],
            folded,
        }
    }
    return new SlotHost(state)
}

function expectSameElements(actual: readonly FakeElement[], expected: readonly FakeElement[]): void {
    expect(actual).toHaveLength(expected.length)
    actual.forEach((element, index) => expect(element).toBe(expected[index]!))
}

describe("a host slot update re-renders only its file", () => {
    test("loading rebuilds only that file's slots and keeps earlier loaded identities stable", async () => {
        const files = [
            fileAt("src/a.ts", { content: { kind: "withheld" } }),
            fileAt("src/b.ts", { content: { kind: "withheld" } }),
            fileAt("src/c.ts"),
        ]
        const host = hostOf(files)
        const mounted = await mount(files, host.slots(), {
            loadFile: candidate => Promise.resolve(fileAt(fileKey(candidate))),
        })
        try {
            await press(byClass(sectionOf(mounted.container, "src/a.ts"), "diff-load")[0]!)
            expect(host.builds.get("src/a.ts")).toBe(2)
            expect(host.builds.get("src/b.ts")).toBe(1)
            expect(host.builds.get("src/c.ts")).toBe(1)
            const rows = byClass(sectionOf(mounted.container, "src/a.ts"), "diff-row")
            host.resetCalls()
            await press(byClass(sectionOf(mounted.container, "src/b.ts"), "diff-load")[0]!)
            expect(host.builds.get("src/a.ts")).toBe(2)
            expect(host.builds.get("src/b.ts")).toBe(2)
            expect(host.builds.get("src/c.ts")).toBe(1)
            expect(host.calls("src/a.ts")).toBe(0)
            expect(host.calls("src/c.ts")).toBe(0)
            expectSameElements(byClass(sectionOf(mounted.container, "src/a.ts"), "diff-row"), rows)
        } finally {
            await mounted.unmount()
        }
    })

    test("changing one file's header, preamble and line re-renders that file alone", async () => {
        const files = [fileAt("src/a.ts"), fileAt("src/b.ts"), fileAt("src/c.ts")]
        const host = hostOf(files)
        const mounted = await mount(files, host.slots())
        try {
            expect(host.builds.size).toBe(3)
            host.resetCalls()
            const rowsA = byClass(sectionOf(mounted.container, "src/a.ts"), "diff-row")
            const rowsC = byClass(sectionOf(mounted.container, "src/c.ts"), "diff-row")

            host.state["src/b.ts"] = {
                header: "B header changed",
                preamble: "B preamble changed",
                lines: [["new", 22, "B line changed"]],
                folded: false,
            }
            await mounted.rerender(files, host.slots())

            expect(host.calls("src/b.ts")).toBeGreaterThan(0)
            expect(host.calls("src/a.ts")).toBe(0)
            expect(host.calls("src/c.ts")).toBe(0)
            expect(host.builds.get("src/b.ts")).toBe(2)
            expect(host.builds.get("src/a.ts")).toBe(1)
            expect(host.builds.get("src/c.ts")).toBe(1)
            const b = sectionOf(mounted.container, "src/b.ts")
            expect(b.textContent).toContain("B header changed")
            expect(b.textContent).toContain("B preamble changed")
            expect(slotTexts(b)).toEqual(["B line changed"])
            expectSameElements(byClass(sectionOf(mounted.container, "src/a.ts"), "diff-row"), rowsA)
            expectSameElements(byClass(sectionOf(mounted.container, "src/c.ts"), "diff-row"), rowsC)
        } finally {
            await mounted.unmount()
        }
    })

    test("a host render that changes no slot touches no section and keeps the shown state", async () => {
        const files = [fileAt("src/a.ts"), fileAt("src/b.ts"), fileAt("src/c.ts")]
        const host = hostOf(files)
        // Only A is host-folded: B and C keep visible lines, so their line
        // lookup still proves whether their section rendered.
        host.state["src/a.ts"] = { ...host.state["src/a.ts"]!, folded: true }
        const mounted = await mount(files, host.slots())
        try {
            const fold = byClass(sectionOf(mounted.container, "src/a.ts"), "diff-hunk-fold")[0]!
            expect(fold.textContent).toBe("Show 3 lines")
            await press(fold)
            expect(slotTexts(sectionOf(mounted.container, "src/a.ts"))).toEqual(["src/a.ts note"])

            host.resetCalls()
            const builds = new Map(host.builds)
            await mounted.rerender(files, host.slots())

            expect(host.calls("src/a.ts")).toBe(0)
            expect(host.calls("src/b.ts")).toBe(0)
            expect(host.calls("src/c.ts")).toBe(0)
            expect(host.builds).toEqual(builds)
            const a = sectionOf(mounted.container, "src/a.ts")
            expect(byClass(a, "diff-hunk-fold")[0]?.textContent).toBe("Hide")
            expect(slotTexts(a)).toEqual(["src/a.ts note"])
        } finally {
            await mounted.unmount()
        }
    })

    test("a refold drops that file's shown state and leaves other files alone", async () => {
        const files = [fileAt("src/a.ts"), fileAt("src/b.ts"), fileAt("src/c.ts")]
        const host = hostOf(files)
        // Only A is host-folded: B and C keep visible lines, so their line
        // lookup still proves whether their section rendered.
        host.state["src/a.ts"] = { ...host.state["src/a.ts"]!, folded: true }
        const mounted = await mount(files, host.slots())
        try {
            await press(byClass(sectionOf(mounted.container, "src/a.ts"), "diff-hunk-fold")[0]!)
            expect(slotTexts(sectionOf(mounted.container, "src/a.ts"))).toEqual(["src/a.ts note"])

            host.resetCalls()
            host.state["src/b.ts"] = {
                ...host.state["src/b.ts"]!,
                lines: [["new", 22, "B line changed"]],
            }
            await mounted.rerender(files, host.slots())
            expect(host.calls("src/b.ts")).toBeGreaterThan(0)
            expect(host.calls("src/a.ts")).toBe(0)
            expect(host.calls("src/c.ts")).toBe(0)
            expect(
                byClass(sectionOf(mounted.container, "src/a.ts"), "diff-hunk-fold")[0]?.textContent,
            ).toBe("Hide")
            expect(slotTexts(sectionOf(mounted.container, "src/a.ts"))).toEqual(["src/a.ts note"])

            host.state["src/a.ts"] = { ...host.state["src/a.ts"]!, folded: false }
            host.resetCalls()
            await mounted.rerender(files, host.slots())
            expect(host.calls("src/b.ts")).toBe(0)
            expect(host.calls("src/c.ts")).toBe(0)
            expect(byClass(sectionOf(mounted.container, "src/a.ts"), "diff-hunk-fold")).toHaveLength(0)
            expect(slotTexts(sectionOf(mounted.container, "src/a.ts"))).toEqual(["src/a.ts note"])

            host.state["src/a.ts"] = { ...host.state["src/a.ts"]!, folded: true }
            host.resetCalls()
            await mounted.rerender(files, host.slots())
            expect(host.calls("src/b.ts")).toBe(0)
            expect(host.calls("src/c.ts")).toBe(0)
            expect(
                byClass(sectionOf(mounted.container, "src/a.ts"), "diff-hunk-fold")[0]?.textContent,
            ).toBe("Show 3 lines")
            expect(slotTexts(sectionOf(mounted.container, "src/a.ts"))).toEqual([])
        } finally {
            await mounted.unmount()
        }
    })

    test("a file loaded on request keeps its identity when another file's slots change", async () => {
        const files = [
            fileAt("src/a.ts", { content: { kind: "withheld" } }),
            fileAt("src/b.ts"),
            fileAt("src/c.ts"),
        ]
        const host = hostOf(files)
        const mounted = await mount(files, host.slots(), {
            loadFile: (candidate) =>
                Promise.resolve(fileAt(candidate.oldPath ?? candidate.newPath ?? "src/a.ts")),
        })
        try {
            expect(sectionOf(mounted.container, "src/a.ts").textContent).toContain("Load diff")
            await press(byClass(sectionOf(mounted.container, "src/a.ts"), "diff-load")[0]!)
            expect(sectionOf(mounted.container, "src/a.ts").textContent).toContain("const answer = 42")
            expect(slotTexts(sectionOf(mounted.container, "src/a.ts"))).toEqual(["src/a.ts note"])
            const rows = byClass(sectionOf(mounted.container, "src/a.ts"), "diff-row")
            expect(rows.length).toBeGreaterThan(0)

            host.resetCalls()
            const aBuilds = host.builds.get("src/a.ts")
            host.state["src/b.ts"] = {
                ...host.state["src/b.ts"]!,
                lines: [["new", 22, "B line changed"]],
            }
            await mounted.rerender(files, host.slots())

            expect(host.calls("src/b.ts")).toBeGreaterThan(0)
            expect(host.calls("src/a.ts")).toBe(0)
            expect(host.calls("src/c.ts")).toBe(0)
            expect(host.builds.get("src/a.ts")).toBe(aBuilds)
            expectSameElements(byClass(sectionOf(mounted.container, "src/a.ts"), "diff-row"), rows)
            expect(sectionOf(mounted.container, "src/a.ts").textContent).toContain("const answer = 42")
            expect(slotTexts(sectionOf(mounted.container, "src/a.ts"))).toEqual(["src/a.ts note"])
        } finally {
            await mounted.unmount()
        }
    })
})
