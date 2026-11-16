import { describe, expect, test } from "bun:test"
import {
    checkSummary, groupedChecks, matchesPanel, nextCandidate, orderedReviewThreads, panelRowKey, progressFilter,
    fileTabStatus, readWorkspace, restoredOffset, reviewFileList, stablePanelRows, workspaceKey, workspaceScrollPort, writeWorkspace,
} from "./pullRequestWorkspace"
import type {
    DiffFile, PullRequestCheck, PullRequestReference, PullRequestSummary, ReviewProgress, ReviewThread,
} from "./types"

const reference: PullRequestReference = { provider: "github", owner: "ACME", repo: "App", number: 7 }
const file = (path: string): DiffFile => ({
    oldPath: path, newPath: path, oldMode: "100644", newMode: "100644",
    status: { kind: "modified" }, additions: 1, deletions: 0,
    content: { kind: "hunks", hunks: [] },
})
const files = ["src/a.ts", "src/b.ts", "docs/c.md", "bun.lock"].map(file)
const progress: ReviewProgress = {
    headCommit: "head", baseCommit: "base", lastMarkedHead: "before",
    viewed: 1, skipped: 1, total: 4, changedSinceViewed: 1,
    files: files.map((file, i) => ({
        path: file.newPath!, state: (["viewed", "changedSinceViewed", "unviewed", "skipped"] as const)[i],
        keyedByHead: false, hunks: null, matched: null, included: false,
    })),
}
const thread: ReviewThread = {
    id: "t1", path: "src/b.ts", side: "new", line: 1, startSide: null, startLine: null,
    originalLine: null, originalStartLine: null, resolved: false, outdated: false,
    replyable: true, comments: [],
}
function row(id: number, overrides: Partial<PullRequestSummary> = {}): PullRequestSummary {
    return {
        id, title: "Find a workspace", repoFullName: "acme/app", sourceRepoFullName: "acme/app",
        sourceBranch: "feature/find", destinationBranch: "main", url: `https://github.com/acme/app/pull/${id}`,
        draft: false, updatedAtUnix: 0, review: null, openTasks: 0, author: null, checks: null,
        conflicting: false, unresolvedThreads: 0, ...overrides,
    }
}
function store() {
    const values = new Map<string, string>()
    return { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => { values.set(key, value) } }
}

describe("workspace presentation records", () => {
    test("a detached pane can be its own scroll port while the main pane uses an ancestor", () => {
        const original = Object.getOwnPropertyDescriptor(globalThis, "getComputedStyle")
        const outer = { parentElement: null, overflow: "auto" }
        const inner = { parentElement: outer, overflow: "visible" }
        const outerElement = outer as unknown as HTMLElement
        const innerElement = inner as unknown as HTMLElement
        Object.defineProperty(globalThis, "getComputedStyle", {
            configurable: true, writable: true,
            value: (element: typeof outer) => ({ overflowY: element.overflow }),
        })
        try {
            expect(workspaceScrollPort(outerElement)).toBe(outerElement)
            expect(workspaceScrollPort(innerElement)).toBe(outerElement)
            expect(workspaceScrollPort(null)).toBeNull()
        } finally {
            if (original) { Object.defineProperty(globalThis, "getComputedStyle", original) }
            else { Reflect.deleteProperty(globalThis, "getComputedStyle") }
        }
    })
    test("canonical identity is independent of casing but separates surfaces and providers", () => {
        expect(workspaceKey(reference, "main")).toBe(workspaceKey({ ...reference, owner: "acme", repo: "app" }, "main"))
        expect(workspaceKey(reference, "main")).not.toBe(workspaceKey(reference, "detached"))
        expect(workspaceKey(reference, "main")).not.toBe(workspaceKey({ ...reference, provider: "bitbucket" }, "main"))
    })
    test("keeps 50 recently written records, preserving tab and file place", () => {
        const storage = store()
        for (let i = 0; i < 51; i++) { writeWorkspace(`bounded-${i}`, { tab: "files", place: null }, storage) }
        expect(readWorkspace("bounded-0", storage).tab).toBe("overview")
        expect(readWorkspace("bounded-1", storage).tab).toBe("files")
        const place = { path: "src/a.ts", offset: 42, head: "h", base: "b" }
        writeWorkspace("bounded-1", { tab: "discussion", place }, storage)
        writeWorkspace("bounded-51", { tab: "files", place: null }, storage)
        expect(readWorkspace("bounded-2", storage).tab).toBe("overview")
        expect(readWorkspace("bounded-1", storage)).toEqual({ tab: "discussion", place })
    })
    test("storage failure retains in-memory presentation", () => {
        const blocked = { getItem() { throw Error("blocked") }, setItem() { throw Error("blocked") } }
        writeWorkspace("fallback", { tab: "files", place: null }, blocked)
        expect(readWorkspace("fallback", blocked).tab).toBe("files")
    })
    test("invalid records are ignored, and changed commits discard old offsets", () => {
        const storage = { getItem: () => '[["bad", {"tab":"oops","place":null}]]', setItem() {} }
        expect(readWorkspace("bad", storage).tab).toBe("overview")
        const place = { path: "a", offset: 99, head: "h", base: "b" }
        expect(restoredOffset(place, "h", "b")).toBe(99)
        expect(restoredOffset(place, "new", "b")).toBe(0)
        expect(restoredOffset(place, "h", "new")).toBe(0)
    })
})

