import { describe, expect, test } from "bun:test"
import type { PaletteColor, RegisteredWorkspace, WorkspaceFileRow } from "./types"
import {
    copyLabels,
    copyWorktrees,
    defaultCopy,
    divergentPaths,
    resolveCopy,
    rowForPath,
} from "./fileCopies"

function row(
    path: string,
    worktrees: string[],
    differs = false,
): WorkspaceFileRow {
    return {
        path,
        copies: worktrees.map((worktreePath) => ({ worktreePath })),
        differs,
    }
}

function ws(
    uri: string,
    displayName: string | null,
    repoId: string | null,
): RegisteredWorkspace {
    return {
        uri,
        name: uri,
        isMissing: false,
        displayName,
        color: null as PaletteColor | null,
        repoId,
        disabled: false,
    }
}

describe("rowForPath / copyWorktrees", () => {
    const rows = [row("a.md", ["/wt/main"]), row("docs/b.md", ["/wt/a", "/wt/b"])]

    test("finds a row by its exact path", () => {
        expect(copyWorktrees(rowForPath(rows, "docs/b.md"))).toEqual([
            "/wt/a",
            "/wt/b",
        ])
    })

    test("a path in no tracked worktree has no row and no copies", () => {
        expect(rowForPath(rows, "gone.md")).toBeNull()
        expect(copyWorktrees(rowForPath(rows, "gone.md"))).toEqual([])
    })

    test("no selection is not a lookup", () => {
        expect(rowForPath(rows, null)).toBeNull()
    })
})

describe("defaultCopy", () => {
    test("prefers the main worktree when it holds the file", () => {
        expect(defaultCopy(["/wt/feature", "/wt/main"], "/wt/main")).toBe("/wt/main")
    })

    test("falls back to the first copy when the main worktree lacks it", () => {
        expect(defaultCopy(["/wt/feature", "/wt/other"], "/wt/main")).toBe(
            "/wt/feature",
        )
    })

    test("a flat workspace has no main worktree and opens its only copy", () => {
        expect(defaultCopy(["/ws/flat"], null)).toBe("/ws/flat")
    })

    test("a path held by no worktree opens nothing", () => {
        expect(defaultCopy([], "/wt/main")).toBeNull()
    })
})

describe("resolveCopy", () => {
    test("the pinned copy wins over the default", () => {
        expect(
            resolveCopy(["/wt/feature", "/wt/main"], "/wt/feature", "/wt/main"),
        ).toBe("/wt/feature")
    })

    test("an unpinned preview opens the default copy", () => {
        expect(resolveCopy(["/wt/feature", "/wt/main"], null, "/wt/main")).toBe(
            "/wt/main",
        )
    })

    // The pin is a worktree path, not an index, so a refresh that adds a
    // worktree ahead of it in the copy order leaves the preview where it was.
    test("the pin survives a refresh that reorders the copies", () => {
        expect(
            resolveCopy(
                ["/wt/aaa-new", "/wt/feature", "/wt/main"],
                "/wt/feature",
                "/wt/main",
            ),
        ).toBe("/wt/feature")
    })

    test("a pinned copy that no longer holds the file falls back", () => {
        expect(resolveCopy(["/wt/main"], "/wt/feature", "/wt/main")).toBe("/wt/main")
    })
})

describe("divergentPaths", () => {
    test("marks only the rows whose copies differ", () => {
        const set = divergentPaths([
            row("same.md", ["/wt/a", "/wt/b"], false),
            row("drifted.md", ["/wt/a", "/wt/b"], true),
            row("solo.md", ["/wt/a"], false),
        ])
        expect(set.has("drifted.md")).toBe(true)
        expect(set.has("same.md")).toBe(false)
        expect(set.has("solo.md")).toBe(false)
        expect(set.size).toBe(1)
    })
})

describe("copyLabels", () => {
    test("labels by worktree basename", () => {
        expect(copyLabels(["/r/main", "/r/feature"], [])).toEqual([
            "main",
            "feature",
        ])
    })

    // A repository's display-name override is stored per repository, so every
    // worktree of it shares one — printing it against each names nothing.
    test("a repo worktree's display name is not used as a copy label", () => {
        expect(
            copyLabels(["/r/main"], [ws("/r/main", "My Repo", "/r/.git")]),
        ).toEqual(["main"])
    })

    test("a flat workspace's own display name is used", () => {
        expect(
            copyLabels(["/ws/notes"], [ws("/ws/notes", "Notes", null)]),
        ).toEqual(["Notes"])
    })

    // Two worktrees whose folders share a basename would otherwise label
    // identically; only the colliding ones are qualified.
    test("colliding basenames are qualified by path, others are not", () => {
        expect(copyLabels(["/a/wt", "/b/wt", "/c/other"], [])).toEqual([
            "wt · /a/wt",
            "wt · /b/wt",
            "other",
        ])
    })
})
