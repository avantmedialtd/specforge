import { describe, expect, test } from "bun:test"
import {
    bodyBytes,
    blockedReasonText,
    conflictingDraft,
    decisionCapability,
    decisionLabel,
    draftRefusal,
    isTerminalPhase,
    MAX_BODY_BYTES,
    MAX_DRAFT_ITEMS,
    refusalText,
    REVIEW_DECISIONS,
    reviewCapability,
    submitEligibility,
} from "./reviewActions"
import type {
    BlockedReason,
    DraftMutationRefusal,
    OperationPhase,
    PullRequestReference,
    ReviewActionState,
    ReviewCapability,
    ReviewDraft,
} from "./types"

// `pull-request-actions`: *Draft Items and Decisions Are Explicit and Valid*,
// *Current State Authorises Every Draft and Submission Boundary*. The
// composer's rules, tested without a DOM: which decision is offered, what a
// refusal means, and whether a review can be submitted.

const REFERENCE: PullRequestReference = {
    provider: "github",
    owner: "acme",
    repo: "api",
    number: 42,
}

const CAPABILITIES: ReviewCapability[] = [
    { name: "comment", available: true, reason: null },
    { name: "approve", available: true, reason: null },
    { name: "requestChanges", available: true, reason: null },
    { name: "inlineComment", available: true, reason: null },
    { name: "reply", available: true, reason: null },
]

function draft(overrides: Partial<ReviewDraft> = {}): ReviewDraft {
    return {
        id: "draft-1",
        reference: REFERENCE,
        revision: 1,
        basis: { head: "head1", base: "base1" },
        decision: "comment",
        summary: "",
        items: [],
        ...overrides,
    }
}

function state(overrides: Partial<ReviewActionState> = {}): ReviewActionState {
    return {
        reference: REFERENCE,
        draft: draft(),
        capabilities: CAPABILITIES,
        blockedReason: null,
        operation: null,
        staleBasis: false,
        ...overrides,
    }
}

describe("the composer's wording", () => {
    test("names each decision as a provider-neutral verb", () => {
        expect(REVIEW_DECISIONS.map(decisionLabel)).toEqual([
            "Comment",
            "Approve",
            "Request changes",
        ])
    })

    test("gives every blocking reason words a reviewer can act on", () => {
        const reasons: BlockedReason[] = [
            "actionStoreCorrupt",
            "notListed",
            "detailRequired",
            "staleCommits",
            "legacyCredential",
            "credentialChanged",
            "actorChanged",
            "decisionUnavailable",
        ]
        const words = reasons.map(blockedReasonText)
        for (const text of words) expect(text.length).toBeGreaterThan(0)
        expect(new Set(words).size).toBe(reasons.length)
        expect(blockedReasonText("actionStoreCorrupt")).toContain("damaged")
        expect(blockedReasonText("staleCommits")).toContain("new commits")
    })

    test("only submitted and abandoned operations are terminal", () => {
        const phases: OperationPhase[] = [
            "running",
            "attempting",
            "reconciling",
            "paused",
            "submitted",
            "abandoned",
        ]
        expect(phases.filter(isTerminalPhase)).toEqual(["submitted", "abandoned"])
    })
})

describe("capabilities", () => {
    test("finds the named action, and no action the state did not name", () => {
        const current = state({
            capabilities: [
                { name: "comment", available: true, reason: null },
                { name: "approve", available: false, reason: "you authored it" },
            ],
        })
        expect(reviewCapability(current, "approve")).toEqual({
            name: "approve",
            available: false,
            reason: "you authored it",
        })
        expect(decisionCapability(current, "requestChanges")).toBeNull()
    })
})

describe("body sizes", () => {
    test("counts UTF-8 bytes, not characters", () => {
        expect(bodyBytes("abc")).toBe(3)
        expect(bodyBytes("é")).toBe(2)
        expect(bodyBytes("😀")).toBe(4)
        expect(bodyBytes("")).toBe(0)
    })

    test("mirrors the service's 30 KiB and 100-item bounds", () => {
        expect(MAX_BODY_BYTES).toBe(30 * 1024)
        expect(MAX_DRAFT_ITEMS).toBe(100)
    })
})

