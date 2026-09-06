import { describe, expect, test } from "bun:test"
import type { ArtifactStatus, ChangeData, ChangeInstance, WorkspaceView } from "../types"
import { instanceToken, matchInstance, matchSlug, scopeFor, shortHash, slugFor, slugify } from "./slug"

// ---- Fixture builders --------------------------------------------------

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

function flat(uri: string, name: string, displayName: string | null = null): WorkspaceView {
    return {
        kind: "flat",
        workspace: { uri, name },
        changes: [],
        displayName,
        color: null,
    }
}

function instance(worktreePath: string, changeId: string, branch: string | null = null): ChangeInstance {
    return {
        worktreePath,
        branch,
        isMainWorktree: false,
        isDefaultBranch: false,
        isArchivedHere: false,
        change: change(changeId),
        modifiedAt: 0,
        divergence: null,
        specCommitState: "committed",
    }
}

function repo(
    repoId: string,
    name: string,
    mainWorktree: string,
    active: { name: string; instances: ChangeInstance[] }[] = [],
    displayName: string | null = null,
): WorkspaceView {
    return {
        kind: "repo",
        repoId,
        mainWorktree,
        name,
        defaultBranch: "main",
        active,
        displayName,
        color: null,
        dirty: false,
        dirtyWorktrees: [],
        hasUncommittedSpecs: false,
        worktrees: [mainWorktree],
    }
}

// ---- slugify / shortHash -------------------------------------------------

describe("slugify", () => {
    test("lowercases and hyphenates", () => {
        expect(slugify("My Project")).toBe("my-project")
    })
    test("collapses runs of non-alphanumerics into one hyphen", () => {
        expect(slugify("foo___bar!!baz")).toBe("foo-bar-baz")
    })
    test("trims leading/trailing hyphens", () => {
        expect(slugify("--foo--")).toBe("foo")
    })
    test("strips path separators (never leaks structure)", () => {
        expect(slugify("/Users/istvan/dev/foo")).toBe("users-istvan-dev-foo")
    })
    test("falls back to a placeholder for an all-punctuation name", () => {
        expect(slugify("!!!")).toBe("x")
    })
})

describe("shortHash", () => {
    test("is deterministic", () => {
        expect(shortHash("/Users/istvan/dev/foo")).toBe(shortHash("/Users/istvan/dev/foo"))
    })
    test("differs for different inputs", () => {
        expect(shortHash("/a")).not.toBe(shortHash("/b"))
    })
    test("is alphanumeric (a safe path segment)", () => {
        expect(shortHash("anything at all")).toMatch(/^[a-z0-9]+$/)
    })
})

// ---- slugFor / scopeFor (emission) ----------------------------------

describe("slugFor", () => {
    test("a unique workspace gets its bare slug", () => {
        const views = [flat("/a", "myproject"), flat("/b", "other")]
        expect(slugFor(views[0]!, views)).toBe("myproject")
    })

    test("derivation is stable across a displayName change — never derived from it", () => {
        const before = flat("/a", "myproject", null)
        const after = flat("/a", "myproject", "Renamed For Display")
        const views = [flat("/b", "other")]
        expect(slugFor(before, [before, ...views])).toBe(slugFor(after, [after, ...views]))
        expect(slugFor(after, [after, ...views])).toBe("myproject")
    })

    test("colliding workspaces each receive a distinguishing suffix", () => {
        const a = flat("/a", "myproject")
        const b = flat("/b", "myproject")
        const views = [a, b]
        const slugA = slugFor(a, views)
        const slugB = slugFor(b, views)
        expect(slugA).not.toBe(slugB)
        expect(slugA.startsWith("myproject-")).toBe(true)
        expect(slugB.startsWith("myproject-")).toBe(true)
    })

    test("a workspace and a repo sharing a base name do not force each other into a suffix", () => {
        const ws = flat("/a", "shared")
        const rp = repo("/repo/.git", "shared", "/repo")
        const views = [ws, rp]
        expect(slugFor(ws, views)).toBe("shared")
        expect(slugFor(rp, views)).toBe("shared")
    })

    test("repos collide independently of workspaces", () => {
        const a = repo("/repoA/.git", "dup", "/repoA")
        const b = repo("/repoB/.git", "dup", "/repoB")
        const views = [a, b]
        expect(slugFor(a, views)).not.toBe(slugFor(b, views))
    })
})

