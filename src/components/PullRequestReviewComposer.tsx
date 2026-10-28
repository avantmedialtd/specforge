import { useReviewDraftEditorContext } from "../hooks/useReviewDraftEditor"
import {
    bodyBytes,
    blockedReasonText,
    decisionCapability,
    decisionLabel,
    MAX_BODY_BYTES,
    REVIEW_DECISIONS,
    submitEligibility,
} from "../reviewActions"
import type { PullRequestReference, ReviewDecision } from "../types"
import { ChoiceGroup, type ChoiceOption } from "./ChoiceGroup"
import { prettifyError } from "./errors"

// `pull-request-actions`: *Desktop-Only Pull-Request Reviews*, *Draft Items
// and Decisions Are Explicit and Valid*. The native review composer: the
// draft's exact summary, one explicit decision from the current capabilities,
// and a submission that freezes the revision the reviewer read. Mounted only
// from `PullRequestView` under `isTauri()`, inside the one draft editor the
// diff's line and thread affordances also write through; the browser skin
// renders none of it, and no wrapper here would send a command there anyway.

/// One pull request's review composer, reading the action state and draft
/// editor its pull-request view provides so the center pane and the detached
/// window show one draft, one revision and one blocking reason.
export function PullRequestReviewComposer({ reference }: { reference: PullRequestReference }) {
    const editor = useReviewDraftEditorContext()
    if (!editor) throw new Error("the review composer renders only inside a pull-request view")
    const {
        state,
        error,
        reload,
        draft,
        summary,
        decision,
        setSummary,
        setDecision,
        saveForm,
        notice,
        saving,
        submitting,
        submit,
        editable,
        inlineUnavailable,
        replyUnavailable,
        atItemLimit,
    } = editor
    const blocked = state?.blockedReason ?? null

    const decisions = state
        ? REVIEW_DECISIONS.map((value) => ({
              value,
              capability: decisionCapability(state, value),
          }))
        : []
    const options: ChoiceOption<ReviewDecision>[] = decisions
        .filter(({ capability }) => capability?.available === true)
        .map(({ value }) => ({ value, label: decisionLabel(value) }))
    const eligibility = submitEligibility(
        state,
        decision,
        summary,
        draft?.items.length ?? 0,
    )
    const loading = state === null && error === null
    const reasonId = "pull-request-composer-submit-reason"

    return (
        <section
            className="pull-request-view-section pull-request-composer"
            aria-label={`Review of pull request #${reference.number}`}
        >
            <h2 className="pull-request-view-heading">Review</h2>
            {error && (
                <div className="pull-request-composer-status">
                    <p className="pull-request-composer-error" role="alert">
                        Couldn't read the review state — {prettifyError(error)}
                    </p>
                    <button
                        type="button"
                        className="pull-request-view-action"
                        onClick={() => void reload()}
                    >
                        Try again
                    </button>
                </div>
            )}
            {loading && (
                <p className="pull-request-view-note" role="status">
                    Reading review state…
                </p>
            )}
            {blocked && (
                <p className="pull-request-composer-blocked" role="status">
                    {blockedReasonText(blocked)}
                </p>
            )}
            {(editable || draft) && (
                <>
                    <div className="pull-request-composer-field">
                        <label
                            className="pull-request-composer-label"
                            htmlFor="pull-request-composer-summary"
                        >
                            Summary{" "}
                            <span className="pull-request-composer-optional">optional</span>
                        </label>
                        <textarea
                            id="pull-request-composer-summary"
                            className="settings-text-input pull-request-composer-summary"
                            rows={4}
                            value={summary}
                            placeholder="Leave a summary for this review."
                            disabled={!editable}
                            onChange={(event) => setSummary(event.currentTarget.value)}
                            onBlur={saveForm}
                        />
                        <span className="pull-request-composer-count">
                            {bodyBytes(summary)} / {MAX_BODY_BYTES} bytes
                        </span>
                    </div>
                    <div className="pull-request-composer-decisions">
                        <span className="pull-request-composer-label">Decision</span>
                        {!editable ? (
                            <p className="pull-request-composer-decision">{decisionLabel(decision)}</p>
                        ) : options.length > 0 ? (
                            <ChoiceGroup
                                options={options}
                                value={decision}
                                onChange={(next) => {
                                    setDecision(next)
                                    saveForm()
                                }}
                                label="Review decision"
                                describedBy={!eligibility.eligible ? reasonId : undefined}
                            />
                        ) : (
                            <p className="pull-request-view-note">
                                No decision is available for this pull request.
                            </p>
                        )}
                        {decisions.map(({ value, capability }) =>
                            !capability || !capability.available ? (
                                <p key={value} className="pull-request-composer-unavailable">
                                    {decisionLabel(value)} is unavailable:{" "}
                                    {capability?.reason ?? "not offered for this pull request"}
                                </p>
                            ) : null,
                        )}
                    </div>
                    {state && (
                        <div className="pull-request-composer-capabilities">
                            {inlineUnavailable && (
                                <p className="pull-request-composer-unavailable">
                                    Inline comments are unavailable: {inlineUnavailable}
                                </p>
                            )}
                            {replyUnavailable && (
                                <p className="pull-request-composer-unavailable">
                                    Replies are unavailable: {replyUnavailable}
                                </p>
                            )}
                            {atItemLimit && (
                                <p className="pull-request-composer-unavailable">
                                    This review holds the maximum of 100 comments and replies.
                                </p>
                            )}
                        </div>
                    )}
                    <div className="pull-request-composer-actions">
                        <button
                            type="button"
                            className="btn-primary pull-request-composer-submit"
                            disabled={!eligibility.eligible || saving || submitting}
                            aria-describedby={!eligibility.eligible ? reasonId : undefined}
                            onClick={submit}
                        >
                            {submitting ? "Submitting…" : "Submit review"}
                        </button>
                        {saving && (
                            <span className="pull-request-composer-saving" role="status">
                                Saving draft…
                            </span>
                        )}
                        {!eligibility.eligible && (
                            <p className="pull-request-composer-reason" id={reasonId}>
                                {eligibility.reason}
                            </p>
                        )}
                    </div>
                </>
            )}
            {notice && (
                <p
                    className={`pull-request-composer-notice${notice.failed ? " pull-request-composer-notice--failed" : ""}`}
                    role={notice.failed ? "alert" : "status"}
                >
                    {notice.text}
                </p>
            )}
        </section>
    )
}
