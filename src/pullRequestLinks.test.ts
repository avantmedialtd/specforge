import { describe, expect, test } from "bun:test"
import {
    EMPTY_LINKS,
    headerChips,
    linksForWorktree,
    MAX_HEADER_CHIPS,
    switcherMarkerText,
    worktreeDestination,
    worktreeMarker,
    worktreeName,
    worktreesForPullRequest,
} from "./pullRequestLinks"
import type {
    ArtifactStatus,
    ChangeData,
    ChangeInstance,
    LinkedPullRequest,
    LinkedWorktree,
    PullRequestLinks,
    WorkspaceView,
} from "./types"

// ---- Fixture builders --------------------------------------------------

const PROPOSAL_ONLY: ArtifactStatus = { proposal: true, design: false, tasks: false, specs: [] }
const DESIGN_ONLY: ArtifactStatus = { proposal: false, design: true, tasks: false, specs: [] }
const NOTHING: ArtifactStatus = { proposal: false, design: false, tasks: false, specs: [] }

function change(artifacts: ArtifactStatus): ChangeData {
    return { artifacts, completedTasks: 0, totalTasks: 0 } as unknown as ChangeData
}

function instance(
    worktreePath: string,
    modifiedAt: number,
    artifacts: ArtifactStatus = PROPOSAL_ONLY,
): ChangeInstance {
    return {
        worktreePath,
        branch: "feature",
        isMainWorktree: false,
        isDefaultBranch: false,
        isArchivedHere: false,
        change: change(artifacts),
        modifiedAt,
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
        mainWorktree: "/code/api",
        name: "api",
        defaultBranch: "main",
        active: logicals,
        displayName: null,
        color: null,
        dirty: false,
        dirtyWorktrees: [],
        hasUncommittedSpecs: false,
        worktrees: ["/code/api", "/code/api-wt"],
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

function linked(id: number, overrides: Partial<LinkedPullRequest> = {}): LinkedPullRequest {
    return {
        provider: "github",
        role: "authored",
        id,
        title: `PR ${id}`,
        url: `https://github.com/acme/api/pull/${id}`,
        repoFullName: "acme/api",
        draft: false,
        checks: null,
        conflicting: false,
        review: null,
        ...overrides,
    }
}

function worktree(worktreePath: string, branch: string | null = "feature"): LinkedWorktree {
    return { repoId: "/code/api/.git", worktreePath, branch }
}

const REPO = "/code/api/.git"
const WT = "/code/api-wt"

// ---- lookups -----------------------------------------------------------

describe("linksForWorktree", () => {
    const links: PullRequestLinks = {
        worktrees: [{ worktreePath: WT, pullRequests: [linked(7), linked(9)] }],
        pullRequests: [],
    }

    test("returns the worktree's pull requests in snapshot order", () => {
        expect(linksForWorktree(links, WT).map((pr) => pr.id)).toEqual([7, 9])
    })

    test("an unlinked worktree, an empty snapshot and no snapshot link nothing", () => {
        expect(linksForWorktree(links, "/code/api")).toEqual([])
        expect(linksForWorktree(EMPTY_LINKS, WT)).toEqual([])
        expect(linksForWorktree(null, WT)).toEqual([])
        expect(linksForWorktree(undefined, WT)).toEqual([])
    })
})

describe("worktreesForPullRequest", () => {
    const url = "https://github.com/acme/api/pull/7"
    const links: PullRequestLinks = {
        worktrees: [],
        pullRequests: [{ url, worktrees: [worktree("/code/api", "main"), worktree(WT)] }],
    }

    test("returns the pull request's worktrees in snapshot order", () => {
        expect(worktreesForPullRequest(links, url).map((w) => w.worktreePath)).toEqual([
            "/code/api",
            WT,
        ])
    })

    test("an unlinked URL, the empty URL and no snapshot link nothing", () => {
        expect(worktreesForPullRequest(links, "https://github.com/acme/api/pull/8")).toEqual([])
        expect(worktreesForPullRequest(links, "")).toEqual([])
        expect(
            worktreesForPullRequest({ worktrees: [], pullRequests: [{ url: "", worktrees: [worktree(WT)] }] }, ""),
        ).toEqual([])
        expect(worktreesForPullRequest(null, url)).toEqual([])
    })
})

// ---- where the marker lands --------------------------------------------

describe("worktreeDestination", () => {
    test("one active change in the worktree opens its default artifact there", () => {
        const views = [repoView(REPO, [{ name: "add-rate-limits", instances: [instance(WT, 10)] }])]
        expect(worktreeDestination(views, REPO, WT)).toEqual({
            kind: "artifact",
            workspace: WT,
            changeId: "add-rate-limits",
            artifactKind: "proposal",
        })
    })

    test("several changes: the most recently modified wins, with its own default artifact", () => {
        const views = [
            repoView(REPO, [
                { name: "older", instances: [instance(WT, 10)] },
                { name: "newer", instances: [instance(WT, 50, DESIGN_ONLY)] },
                { name: "middle", instances: [instance(WT, 30)] },
            ]),
        ]
        expect(worktreeDestination(views, REPO, WT)).toEqual({
            kind: "artifact",
            workspace: WT,
            changeId: "newer",
            artifactKind: "design",
        })
    })

    test("a tie keeps aggregation order", () => {
        const views = [
            repoView(REPO, [
                { name: "first", instances: [instance(WT, 20)] },
                { name: "second", instances: [instance(WT, 20)] },
            ]),
        ]
        expect(worktreeDestination(views, REPO, WT)).toMatchObject({ changeId: "first" })
    })

    test("a change hosted only in another worktree does not count", () => {
        const views = [
            repoView(REPO, [
                { name: "elsewhere", instances: [instance("/code/api", 99)] },
                { name: "here", instances: [instance(WT, 1)] },
            ]),
        ]
        expect(worktreeDestination(views, REPO, WT)).toMatchObject({ changeId: "here", workspace: WT })
    })

    test("a spec-only change carries its capability", () => {
        const views = [
            repoView(REPO, [
                {
                    name: "specs-only",
                    instances: [instance(WT, 1, { proposal: false, design: false, tasks: false, specs: ["web-ui"] })],
                },
            ]),
        ]
        expect(worktreeDestination(views, REPO, WT)).toEqual({
            kind: "artifact",
            workspace: WT,
            changeId: "specs-only",
            artifactKind: "spec",
            capability: "web-ui",
        })
    })

    test("a worktree hosting no change opens the repository's file browser", () => {
        const views = [repoView(REPO, [{ name: "elsewhere", instances: [instance("/code/api", 5)] }])]
        expect(worktreeDestination(views, REPO, WT)).toEqual({ kind: "files", root: REPO })
    })

    test("a change with no artifact to open falls back to the file browser too", () => {
        const views = [repoView(REPO, [{ name: "empty", instances: [instance(WT, 5, NOTHING)] }])]
        expect(worktreeDestination(views, REPO, WT)).toEqual({ kind: "files", root: REPO })
    })

    test("an unknown repository — disabled or removed — lands nowhere", () => {
        const views = [repoView(REPO, [{ name: "x", instances: [instance(WT, 1)] }]), flatView("/flat")]
        expect(worktreeDestination(views, "/other/.git", WT)).toBeNull()
        expect(worktreeDestination([], REPO, WT)).toBeNull()
        // A flat workspace is never a repository, even at the same path.
        expect(worktreeDestination([flatView(REPO)], REPO, WT)).toBeNull()
    })
})

// ---- wording -------------------------------------------------------------

describe("worktreeMarker", () => {
    test("names the first worktree by branch and lists every one", () => {
        expect(worktreeMarker([worktree("/code/api", "main"), worktree(WT, "feature")])).toEqual({
            text: "main",
            label: "Open in SpecForge: main (/code/api), feature (/code/api-wt)",
        })
    })

    test("a branchless worktree is named by its folder", () => {
        expect(worktreeName(worktree("/code/api-wt/", null))).toBe("api-wt")
        expect(worktreeMarker([worktree(WT, null)])?.text).toBe("api-wt")
    })

    test("no worktree, no marker", () => {
        expect(worktreeMarker([])).toBeNull()
    })
})

describe("switcherMarkerText", () => {
    test("the first number, then +N for the rest", () => {
        expect(switcherMarkerText([7])).toBe("#7")
        expect(switcherMarkerText([7, 9])).toBe("#7 +1")
        expect(switcherMarkerText([7, 9, 11])).toBe("#7 +2")
    })

    test("nothing linked, no marker", () => {
        expect(switcherMarkerText([])).toBeNull()
    })
})

describe("headerChips", () => {
    test("shows up to two and folds the rest into the summary", () => {
        const three = [linked(1), linked(2), linked(3)]
        const { shown, overflow } = headerChips(three)
        expect(MAX_HEADER_CHIPS).toBe(2)
        expect(shown.map((pr) => pr.id)).toEqual([1, 2])
        expect(overflow.map((pr) => pr.id)).toEqual([3])
    })

    test("two or fewer leave nothing to summarise", () => {
        expect(headerChips([linked(1), linked(2)]).overflow).toEqual([])
        expect(headerChips([])).toEqual({ shown: [], overflow: [] })
    })
})
