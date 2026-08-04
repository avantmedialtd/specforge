import { describe, expect, test } from "bun:test"
import {
    artifactNodeId,
    changeRowId,
    flatWorkspaceId,
    instanceId,
    logicalChangeId,
    repoId,
    specNodeId,
} from "../components/WorkspaceTree"
import type { ArtifactStatus, ChangeData, ChangeInstance, WorkspaceView } from "../types"
import { addressToNodePath } from "./nodeId"
import { instanceToken } from "./slug"

// ---- Fixture builders (mirrors routing/slug.test.ts's shape) -----------

const PRESENT: ArtifactStatus = { proposal: true, design: true, tasks: true, specs: ["view-routing"] }

function change(changeId: string, artifacts: ArtifactStatus = PRESENT): ChangeData {
    return {
        changeId,
        title: null,
        sections: [],
        totalTasks: 0,
        completedTasks: 0,
        artifacts,
        workspace: { uri: `/ws/${changeId}`, name: changeId },
    }
}

function flatView(uri: string, name: string, changes: ChangeData[]): WorkspaceView {
    return { kind: "flat", workspace: { uri, name }, changes, displayName: null, color: null }
}

function instance(worktreePath: string, changeId: string): ChangeInstance {
    return {
        worktreePath,
        branch: null,
        isMainWorktree: false,
        isDefaultBranch: false,
        isArchivedHere: false,
        change: change(changeId),
        modifiedAt: 0,
        divergence: null,
        specCommitState: "committed",
    }
}

function repoView(
    id: string,
    name: string,
    mainWorktree: string,
    active: { name: string; instances: ChangeInstance[] }[],
): WorkspaceView {
    return {
        kind: "repo",
        repoId: id,
        mainWorktree,
        name,
        defaultBranch: "main",
        active,
        displayName: null,
        color: null,
        dirty: false,
        dirtyWorktrees: [],
        hasUncommittedSpecs: false,
    }
}

// ---- Tests ---------------------------------------------------------------