describe("review-aware file candidates", () => {
    const detail = { files, unlistedFiles: 0, headCommit: "head", baseCommit: "base" }

    test("the Files tab keeps its total count and measures viewed non-skipped files", () => {
        const status = fileTabStatus(detail, progress)!
        expect(status.count).toBe(4)
        expect(status.review).toEqual({ completed: 1, total: 3, complete: false })
        expect(status.label).toContain("4 changed files")
        expect(status.label).toContain("1 of 3 files viewed")
        expect(status.label).toContain("1 skipped")
        expect(status.label).toContain("1 changed since viewed")
    })

    test("unknown, stale or mismatched progress never becomes a zero-progress meter", () => {
        expect(fileTabStatus(null, progress)).toBeNull()
        for (const unavailable of [
            null,
            { ...progress, headCommit: "old-head" },
            { ...progress, baseCommit: "old-base" },
            { ...progress, total: 3 },
        ]) {
            const status = fileTabStatus(detail, unavailable)!
            expect(status.count).toBe(4)
            expect(status.review).toBeNull()
            expect(status.label).toContain("Review progress unavailable")
        }
    })

    test("completion means every included file is viewed, not skipped", () => {
        const complete = fileTabStatus(detail, { ...progress, viewed: 3, changedSinceViewed: 0 })!
        expect(complete.review).toEqual({ completed: 3, total: 3, complete: true })
        const skipped = fileTabStatus(detail, { ...progress, viewed: 0, skipped: 4, changedSinceViewed: 0 })!
        expect(skipped.review).toBeNull()
        expect(skipped.label).toContain("4 skipped")
        expect(fileTabStatus(
            { ...detail, files: [] },
            { ...progress, files: [], total: 0, skipped: 0, viewed: 0, changedSinceViewed: 0 },
        )!.review).toBeNull()
    })

    test("unlisted files remain counted and prevent a misleading completion check", () => {
        const status = fileTabStatus(
            { ...detail, unlistedFiles: 2 },
            { ...progress, viewed: 3, changedSinceViewed: 0 },
        )!
        expect(status.count).toBe(6)
        expect(status.review).toEqual({ completed: 3, total: 3, complete: false })
        expect(status.label).toContain("6 changed files")
        expect(status.label).toContain("2 files not listed")
    })

    test("intersects path search and review state without filtering the source model", () => {
        const list = reviewFileList(files, progress, [thread], "SRC/", "unreviewed")
        expect([...list.candidates]).toEqual(["src/b.ts"])
        expect([...list.remaining]).toEqual(["src/b.ts"])
        expect(files).toHaveLength(4)
        expect(list.badges.get("src/b.ts")).toBe("Changed since review · 1 discussion")
        expect([...reviewFileList(files, progress, [], "", "skipped").candidates]).toEqual(["bun.lock"])
        expect([...reviewFileList(files, progress, [], "", "changed").candidates]).toEqual(["src/b.ts"])
    })
    test("unknown progress is never interpreted as unviewed or skipped", () => {
        const all = reviewFileList(files, null, [thread], "", "all")
        expect(all.candidates.size).toBe(4)
        expect(all.remaining.size).toBe(0)
        expect(reviewFileList(files, null, [thread], "", "unreviewed").candidates.size).toBe(0)
        expect([...reviewFileList(files, null, [thread], "", "discussion").candidates]).toEqual(["src/b.ts"])
        expect(all.badges.get("src/a.ts")).toBe("Review progress unknown")
        expect(progressFilter("discussion")).toBe(false)
        expect(progressFilter("changed")).toBe(true)
    })
    test("renamed-file threads follow their current file before unlisted threads", () => {
        const renamed = file("src/new.ts")
        renamed.oldPath = "src/old.ts"
        const old = { ...thread, id: "old-side", path: "src/old.ts" }
        const unlisted = { ...thread, id: "unlisted", path: "removed.ts" }
        expect(orderedReviewThreads([unlisted, old], [renamed]).map((thread) => thread.id))
            .toEqual(["old-side", "unlisted"])
        expect([...reviewFileList([renamed], null, [old], "", "discussion").candidates])
            .toEqual(["src/new.ts"])
    })
    test("threads on a renamed file's old path still belong to its new navigator row", () => {
        const renamed = file("src/new.ts")
        renamed.oldPath = "src/old.ts"
        renamed.status = { kind: "renamed", similarity: 90 }
        const list = reviewFileList([renamed], null, [{ ...thread, path: "src/old.ts" }], "", "discussion")
        expect([...list.candidates]).toEqual(["src/new.ts"])
        expect(list.badges.get("src/new.ts")).toContain("1 discussion")
    })
    test("next target follows file order, wraps once, and excludes the current item", () => {
        const order = ["a", "b", "c"]
        expect(nextCandidate(order, new Set(["b", "c"]), null)).toBe("b")
        expect(nextCandidate(order, new Set(["b", "c"]), "c")).toBe("b")
        expect(nextCandidate(order, new Set(["c"]), "c")).toBeNull()
        expect(nextCandidate([], new Set(), null)).toBeNull()
    })
})

