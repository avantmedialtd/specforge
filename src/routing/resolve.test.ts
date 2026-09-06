import { describe, expect, test } from "bun:test"
import type {
    ArtifactStatus,
    ChangeData,
    ChangeInstance,
    RegisteredWorkspace,
    WorkspaceView,
} from "../types"
import { encodeAddress } from "./codec"
import { renderTargetToAddress, resolveAddress } from "./resolve"
import { instanceToken, scopeFor, shortHash } from "./slug"

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
    worktrees: string[] = [mainWorktree],
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
        worktrees,
    }
}

function registered(
    uri: string,
    name: string,
    overrides: Partial<RegisteredWorkspace> = {},
): RegisteredWorkspace {
    return {
        uri,
        name,
        isMissing: false,
        displayName: null,
        color: null,
        repoId: null,
        disabled: false,
        ...overrides,
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

// ---- An address into a DISABLED workspace is its own outcome -----------
//
// A parked row is filtered out of `views` (design.md D3) but kept, flagged, in
// the registered listing — so the resolver can tell "registered but parked"
// apart from "gone", which is the whole promise of the feature.

describe("resolveAddress against a parked workspace", () => {
    const parkedRepoRow = registered("/proj", "proj", { repoId: "/proj/.git", disabled: true })
    const parkedFlatRow = registered("/notes", "notes", { disabled: true })

    test("a files address naming a parked repository reports disabled, carrying the row", () => {
        const result = resolveAddress(
            { kind: "files", scope: { kind: "repo", repo: "proj" } },
            [],
            [parkedRepoRow],
        )
        expect(result.status).toBe("disabled")
        if (result.status !== "disabled") return
        expect(result.workspaces).toEqual([parkedRepoRow])
    })

    test("an artifact address naming a parked repository reports disabled", () => {
        const result = resolveAddress(
            {
                kind: "artifact",
                scope: { kind: "repo", repo: `proj-${shortHash("/proj/.git")}` },
                changeId: "mychange",
                artifactKind: "proposal",
            },
            [],
            [parkedRepoRow],
        )
        expect(result.status).toBe("disabled")
    })

    test("an archive address naming a parked FLAT workspace reports disabled", () => {
        // Archive resolution searches both pools together (C1), so the parked
        // lookup must not be narrowed to one kind either.
        const result = resolveAddress(
            { kind: "archive", selection: { workspace: "notes", archiveDir: "2026-01-01-x" } },
            [],
            [parkedFlatRow],
        )
        expect(result.status).toBe("disabled")
        if (result.status !== "disabled") return
        expect(result.workspaces).toEqual([parkedFlatRow])
    })

    test("a `/w/` token never resolves to a parked REPOSITORY, and vice versa", () => {
        const asWorkspace = resolveAddress(
            { kind: "files", scope: { kind: "workspace", workspace: "proj" } },
            [],
            [parkedRepoRow],
        )
        expect(asWorkspace.status).toBe("notFound")
        const asRepo = resolveAddress(
            { kind: "files", scope: { kind: "repo", repo: "notes" } },
            [],
            [parkedFlatRow],
        )
        expect(asRepo.status).toBe("notFound")
    })

    test("a token matching nothing is still notFound while parked rows exist", () => {
        const result = resolveAddress(
            { kind: "files", scope: { kind: "repo", repo: "unrelated" } },
            [],
            [parkedRepoRow, parkedFlatRow],
        )
        expect(result.status).toBe("notFound")
    })

    test("an enabled view wins over a parked row that slugifies identically", () => {
        const result = resolveAddress(
            { kind: "files", scope: { kind: "repo", repo: "proj" } },
            [repoView("/other/.git", "proj", "/other")],
            [parkedRepoRow],
        )
        expect(result.status).toBe("resolved")
    })

    test("a missing change inside a RESOLVABLE workspace stays notFound", () => {
        // Only the scope-miss sites consult the parked listing: a change or
        // artifact absent from a workspace that DID resolve is a genuine miss.
        const result = resolveAddress(
            {
                kind: "artifact",
                scope: { kind: "workspace", workspace: "notes" },
                changeId: "nope",
                artifactKind: "proposal",
            },
            [flatView("/notes", "notes")],
            [parkedRepoRow],
        )
        expect(result.status).toBe("notFound")
    })

    test("omitting the registered listing keeps every existing caller's behaviour", () => {
        const result = resolveAddress({ kind: "files", scope: { kind: "repo", repo: "proj" } }, [])
        expect(result.status).toBe("notFound")
    })
})

// ---- The archive worktreeHint names a worktree, not just an active one ----
//
// The hint exists because a change is archived from inside a feature worktree
// whose archival commit need not be merged into the repo's main worktree — so
// resolving to `mainWorktree` shows an archive listing without the very change
// the link named. `RepoView.archived` is never serialized, so such a worktree
// appears in NO active instance and the hint has to be invertible from the
// registered listing too.

describe("resolveArchive inverts the worktree hint", () => {
    // One active change in the main worktree, one in a worktree of its own —
    // the second is what proves the active pass finds something the
    // main-worktree fallback would not have produced anyway.
    const ACTIVE_WT = "/proj/.claude/worktrees/other"
    const view = repoView("/proj/.git", "proj", "/proj", [
        { name: "here", instances: [instance("/proj", "here")] },
        { name: "other", instances: [instance(ACTIVE_WT, "other")] },
    ])
    const FEATURE = "/proj/.claude/worktrees/add-thing"
    const rows = [
        registered("/proj", "proj", { repoId: "/proj/.git" }),
        registered(FEATURE, "add-thing", { repoId: "/proj/.git" }),
    ]

    function resolvedUri(hint: string | undefined, listing = rows): string | undefined {
        const result = resolveAddress(
            {
                kind: "archive",
                selection: { workspace: "proj", archiveDir: "2026-08-11-add-thing", worktreeHint: hint },
            },
            [view],
            listing,
        )
        if (result.status !== "resolved" || result.view.kind !== "archive") return undefined
        return result.view.selection?.workspaceUri
    }

    test("a registered worktree hosting no ACTIVE change is still reachable by hint", () => {
        expect(resolvedUri(shortHash(FEATURE))).toBe(FEATURE)
    })

    test("an ACTIVE instance is matched without any registered listing at all", () => {
        // Deliberately a worktree that is neither the main one nor registered:
        // asserting on a path the fallback also produces would pass with the
        // active scan deleted.
        expect(resolvedUri(shortHash(ACTIVE_WT), [])).toBe(ACTIVE_WT)
    })

    test("a hint naming ANOTHER repository's registered worktree is ignored", () => {
        // `shortHash` is a 32-bit token over a bare path with no repository in
        // it; an unrestricted scan would hand back a wholly unrelated
        // repository's folder as the archive to read.
        const foreign = registered("/elsewhere/wt", "wt", { repoId: "/elsewhere/.git" })
        expect(resolvedUri(shortHash("/elsewhere/wt"), [...rows, foreign])).toBe("/proj")
    })

    test("a hint matching nothing, and no hint at all, both fall back to the main worktree", () => {
        expect(resolvedUri(shortHash("/proj/.claude/worktrees/removed"))).toBe("/proj")
        expect(resolvedUri(undefined)).toBe("/proj")
    })

    test("a DISCOVERED worktree, in neither older pool, keeps the pre-selection", () => {
        // The case the two older pools BOTH miss, and the one the today's-ships
        // link actually produces: the worktree hosts no active change (so it is
        // in no `view.active` instance — `RepoView.archived` is never
        // serialized) and SpecForge auto-discovered it rather than the user
        // registering it (so it is in no `list_workspaces` row). Only the
        // repository's tracked-worktree list has it, and without that the
        // address would silently degrade to the main worktree — whose archive
        // does not contain the change, because the branch has not merged.
        const DISCOVERED = "/proj/.claude/worktrees/browse-archive"
        const tracked = repoView(
            "/proj/.git",
            "proj",
            "/proj",
            [{ name: "here", instances: [instance("/proj", "here")] }],
            ["/proj", DISCOVERED],
        )
        const registeredOnly = [registered("/proj", "proj", { repoId: "/proj/.git" })]
        expect(registeredOnly.some((w) => w.uri === DISCOVERED)).toBe(false) // precondition

        const result = resolveAddress(
            {
                kind: "archive",
                selection: {
                    workspace: "proj",
                    archiveDir: "2026-08-11-add-thing",
                    worktreeHint: shortHash(DISCOVERED),
                },
            },
            [tracked],
            registeredOnly,
        )
        expect(result).toEqual({
            status: "resolved",
            view: {
                kind: "archive",
                selection: { workspaceUri: DISCOVERED, archiveDir: "2026-08-11-add-thing" },
            },
        })
    })
})

// ---- File addresses (view-routing: File Addresses) --------------------

describe("file addresses", () => {
    const views: WorkspaceView[] = [
        flatView("/ws/notes", "notes"),
        repoView("/repos/specforge/.git", "specforge", "/repos/specforge"),
    ]

    test("resolves to the browse root with the file selected", () => {
        const result = resolveAddress(
            { kind: "file", scope: { kind: "workspace", workspace: "notes" }, path: "README.md" },
            views,
        )
        expect(result).toEqual({
            status: "resolved",
            view: {
                kind: "target",
                target: { kind: "files", root: "/ws/notes", selectedPath: "README.md" },
            },
        })
    })

    test("a repo-scoped file address names the main worktree", () => {
        const result = resolveAddress(
            {
                kind: "file",
                scope: { kind: "repo", repo: "specforge" },
                path: "openspec/specs/web-ui/spec.md",
            },
            views,
        )
        expect(result).toEqual({
            status: "resolved",
            view: {
                kind: "target",
                target: {
                    kind: "files",
                    root: "/repos/specforge",
                    selectedPath: "openspec/specs/web-ui/spec.md",
                },
            },
        })
    })

    test("an unknown slug reads nothing", () => {
        expect(
            resolveAddress(
                { kind: "file", scope: { kind: "workspace", workspace: "nope" }, path: "x.md" },
                views,
            ),
        ).toEqual({ status: "notFound" })
    })

    test("a files address still resolves without a selection", () => {
        const result = resolveAddress(
            { kind: "files", scope: { kind: "workspace", workspace: "notes" } },
            views,
        )
        expect(result).toEqual({
            status: "resolved",
            view: { kind: "target", target: { kind: "files", root: "/ws/notes" } },
        })
    })

    test("a files target round-trips back to a file address when it carries a selection", () => {
        const address = renderTargetToAddress(
            { kind: "files", root: "/ws/notes", selectedPath: "docs/a.md" },
            views,
        )
        expect(address).toEqual({
            kind: "file",
            scope: { kind: "workspace", workspace: "notes" },
            path: "docs/a.md",
        })
        expect(encodeAddress(address!)).toBe("/w/notes/file/docs/a.md")
    })

    test("a files target with no selection still round-trips to a files address", () => {
        expect(renderTargetToAddress({ kind: "files", root: "/ws/notes" }, views)).toEqual({
            kind: "files",
            scope: { kind: "workspace", workspace: "notes" },
        })
    })

    /// Two same-named roots make the address ambiguous; each candidate must
    /// keep naming the same file, or picking one would open the browse root
    /// and silently drop the document.
    test("ambiguous candidates carry the selected path forward", () => {
        const colliding: WorkspaceView[] = [
            flatView("/a/notes", "notes"),
            flatView("/b/notes", "notes"),
        ]
        const result = resolveAddress(
            { kind: "file", scope: { kind: "workspace", workspace: "notes" }, path: "x.md" },
            colliding,
        )
        expect(result.status).toBe("ambiguous")
        if (result.status !== "ambiguous") return
        for (const candidate of result.candidates) {
            expect(candidate.address.kind).toBe("file")
            if (candidate.address.kind === "file") {
                expect(candidate.address.path).toBe("x.md")
            }
        }
    })
})
