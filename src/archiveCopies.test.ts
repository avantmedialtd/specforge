import { describe, expect, test } from "bun:test"
import { copyKey, copyLabels, copyOptions } from "./archiveCopies"
import type { ArchivedChangeCopy, RegisteredWorkspace } from "./types"

function copy(worktreePath: string, archiveDir: string): ArchivedChangeCopy {
    return { worktreePath, archiveDir, date: archiveDir.slice(0, 10) }
}

function workspace(
    uri: string,
    overrides: Partial<RegisteredWorkspace> = {},
): RegisteredWorkspace {
    return {
        uri,
        name: uri.split("/").pop() ?? uri,
        isMissing: false,
        displayName: null,
        color: null,
        repoId: null,
        disabled: false,
        ...overrides,
    }
}

describe("copyLabels", () => {
    test("names each copy by its worktree basename when nothing collides", () => {
        const copies = [copy("/a/main", "2026-01-01-x"), copy("/b/feature", "2026-01-01-x")]
        expect(copyLabels(copies, [])).toEqual(["main", "feature"])
    })

    test("a flat workspace's display-name override names its copy", () => {
        const copies = [copy("/a/main", "2026-01-01-x")]
        expect(copyLabels(copies, [workspace("/a/main", { displayName: "Notes" })])).toEqual([
            "Notes",
        ])
    })

    test("a repository's display-name override does NOT name its worktrees", () => {
        const copies = [copy("/a/main", "2026-01-01-x")]
        const ws = workspace("/a/main", { displayName: "My Repo", repoId: "/a/main/.git" })
        expect(copyLabels(copies, [ws])).toEqual(["main"])
    })

    test("differing archive directories append the directory to every label", () => {
        const copies = [copy("/a/main", "2026-01-01-x"), copy("/b/feature", "2026-02-02-x")]
        expect(copyLabels(copies, [])).toEqual([
            "main · 2026-01-01-x",
            "feature · 2026-02-02-x",
        ])
    })

    test("colliding basenames under one directory name fall back to the worktree path", () => {
        const copies = [copy("/a/wt", "2026-01-01-x"), copy("/b/wt", "2026-01-01-x")]
        expect(copyLabels(copies, [])).toEqual(["wt · /a/wt", "wt · /b/wt"])
    })
})

describe("copyOptions", () => {
    const copies = [copy("/a/main", "2026-01-01-x"), copy("/b/feature", "2026-02-02-x")]
    const workspaces = [workspace("/a/main"), workspace("/b/feature")]

    // The reader's control changed shape when it adopted the shared switcher.
    // What it SAYS did not (`archive-browser`: *Read-Only Artifact
    // Navigation*), so the switcher is pinned to the same label function the
    // retired `<select>`'s options were built from.
    test("offers exactly the labels the old copy <select> offered", () => {
        expect(copyOptions(copies, workspaces).map((o) => o.label)).toEqual(
            copyLabels(copies, workspaces),
        )
    })

    test("no branch — and nothing to tint — reaches the switcher for an archived change", () => {
        for (const option of copyOptions(copies, workspaces)) {
            expect(option.branch).toBeNull()
            expect(option.color).toBeNull()
            expect(option.divergence).toBeNull()
        }
    })

    test("each option is keyed by its (worktree, directory) pair, so a dated and an un-dated twin stay distinct", () => {
        const twins = [copy("/a/main", "2026-01-01-x"), copy("/a/main", "x")]
        const keys = copyOptions(twins, workspaces).map((o) => o.key)
        expect(keys).toEqual(twins.map(copyKey))
        expect(new Set(keys).size).toBe(2)
    })

    test("one copy still yields one option, in `copies` order", () => {
        expect(copyOptions([copies[0]!], workspaces)).toHaveLength(1)
    })
})
