import { describe, expect, test } from "bun:test"
import {
    artifactNodeId,
    changeRowId,
    flatWorkspaceId,
    instanceId,
    repoId,
    specNodeId,
} from "../components/WorkspaceTree"
import type { ArtifactStatus, ChangeData, ChangeInstance, WorkspaceView } from "../types"
import { addressToNodeId } from "./nodeId"
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

describe("addressToNodeId", () => {
    test("home / settings / archive / an ambiguous or not-found address all reveal nothing", () => {
        const views: WorkspaceView[] = [flatView("/a", "myproject", [change("chg")])]
        expect(addressToNodeId({ kind: "home" }, views)).toBeNull()
        expect(addressToNodeId({ kind: "settings" }, views)).toBeNull()
        expect(addressToNodeId({ kind: "archive", selection: null }, views)).toBeNull()
        expect(
            addressToNodeId(
                { kind: "files", scope: { kind: "workspace", workspace: "nope" } },
                views,
            ),
        ).toBeNull()
    })

    test("a flat workspace files address reveals the flat-workspace node", () => {
        const views: WorkspaceView[] = [flatView("/a", "myproject", [])]
        const address = { kind: "files" as const, scope: { kind: "workspace" as const, workspace: "myproject" } }
        expect(addressToNodeId(address, views)).toBe(flatWorkspaceId("/a"))
    })

    test("a repo files address reveals the repo node", () => {
        const views: WorkspaceView[] = [repoView("/r/.git", "myrepo", "/r", [])]
        const address = { kind: "files" as const, scope: { kind: "repo" as const, repo: "myrepo" } }
        expect(addressToNodeId(address, views)).toBe(repoId("/r/.git"))
    })

    test("a flat workspace artifact address reveals the artifact node — matching WorkspaceTree's exact (containerId-doubled) id scheme", () => {
        const views: WorkspaceView[] = [flatView("/a", "myproject", [change("chg")])]
        const address = {
            kind: "artifact" as const,
            scope: { kind: "workspace" as const, workspace: "myproject" },
            changeId: "chg",
            artifactKind: "design" as const,
        }
        // Cross-check against literally how FlatChangeNode/ArtifactNode
        // build the id at render time: ArtifactSubtree's `containerId` is
        // FlatChangeNode's OWN nodeId, i.e. changeRowId(flatWorkspaceId(uri), changeId).
        const expected = artifactNodeId(changeRowId(flatWorkspaceId("/a"), "chg"), "chg", "design")
        expect(addressToNodeId(address, views)).toBe(expected)
    })

    test("a flat workspace spec address reveals the spec node", () => {
        const views: WorkspaceView[] = [flatView("/a", "myproject", [change("chg")])]
        const address = {
            kind: "artifact" as const,
            scope: { kind: "workspace" as const, workspace: "myproject" },
            changeId: "chg",
            artifactKind: "spec" as const,
            capability: "view-routing",
        }
        const expected = specNodeId(changeRowId(flatWorkspaceId("/a"), "chg"), "chg", "view-routing")
        expect(addressToNodeId(address, views)).toBe(expected)
    })

    test("a single-instance repo change reveals the instance's artifact node", () => {
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
        const expected = artifactNodeId(instanceId("/r/.git", "chg", "/repo"), "chg", "proposal")
        expect(addressToNodeId(address, views)).toBe(expected)
    })

    test("a multi-instance repo change reveals the addressed instance specifically", () => {
        const a = instance("/wt-a", "chg")
        const b = instance("/wt-b", "chg")
        const views: WorkspaceView[] = [
            repoView("/r/.git", "myrepo", "/wt-a", [{ name: "chg", instances: [a, b] }]),
        ]
        // Resolve the address for instance b explicitly by round-tripping
        // through slug.ts's own instanceToken, mirroring how resolve.ts
        // would have produced it.
        const token = instanceToken("/wt-b", [a, b])
        const address = {
            kind: "artifact" as const,
            scope: { kind: "repo" as const, repo: "myrepo", instance: token },
            changeId: "chg",
            artifactKind: "tasks" as const,
        }
        const expected = artifactNodeId(instanceId("/r/.git", "chg", "/wt-b"), "chg", "tasks")
        expect(addressToNodeId(address, views)).toBe(expected)
    })

    test("a stale/unresolvable address reveals nothing", () => {
        const views: WorkspaceView[] = [flatView("/a", "myproject", [change("chg")])]
        const address = {
            kind: "artifact" as const,
            scope: { kind: "workspace" as const, workspace: "myproject" },
            changeId: "does-not-exist",
            artifactKind: "proposal" as const,
        }
        expect(addressToNodeId(address, views)).toBeNull()
    })
})