describe("scopeFor", () => {
    test("a flat workspace yields a workspace-kind scope", () => {
        const views = [flat("/a", "myproject")]
        expect(scopeFor(views[0]!, views)).toEqual({ kind: "workspace", workspace: "myproject" })
    })
    test("a repo yields a repo-kind scope, never an instance", () => {
        const views = [repo("/r/.git", "myrepo", "/r")]
        expect(scopeFor(views[0]!, views)).toEqual({ kind: "repo", repo: "myrepo" })
    })
})

// ---- matchSlug (resolution) --------------------------------------------

describe("matchSlug", () => {
    test("a unique bare slug resolves to exactly one view", () => {
        const views = [flat("/a", "myproject"), flat("/b", "other")]
        expect(matchSlug("myproject", views)).toEqual([views[0]])
    })

    test("an unknown slug resolves to nothing", () => {
        const views = [flat("/a", "myproject")]
        expect(matchSlug("nope", views)).toEqual([])
    })

    test("a fully-suffixed token still resolves uniquely amid a real collision", () => {
        const a = flat("/a", "myproject")
        const b = flat("/b", "myproject")
        const views = [a, b]
        const suffixed = slugFor(a, views)
        expect(matchSlug(suffixed, views)).toEqual([a])
    })

    test("an address that has become ambiguous presents a choice", () => {
        // The bare slug "myproject" was unique when a lone workspace with
        // that name existed and an address for it was emitted with no
        // suffix. A second, colliding workspace is registered afterwards —
        // resolving the OLD bare token must now find both, not silently
        // report not-found (neither current view emits the bare form any
        // more) and not silently pick one.
        const original = flat("/a", "myproject")
        const bareToken = slugFor(original, [original])

        const collider = flat("/b", "myproject")
        const later = [original, collider]

        const matches = matchSlug(bareToken, later)
        expect(matches.length).toBe(2)
        expect(matches).toEqual(expect.arrayContaining([original, collider]))
    })

    test("kind filters to one pool even when the token's base also matches the other pool", () => {
        const ws = flat("/a", "shared")
        const rp = repo("/repo/.git", "shared", "/repo")
        const views = [ws, rp]
        expect(matchSlug("shared", views, "flat")).toEqual([ws])
        expect(matchSlug("shared", views, "repo")).toEqual([rp])
    })

    test("archive-style cross-pool lookup (no kind filter) reports the cross-kind collision", () => {
        // Distinct from the previous case: here the two entries genuinely
        // collide as far as an unscoped (archive) lookup is concerned.
        const ws = flat("/a", "dup")
        const rp = repo("/repo/.git", "dup", "/repo")
        const views = [ws, rp]
        expect(matchSlug("dup", views).length).toBe(2)
    })
})

// ---- instanceToken / matchInstance -----------------------------------

describe("instanceToken", () => {
    test("a single-instance change omits the instance segment", () => {
        const instances = [instance("/wt1", "chg")]
        expect(instanceToken("/wt1", instances)).toBeUndefined()
    })

    test("a multi-instance change names its instance", () => {
        const instances = [instance("/wt1", "chg"), instance("/wt2", "chg")]
        const token = instanceToken("/wt1", instances)
        expect(token).toBeDefined()
        expect(instanceToken("/wt2", instances)).not.toBe(token)
    })
})

describe("matchInstance", () => {
    test("finds the instance whose token matches", () => {
        const instances = [instance("/wt1", "chg"), instance("/wt2", "chg")]
        const token = instanceToken("/wt2", instances)!
        expect(matchInstance(token, instances)).toBe(instances[1])
    })

    test("a stale instance token (worktree gone) matches nothing", () => {
        const instances = [instance("/wt1", "chg"), instance("/wt2", "chg")]
        const staleToken = instanceToken("/wt2", instances)!
        expect(matchInstance(staleToken, [instances[0]!])).toBeUndefined()
    })
})