describe("status and panel triage", () => {
    test("sorts check groups stably and preserves non-passing provider states", () => {
        const checks: PullRequestCheck[] = [
            { name: "success", state: "passing", url: null },
            { name: "neutral", state: "neutral", url: null },
            { name: "fail one", state: "failing", url: null },
            { name: "pending", state: "pending", url: null },
            { name: "fail two", state: "failing", url: null },
            { name: "unknown", state: "unknown", url: null },
        ]
        expect(groupedChecks(checks).other.map((check) => check.name)).toEqual(["fail one", "fail two", "pending", "neutral", "unknown"])
        expect(groupedChecks(checks).passing.map((check) => check.name)).toEqual(["success"])
        expect(checkSummary(checks)).toBe("2 failed · 1 pending · 1 passed · 2 other")
        expect(checkSummary([])).toBe("No checks reported")
    })
    test("search covers number, title, repository and both branches", () => {
        for (const text of ["#7", "FIND A", "ACME", "FEATURE/FIND", "main"]) {
            expect(matchesPanel(row(7), text, "all", false)).toBe(true)
        }
        expect(matchesPanel(row(7), "elsewhere", "all", false)).toBe(false)
        expect(matchesPanel(row(7), "", "failing", false)).toBe(false)
        expect(matchesPanel(row(7, { checks: "failing" }), "workspace", "failing", false)).toBe(true)
        expect(matchesPanel(row(7, { draft: true }), "", "drafts", false)).toBe(true)
        expect(matchesPanel(row(7, { conflicting: true }), "", "conflicts", false)).toBe(true)
        expect(matchesPanel(row(7), "", "requested", false)).toBe(false)
        expect(matchesPanel(row(7), "", "requested", true)).toBe(true)
    })
    test("refresh retains surviving positions and appends newcomers", () => {
        const updated = row(1, { checks: "failing" })
        const next = stablePanelRows([row(1), row(2), row(3)], [row(4), row(3), updated])
        expect(next.map((pr) => pr.id)).toEqual([1, 3, 4])
        expect(next[0]).toBe(updated)
        expect(panelRowKey(row(1, { repoFullName: "ACME/APP" }))).toBe(panelRowKey(updated))
        expect(stablePanelRows([row(1), row(2)], [row(2), row(1, { repoFullName: "ACME/APP" })])
            .map((pr) => pr.id)).toEqual([1, 2])
    })
})
