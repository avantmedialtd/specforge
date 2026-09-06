import { describe, expect, test } from "bun:test"
import type { RegisteredWorkspace, ShipEntry, WorkspaceView } from "./types"
import type { Address } from "./routing/address"
import { resolveAddress } from "./routing/resolve"
import { archiveSlugFor, shortHash, slugFor, slugify } from "./routing/slug"
import {
    disabledRowCount,
    disabledRows,
    matchParkedSlug,
    rowKey,
    rowLabel,
    shipRowState,
    siblingsOf,
} from "./workspaceRows"

// ---- Fixture builders --------------------------------------------------

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

function repoView(
    repoId: string,
    name: string,
    mainWorktree: string,
    worktrees: string[] = [mainWorktree],
): WorkspaceView {
    return {
        kind: "repo",
        repoId,
        mainWorktree,
        name,
        defaultBranch: "main",
        active: [],
        displayName: null,
        color: null,
        dirty: false,
        dirtyWorktrees: [],
        hasUncommittedSpecs: false,
        worktrees,
    }
}

function flatView(uri: string, name: string): WorkspaceView {
    return { kind: "flat", workspace: { uri, name }, changes: [], displayName: null, color: null }
}

function ship(repoId: string, worktreePath: string): ShipEntry {
    return {
        changeId: "add-thing",
        title: null,
        workspaceLabel: "proj",
        repoId,
        worktreePath,
        archiveDir: "2026-08-11-add-thing",
        archivedAt: null,
    }
}

// ---- rowKey ------------------------------------------------------------

describe("rowKey", () => {
    test("keys a repo row on its repoId and a flat row on its own path", () => {
        expect(rowKey(registered("/proj", "proj", { repoId: "/proj/.git" }))).toBe("repo:/proj/.git")
        expect(rowKey(registered("/notes", "notes"))).toBe("flat:/notes")
    })

    test("a flat row whose path equals another row's repoId never collides with it", () => {
        // The `repo:`/`flat:` prefix is what keeps the key total, exactly as
        // `PresentationKey`'s two variants are.
        const flat = registered("/proj/.git", "shared")
        const repo = registered("/proj", "proj", { repoId: "/proj/.git" })
        expect(rowKey(flat)).not.toBe(rowKey(repo))
    })
})

// ---- disabledRows / disabledRowCount (F5) ------------------------------

describe("disabledRowCount counts ROWS the tree drops, not registered folders", () => {
    test("two registered worktrees of one disabled repository count once", () => {
        // The regression: the flag is stored per repository, so both Settings
        // entries report `disabled` while the tree loses exactly one row.
        const rows = [
            registered("/proj", "proj", { repoId: "/proj/.git", disabled: true }),
            registered("/proj-feature", "proj-feature", { repoId: "/proj/.git", disabled: true }),
        ]
        expect(disabledRowCount(rows)).toBe(1)
        expect(disabledRows(rows).map((w) => w.uri)).toEqual(["/proj"])
    })

    test("a disabled repository group and a disabled flat workspace count two", () => {
        const rows = [
            registered("/proj", "proj", { repoId: "/proj/.git", disabled: true }),
            registered("/proj-feature", "proj-feature", { repoId: "/proj/.git", disabled: true }),
            registered("/notes", "notes", { disabled: true }),
        ]
        expect(disabledRowCount(rows)).toBe(2)
    })

    test("two distinct disabled flat workspaces count two, never folded together", () => {
        // Guards the naive `repoId ?? …` grouping, which would bucket every
        // flat row under one null key.
        const rows = [
            registered("/notes", "notes", { disabled: true }),
            registered("/docs", "docs", { disabled: true }),
        ]
        expect(disabledRowCount(rows)).toBe(2)
    })

    test("enabled rows never count", () => {
        const rows = [
            registered("/proj", "proj", { repoId: "/proj/.git" }),
            registered("/notes", "notes"),
        ]
        expect(disabledRowCount(rows)).toBe(0)
    })
})

// ---- siblingsOf (F11) --------------------------------------------------

