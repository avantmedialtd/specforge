import type {
    BlockedReason,
    DraftItem,
    DraftMutationRefusal,
    LedgerOutcome,
    LedgerStep,
    OperationPhase,
    ReviewActionState,
    ReviewCapability,
    ReviewCapabilityName,
    ReviewDecision,
    ReviewDraft,
    ReviewOperationState,
} from "./types"

// `pull-request-actions`: *Draft Items and Decisions Are Explicit and Valid*,
// *Current State Authorises Every Draft and Submission Boundary*. The
// composer's provider-neutral wording and eligibility. Pure, so the rules are
// tested without a DOM.

/// The bound one summary or item body may hold, as UTF-8 bytes (30 × 1024),
/// mirroring `MAX_BODY_BYTES` in `crates/openspec-app/src/review_actions.rs`.
export const MAX_BODY_BYTES = 30 * 1024

/// The most comment and reply items one draft may hold, mirroring
/// `MAX_DRAFT_ITEMS` in the same file.
export const MAX_DRAFT_ITEMS = 100

/// The decisions a review can submit, in the order the composer offers them.
export const REVIEW_DECISIONS: readonly ReviewDecision[] = [
    "comment",
    "approve",
    "requestChanges",
]

/// A decision's visible text, which is also its accessible name.
export function decisionLabel(decision: ReviewDecision): string {
    switch (decision) {
        case "comment":
            return "Comment"
        case "approve":
            return "Approve"
        case "requestChanges":
            return "Request changes"
    }
}

/// Why the service blocked every draft and submission boundary, in words a
/// reviewer can act on.
export function blockedReasonText(reason: BlockedReason): string {
    switch (reason) {
        case "actionStoreCorrupt":
            return "The review action store is damaged. Review actions stay blocked until the file is repaired or removed outside SpecForge."
        case "notListed":
            return "This pull request is no longer in the provider's list."
        case "detailRequired":
            return "A current read of this pull request is required before reviewing."
        case "staleCommits":
            return "The branch has new commits since this review was started."
        case "legacyCredential":
            return "This credential was saved in plain text. Save it again in Settings to review."
        case "credentialChanged":
            return "The credential changed since this review was started."
        case "actorChanged":
            return "A different account is signed in since this review was started."
        case "decisionUnavailable":
            return "The selected decision is no longer available for this pull request."
    }
}

/// Whether a phase is terminal: only `submitted` and `abandoned` are, so any
/// other operation is unfinished and fences another submission.
export function isTerminalPhase(phase: OperationPhase): boolean {
    return phase === "submitted" || phase === "abandoned"
}

/// One ledger step's visible text, provider-neutral like the composer's verbs.
export function ledgerStepText(step: LedgerStep): string {
    switch (step.kind) {
        case "inlineComment":
            return `Inline comment ${step.item}`
        case "reply":
            return `Reply ${step.item}`
        case "summary":
            return "Summary"
        case "decision":
            return "Decision"
    }
}

/// What one write came to at the provider, in words: a confirmed write names
/// its provider id when it has one, an uncertain one says the provider may or
/// may not hold it, and a failure keeps the provider's reason.
export function ledgerOutcomeText(outcome: LedgerOutcome): string {
    switch (outcome.kind) {
        case "confirmed":
            return outcome.providerId === null
                ? "confirmed"
                : `confirmed (${outcome.providerId})`
        case "uncertain":
            return "unresolved — the provider may or may not hold it"
        case "failed":
            return `failed — ${outcome.reason}`
    }
}

/// Whether an explicit Resume may act on `operation`: only a paused operation
/// whose first unresolved step is eligible for another attempt. A step left
/// uncertain has not been reconciled, and a failed step is never retried under
/// the same credential, so both keep Resume unavailable; Abandon stays.
export function resumeEligible(operation: ReviewOperationState): boolean {
    if (operation.phase !== "paused") return false
    if (operation.partial) {
        return (
            operation.partial.unresolved.length === 0 &&
            operation.partial.failed.length === 0
        )
    }
    return operation.ledger.every((entry) => entry.outcome.kind === "confirmed")
}

/// One of the five actions the current state offers, or null when the service
/// did not name it.
export function reviewCapability(
    state: ReviewActionState,
    name: ReviewCapabilityName,
): ReviewCapability | null {
    return state.capabilities.find((capability) => capability.name === name) ?? null
}

/// Whether a decision is currently offered. A missing capability is treated as
/// unavailable: the service always names all five, and one it did not name is
/// no permission to act.
export function decisionCapability(
    state: ReviewActionState,
    decision: ReviewDecision,
): ReviewCapability | null {
    return reviewCapability(state, decision)
}

/// A body's size as UTF-8, the bound the service enforces.
export function bodyBytes(body: string): number {
    return new TextEncoder().encode(body).length
}

/// What the composer says about submitting the review it currently holds.
export interface SubmitEligibility {
    eligible: boolean
    /// Why the review cannot be submitted now; null exactly when eligible.
    reason: string | null
}

