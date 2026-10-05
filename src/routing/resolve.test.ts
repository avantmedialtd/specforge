import { describe, expect, test } from "bun:test"
import type { PanelSnapshot } from "../components/PullRequestPanel"
import type {
    ArtifactStatus,
    ChangeData,
    ChangeInstance,
    PullRequestsStatus,
    PullRequestSummary,
    RegisteredWorkspace,
    WorkspaceView,
} from "../types"
import type { PullRequestAddress } from "./address"
import { decodeAddress, encodeAddress } from "./codec"
import {
    findViewByRoot,
    renderTargetToAddress,
    resolveAddress,
    resolvePullRequestAddress,
    type PullRequestProviderFlags,
    type PullRequestResolution,
    type PullRequestSnapshots,
} from "./resolve"
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

/// `worktrees` defaults to the main worktree plus every active instance's
/// path, matching production: `RepoView.worktrees` is built from every registry
/// entry of the repository with no filesystem check, so it is always a superset
/// of the paths its instances name. Defaulting it to the main worktree alone
/// would build views that `build_repo_view` cannot produce.
function repoView(
    id: string,
    name: string,
    mainWorktree: string,
    active: { name: string; instances: ChangeInstance[] }[] = [],
    worktrees: string[] = [
        ...new Set([
            mainWorktree,
            ...active.flatMap((lc) => lc.instances.map((i) => i.worktreePath)),
        ]),
    ],
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

describe("resolveAddress for a settings address", () => {
    test("resolves to its own group without consulting any workspace", () => {
        expect(resolveAddress({ kind: "settings", group: "layout" }, [])).toEqual({
            status: "resolved",
            view: { kind: "settings", group: "layout" },
        })
        expect(resolveAddress({ kind: "settings", group: "desktop" }, [])).toEqual({
            status: "resolved",
            view: { kind: "settings", group: "desktop" },
        })
    })

    test("yields the same object for the same group, as the old constant did", () => {
        const a = resolveAddress({ kind: "settings", group: "identity" }, [])
        const b = resolveAddress({ kind: "settings", group: "identity" }, [])
        expect(Object.is(a, b)).toBe(true)
        expect(Object.is(a, resolveAddress({ kind: "settings", group: "workspaces" }, []))).toBe(false)
    })
})

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
    const FEATURE = "/proj/.claude/worktrees/add-thing"
    // Every tracked worktree of the repository, which is what
    // `RepoView.worktrees` carries in production: the main checkout, the one
    // hosting an active change, and a registered one hosting none.
    const view = repoView(
        "/proj/.git",
        "proj",
        "/proj",
        [
            { name: "here", instances: [instance("/proj", "here")] },
            { name: "other", instances: [instance(ACTIVE_WT, "other")] },
        ],
        ["/proj", ACTIVE_WT, FEATURE],
    )
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

    // Supersedes the previous contract, under which a repo-scoped file address
    // named the repository's MAIN WORKTREE. The listing is now pooled across
    // every tracked worktree of the repository, and which worktree's copy is
    // read is resolved at load time from that listing — so naming the main
    // worktree here would report not found for a file that lives only in a
    // feature worktree (`view-routing`: *File Addresses*).
    test("a repo-scoped file address names the repository, not a worktree", () => {
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
                    root: "/repos/specforge/.git",
                    selectedPath: "openspec/specs/web-ui/spec.md",
                },
            },
        })
    })

    // The reverse mapping has to invert the one above, or a click in the
    // browser would form no address at all.
    test("the repository-rooted target maps back to its file address", () => {
        expect(
            renderTargetToAddress(
                {
                    kind: "files",
                    root: "/repos/specforge/.git",
                    selectedPath: "openspec/specs/web-ui/spec.md",
                },
                views,
            ),
        ).toEqual({
            kind: "file",
            scope: { kind: "repo", repo: "specforge" },
            path: "openspec/specs/web-ui/spec.md",
        })
    })

    // `findViewByRoot` does double duty: it also maps an ARTIFACT's worktree
    // path back to a view for labelling, so the main-worktree arm must survive
    // alongside the new repository-identifier one.
    test("a repo view is found by its identifier and by its main worktree", () => {
        expect(findViewByRoot("/repos/specforge/.git", views)).toBe(views[1]!)
        expect(findViewByRoot("/repos/specforge", views)).toBe(views[1]!)
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

// ---- Pull-request addresses (view-routing: Pull-Request Addresses, Cold-Load
// Address Resolution) ------------------------------------------------------

function prRow(
    repoFullName: string,
    id: number,
    overrides: Partial<PullRequestSummary> = {},
): PullRequestSummary {
    return {
        id,
        title: `Pull request ${id}`,
        repoFullName,
        sourceRepoFullName: repoFullName,
        sourceBranch: "feature",
        destinationBranch: "main",
        url: `https://example.test/${repoFullName}/${id}`,
        draft: false,
        updatedAtUnix: 1_700_000_000,
        review: null,
        openTasks: 0,
        author: null,
        checks: null,
        conflicting: false,
        unresolvedThreads: 0,
        ...overrides,
    }
}

function githubPanel(
    status: PullRequestsStatus,
    lists: { authored?: PullRequestSummary[]; reviewRequested?: PullRequestSummary[] } = {},
    stale = false,
): PanelSnapshot {
    return {
        provider: "github",
        snapshot: {
            status,
            stale,
            fetchedAtUnix: status === "ok" ? 1_700_000_000 : null,
            authored: lists.authored ?? [],
            reviewRequested: lists.reviewRequested ?? [],
            withheld: 0,
        },
    }
}

function bitbucketPanel(
    status: PullRequestsStatus,
    pullRequests: PullRequestSummary[] = [],
    stale = false,
): PanelSnapshot {
    return {
        provider: "bitbucket",
        snapshot: {
            status,
            stale,
            fetchedAtUnix: status === "ok" ? 1_700_000_000 : null,
            pullRequests,
            skippedWorkspaces: [],
        },
    }
}

/// `path` decoded, which must name a pull request.
function prAddress(path: string): PullRequestAddress {
    const address = decodeAddress(path)
    if (address.kind !== "pullRequest") throw new Error(`${path} names no pull request`)
    return address
}

const BOTH_ON: PullRequestProviderFlags = { github: true, bitbucket: true }
const NOTHING_READ: PullRequestSnapshots = { github: null, bitbucket: null }

describe("resolvePullRequestAddress", () => {
    const acmeApi42 = prRow("Acme/API", 42)

    test("references compare names ignoring case", () => {
        const snapshots: PullRequestSnapshots = {
            github: githubPanel("ok", { authored: [acmeApi42] }),
            bitbucket: bitbucketPanel("ok"),
        }
        const result = resolvePullRequestAddress(
            prAddress("/pr/github/acme/api/42"),
            BOTH_ON,
            snapshots,
        )
        expect(result.status).toBe("listed")
        if (result.status === "listed") expect(result.row).toBe(acmeApi42)

        // The same names and number under the other provider match nothing:
        // BitBucket's own list does not hold it.
        expect(
            resolvePullRequestAddress(prAddress("/pr/bitbucket/acme/api/42"), BOTH_ON, snapshots),
        ).toEqual({ status: "notListed" })
    })

    test("a case variant takes the row's spelling", () => {
        const result = resolvePullRequestAddress(prAddress("/pr/github/acme/api/42"), BOTH_ON, {
            ...NOTHING_READ,
            github: githubPanel("ok", { authored: [acmeApi42] }),
        })
        expect(result).toEqual({
            status: "listed",
            row: acmeApi42,
            address: { kind: "pullRequest", provider: "github", owner: "Acme", repo: "API", number: 42 },
        })
        if (result.status === "listed") {
            expect(encodeAddress(result.address)).toBe("/pr/github/Acme/API/42")
        }
    })

    test("only ASCII letters fold", () => {
        const snapshots: PullRequestSnapshots = {
            ...NOTHING_READ,
            github: githubPanel("ok", { authored: [prRow("Ärger/api", 42)] }),
        }
        expect(
            resolvePullRequestAddress(prAddress("/pr/github/%C3%A4rger/api/42"), BOTH_ON, snapshots)
                .status,
        ).toBe("notListed")
        expect(
            resolvePullRequestAddress(prAddress("/pr/github/%C3%84RGER/API/42"), BOTH_ON, snapshots)
                .status,
        ).toBe("listed")
    })

    test("another number, owner or repository is not listed", () => {
        const snapshots: PullRequestSnapshots = {
            ...NOTHING_READ,
            github: githubPanel("ok", { authored: [acmeApi42] }),
        }
        for (const path of [
            "/pr/github/acme/api/43",
            "/pr/github/acme/web/42",
            "/pr/github/acmes/api/42",
        ]) {
            expect(resolvePullRequestAddress(prAddress(path), BOTH_ON, snapshots)).toEqual({
                status: "notListed",
            })
        }
    })

    test("a row awaiting review resolves as an authored one does", () => {
        const result = resolvePullRequestAddress(prAddress("/pr/github/acme/api/42"), BOTH_ON, {
            ...NOTHING_READ,
            github: githubPanel("ok", { reviewRequested: [acmeApi42] }),
        })
        expect(result.status).toBe("listed")
    })

    test("a BitBucket address is looked up by workspace, repository and id", () => {
        const row = prRow("acme/api", 7)
        expect(
            resolvePullRequestAddress(prAddress("/pr/bitbucket/acme/api/7"), BOTH_ON, {
                ...NOTHING_READ,
                bitbucket: bitbucketPanel("ok", [row]),
            }),
        ).toEqual({
            status: "listed",
            row,
            address: { kind: "pullRequest", provider: "bitbucket", owner: "acme", repo: "api", number: 7 },
        })
    })

    test("pending is told from provider off by the flag", () => {
        const snapshots: PullRequestSnapshots = {
            github: githubPanel("disabled"),
            bitbucket: bitbucketPanel("disabled"),
        }
        const address = prAddress("/pr/github/acme/api/42")
        expect(
            resolvePullRequestAddress(address, { github: true, bitbucket: true }, snapshots),
        ).toEqual({ status: "pending" })
        expect(
            resolvePullRequestAddress(address, { github: false, bitbucket: true }, snapshots),
        ).toEqual({ status: "providerOff" })
    })

    test("a pull-request address waits for its provider's first list", () => {
        const address = prAddress("/pr/github/acme/api/42")
        // Loaded cold: nothing read yet, then the flag and a snapshot still
        // `disabled` while the first poll runs, then the poll's list.
        expect(
            resolvePullRequestAddress(address, { github: null, bitbucket: null }, NOTHING_READ),
        ).toEqual({ status: "pending" })
        expect(
            resolvePullRequestAddress(address, BOTH_ON, {
                ...NOTHING_READ,
                github: githubPanel("disabled"),
            }),
        ).toEqual({ status: "pending" })
        expect(
            resolvePullRequestAddress(address, BOTH_ON, {
                ...NOTHING_READ,
                github: githubPanel("ok", { authored: [prRow("acme/api", 42)] }),
            }).status,
        ).toBe("listed")
    })

    test("enabling the provider resolves the pull-request address", () => {
        const address = prAddress("/pr/github/acme/api/42")
        const snapshots: PullRequestSnapshots = { ...NOTHING_READ, github: githubPanel("disabled") }
        expect(
            resolvePullRequestAddress(address, { github: false, bitbucket: null }, snapshots),
        ).toEqual({ status: "providerOff" })
        expect(
            resolvePullRequestAddress(address, { github: true, bitbucket: null }, snapshots),
        ).toEqual({ status: "pending" })
        expect(
            resolvePullRequestAddress(
                address,
                { github: true, bitbucket: null },
                { ...NOTHING_READ, github: githubPanel("ok", { authored: [prRow("acme/api", 42)] }) },
            ).status,
        ).toBe("listed")
    })

    test("a provider waiting out a deadline resolves as unavailable", () => {
        // Re-enabled inside a backoff deadline, BitBucket publishes an
        // `unavailable` snapshot at once, so its addresses are not pending.
        expect(
            resolvePullRequestAddress(prAddress("/pr/bitbucket/acme/api/7"), BOTH_ON, {
                ...NOTHING_READ,
                bitbucket: bitbucketPanel("unavailable"),
            }),
        ).toEqual({ status: "unavailable", reason: "unavailable" })
    })

    test("an unauthenticated list says so", () => {
        expect(
            resolvePullRequestAddress(prAddress("/pr/github/acme/api/42"), BOTH_ON, {
                ...NOTHING_READ,
                github: githubPanel("unauthenticated"),
            }),
        ).toEqual({ status: "unavailable", reason: "unauthenticated" })
    })

    test("a stale list still resolves", () => {
        const result = resolvePullRequestAddress(prAddress("/pr/github/acme/api/42"), BOTH_ON, {
            ...NOTHING_READ,
            github: githubPanel("ok", { authored: [acmeApi42] }, true),
        })
        expect(result.status).toBe("listed")
    })

    test("a row without a URL never resolves", () => {
        const foreign = prRow("acme/api", 42, { url: "" })
        expect(
            resolvePullRequestAddress(prAddress("/pr/github/acme/api/42"), BOTH_ON, {
                ...NOTHING_READ,
                github: githubPanel("ok", { authored: [foreign] }),
            }),
        ).toEqual({ status: "notListed" })
    })

    test("the address's own provider decides; the other's state is irrelevant", () => {
        const snapshots: PullRequestSnapshots = {
            github: githubPanel("ok", { authored: [acmeApi42] }),
            bitbucket: null,
        }
        expect(
            resolvePullRequestAddress(
                prAddress("/pr/github/acme/api/42"),
                { github: true, bitbucket: null },
                snapshots,
            ).status,
        ).toBe("listed")
        expect(
            resolvePullRequestAddress(
                prAddress("/pr/bitbucket/acme/api/42"),
                { github: true, bitbucket: null },
                snapshots,
            ).status,
        ).toBe("pending")
    })

    // Every combination of the provider's flag (unread, off, on) and its
    // snapshot (unread, or read in each status, holding the pull request or
    // not) reaches exactly one outcome, and the one the outcome table gives.
    describe("a pull-request address is never ambiguous", () => {
        const address = prAddress("/pr/github/acme/api/42")
        const snapshotStates: Record<string, PanelSnapshot | null> = {
            unread: null,
            disabled: githubPanel("disabled"),
            unauthenticated: githubPanel("unauthenticated"),
            unavailable: githubPanel("unavailable"),
            holding: githubPanel("ok", { authored: [acmeApi42] }),
            "holding, stale": githubPanel("ok", { authored: [acmeApi42] }, true),
            "not holding": githubPanel("ok", { authored: [prRow("acme/api", 41)] }),
        }
        const expected: Record<string, Record<string, PullRequestResolution["status"]>> = {
            unread: {
                unread: "pending",
                disabled: "pending",
                unauthenticated: "pending",
                unavailable: "pending",
                holding: "pending",
                "holding, stale": "pending",
                "not holding": "pending",
            },
            off: {
                unread: "pending",
                disabled: "providerOff",
                unauthenticated: "providerOff",
                unavailable: "providerOff",
                holding: "providerOff",
                "holding, stale": "providerOff",
                "not holding": "providerOff",
            },
            on: {
                unread: "pending",
                disabled: "pending",
                unauthenticated: "unavailable",
                unavailable: "unavailable",
                holding: "listed",
                "holding, stale": "listed",
                "not holding": "notListed",
            },
        }
        const flagStates: Record<string, boolean | null> = { unread: null, off: false, on: true }
        for (const [flagName, flag] of Object.entries(flagStates)) {
            for (const [snapshotName, panel] of Object.entries(snapshotStates)) {
                test(`flag ${flagName}, snapshot ${snapshotName}`, () => {
                    const result = resolvePullRequestAddress(
                        address,
                        { github: flag, bitbucket: true },
                        { github: panel, bitbucket: bitbucketPanel("ok") },
                    )
                    expect(result.status).toBe(expected[flagName]![snapshotName]!)
                })
            }
        }
    })
})

describe("resolveAddress for a pull-request address", () => {
    const address = prAddress("/pr/bitbucket/acme/api/7")

    test("answers not found: the workspaces do not resolve it", () => {
        expect(resolveAddress(address, [])).toEqual({ status: "notFound" })
    })

    test("a pull-request address is not a registry slug", () => {
        // A registered workspace whose slug is the BitBucket workspace's name,
        // holding a change named like the repository, changes nothing either
        // way: the pull request is looked up in BitBucket's snapshot alone.
        const acme = flatView("/ws/acme", "acme", [change("api")])
        const parked = registered("/ws/acme", "acme", { disabled: true })
        expect(resolveAddress(address, [acme], [parked])).toEqual({ status: "notFound" })

        const snapshots: PullRequestSnapshots = {
            ...NOTHING_READ,
            bitbucket: bitbucketPanel("ok", [prRow("acme/api", 7)]),
        }
        expect(resolvePullRequestAddress(address, BOTH_ON, snapshots).status).toBe("listed")
    })
})