describe("siblingsOf", () => {
    test("each of two worktrees of one repository sees the other", () => {
        const main = registered("/proj", "proj", { repoId: "/proj/.git" })
        const feature = registered("/proj-feature", "proj-feature", { repoId: "/proj/.git" })
        const all = [main, feature]
        expect(siblingsOf(main, all).map((w) => w.uri)).toEqual(["/proj-feature"])
        expect(siblingsOf(feature, all).map((w) => w.uri)).toEqual(["/proj"])
    })

    test("a repository registered once has no siblings", () => {
        const main = registered("/proj", "proj", { repoId: "/proj/.git" })
        expect(siblingsOf(main, [main, registered("/notes", "notes")])).toEqual([])
    })

    test("a flat workspace never has siblings, however many flat rows exist", () => {
        const notes = registered("/notes", "notes")
        const docs = registered("/docs", "docs")
        expect(siblingsOf(notes, [notes, docs])).toEqual([])
    })

    test("a three-worktree repository reports two siblings per row", () => {
        const all = [
            registered("/proj", "proj", { repoId: "/proj/.git" }),
            registered("/proj-a", "proj-a", { repoId: "/proj/.git" }),
            registered("/proj-b", "proj-b", { repoId: "/proj/.git" }),
        ]
        expect(siblingsOf(all[0]!, all).length).toBe(2)
    })
})

// ---- matchParkedSlug (F10) ---------------------------------------------

describe("matchParkedSlug", () => {
    const parkedRepo = registered("/proj", "proj", { repoId: "/proj/.git", disabled: true })
    const parkedFlat = registered("/notes", "My Notes", { disabled: true })

    test("matches a parked flat row on its bare slug and on its suffixed form", () => {
        expect(matchParkedSlug("my-notes", [parkedFlat])).toEqual([parkedFlat])
        expect(matchParkedSlug(`my-notes-${shortHash("/notes")}`, [parkedFlat])).toEqual([
            parkedFlat,
        ])
    })

    test("matches a parked repo row on its bare slug and on its repoId-hashed form", () => {
        expect(matchParkedSlug("proj", [parkedRepo])).toEqual([parkedRepo])
        expect(matchParkedSlug(`proj-${shortHash("/proj/.git")}`, [parkedRepo])).toEqual([
            parkedRepo,
        ])
    })

    test("kind narrows the pool the way matchSlug's does", () => {
        const rows = [parkedRepo, registered("/proj2", "proj", { disabled: true })]
        expect(matchParkedSlug("proj", rows, "repo")).toEqual([parkedRepo])
        expect(matchParkedSlug("proj", rows, "flat").map((w) => w.uri)).toEqual(["/proj2"])
        expect(matchParkedSlug("proj", rows).length).toBe(2)
    })

    test("an ENABLED row is never a parked match", () => {
        expect(matchParkedSlug("proj", [registered("/proj", "proj", { repoId: "/proj/.git" })])).toEqual(
            [],
        )
    })

    test("an unrelated token matches nothing", () => {
        expect(matchParkedSlug("something-else", [parkedRepo, parkedFlat])).toEqual([])
    })

    test("two registered worktrees of one parked repository yield ONE match", () => {
        const rows = [
            parkedRepo,
            registered("/proj-feature", "proj", { repoId: "/proj/.git", disabled: true }),
        ]
        expect(matchParkedSlug("proj", rows)).toEqual([parkedRepo])
    })

    // ---- The repository half of the contract, stated honestly ----------
    //
    // A repository row's name is its MAIN worktree's basename, but the listing
    // is per registered FOLDER — so the reconstruction is approximate in both
    // directions. These pin what it actually does, not what would be nicer.

    test("a repository registered only at a SECONDARY worktree is found by that folder's basename, not by the row's name", () => {
        const feature = registered("/wt/proj-feature", "proj-feature", {
            repoId: "/proj/.git",
            disabled: true,
        })
        // The row's real slug (main worktree's basename) misses: nothing in
        // the listing carries that name.
        expect(matchParkedSlug("proj", [feature])).toEqual([])
        expect(matchParkedSlug("proj-feature", [feature])).toEqual([feature])
    })

    test("a token equal to a secondary worktree's basename matches the repository row, though no address ever named it", () => {
        // The false match the doc comment now admits to: `proj-feature` is a
        // folder of the `proj` repository, never a row of its own, so this
        // token addressed nothing — and the user is told "proj is disabled"
        // instead of "not found". Kept deliberately: narrowing to the main
        // worktree would need path arithmetic over `repoId`, which misfires on
        // `--separate-git-dir`/submodule layouts and would turn REAL parked
        // links into not-found.
        const main = registered("/proj", "proj", { repoId: "/proj/.git", disabled: true })
        const feature = registered("/wt/proj-feature", "proj-feature", {
            repoId: "/proj/.git",
            disabled: true,
        })
        const matches = matchParkedSlug("proj-feature", [main, feature])
        // Still ONE row — the wrong answer never becomes a doubled one.
        expect(matches.length).toBe(1)
        expect(rowKey(matches[0]!)).toBe("repo:/proj/.git")
    })

    test("the reconstruction uses the same primitives the slug emitter does", () => {
        // Not a restatement of the implementation: it pins the reconstruction
        // to `slugify(name)` + `shortHash(identity)`, which is what makes a
        // link minted while the row was enabled still recognisable.
        const ws = registered("/x", "Weird Name!", { disabled: true })
        expect(matchParkedSlug(slugify("Weird Name!"), [ws])).toEqual([ws])
    })
})

