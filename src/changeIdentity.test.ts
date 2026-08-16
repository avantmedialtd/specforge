import { describe, expect, test } from "bun:test"
import {
    branchForWorktree,
    changeDirectoryName,
    isArchivedChangeId,
} from "./changeIdentity"
import type { ChangeData, ChangeInstance, WorkspaceView } from "./types"

// ---- Fixture builders --------------------------------------------------

function change(id: string): ChangeData {
    return {
        changeId: id,
        title: null,
        artifacts: { proposal: true, design: false, tasks: false, specs: [] },
        completedTasks: 0,
        totalTasks: 0,
    } as unknown as ChangeData
}

function instance(
    worktreePath: string,
    branch: string | null,
    changeId = "some-change",
): ChangeInstance {
    return {
        worktreePath,
        branch,
        isMainWorktree: true,
        isDefaultBranch: false,
        isArchivedHere: false,
        change: change(changeId),
        modifiedAt: 0,
        divergence: null,
        specCommitState: "committed",
    } as unknown as ChangeInstance
}

function repoView(
    repoId: string,
    logicals: { name: string; instances: ChangeInstance[] }[],
): WorkspaceView {
    return {
        kind: "repo",
        repoId,
        mainWorktree: `/repos/${repoId}`,
        name: repoId,
        defaultBranch: "master",
        active: logicals,
        displayName: null,
        color: null,
        dirty: false,
        dirtyWorktrees: [],
        hasUncommittedSpecs: false,
    } as unknown as WorkspaceView
}

function flatView(uri: string): WorkspaceView {
    return {
        kind: "flat",
        workspace: { uri, name: "flat" },
        changes: [],
        displayName: null,
        color: null,
    } as unknown as WorkspaceView
}

// ---- branchForWorktree -------------------------------------------------

describe("branchForWorktree", () => {
    test("returns the branch of the instance at that worktree path", () => {
        const views = [
            repoView("r1", [
                { name: "add-thing", instances: [instance("/repos/r1", "master")] },
            ]),
        ]
        expect(branchForWorktree("/repos/r1", views)).toBe("master")
    })

    test("finds an instance in a secondary worktree, not just the main one", () => {
        const views = [
            repoView("r1", [
                {
                    name: "add-thing",
                    instances: [
                        instance("/repos/r1", "master"),
                        instance("/repos/r1/wt/feature", "feature/x"),
                    ],
                },
            ]),
        ]
        expect(branchForWorktree("/repos/r1/wt/feature", views)).toBe("feature/x")
    })

    test("scans every logical change, not only the first", () => {
        const views = [
            repoView("r1", [
                { name: "first", instances: [instance("/repos/r1", "master")] },
                { name: "second", instances: [instance("/repos/r1/wt/b", "topic-b")] },
            ]),
        ]
        expect(branchForWorktree("/repos/r1/wt/b", views)).toBe("topic-b")
    })

    test("scans every repo view, not only the first", () => {
        const views = [
            repoView("r1", [
                { name: "a", instances: [instance("/repos/r1", "master")] },
            ]),
            repoView("r2", [
                { name: "b", instances: [instance("/repos/r2", "develop")] },
            ]),
        ]
        expect(branchForWorktree("/repos/r2", views)).toBe("develop")
    })

    // Detached HEAD / bare worktree: the instance exists but names no branch.
    test("returns null when the matched instance carries no branch", () => {
        const views = [
            repoView("r1", [
                { name: "add-thing", instances: [instance("/repos/r1", null)] },
            ]),
        ]
        expect(branchForWorktree("/repos/r1", views)).toBeNull()
    })

    // A flat workspace has no git worktree identity, so its uri matches nothing.
    test("returns null for a flat workspace path", () => {
        const views = [
            flatView("/notes"),
            repoView("r1", [
                { name: "add-thing", instances: [instance("/repos/r1", "master")] },
            ]),
        ]
        expect(branchForWorktree("/notes", views)).toBeNull()
    })

    test("returns null for a path that matches no instance", () => {
        const views = [
            repoView("r1", [
                { name: "add-thing", instances: [instance("/repos/r1", "master")] },
            ]),
        ]
        expect(branchForWorktree("/repos/elsewhere", views)).toBeNull()
    })

    test("returns null when there are no views at all", () => {
        expect(branchForWorktree("/repos/r1", [])).toBeNull()
    })

    // Matching is exact, never by prefix: a sibling worktree whose path extends
    // another's would otherwise inherit the wrong branch.
    test("matches the worktree path exactly, not by prefix", () => {
        const views = [
            repoView("r1", [
                { name: "a", instances: [instance("/repos/r1", "master")] },
            ]),
        ]
        expect(branchForWorktree("/repos/r1-other", views)).toBeNull()
        expect(branchForWorktree("/repos/r1/nested", views)).toBeNull()
    })
})

// ---- changeDirectoryName -----------------------------------------------

describe("changeDirectoryName", () => {
    test("strips the archive/ prefix from an archived change id", () => {
        expect(changeDirectoryName("archive/2026-08-14-add-web-ui-touch-support")).toBe(
            "2026-08-14-add-web-ui-touch-support",
        )
    })

    test("leaves an active change id unchanged", () => {
        expect(changeDirectoryName("add-change-identity-headers")).toBe(
            "add-change-identity-headers",
        )
    })

    // The prefix is stripped only at the start — a change whose own name
    // contains the word must survive intact.
    test("only strips a leading prefix", () => {
        expect(changeDirectoryName("add-archive/browser")).toBe("add-archive/browser")
        expect(changeDirectoryName("archive-browser-fixes")).toBe("archive-browser-fixes")
    })

    test("strips exactly one prefix", () => {
        expect(changeDirectoryName("archive/archive/nested")).toBe("archive/nested")
    })
})

// ---- isArchivedChangeId ------------------------------------------------

describe("isArchivedChangeId", () => {
    test("recognises an archived change id", () => {
        expect(isArchivedChangeId("archive/2026-08-14-add-web-ui-touch-support")).toBe(
            true,
        )
    })

    test("rejects an active change id", () => {
        expect(isArchivedChangeId("add-change-identity-headers")).toBe(false)
    })

    // The guard is what keeps an archived change from being labelled with its
    // HOST worktree's branch, so a false negative shows a branch that was never
    // the archived change's. Anchored at the start for that reason.
    test("only matches a leading prefix", () => {
        expect(isArchivedChangeId("add-archive/browser")).toBe(false)
        expect(isArchivedChangeId("archive-browser-fixes")).toBe(false)
    })

    test("agrees with changeDirectoryName about what is archived", () => {
        for (const id of [
            "archive/2026-08-14-thing",
            "add-thing",
            "archive-browser-fixes",
        ]) {
            const stripped = changeDirectoryName(id) !== id
            expect(stripped).toBe(isArchivedChangeId(id))
        }
    })
})
