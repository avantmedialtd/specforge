import { describe, expect, test } from "bun:test"
import type { ArtifactStatus, ChangeData, ChangeInstance, WorkspaceView } from "../types"
import { encodeAddress } from "./codec"
import { resolveAddress } from "./resolve"
import { instanceToken, scopeFor } from "./slug"

// ---- Fixture builders (mirrors routing/slug.test.ts's / nodeId.test.ts's shape) ----

const PRESENT: ArtifactStatus = { proposal: true, design: true, tasks: true, specs: [] }

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

function flatView(uri: string, name: string, changes: ChangeData[] = []): WorkspaceView {
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
    active: { name: string; instances: ChangeInstance[] }[] = [],
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

// ---- C1: archive resolution across a cross-kind slug collision --------

describe("resolveArchive across a same-named flat workspace and repo (C1)", () => {
    test("an unqualified archive address is ambiguous, and its candidates never encode to the same path", () => {
        const ws = flatView("/a", "specs")
        const repo = repoView("/repo/.git", "specs", "/repo")
        const views = [ws, repo]

        const result = resolveAddress(
            { kind: "archive", selection: { workspace: "specs", archiveDir: "2026-01-01-x" } },
            views,
        )

        expect(result.status).toBe("ambiguous")
        if (result.status !== "ambiguous") return
        expect(result.candidates.length).toBe(2)

        const paths = result.candidates.map((c) => encodeAddress(c.address))
        expect(new Set(paths).size).toBe(paths.length)

        // Picking either candidate must resolve UNIQUELY, not loop back to
        // the same "which one?" chooser — the actual bug's symptom.
        for (const candidate of result.candidates) {
            const reResolved = resolveAddress(candidate.address, views)
            expect(reResolved.status).toBe("resolved")
        }
    })

    test("a flat workspace and repo with DIFFERENT names never become ambiguous with each other", () => {
        const ws = flatView("/a", "myproject")
        const repo = repoView("/repo/.git", "otherproject", "/repo")
        const views = [ws, repo]

        const result = resolveAddress(
            { kind: "archive", selection: { workspace: "myproject", archiveDir: "2026-01-01-x" } },
            views,
        )
        expect(result.status).toBe("resolved")
    })
})

// ---- B3: ambiguity candidates must preserve an instance segment -------

describe("resolveArtifact candidate construction preserves the instance segment (B3)", () => {
    test("a scope-level collision candidate keeps the original address's instance token", () => {
        // Two repos happen to share a slug ("bar") — a genuine scope-level
        // collision — and repoA has a multi-instance change the original
        // address already named one specific instance of.
        const instA1 = instance("/a/wt1", "mychange")
        const instA2 = instance("/a/wt2", "mychange")
        const repoA = repoView("/a/.git", "bar", "/a/wt1", [
            { name: "mychange", instances: [instA1, instA2] },
        ])
        const repoB = repoView("/b/.git", "bar", "/b")
        const views = [repoA, repoB]

        const token = instanceToken("/a/wt2", [instA1, instA2])!

        const address = {
            kind: "artifact" as const,
            scope: { kind: "repo" as const, repo: "bar", instance: token },
            changeId: "mychange",
            artifactKind: "proposal" as const,
        }

        const result = resolveAddress(address, views)
        expect(result.status).toBe("ambiguous")
        if (result.status !== "ambiguous") return
        expect(result.candidates.length).toBe(2)

        for (const candidate of result.candidates) {
            expect(candidate.address.kind).toBe("artifact")
            if (candidate.address.kind !== "artifact") continue
            expect(candidate.address.scope.kind).toBe("repo")
            if (candidate.address.scope.kind !== "repo") continue
            // The instance token must survive the scope rebuild — dropping
            // it would force a second, spurious instance chooser once this
            // candidate resolves further.
            expect(candidate.address.scope.instance).toBe(token)
        }

        // Picking the candidate that's ACTUALLY repoA must resolve directly
        // to the instance the original address named — no second chooser.
        // (Identify it by its OWN currently-assigned slug, not by literal
        // string content — both repos are named "bar", so only `scopeFor`
        // against the live view set says which suffixed slug is which.)
        const repoASlug = scopeFor(repoA, views)
        expect(repoASlug.kind).toBe("repo")
        const repoACandidate = result.candidates.find(
            (c) =>
                c.address.kind === "artifact" &&
                c.address.scope.kind === "repo" &&
                repoASlug.kind === "repo" &&
                c.address.scope.repo === repoASlug.repo,
        )
        expect(repoACandidate).toBeDefined()
        const reResolved = resolveAddress(repoACandidate!.address, views)
        expect(reResolved.status).toBe("resolved")
        if (reResolved.status === "resolved" && reResolved.view.kind === "target") {
            const target = reResolved.view.target
            expect(target.kind).toBe("artifact")
            if (target.kind === "artifact") expect(target.workspace).toBe("/a/wt2")
        }
    })
})