// ---- matchParkedSlug is pinned to what slug.ts actually emits -----------
//
// The reconstruction only earns its keep if it recognises the very tokens a
// link carried while the row was still enabled. Asserting against a literal
// would only restate `matchParkedSlug`; these cases mint the token with the
// EMITTER (`slugFor` / `archiveSlugFor`) and match it with the reconstruction,
// so any drift between the two — a different base name, a different hashed
// identity, a different suffix separator, a changed `slugify` — fails here
// instead of silently degrading a parked address to "not found", which is the
// defect this whole path exists to remove.

describe("matchParkedSlug recognises the slug the emitter would have minted", () => {
    test("a flat row, in both its bare and its collision-suffixed form", () => {
        const view = flatView("/notes", "My Notes")
        const row = registered("/notes", "My Notes", { disabled: true })
        expect(matchParkedSlug(slugFor(view, [view]), [row], "flat")).toEqual([row])

        // A same-named flat workspace forces the suffix, which is hashed from
        // the workspace's own uri.
        const collider = flatView("/archive/notes", "My Notes")
        const suffixed = slugFor(view, [view, collider])
        expect(suffixed).not.toBe(slugFor(view, [view]))
        expect(matchParkedSlug(suffixed, [row], "flat")).toEqual([row])
    })

    test("a repository registered at its main worktree, in both forms", () => {
        // A parked repository is gathered cold, where its view name is the
        // basename of its main worktree — the same string the registered row
        // carries when that worktree is the registered one.
        const view = repoView("/proj/.git", "proj", "/proj")
        const row = registered("/proj", "proj", { repoId: "/proj/.git", disabled: true })
        expect(matchParkedSlug(slugFor(view, [view]), [row], "repo")).toEqual([row])

        const collider = repoView("/elsewhere/proj/.git", "proj", "/elsewhere/proj")
        const suffixed = slugFor(view, [view, collider])
        expect(suffixed).not.toBe(slugFor(view, [view]))
        expect(matchParkedSlug(suffixed, [row], "repo")).toEqual([row])
    })

    test("an archive token, which suffixes across BOTH pools", () => {
        // `archiveSlugFor` counts collisions regardless of kind (C1), so a flat
        // workspace sharing a name with a repository is suffixed here even
        // though `slugFor` would leave it bare — and `resolveArchive`'s parked
        // lookup, which likewise passes no kind, has to recognise that form.
        const view = flatView("/notes", "notes")
        const repoCollider = repoView("/notes/.git", "notes", "/notes-repo")
        const row = registered("/notes", "notes", { disabled: true })
        const token = archiveSlugFor(view, [view, repoCollider])
        expect(token).toBe(`notes-${shortHash("/notes")}`)
        expect(matchParkedSlug(token, [row])).toEqual([row])
    })
})

// ---- shipRowState (F2) -------------------------------------------------

describe("shipRowState", () => {
    test("a ship whose repository is in the views is openable, carrying that view", () => {
        const view = repoView("/proj/.git", "proj", "/proj")
        const state = shipRowState(ship("/proj/.git", "/proj"), [view, flatView("/notes", "notes")], [])
        expect(state.kind).toBe("openable")
        if (state.kind === "openable") expect(state.view).toBe(view)
    })

    test("a ship archived in a worktree hosting no ACTIVE change is still openable", () => {
        // The path-keyed lookup this replaces missed exactly this: the feature
        // worktree a change was archived from is neither the repo's main
        // worktree nor any active instance's path.
        const view = repoView("/proj/.git", "proj", "/proj")
        const state = shipRowState(ship("/proj/.git", "/proj/.claude/worktrees/gone"), [view], [])
        expect(state.kind).toBe("openable")
    })

    test("a ship whose repository is parked reports the parked row", () => {
        const parked = registered("/proj", "proj", { repoId: "/proj/.git", disabled: true })
        const state = shipRowState(ship("/proj/.git", "/proj"), [], [parked])
        expect(state.kind).toBe("parked")
        if (state.kind === "parked") expect(state.workspace).toBe(parked)
    })

    test("a ship whose repository is neither viewable nor parked is unavailable", () => {
        const state = shipRowState(ship("/gone/.git", "/gone"), [repoView("/proj/.git", "proj", "/proj")], [
            registered("/proj", "proj", { repoId: "/proj/.git" }),
        ])
        expect(state.kind).toBe("unavailable")
    })

    test("an ENABLED registered row never makes a ship read as parked", () => {
        const state = shipRowState(ship("/proj/.git", "/proj"), [], [
            registered("/proj", "proj", { repoId: "/proj/.git" }),
        ])
        expect(state.kind).toBe("unavailable")
    })
})

