import { useReviewDraftEditorContext } from "../hooks/useReviewDraftEditor"
import {
    blockedReasonText,
    decisionLabel,
    isTerminalPhase,
    ledgerOutcomeText,
    ledgerStepText,
    resumeEligible,
} from "../reviewActions"
import type {
    LedgerEntry,
    OperationPhase,
    PartialResult,
    ReviewOperationState,
} from "../types"

// `pull-request-actions`: *Current State Authorises Every Draft and Submission
// Boundary*, *Interrupted Operations Reconcile Without Automatic Mutation*,
// *Review Operations Respect Provider Failure Boundaries*. The native review
// status: the blocking reason, explicit stale recovery, a live or paused
// operation's ledger-derived partial result, terminal receipts, and the
// explicit Resume, Abandon and Start over actions. Every control reads the
// draft editor its pull-request view provides, so no browser surface renders
// one; the composer mounts this block above its form.

/// What an operation's phase says to a reviewer. The three live phases mean a
/// process still holds the execution lock, so no local action may touch it.
function phaseText(phase: OperationPhase): string {
    switch (phase) {
        case "running":
        case "attempting":
        case "reconciling":
            return "This review is running in another SpecForge process. Resume and Abandon stay unavailable until it finishes."
        case "paused":
            return "An earlier review is paused before it finished."
        case "submitted":
            return "Review submitted."
        case "abandoned":
            return "Review abandoned."
    }
}

/// A submitted operation's one line: the decision the provider applied, when
/// the receipt names one — BitBucket's Comment applies none.
function submittedText(operation: ReviewOperationState): string {
    const decision = operation.receipt?.decision ?? null
    return decision === null
        ? "Review submitted."
        : `Review submitted: ${decisionLabel(decision)}.`
}

/// The ledger's three groups in write order within each group: what the
/// provider confirmed, what no bounded read has decided, and what failed.
function PartialLedger({ partial }: { partial: PartialResult }) {
    const groups: Array<[string, string, LedgerEntry[]]> = [
        ["Confirmed", "confirmed", partial.succeeded],
        ["Unresolved", "unresolved", partial.unresolved],
        ["Failed", "failed", partial.failed],
    ]
    const total = partial.succeeded.length + partial.unresolved.length + partial.failed.length
    if (total === 0) {
        return (
            <p className="pull-request-review-status-note">
                No provider writes were recorded before this review stopped.
            </p>
        )
    }
    return (
        <div className="pull-request-review-status-partial">
            {groups.map(([label, kind, entries]) =>
                entries.length === 0 ? null : (
                    <div
                        key={kind}
                        className={`pull-request-review-status-group pull-request-review-status-group--${kind}`}
                    >
                        <p className="pull-request-review-status-label">{label}</p>
                        <ul className="pull-request-review-status-list">
                            {entries.map((entry, index) => (
                                <li
                                    key={`${kind}-${index}`}
                                    className="pull-request-review-status-entry"
                                >
                                    <span className="pull-request-review-status-step">
                                        {ledgerStepText(entry.step)}
                                    </span>{" "}
                                    <span className="pull-request-review-status-outcome">
                                        {ledgerOutcomeText(entry.outcome)}
                                    </span>
                                </li>
                            ))}
                        </ul>
                    </div>
                ),
            )}
        </div>
    )
}

/// A terminal operation's receipt. A submitted review says so with its
/// decision, on the phase line itself (`submittedText`), so it renders nothing
/// here: the provider's review id is an opaque node id that tells a reviewer
/// nothing, and it stays in the journal. An abandoned one keeps every
/// confirmed or uncertain effect and names a pending review that may need
/// manual handling at the provider.
function Receipt({ operation }: { operation: ReviewOperationState }) {
    const receipt = operation.receipt
    if (operation.phase === "abandoned") {
        return (
            <div className="pull-request-review-status-receipt" role="status">
                <p>
                    Review abandoned. Confirmed and uncertain provider effects stay in its
                    receipt.
                </p>
                {receipt?.reviewId && (
                    <p className="pull-request-review-status-note">
                        Provider review {receipt.reviewId} may remain; nothing was rolled back.
                    </p>
                )}
                {receipt?.remainingPendingReview && (
                    <p className="pull-request-review-status-note pull-request-review-status-note--manual">
                        A pending review may remain at the provider (
                        {receipt.remainingPendingReview}). Remove it there if needed.
                    </p>
                )}
            </div>
        )
    }
    return null
}

