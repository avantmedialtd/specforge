import { afterAll, beforeAll, beforeEach, describe, expect, test } from "bun:test"
import { act, type ReactNode } from "react"
import { FakeElement, FakeNode, installFakeDom, type FakeDocument } from "../testing/fakeDom"
import { fileKey } from "../diffFiles"
import { DIFF_LAYOUT_STORAGE_KEY } from "../diffLayout"
import { createSlotCache, lineSlotLookup } from "../slotCache"
import type { DiffFile, Hunk, Line } from "../types"
import { DiffView, type FileSlots } from "./DiffView"

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

function closestWithAttribute(element: FakeElement, name: string): FakeElement | null {
    for (let node: FakeNode | null = element; node; node = node.parentNode) {
        if (node instanceof FakeElement && node.hasAttribute(name)) return node
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
    loadFile?: (file: DiffFile) => Promise<DiffFile>
    readImage?: () => Promise<never>
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
    const root = createRoot(container as unknown as Element)
    // Stable, as both hosts keep theirs: `FileSection` compares it by identity.
    const sideNames = { old: "base", new: "head" }
    const render = (
        nextFiles: DiffFile[],
        nextSlots: ((file: DiffFile) => FileSlots | undefined) | undefined,
    ) =>
        root.render(
            <DiffView
                files={nextFiles}
                sideNames={sideNames}
                loadFile={options.loadFile ?? ((candidate) => Promise.resolve(candidate))}
                fileSlots={nextSlots}
                readImage={options.readImage}
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
                for (const observer of FakeResizeObserver.all) observer.trigger()
            })
        },
        async unmount() {
            await act(async () => root.unmount())
        },
    }
}

function lineSlots(container: FakeElement): FakeElement[] {
    return byClass(container, "diff-line-slot")
}

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
                    if (side === "old" && number === 22) return "under the removed line"
                    if (side === "new" && number === 23) return "not in the model"
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
                if (side === "old" && number === 21) return "old content"
                if (side === "new" && number === 22) return "new content"
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
            if (state === undefined) return undefined
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
    if (found === undefined) throw new Error(`no section for ${key}`)
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
