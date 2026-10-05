import { describe, expect, test } from "bun:test"
import type { LinkedPullRequest } from "../types"
import { overflowChipLabel, pullRequestChipLabel } from "./PullRequestChip"

function linked(overrides: Partial<LinkedPullRequest> = {}): LinkedPullRequest {
    return {
        provider: "github",
        role: "authored",
        id: 42,
        title: "Add the panel",
        url: "https://github.com/acme/api/pull/42",
        repoFullName: "acme/api",
        draft: false,
        checks: null,
        conflicting: false,
        review: { approvals: 1, changesRequested: 0, pending: 2 },
        ...overrides,
    }
}

describe("pullRequestChipLabel", () => {
    test("states provider, repository, number, title, role and review in words", () => {
        expect(pullRequestChipLabel(linked())).toBe(
            "GitHub pull request #42 in acme/api: Add the panel. Yours. " +
                "1 approved · 0 changes requested · 2 pending",
        )
    })

    test("says the checks state in words, and the draft and conflict markers", () => {
        const label = pullRequestChipLabel(
            linked({ checks: "failing", draft: true, conflicting: true }),
        )
        expect(label).toContain("Checks failing")
        expect(label).toContain("Draft")
        expect(label).toContain("Merge conflicts")
    })

    test("a review request and a BitBucket pull request are named as such", () => {
        expect(pullRequestChipLabel(linked({ role: "reviewRequested" }))).toContain(
            "Awaiting your review",
        )
        expect(pullRequestChipLabel(linked({ provider: "bitbucket" }))).toStartWith(
            "BitBucket pull request #42",
        )
    })

    test("an unknown review state is said to be unknown, not zero", () => {
        expect(pullRequestChipLabel(linked({ review: null }))).toContain("Review state unknown")
    })

    test("no checks state, no checks words", () => {
        expect(pullRequestChipLabel(linked())).not.toContain("Checks")
    })
})

describe("overflowChipLabel", () => {
    test("lists every pull request the +N chip stands for", () => {
        expect(
            overflowChipLabel([
                linked({ id: 3, title: "Third" }),
                linked({ id: 4, title: "Fourth", provider: "bitbucket" }),
            ]),
        ).toBe("2 more linked pull requests:\nGitHub #3 — Third\nBitBucket #4 — Fourth")
        expect(overflowChipLabel([linked({ id: 3, title: "Third" })])).toStartWith(
            "1 more linked pull request:",
        )
    })
})
