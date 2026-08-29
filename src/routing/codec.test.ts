import { describe, expect, test } from "bun:test"
import type { Address } from "./address"
import { decodeAddress, encodeAddress } from "./codec"

/// One representative Address per variant the codec must round-trip.
const SAMPLES: Address[] = [
    { kind: "home" },
    { kind: "settings" },
    { kind: "archive", selection: null },
    {
        kind: "archive",
        selection: { workspace: "myproject", archiveDir: "2026-01-15-add-thing" },
    },
    {
        kind: "archive",
        selection: {
            workspace: "specforge",
            archiveDir: "2026-01-15-add-thing",
            worktreeHint: "a1b2c3",
        },
    },
    { kind: "files", scope: { kind: "workspace", workspace: "myproject" } },
    { kind: "files", scope: { kind: "repo", repo: "specforge" } },
    { kind: "file", scope: { kind: "workspace", workspace: "notes" }, path: "README.md" },
    {
        kind: "file",
        scope: { kind: "repo", repo: "specforge" },
        path: "openspec/specs/web-ui/spec.md",
    },
    {
        kind: "artifact",
        scope: { kind: "workspace", workspace: "myproject" },
        changeId: "add-thing",
        artifactKind: "proposal",
    },
    {
        kind: "artifact",
        scope: { kind: "workspace", workspace: "myproject" },
        changeId: "add-thing",
        artifactKind: "design",
    },
    {
        kind: "artifact",
        scope: { kind: "workspace", workspace: "myproject" },
        changeId: "add-thing",
        artifactKind: "tasks",
    },
    {
        kind: "artifact",
        scope: { kind: "workspace", workspace: "myproject" },
        changeId: "add-thing",
        artifactKind: "spec",
        capability: "view-routing",
    },
    {
        kind: "artifact",
        scope: { kind: "repo", repo: "specforge" },
        changeId: "add-view-routing",
        artifactKind: "proposal",
    },
    {
        kind: "artifact",
        scope: { kind: "repo", repo: "specforge" },
        changeId: "add-view-routing",
        artifactKind: "spec",
        capability: "web-ui",
    },
    {
        kind: "artifact",
        scope: { kind: "repo", repo: "specforge", instance: "a1b2c3" },
        changeId: "add-view-routing",
        artifactKind: "tasks",
    },
    {
        kind: "artifact",
        scope: { kind: "repo", repo: "specforge", instance: "a1b2c3" },
        changeId: "add-view-routing",
        artifactKind: "spec",
        capability: "web-ui",
    },
]

describe("encodeAddress / decodeAddress round trip", () => {
    for (const address of SAMPLES) {
        test(`round-trips ${JSON.stringify(address)}`, () => {
            const path = encodeAddress(address)
            expect(decodeAddress(path)).toEqual(address)
        })
    }
})

describe("encodeAddress produces the documented grammar", () => {
    test("home", () => expect(encodeAddress({ kind: "home" })).toBe("/"))
    test("settings", () => expect(encodeAddress({ kind: "settings" })).toBe("/settings"))
    test("archive, no selection", () =>
        expect(encodeAddress({ kind: "archive", selection: null })).toBe("/archive"))
    test("archive, with selection", () =>
        expect(
            encodeAddress({
                kind: "archive",
                selection: { workspace: "foo", archiveDir: "2026-01-01-bar" },
            }),
        ).toBe("/archive/foo/2026-01-01-bar"))
    test("archive, with selection and a worktree hint (C2)", () =>
        expect(
            encodeAddress({
                kind: "archive",
                selection: {
                    workspace: "foo",
                    archiveDir: "2026-01-01-bar",
                    worktreeHint: "a1b2c3",
                },
            }),
        ).toBe("/archive/foo/2026-01-01-bar/a1b2c3"))
    test("decoding the 4-segment archive form recovers the hint", () =>
        expect(decodeAddress("/archive/foo/2026-01-01-bar/a1b2c3")).toEqual({
            kind: "archive",
            selection: {
                workspace: "foo",
                archiveDir: "2026-01-01-bar",
                worktreeHint: "a1b2c3",
            },
        }))
    test("flat workspace files", () =>
        expect(
            encodeAddress({ kind: "files", scope: { kind: "workspace", workspace: "foo" } }),
        ).toBe("/w/foo"))
    test("repo files (main worktree, no instance)", () =>
        expect(encodeAddress({ kind: "files", scope: { kind: "repo", repo: "bar" } })).toBe(
            "/r/bar",
        ))
    test("flat workspace artifact", () =>
        expect(
            encodeAddress({
                kind: "artifact",
                scope: { kind: "workspace", workspace: "foo" },
                changeId: "chg",
                artifactKind: "proposal",
            }),
        ).toBe("/w/foo/chg/proposal"))
    test("flat workspace spec", () =>
        expect(
            encodeAddress({
                kind: "artifact",
                scope: { kind: "workspace", workspace: "foo" },
                changeId: "chg",
                artifactKind: "spec",
                capability: "cap",
            }),
        ).toBe("/w/foo/chg/specs/cap"))
    test("repo single-instance artifact has no instance segment", () =>
        expect(
            encodeAddress({
                kind: "artifact",
                scope: { kind: "repo", repo: "bar" },
                changeId: "chg",
                artifactKind: "design",
            }),
        ).toBe("/r/bar/chg/design"))
    test("repo multi-instance artifact names its instance", () =>
        expect(
            encodeAddress({
                kind: "artifact",
                scope: { kind: "repo", repo: "bar", instance: "xyz" },
                changeId: "chg",
                artifactKind: "tasks",
            }),
        ).toBe("/r/bar/chg/xyz/tasks"))
    test("repo multi-instance spec", () =>
        expect(
            encodeAddress({
                kind: "artifact",
                scope: { kind: "repo", repo: "bar", instance: "xyz" },
                changeId: "chg",
                artifactKind: "spec",
                capability: "cap",
            }),
        ).toBe("/r/bar/chg/xyz/specs/cap"))
})