/// One pull request's native review status, rendered by its composer. Nothing
/// here edits frozen content: Resume and Abandon act on the operation, Start
/// over replaces the current draft only when no operation is unfinished, and
/// recovery is one explicit click that never remaps an inline comment.
export function PullRequestReviewStatus() {
    const editor = useReviewDraftEditorContext()
    if (!editor) return null
    const { state, draft, actionBusy, recover, startOver, resume, abandon } = editor
    if (!state) return null

    const operation = state.operation
    const blocked = state.blockedReason
    const unfinished = operation !== null && !isTerminalPhase(operation.phase)
    // Start over needs a fresh draft-creation boundary: a store that cannot be
    // read or written, a pull request no longer listed, a missing current
    // detail or a legacy plaintext credential refuses it before it begins.
    const startBlocked =
        blocked === "actionStoreCorrupt" ||
        blocked === "notListed" ||
        blocked === "detailRequired" ||
        blocked === "legacyCredential"
    const startable = draft !== null && !unfinished && !startBlocked
    const busy = actionBusy !== null
    const canResume = operation !== null && resumeEligible(operation) && blocked === null
    const resumeReason =
        operation !== null && operation.phase === "paused" && blocked === null && !canResume
            ? operation.partial && operation.partial.unresolved.length > 0
                ? "Resume waits until the unresolved step is reconciled by a fresh state read."
                : "The first unresolved step failed and is not retried; abandon this review and start over."
            : null
    const resumeReasonId = "pull-request-review-status-resume-reason"

    return (
        <div className="pull-request-review-status">
            {blocked && (
                <p className="pull-request-composer-blocked" role="status">
                    {blockedReasonText(blocked)}
                </p>
            )}
            {state.staleBasis && draft !== null && (
                <div className="pull-request-review-status-recovery" role="status">
                    <p className="pull-request-review-status-note">
                        Recover this draft onto the current commits. It keeps the summary and
                        any reply that is still valid; every inline comment becomes unplaced
                        until you choose a current line. Nothing is moved automatically.
                    </p>
                    <button
                        type="button"
                        className="pull-request-view-action"
                        disabled={busy}
                        onClick={() => void recover()}
                    >
                        {actionBusy === "recover"
                            ? "Recovering…"
                            : "Recover onto current commits"}
                    </button>
                </div>
            )}
            {operation && (
                <div className="pull-request-review-status-operation">
                    <p className="pull-request-review-status-phase" role="status">
                        {operation.phase === "submitted"
                            ? submittedText(operation)
                            : phaseText(operation.phase)}
                    </p>
                    {!isTerminalPhase(operation.phase) && operation.partial && (
                        <PartialLedger partial={operation.partial} />
                    )}
                    {isTerminalPhase(operation.phase) && <Receipt operation={operation} />}
                    {operation.phase === "paused" && (
                        <div className="pull-request-review-status-actions">
                            <button
                                type="button"
                                className="pull-request-view-action pull-request-review-status-resume"
                                disabled={busy || !canResume}
                                aria-describedby={resumeReason ? resumeReasonId : undefined}
                                onClick={() => void resume()}
                            >
                                {actionBusy === "resume" ? "Resuming…" : "Resume review"}
                            </button>
                            <button
                                type="button"
                                className="pull-request-view-action pull-request-review-status-abandon"
                                disabled={busy}
                                onClick={() => void abandon()}
                            >
                                {actionBusy === "abandon" ? "Abandoning…" : "Abandon review"}
                            </button>
                            {resumeReason && (
                                <p
                                    className="pull-request-review-status-note"
                                    id={resumeReasonId}
                                >
                                    {resumeReason}
                                </p>
                            )}
                        </div>
                    )}
                </div>
            )}
            {startable && (
                <div className="pull-request-review-status-start-over">
                    <button
                        type="button"
                        className="pull-request-view-action pull-request-review-status-restart"
                        disabled={busy}
                        onClick={() => void startOver()}
                    >
                        {actionBusy === "startOver" ? "Starting over…" : "Start over"}
                    </button>
                    <p className="pull-request-review-status-note">
                        Discards this draft and starts an empty one. Nothing is sent to the
                        provider.
                    </p>
                </div>
            )}
        </div>
    )
}
