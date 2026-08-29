import { describe, expect, test } from "bun:test"
import type { Address } from "./routing/address"
import { readerTitle } from "./readerTitle"

const REPO = { kind: "repo", repo: "specforge" } as const
const WS = { kind: "workspace", workspace: "notes" } as const

describe("readerTitle", () => {
    test("an artifact names its file, its change, and its workspace", () => {
        const address: Address = {
            kind: "artifact",
            scope: REPO,
            changeId: "add-dark-mode",
            artifactKind: "proposal",
        }
        expect(readerTitle(address, "SpecForge")).toBe(
            "proposal.md — add-dark-mode — SpecForge",
        )
    })

    /// Every capability's file is called `spec.md`, so the capability has to be
    /// in the title or two open specs are indistinguishable in the Window menu.
    test("a capability spec names the capability, not just spec.md", () => {
        const address: Address = {
            kind: "artifact",
            scope: REPO,
            changeId: "add-dark-mode",
            artifactKind: "spec",
            capability: "web-ui",
        }
        expect(readerTitle(address, "SpecForge")).toBe(
            "spec.md — web-ui — add-dark-mode — SpecForge",
        )
    })

    test("a file at the browse root names just the file and the workspace", () => {
        const address: Address = { kind: "file", scope: WS, path: "README.md" }
        expect(readerTitle(address, "Notes")).toBe("README.md — Notes")
    })

    /// The same reason as the capability case: a main spec's file name is
    /// `spec.md` and its directory is the only thing that identifies it.
    test("a nested file names its directory", () => {
        const address: Address = {
            kind: "file",
            scope: REPO,
            path: "openspec/specs/web-ui/spec.md",
        }
        expect(readerTitle(address, "SpecForge")).toBe("spec.md — web-ui — SpecForge")
    })

    test("a blank workspace label leaves no dangling separator", () => {
        const address: Address = { kind: "file", scope: WS, path: "README.md" }
        expect(readerTitle(address, "")).toBe("README.md")
    })

    test("an address that names no document has no title", () => {
        expect(readerTitle({ kind: "home" }, "SpecForge")).toBe("")
        expect(readerTitle({ kind: "files", scope: WS }, "Notes")).toBe("")
        expect(readerTitle({ kind: "settings" }, "SpecForge")).toBe("")
    })
})
