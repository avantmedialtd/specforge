import { describe, expect, test } from "bun:test"
import {
    branchChipForWorktree,
    changeDirectoryName,
    isArchivedChangeId,
} from "./changeIdentity"
import type {
    ChangeData,
    ChangeInstance,
    PaletteColor,
    WorkspaceView,
} from "./types"

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
    color: PaletteColor | null = null,
): WorkspaceView {
    return {
        kind: "repo",
        repoId,
        mainWorktree: `/repos/${repoId}`,
        name: repoId,
        defaultBranch: "master",
        active: logicals,
        displayName: null,
        color,
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

// ---- branchChipForWorktree ---------------------------------------------

describe("branchChipForWorktree", () => {
    test("returns the branch of the instance at that worktree path", () => {
        const views = [
            repoView("r1", [
                { name: "add-thing", instances: [instance("/repos/r1", "master")] },
            ]),
        ]
        expect(branchChipForWorktree("/repos/r1", views).branch).toBe("master")
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
        expect(branchChipForWorktree("/repos/r1/wt/feature", views).branch).toBe(
            "feature/x",
        )
    })

    test("scans every logical change, not only the first", () => {
        const views = [
            repoView("r1", [
                { name: "first", instances: [instance("/repos/r1", "master")] },
                { name: "second", instances: [instance("/repos/r1/wt/b", "topic-b")] },
            ]),
        ]
        expect(branchChipForWorktree("/repos/r1/wt/b", views).branch).toBe("topic-b")
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
        expect(branchChipForWorktree("/repos/r2", views).branch).toBe("develop")
    })

    // Detached HEAD / bare worktree: the instance exists but names no branch.
    test("returns a null branch when the matched instance carries no branch", () => {
        const views = [
            repoView("r1", [
                { name: "add-thing", instances: [instance("/repos/r1", null)] },
            ]),
        ]
        expect(branchChipForWorktree("/repos/r1", views).branch).toBeNull()
    })

    // A flat workspace has no git worktree identity, so its uri matches nothing.
    test("returns a null branch for a flat workspace path", () => {
        const views = [
            flatView("/notes"),
            repoView("r1", [
                { name: "add-thing", instances: [instance("/repos/r1", "master")] },
            ]),
        ]
        expect(branchChipForWorktree("/notes", views).branch).toBeNull()
    })

    test("returns a null branch for a path that matches no instance", () => {
        const views = [
            repoView("r1", [
                { name: "add-thing", instances: [instance("/repos/r1", "master")] },
            ]),
        ]
        expect(branchChipForWorktree("/repos/elsewhere", views).branch).toBeNull()
    })

    test("returns a null branch when there are no views at all", () => {
        expect(branchChipForWorktree("/repos/r1", []).branch).toBeNull()
    })

    // Matching is exact, never by prefix: a sibling worktree whose path extends
    // another's would otherwise inherit the wrong branch.
    test("matches the worktree path exactly, not by prefix", () => {
        const views = [
            repoView("r1", [
                { name: "a", instances: [instance("/repos/r1", "master")] },
            ]),
        ]
        expect(branchChipForWorktree("/repos/r1-other", views).branch).toBeNull()
        expect(branchChipForWorktree("/repos/r1/nested", views).branch).toBeNull()
    })

    // ---- palette colour ------------------------------------------------

    test("reports the owning workspace's palette colour beside the branch", () => {
        const views = [
            repoView(
                "r1",
                [{ name: "a", instances: [instance("/repos/r1", "master")] }],
                "indigo",
            ),
        ]
        expect(branchChipForWorktree("/repos/r1", views)).toEqual({
            branch: "master",
            color: "indigo",
        })
    })

    // No configured colour means the chip renders neutral — never a derived or
    // substituted one (`spec-browser`: *Change Identity Header in the Detail
    // Pane*, "the branch chip stays neutral when no palette colour is
    // configured").
    test("reports a null colour when the workspace has none configured", () => {
        const views = [
            repoView("r1", [
                { name: "a", instances: [instance("/repos/r1", "master")] },
            ]),
        ]
        expect(branchChipForWorktree("/repos/r1", views)).toEqual({
            branch: "master",
            color: null,
        })
    })

    // The whole point of resolving both from one match: the colour must belong
    // to the workspace that owns the matched worktree, not to whichever repo
    // the scan happened to reach first.
    test("takes the colour from the repo that matched, not the first scanned", () => {
        const views = [
            repoView(
                "r1",
                [{ name: "a", instances: [instance("/repos/r1", "master")] }],
                "rose",
            ),
            repoView(
                "r2",
                [{ name: "b", instances: [instance("/repos/r2", "develop")] }],
                "teal",
            ),
        ]
        expect(branchChipForWorktree("/repos/r2", views)).toEqual({
            branch: "develop",
            color: "teal",
        })
    })

    // Reported from whatever matched, even with no branch to show. The caller
    // decides to render nothing; the lookup does not withhold a fact it holds.
    test("reports the colour even when the matched instance names no branch", () => {
        const views = [
            repoView(
                "r1",
                [{ name: "a", instances: [instance("/repos/r1", null)] }],
                "amber",
            ),
        ]
        expect(branchChipForWorktree("/repos/r1", views)).toEqual({
            branch: null,
            color: "amber",
        })
    })

    test("reports a null colour when nothing matched at all", () => {
        const views = [
            repoView(
                "r1",
                [{ name: "a", instances: [instance("/repos/r1", "master")] }],
                "purple",
            ),
        ]
        expect(branchChipForWorktree("/repos/elsewhere", views)).toEqual({
            branch: null,
            color: null,
        })
        expect(branchChipForWorktree("/repos/r1", [])).toEqual({
            branch: null,
            color: null,
        })
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