// ---- A ship click has to LAND where the ship was archived ---------------
//
// `shipRowState` only says the row is openable; the promise the `dashboard`
// spec delta makes ("opens the Archive browser for that repository with the
// change pre-selected") is about the destination. These mint the address
// exactly as `App.tsx`'s `handleOpenShip` does and resolve it back, because
// classification and destination are computed by different modules and only
// the round trip catches them disagreeing.

/// Precisely what `handleOpenShip` builds for an openable ship.
function shipAddress(entry: ShipEntry, views: WorkspaceView[], all: RegisteredWorkspace[]): Address {
    const state = shipRowState(entry, views, all)
    if (state.kind !== "openable") throw new Error(`expected an openable ship, got ${state.kind}`)
    return {
        kind: "archive",
        selection: {
            workspace: archiveSlugFor(state.view, views),
            archiveDir: entry.archiveDir,
            worktreeHint:
                state.view.kind === "repo" ? shortHash(entry.worktreePath) : undefined,
        },
    }
}

describe("a ship click resolves back to the worktree it was archived from", () => {
    const FEATURE = "/proj/.claude/worktrees/add-thing"

    test("a worktree hosting NO active change is still the archive that opens", () => {
        // The regression: `RepoView.archived` is never serialized, so this
        // worktree appears in no active instance — resolving the hint against
        // `view.active` alone silently degraded to the main worktree, whose
        // archive does not hold the change (the archival commit isn't merged).
        const view = repoView("/proj/.git", "proj", "/proj")
        const rows = [
            registered("/proj", "proj", { repoId: "/proj/.git" }),
            registered(FEATURE, "add-thing", { repoId: "/proj/.git" }),
        ]
        const result = resolveAddress(shipAddress(ship("/proj/.git", FEATURE), [view], rows), [view], rows)

        expect(result.status).toBe("resolved")
        if (result.status !== "resolved" || result.view.kind !== "archive") return
        expect(result.view.selection).toEqual({
            workspaceUri: FEATURE,
            archiveDir: "2026-08-11-add-thing",
        })
    })

    test("a ship archived in the MAIN worktree still lands there", () => {
        const view = repoView("/proj/.git", "proj", "/proj")
        const rows = [registered("/proj", "proj", { repoId: "/proj/.git" })]
        const result = resolveAddress(shipAddress(ship("/proj/.git", "/proj"), [view], rows), [view], rows)

        expect(result.status).toBe("resolved")
        if (result.status !== "resolved" || result.view.kind !== "archive") return
        expect(result.view.selection?.workspaceUri).toBe("/proj")
    })

    test("a ship archived in an AUTO-DISCOVERED worktree still lands there", () => {
        // What this replaces asserted that the resolved uri is one of the
        // registered rows, on the premise that "a ship's `worktreePath` is
        // always a registered folder" — and passed only because its own
        // fixture registered the feature worktree. The premise is false for the
        // ordinary case: SpecForge auto-discovers a repository's sibling
        // worktrees, and the Settings listing deliberately carries only
        // user-registered folders (`workspace-registry`). So the destination
        // has to come from the repository's TRACKED worktrees, and the
        // pre-selection must survive the worktree being absent from the
        // registered listing.
        const view = repoView("/proj/.git", "proj", "/proj", ["/proj", FEATURE])
        const rows = [registered("/proj", "proj", { repoId: "/proj/.git" })]
        expect(rows.some((w) => w.uri === FEATURE)).toBe(false) // precondition

        const result = resolveAddress(shipAddress(ship("/proj/.git", FEATURE), [view], rows), [view], rows)
        if (result.status !== "resolved" || result.view.kind !== "archive") throw new Error("unresolved")
        expect(result.view.selection).toEqual({
            workspaceUri: FEATURE,
            archiveDir: "2026-08-11-add-thing",
        })
    })
})

// ---- rowLabel ----------------------------------------------------------

describe("rowLabel", () => {
    test("prefers the configured display name, falling back to the registered name", () => {
        expect(rowLabel(registered("/proj", "proj"))).toBe("proj")
        expect(rowLabel(registered("/proj", "proj", { displayName: "Project" }))).toBe("Project")
    })
})