describe("addressToNodePath", () => {
    test("home / settings / archive / an ambiguous or not-found address all reveal nothing", () => {
        const views: WorkspaceView[] = [flatView("/a", "myproject", [change("chg")])]
        expect(addressToNodePath({ kind: "home" }, views)).toBeNull()
        expect(addressToNodePath({ kind: "settings" }, views)).toBeNull()
        expect(addressToNodePath({ kind: "archive", selection: null }, views)).toBeNull()
        expect(
            addressToNodePath(
                { kind: "files", scope: { kind: "workspace", workspace: "nope" } },
                views,
            ),
        ).toBeNull()
    })

    test("a flat workspace files address reveals a one-element path", () => {
        const views: WorkspaceView[] = [flatView("/a", "myproject", [])]
        const address = { kind: "files" as const, scope: { kind: "workspace" as const, workspace: "myproject" } }
        expect(addressToNodePath(address, views)).toEqual([flatWorkspaceId("/a")])
    })

    test("a repo files address reveals a one-element path", () => {
        const views: WorkspaceView[] = [repoView("/r/.git", "myrepo", "/r", [])]
        const address = { kind: "files" as const, scope: { kind: "repo" as const, repo: "myrepo" } }
        expect(addressToNodePath(address, views)).toEqual([repoId("/r/.git")])
    })

    test("a flat workspace artifact address reveals [workspace, changeRow, artifact] — matching WorkspaceTree's exact (containerId-doubled) leaf id scheme", () => {
        const views: WorkspaceView[] = [flatView("/a", "myproject", [change("chg")])]
        const address = {
            kind: "artifact" as const,
            scope: { kind: "workspace" as const, workspace: "myproject" },
            changeId: "chg",
            artifactKind: "design" as const,
        }
        const wsId = flatWorkspaceId("/a")
        const changeId = changeRowId(wsId, "chg")
        expect(addressToNodePath(address, views)).toEqual([
            wsId,
            changeId,
            artifactNodeId(changeId, "chg", "design"),
        ])
    })

    test("a flat workspace spec address reveals [workspace, changeRow, specsRow, spec]", () => {
        const views: WorkspaceView[] = [flatView("/a", "myproject", [change("chg")])]
        const address = {
            kind: "artifact" as const,
            scope: { kind: "workspace" as const, workspace: "myproject" },
            changeId: "chg",
            artifactKind: "spec" as const,
            capability: "view-routing",
        }
        const wsId = flatWorkspaceId("/a")
        const changeId = changeRowId(wsId, "chg")
        expect(addressToNodePath(address, views)).toEqual([
            wsId,
            changeId,
            artifactNodeId(changeId, "chg", "specs"),
            specNodeId(changeId, "chg", "view-routing"),
        ])
    })

    test("a single-instance repo change reveals [repo, logicalChange, instance, artifact]", () => {
        const inst = instance("/repo", "chg")
        const views: WorkspaceView[] = [
            repoView("/r/.git", "myrepo", "/repo", [{ name: "chg", instances: [inst] }]),
        ]
        const address = {
            kind: "artifact" as const,
            scope: { kind: "repo" as const, repo: "myrepo" },
            changeId: "chg",
            artifactKind: "proposal" as const,
        }
        expect(addressToNodePath(address, views)).toEqual([
            repoId("/r/.git"),
            logicalChangeId("/r/.git", "chg"),
            instanceId("/r/.git", "chg", "/repo"),
            artifactNodeId(instanceId("/r/.git", "chg", "/repo"), "chg", "proposal"),
        ])
    })

    test("a multi-instance repo change reveals the addressed instance specifically", () => {
        const a = instance("/wt-a", "chg")
        const b = instance("/wt-b", "chg")
        const views: WorkspaceView[] = [
            repoView("/r/.git", "myrepo", "/wt-a", [{ name: "chg", instances: [a, b] }]),
        ]
        const token = instanceToken("/wt-b", [a, b])
        const address = {
            kind: "artifact" as const,
            scope: { kind: "repo" as const, repo: "myrepo", instance: token },
            changeId: "chg",
            artifactKind: "tasks" as const,
        }
        expect(addressToNodePath(address, views)).toEqual([
            repoId("/r/.git"),
            logicalChangeId("/r/.git", "chg"),
            instanceId("/r/.git", "chg", "/wt-b"),
            artifactNodeId(instanceId("/r/.git", "chg", "/wt-b"), "chg", "tasks"),
        ])
    })

    test("a stale/unresolvable address reveals nothing", () => {
        const views: WorkspaceView[] = [flatView("/a", "myproject", [change("chg")])]
        const address = {
            kind: "artifact" as const,
            scope: { kind: "workspace" as const, workspace: "myproject" },
            changeId: "does-not-exist",
            artifactKind: "proposal" as const,
        }
        expect(addressToNodePath(address, views)).toBeNull()
    })

    // B2: the regression this whole module exists to guard against — a
    // worktree hosting more than one active change must reveal the ONE
    // actually addressed, not whichever happens to be first.
    describe("a worktree hosting two active changes (B2)", () => {
        const instA = instance("/proj", "add-a")
        const instB = instance("/proj", "add-b")
        const views: WorkspaceView[] = [
            repoView("/r/.git", "myrepo", "/proj", [
                { name: "add-a", instances: [instA] },
                { name: "add-b", instances: [instB] },
            ]),
        ]

        test("addressing add-a reveals add-a's own path, not add-b's", () => {
            const address = {
                kind: "artifact" as const,
                scope: { kind: "repo" as const, repo: "myrepo" },
                changeId: "add-a",
                artifactKind: "proposal" as const,
            }
            expect(addressToNodePath(address, views)).toEqual([
                repoId("/r/.git"),
                logicalChangeId("/r/.git", "add-a"),
                instanceId("/r/.git", "add-a", "/proj"),
                artifactNodeId(instanceId("/r/.git", "add-a", "/proj"), "add-a", "proposal"),
            ])
        })

        test("addressing add-b reveals add-b's own path, not add-a's", () => {
            const address = {
                kind: "artifact" as const,
                scope: { kind: "repo" as const, repo: "myrepo" },
                changeId: "add-b",
                artifactKind: "tasks" as const,
            }
            const path = addressToNodePath(address, views)
            expect(path).toEqual([
                repoId("/r/.git"),
                logicalChangeId("/r/.git", "add-b"),
                instanceId("/r/.git", "add-b", "/proj"),
                artifactNodeId(instanceId("/r/.git", "add-b", "/proj"), "add-b", "tasks"),
            ])
            // The exact failure mode this guards against: mixing add-a's
            // logicalChangeName/instance container with add-b's changeId,
            // producing an id that matches no real row anywhere.
            expect(path).not.toContain(logicalChangeId("/r/.git", "add-a"))
            expect(path!.some((id) => id.includes("lc:add-a") && id.includes("change:add-b"))).toBe(
                false,
            )
        })
    })

    // A2: node ids embed absolute filesystem paths, so a naive "/"-split
    // ancestor derivation can equal a DIFFERENT real node's id whenever one
    // registered path is a directory prefix of another (this repo's own
    // `.claude/worktrees/<name>` layout is exactly that shape). Building the
    // path by construction, never by splitting a leaf id string, must not
    // reproduce that collision.
    describe("a worktree nested inside the repo's main worktree (A2)", () => {
        const mainWt = "/Users/istvan/Developer/specforge"
        const nestedWt = "/Users/istvan/Developer/specforge/.claude/worktrees/add-view-routing"
        const mainInst = instance(mainWt, "add-view-routing")
        const nestedInst = instance(nestedWt, "add-view-routing")
        const views: WorkspaceView[] = [
            repoView("/r/.git", "specforge", mainWt, [
                { name: "add-view-routing", instances: [mainInst, nestedInst] },
            ]),
        ]

        test("revealing the nested worktree's artifact never includes the main worktree instance's id", () => {
            const token = instanceToken(nestedWt, [mainInst, nestedInst])
            const address = {
                kind: "artifact" as const,
                scope: { kind: "repo" as const, repo: "specforge", instance: token },
                changeId: "add-view-routing",
                artifactKind: "proposal" as const,
            }
            const path = addressToNodePath(address, views)
            expect(path).not.toBeNull()
            const mainInstanceId = instanceId("/r/.git", "add-view-routing", mainWt)
            expect(path).not.toContain(mainInstanceId)
            // Confirm the fixture actually exercises the collision: the main
            // worktree instance id IS a literal "/"-prefix of the nested
            // one's — the exact shape a blind string-split must not produce.
            const nestedInstanceId = instanceId("/r/.git", "add-view-routing", nestedWt)
            expect(nestedInstanceId.startsWith(`${mainInstanceId}/`)).toBe(true)
        })
    })
})
