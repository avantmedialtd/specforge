import { describe, expect, test } from "bun:test"
import {
    artifactTabKey,
    artifactTabs,
    defaultArtifactFor,
    defaultInstanceFor,
    instanceOptions,
    worktreeBasename,
} from "./changeNavigation"
import type { ArtifactStatus, ChangeData, ChangeInstance } from "./types"

// ---- Fixture builders (mirrors routing/nodeId.test.ts's shape) -----------

const NONE: ArtifactStatus = { proposal: false, design: false, tasks: false, specs: [] }

function status(overrides: Partial<ArtifactStatus> = {}): ArtifactStatus {
    return { ...NONE, ...overrides }
}

function change(changeId: string, artifacts: ArtifactStatus): ChangeData {
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

function instance(
    worktreePath: string,
    overrides: Partial<ChangeInstance> = {},
): ChangeInstance {
    return {
        worktreePath,
        branch: null,
        isMainWorktree: false,
        isDefaultBranch: false,
        isArchivedHere: false,
        change: change("chg", status({ proposal: true })),
        modifiedAt: 0,
        divergence: null,
        specCommitState: "committed",
        ...overrides,
    }
}

// ---- artifactTabs --------------------------------------------------------

describe("artifactTabs", () => {
    test("offers Proposal, Design, Tasks, then specs in listing order", () => {
        const tabs = artifactTabs(
            status({ proposal: true, design: true, tasks: true, specs: ["b-cap", "a-cap"] }),
        )
        expect(tabs.map((t) => t.label)).toEqual([
            "Proposal",
            "Design",
            "Tasks",
            "b-cap",
            "a-cap",
        ])
        expect(tabs.map((t) => t.kind)).toEqual([
            "proposal",
            "design",
            "tasks",
            "spec",
            "spec",
        ])
    })

    test("an absent artifact gets no tab at all, dimmed or otherwise", () => {
        const tabs = artifactTabs(status({ proposal: true, tasks: true, specs: ["one", "two"] }))
        expect(tabs.map((t) => t.label)).toEqual(["Proposal", "Tasks", "one", "two"])
        expect(tabs.some((t) => t.kind === "design")).toBe(false)
    })

    test("an empty status offers nothing", () => {
        expect(artifactTabs(NONE)).toEqual([])
    })

    test("specs with no proposal still lead with the specs", () => {
        const tabs = artifactTabs(status({ specs: ["view-routing"] }))
        expect(tabs).toHaveLength(1)
        expect(tabs[0]).toMatchObject({
            kind: "spec",
            capability: "view-routing",
            label: "view-routing",
        })
    })

    test("a spec's key is prefixed, so a capability named like a fixed kind cannot collide", () => {
        const tabs = artifactTabs(status({ tasks: true, specs: ["tasks"] }))
        expect(tabs.map((t) => t.key)).toEqual(["tasks", "spec:tasks"])
        expect(new Set(tabs.map((t) => t.key)).size).toBe(2)
    })

    test("artifactTabKey agrees with the keys the strip emits", () => {
        const tabs = artifactTabs(status({ design: true, specs: ["cap"] }))
        expect(tabs.map((t) => t.key)).toEqual([
            artifactTabKey("design"),
            artifactTabKey("spec", "cap"),
        ])
    })
})

// ---- defaultArtifactFor --------------------------------------------------

describe("defaultArtifactFor", () => {
    test("prefers the proposal when it is present", () => {
        const tab = defaultArtifactFor(status({ proposal: true, design: true, tasks: true }))
        expect(tab?.kind).toBe("proposal")
    })

    test("an absent proposal falls through to the design", () => {
        const tab = defaultArtifactFor(status({ design: true, tasks: true }))
        expect(tab?.kind).toBe("design")
    })

    test("only tasks present opens on Tasks", () => {
        expect(defaultArtifactFor(status({ tasks: true }))?.kind).toBe("tasks")
    })

    test("only a spec present opens that spec", () => {
        const tab = defaultArtifactFor(status({ specs: ["document-outline", "spec-browser"] }))
        expect(tab).toMatchObject({ kind: "spec", capability: "document-outline" })
    })

    test("nothing present resolves to null — the row still selects and the pane shows its empty state", () => {
        expect(defaultArtifactFor(NONE)).toBeNull()
    })

    test("the default artifact IS the strip's first tab, by construction", () => {
        for (const s of [
            status({ proposal: true, specs: ["cap"] }),
            status({ design: true, tasks: true }),
            status({ specs: ["cap"] }),
            NONE,
        ]) {
            expect(defaultArtifactFor(s)).toEqual(artifactTabs(s)[0] ?? null)
        }
    })
})

// ---- defaultInstanceFor --------------------------------------------------

describe("defaultInstanceFor", () => {
    test("the main worktree wins when it hosts the change, wherever it sits in the order", () => {
        const feature = instance("/wt/feature")
        const main = instance("/repo", { isMainWorktree: true })
        expect(defaultInstanceFor([feature, main])).toBe(main)
    })

    test("no main worktree falls back to the first instance in aggregation order", () => {
        const first = instance("/wt/a")
        const second = instance("/wt/b")
        expect(defaultInstanceFor([first, second])).toBe(first)
    })

    test("a singleton is its own default", () => {
        const only = instance("/wt/only")
        expect(defaultInstanceFor([only])).toBe(only)
    })

    test("no instances resolves to null", () => {
        expect(defaultInstanceFor([])).toBeNull()
    })
})

// ---- instanceOptions -----------------------------------------------------

describe("instanceOptions", () => {
    test("labels each instance by its worktree basename and carries its branch and divergence", () => {
        const options = instanceOptions(
            [
                instance("/Users/x/proj", { branch: "master" }),
                instance("/Users/x/proj/.claude/worktrees/add-thing", {
                    branch: "wt-add-thing",
                    divergence: "diverged",
                }),
            ],
            "teal",
        )
        expect(options).toEqual([
            {
                key: "/Users/x/proj",
                label: "proj",
                branch: "master",
                color: "teal",
                divergence: null,
            },
            {
                key: "/Users/x/proj/.claude/worktrees/add-thing",
                label: "add-thing",
                branch: "wt-add-thing",
                color: "teal",
                divergence: "diverged",
            },
        ])
    })

    test("a detached-HEAD instance carries no branch, so the switcher renders no chip for it", () => {
        const [option] = instanceOptions([instance("/wt/detached")], null)
        expect(option?.branch).toBeNull()
        expect(option?.color).toBeNull()
    })
})

describe("worktreeBasename", () => {
    test("takes the last non-empty segment, tolerating a trailing slash", () => {
        expect(worktreeBasename("/a/b/c")).toBe("c")
        expect(worktreeBasename("/a/b/c/")).toBe("c")
    })

    test("falls back to the path itself when there is no segment to take", () => {
        expect(worktreeBasename("/")).toBe("/")
    })
})
