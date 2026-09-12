import { describe, expect, test } from "bun:test"
import {
    changeRowId,
    flatWorkspaceId,
    logicalChangeId,
    repoId,
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
        worktrees: [mainWorktree],
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

    test("a flat workspace artifact address reveals [workspace, changeRow] and stops there", () => {
        const views: WorkspaceView[] = [flatView("/a", "myproject", [change("chg")])]
        const address = {
            kind: "artifact" as const,
            scope: { kind: "workspace" as const, workspace: "myproject" },
            changeId: "chg",
            artifactKind: "design" as const,
        }
        const wsId = flatWorkspaceId("/a")
        expect(addressToNodePath(address, views)).toEqual([wsId, changeRowId(wsId, "chg")])
    })

    // The tree stops at the change row, so an artifact address and a spec
    // address of the SAME change reveal the identical path — which artifact is
    // shown is the change header's tab strip's business, not the tree's.
    test("a spec address reveals the same two-element path as any other artifact of that change", () => {
        const views: WorkspaceView[] = [flatView("/a", "myproject", [change("chg")])]
        const base = {
            kind: "artifact" as const,
            scope: { kind: "workspace" as const, workspace: "myproject" },
            changeId: "chg",
        }
        const wsId = flatWorkspaceId("/a")
        expect(
            addressToNodePath(
                { ...base, artifactKind: "spec" as const, capability: "view-routing" },
                views,
            ),
        ).toEqual([wsId, changeRowId(wsId, "chg")])
        expect(
            addressToNodePath({ ...base, artifactKind: "proposal" as const }, views),
        ).toEqual([wsId, changeRowId(wsId, "chg")])
    })

    test("a single-instance repo change reveals [repo, logicalChange]", () => {
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
        ])
    })

    // A change living in several worktrees is ONE row, so an address naming a
    // particular instance reveals that same row — the instance it names is
    // marked in the change header's switcher instead.
    test("a multi-instance repo change reveals the one logical-change row, whichever instance is addressed", () => {
        const a = instance("/wt-a", "chg")
        const b = instance("/wt-b", "chg")
        const views: WorkspaceView[] = [
            repoView("/r/.git", "myrepo", "/wt-a", [{ name: "chg", instances: [a, b] }]),
        ]
        const expected = [repoId("/r/.git"), logicalChangeId("/r/.git", "chg")]
        for (const worktree of ["/wt-a", "/wt-b"]) {
            const address = {
                kind: "artifact" as const,
                scope: {
                    kind: "repo" as const,
                    repo: "myrepo",
                    instance: instanceToken(worktree, [a, b]),
                },
                changeId: "chg",
                artifactKind: "tasks" as const,
            }
            expect(addressToNodePath(address, views)).toEqual(expected)
        }
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

        test("addressing add-a reveals add-a's own row, not add-b's", () => {
            const address = {
                kind: "artifact" as const,
                scope: { kind: "repo" as const, repo: "myrepo" },
                changeId: "add-a",
                artifactKind: "proposal" as const,
            }
            expect(addressToNodePath(address, views)).toEqual([
                repoId("/r/.git"),
                logicalChangeId("/r/.git", "add-a"),
            ])
        })

        test("addressing add-b reveals add-b's own row, not add-a's", () => {
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
            ])
            expect(path).not.toContain(logicalChangeId("/r/.git", "add-a"))
        })
    })

    // A2: node ids embed absolute filesystem paths, so a naive "/"-split
    // ancestor derivation can equal a DIFFERENT real node's id whenever one
    // registered path is a directory prefix of another (this repo's own
    // `.claude/worktrees/<name>` layout is exactly that shape). The path is
    // shorter now, but the collision is still reachable at the CONTAINER
    // level: two flat workspaces, one nested inside the other.
    describe("a workspace nested inside another registered workspace (A2)", () => {
        const outer = "/Users/istvan/Developer/specforge"
        const nested = "/Users/istvan/Developer/specforge/.claude/worktrees/add-view-routing"
        const views: WorkspaceView[] = [
            flatView(outer, "specforge", [change("add-view-routing")]),
            flatView(nested, "add-view-routing-wt", [change("add-view-routing")]),
        ]

        test("revealing the nested workspace's artifact never includes the outer workspace's id", () => {
            const address = {
                kind: "artifact" as const,
                scope: { kind: "workspace" as const, workspace: "add-view-routing-wt" },
                changeId: "add-view-routing",
                artifactKind: "proposal" as const,
            }
            const path = addressToNodePath(address, views)
            expect(path).toEqual([
                flatWorkspaceId(nested),
                changeRowId(flatWorkspaceId(nested), "add-view-routing"),
            ])
            expect(path).not.toContain(flatWorkspaceId(outer))
            // Confirm the fixture actually exercises the collision: the outer
            // workspace's container id IS a literal "/"-prefix of the nested
            // one's — the exact shape a blind string-split must not produce.
            expect(flatWorkspaceId(nested).startsWith(`${flatWorkspaceId(outer)}/`)).toBe(true)
        })
    })
})