describe("submit eligibility", () => {
    test("waits for the first state read", () => {
        expect(submitEligibility(null, "comment", "hello", 0)).toEqual({
            eligible: false,
            reason: "Review state is still loading.",
        })
    })

    test("a blocked state cannot submit, and says why", () => {
        const eligibility = submitEligibility(
            state({ blockedReason: "staleCommits" }),
            "comment",
            "hello",
            0,
        )
        expect(eligibility.eligible).toBe(false)
        expect(eligibility.reason).toBe(blockedReasonText("staleCommits"))
    })

    test("an unfinished operation fences another submission", () => {
        for (const phase of ["running", "attempting", "reconciling", "paused"] as const) {
            const eligibility = submitEligibility(
                state({ operation: { id: "op", phase, ledger: [], partial: null, receipt: null } }),
                "comment",
                "hello",
                0,
            )
            expect(eligibility.eligible).toBe(false)
            expect(eligibility.reason).toContain("unfinished")
        }
        for (const phase of ["submitted", "abandoned"] as const) {
            const eligibility = submitEligibility(
                state({ operation: { id: "op", phase, ledger: [], partial: null, receipt: null } }),
                "comment",
                "hello",
                0,
            )
            expect(eligibility.eligible).toBe(true)
        }
    })

    test("an unavailable or unnamed decision cannot submit, with its reason", () => {
        const unavailable = state({
            capabilities: [{ name: "comment", available: false, reason: "read-only" }],
        })
        expect(submitEligibility(unavailable, "comment", "hello", 0)).toEqual({
            eligible: false,
            reason: "Comment is unavailable: read-only",
        })
        expect(submitEligibility(unavailable, "approve", "hello", 0)).toEqual({
            eligible: false,
            reason: "Approve is unavailable: not offered for this pull request",
        })
    })

    test("refuses a summary beyond 30 KiB and a draft beyond 100 items", () => {
        expect(submitEligibility(state(), "comment", "a".repeat(MAX_BODY_BYTES + 1), 0)).toEqual({
            eligible: false,
            reason: "The summary is longer than 30 KiB.",
        })
        expect(
            submitEligibility(state(), "comment", "a".repeat(MAX_BODY_BYTES), 0).eligible,
        ).toBe(true)
        expect(submitEligibility(state(), "comment", "hello", MAX_DRAFT_ITEMS + 1)).toEqual({
            eligible: false,
            reason: "A review can hold at most 100 comments and replies.",
        })
    })

    test("Comment needs a non-empty summary or item; whitespace is not content", () => {
        for (const summary of ["", "   ", "\n\t "]) {
            expect(submitEligibility(state(), "comment", summary, 0)).toEqual({
                eligible: false,
                reason: "Write a summary or add an inline comment or reply before submitting a comment.",
            })
        }
        expect(submitEligibility(state(), "comment", "  hello  ", 0).eligible).toBe(true)
        expect(submitEligibility(state(), "comment", "", 1).eligible).toBe(true)
    })

    test("Request changes needs an explanation, while Approve may be empty", () => {
        expect(submitEligibility(state(), "requestChanges", "  ", 0)).toEqual({
            eligible: false,
            reason: "Explain what needs to change before requesting changes.",
        })
        expect(submitEligibility(state(), "requestChanges", "", 1).eligible).toBe(true)
        expect(submitEligibility(state(), "approve", "", 0)).toEqual({
            eligible: true,
            reason: null,
        })
    })
})

describe("refusals", () => {
    test("narrows a structured refusal and ignores anything else", () => {
        const refusal: DraftMutationRefusal = { kind: "blocked", reason: "notListed" }
        expect(draftRefusal(refusal)).toBe(refusal)
        expect(draftRefusal(new Error("boom"))).toBeNull()
        expect(draftRefusal({ kind: "notARefusal" })).toBeNull()
        expect(draftRefusal(null)).toBeNull()
        expect(draftRefusal("blocked")).toBeNull()
    })

    test("a revision conflict carries the current draft; other refusals do not", () => {
        const current = draft({ revision: 4 })
        expect(conflictingDraft({ kind: "staleRevision", current })).toBe(current)
        expect(conflictingDraft({ kind: "revisionClaimed", current })).toBe(current)
        expect(conflictingDraft({ kind: "noDraft" })).toBeNull()
        expect(conflictingDraft({ kind: "contentTooLarge" })).toBeNull()
    })

    test("words every refusal, naming a conflict as another window's change", () => {
        const refusals: DraftMutationRefusal[] = [
            { kind: "storeCorrupt", reason: "bad file" },
            { kind: "storeFailure", reason: "disk full" },
            { kind: "blocked", reason: "staleCommits" },
            { kind: "noDraft" },
            { kind: "operationUnfinished" },
            { kind: "noOperation" },
            { kind: "operationLive" },
            { kind: "operationFinished" },
            { kind: "operationNotEligible", reason: "uncertain step" },
            { kind: "staleRevision", current: draft() },
            { kind: "revisionClaimed", current: draft() },
            { kind: "contentTooLarge" },
            { kind: "staleAnchor", at: 0 },
            { kind: "emptyContent", at: null },
            { kind: "noContent" },
        ]
        for (const refusal of refusals) expect(refusalText(refusal).length).toBeGreaterThan(0)
        expect(refusalText({ kind: "storeFailure", reason: "disk full" })).toContain("disk full")
        expect(refusalText({ kind: "operationNotEligible", reason: "uncertain step" })).toBe(
            "uncertain step",
        )
        expect(refusalText({ kind: "staleRevision", current: draft() })).toContain(
            "Another window changed this review.",
        )
    })
})