describe("decodeAddress rejects malformed paths as unresolvable", () => {
    const bad = [
        "/bogus",
        "/w",
        "/w/",
        "/r",
        "/w/foo/chg", // change without an artifact terminal
        "/w/foo/chg/bogus", // unknown artifact keyword
        "/w/foo/chg/instance/tasks", // flat workspaces have no instance segment
        "/w/foo/chg/specs", // specs with no capability
        "/r/bar/chg/inst/bogus", // instance form with unknown artifact keyword
        "/r/bar/chg/inst/bogus/cap", // 6 segments, but not the specs form
        "/archive/foo", // selection missing its archive-dir
        "/archive/foo/bar/baz/qux", // too many segments (4 is now valid: workspace/dir/worktree-hint)
        "/settings/extra",
    ]
    for (const path of bad) {
        test(`"${path}" is unresolvable`, () => {
            expect(decodeAddress(path)).toEqual({ kind: "unresolvable" })
        })
    }
})

describe("decodeAddress never returns a partially-populated Address for a bad path", () => {
    test("unresolvable carries no Address fields", () => {
        const result = decodeAddress("/totally/not/a/route/at/all/either")
        expect(result).toEqual({ kind: "unresolvable" })
        expect(Object.keys(result)).toEqual(["kind"])
    })
})

describe("the codec runs with no DOM, history object, or registered workspaces", () => {
    test("encode and decode succeed with nothing but the Address itself", () => {
        // No `window`, no fetch, no workspace list is referenced anywhere in
        // codec.ts — this test's mere success (in Bun's default DOM-less
        // environment) demonstrates the independence the *pure codec*
        // requirement demands.
        const address: Address = {
            kind: "artifact",
            scope: { kind: "workspace", workspace: "foo" },
            changeId: "chg",
            artifactKind: "proposal",
        }
        expect(decodeAddress(encodeAddress(address))).toEqual(address)
    })
})

describe("segment escaping", () => {
    test("a slug or id containing reserved characters round-trips", () => {
        const address: Address = {
            kind: "artifact",
            scope: { kind: "workspace", workspace: "foo bar/baz" },
            changeId: "chg?weird#id",
            artifactKind: "spec",
            capability: "cap with spaces",
        }
        expect(decodeAddress(encodeAddress(address))).toEqual(address)
    })
})

describe("file addresses", () => {
    test("encode under both scope prefixes with the reserved segment", () => {
        expect(
            encodeAddress({
                kind: "file",
                scope: { kind: "workspace", workspace: "notes" },
                path: "README.md",
            }),
        ).toBe("/w/notes/file/README.md")
        expect(
            encodeAddress({
                kind: "file",
                scope: { kind: "repo", repo: "specforge" },
                path: "openspec/specs/web-ui/spec.md",
            }),
        ).toBe("/r/specforge/file/openspec/specs/web-ui/spec.md")
    })

    test("a nested path keeps its structure through a round trip", () => {
        const address: Address = {
            kind: "file",
            scope: { kind: "repo", repo: "specforge" },
            path: "docs/deep/nested/architecture.md",
        }
        expect(decodeAddress(encodeAddress(address))).toEqual(address)
    })

    /// The whole reason `file` is reserved: without it this path reads as
    /// change `openspec`, the literal `specs`, capability `web-ui`, plus a
    /// trailing segment the grammar has no slot for.
    test("a path beginning openspec/specs decodes as a file, not a capability spec", () => {
        const decoded = decodeAddress("/r/specforge/file/openspec/specs/web-ui/spec.md")
        expect(decoded).toEqual({
            kind: "file",
            scope: { kind: "repo", repo: "specforge" },
            path: "openspec/specs/web-ui/spec.md",
        })
    })

    test("the capability-spec address is unaffected by the reservation", () => {
        expect(decodeAddress("/r/specforge/add-thing/specs/web-ui")).toEqual({
            kind: "artifact",
            scope: { kind: "repo", repo: "specforge" },
            changeId: "add-thing",
            artifactKind: "spec",
            capability: "web-ui",
        })
    })

    test("segments needing escapes survive a round trip", () => {
        const address: Address = {
            kind: "file",
            scope: { kind: "workspace", workspace: "my project" },
            path: "notes & drafts/a b.md",
        }
        const encoded = encodeAddress(address)
        expect(encoded).toBe("/w/my%20project/file/notes%20%26%20drafts/a%20b.md")
        expect(decodeAddress(encoded)).toEqual(address)
    })

    test("the reserved segment with no path names no document", () => {
        expect(decodeAddress("/w/notes/file")).toEqual({ kind: "unresolvable" })
        expect(decodeAddress("/r/specforge/file/")).toEqual({ kind: "unresolvable" })
    })

    test("a file address decodes with no registered-workspace data", () => {
        // Same guarantee the rest of the grammar makes: the reserved keyword
        // is decided from the closed vocabulary alone.
        expect(decodeAddress("/w/anything-at-all/file/x.md")).toEqual({
            kind: "file",
            scope: { kind: "workspace", workspace: "anything-at-all" },
            path: "x.md",
        })
    })
})