/// How many of a draft's items are unplaced: kept after stale recovery but
/// waiting for the reviewer to choose a current line or thread.
export function unplacedCount(items: readonly DraftItem[] | undefined): number {
    return (items ?? []).filter(
        (item) => item.kind === "unplacedInline" || item.kind === "unplacedReply",
    ).length
}

/// Whether the review can be submitted, and why not when it cannot:
///
/// - the state must have been read and not be blocked;
/// - no earlier operation may still be unfinished;
/// - the chosen decision must be one current capabilities offer;
/// - the summary must fit the 30 KiB bound and the draft the 100-item bound;
/// - no item may be unplaced, since the service refuses to freeze one;
/// - Comment and Request changes need a non-empty summary or item, while
///   Approve may be empty. Whitespace alone is not content.
export function submitEligibility(
    state: ReviewActionState | null,
    decision: ReviewDecision,
    summary: string,
    itemCount: number,
    unplaced = 0,
): SubmitEligibility {
    if (!state) return { eligible: false, reason: "Review state is still loading." }
    if (state.blockedReason !== null) {
        return { eligible: false, reason: blockedReasonText(state.blockedReason) }
    }
    if (state.operation !== null && !isTerminalPhase(state.operation.phase)) {
        return {
            eligible: false,
            reason: "A review is already unfinished. Resume or abandon it before submitting another.",
        }
    }
    const capability = decisionCapability(state, decision)
    if (!capability || !capability.available) {
        const reason = capability?.reason ?? "not offered for this pull request"
        return { eligible: false, reason: `${decisionLabel(decision)} is unavailable: ${reason}` }
    }
    if (bodyBytes(summary) > MAX_BODY_BYTES) {
        return {
            eligible: false,
            reason: `The summary is longer than ${MAX_BODY_BYTES / 1024} KiB.`,
        }
    }
    if (itemCount > MAX_DRAFT_ITEMS) {
        return {
            eligible: false,
            reason: `A review can hold at most ${MAX_DRAFT_ITEMS} comments and replies.`,
        }
    }
    if (unplaced > 0) {
        return {
            eligible: false,
            reason:
                unplaced === 1
                    ? "One comment needs a current line or thread before the review can be submitted."
                    : `${unplaced} comments need a current line or thread before the review can be submitted.`,
        }
    }
    if (summary.trim() === "" && itemCount === 0) {
        if (decision === "comment") {
            return {
                eligible: false,
                reason: "Write a summary or add an inline comment or reply before submitting a comment.",
            }
        }
        if (decision === "requestChanges") {
            return {
                eligible: false,
                reason: "Explain what needs to change before requesting changes.",
            }
        }
    }
    return { eligible: true, reason: null }
}

/// The kinds a `DraftMutationRefusal` can have, for narrowing a rejection the
/// transport hands back as a plain object.
const REFUSAL_KINDS: ReadonlySet<string> = new Set([
    "storeCorrupt",
    "storeFailure",
    "blocked",
    "noDraft",
    "operationUnfinished",
    "noOperation",
    "operationLive",
    "operationFinished",
    "operationNotEligible",
    "staleRevision",
    "revisionClaimed",
    "contentTooLarge",
    "staleAnchor",
    "emptyContent",
    "noContent",
])

/// A rejected review-action command as its structured refusal, or null when
/// the rejection is not one.
export function draftRefusal(err: unknown): DraftMutationRefusal | null {
    if (typeof err !== "object" || err === null) return null
    const kind = (err as { kind?: unknown }).kind
    if (typeof kind !== "string" || !REFUSAL_KINDS.has(kind)) return null
    return err as DraftMutationRefusal
}

/// The current draft a revision conflict carries, or null when the refusal is
/// not a conflict. The composer adopts it and re-reads the rest of the state.
export function conflictingDraft(refusal: DraftMutationRefusal): ReviewDraft | null {
    return refusal.kind === "staleRevision" || refusal.kind === "revisionClaimed"
        ? refusal.current
        : null
}

/// A refusal's words, for the composer's notice. A conflict is worded by its
/// caller, which adopts the draft it carries.
export function refusalText(refusal: DraftMutationRefusal): string {
    switch (refusal.kind) {
        case "storeCorrupt":
            return blockedReasonText("actionStoreCorrupt")
        case "storeFailure":
            return `The review action store could not be written — ${refusal.reason}`
        case "blocked":
            return blockedReasonText(refusal.reason)
        case "noDraft":
            return "There is no current draft for this pull request."
        case "operationUnfinished":
            return "An earlier review is still unfinished."
        case "noOperation":
            return "There is no review operation to act on."
        case "operationLive":
            return "The review operation is still running."
        case "operationFinished":
            return "The review operation has already finished."
        case "operationNotEligible":
            return refusal.reason
        case "staleRevision":
        case "revisionClaimed":
            return "Another window changed this review."
        case "contentTooLarge":
            return `A summary or comment is longer than ${MAX_BODY_BYTES / 1024} KiB, or the draft holds more than ${MAX_DRAFT_ITEMS} items.`
        case "staleAnchor":
            return "A comment's line is no longer current."
        case "emptyContent":
            return "A summary or comment is empty."
        case "noContent":
            return "A review needs a summary, comment or reply."
    }
}
