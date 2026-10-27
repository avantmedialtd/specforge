import { useEffect, useRef, useState } from "react"
import { createReviewDraft, editReviewDraft, submitReview } from "../api"
import { useReviewActions } from "../hooks/usePullRequestActions"
import {
    bodyBytes,
    blockedReasonText,
    conflictingDraft,
    decisionCapability,
    decisionLabel,
    draftRefusal,
    MAX_BODY_BYTES,
    refusalText,
    REVIEW_DECISIONS,
    submitEligibility,
} from "../reviewActions"
import type {
    DraftItemInput,
    PullRequestReference,
    ReviewDecision,
    ReviewDraft,
} from "../types"
import { ChoiceGroup, type ChoiceOption } from "./ChoiceGroup"
import { prettifyError } from "./errors"

// `pull-request-actions`: *Desktop-Only Pull-Request Reviews*, *Draft Items
// and Decisions Are Explicit and Valid*. The native review composer: the
// draft's exact summary, one explicit decision from the current capabilities,
// and a submission that freezes the revision the reviewer read. Mounted only
// from `PullRequestView` under `isTauri()`; the browser skin renders none of
// it, and no wrapper here would send a command there anyway.

interface Notice {
    text: string
    failed: boolean
}

/// One pull request's review composer, reading the action state its
/// pull-request view provides so the center pane and the detached window show
/// one draft, one revision and one blocking reason.
export function PullRequestReviewComposer({ reference }: { reference: PullRequestReference }) {
    const { state, error, reload, resolveConflict } = useReviewActions()
    const draft = state?.draft ?? null
    const blocked = state?.blockedReason ?? null

    // The form's own values. A draft already in context seeds them, so a
    // render before the adoption effect (a server render, a test) still shows
    // the stored summary and decision.
    const [summary, setSummary] = useState(() => draft?.summary ?? "")
    const [decision, setDecision] = useState<ReviewDecision>(() => draft?.decision ?? "comment")
    const [notice, setNotice] = useState<Notice | null>(null)
    const [saving, setSaving] = useState(false)
    const [submitting, setSubmitting] = useState(false)

    // The latest state and form values, for the asynchronous saves and the
    // event handlers, which outlive the render that started them.
    const stateRef = useRef(state)
    stateRef.current = state
    const summaryRef = useRef(summary)
    const decisionRef = useRef(decision)

    // Whether the form holds an unsaved edit, and the draft revision that edit
    // is based on: null means the form agrees with the stored draft.
    const dirtyRef = useRef(false)
    const baseRevisionRef = useRef<number | null>(null)
    // Counts local edits, so a save that lands after a newer edit does not
    // adopt the older draft over it.
    const editRef = useRef(0)
    // One save at a time; an edit made during a save asks for one more round,
    // and a submit that lands meanwhile waits for the save it caused.
    const savingRef = useRef(false)
    const savingPromiseRef = useRef<Promise<ReviewDraft | null> | null>(null)
    const pendingRef = useRef(false)
    // The draft revision the form last took up, so the adoption effect ignores
    // the render its own save causes, and the revision a new edit is based on
    // even while a save's re-read has not reached this render yet.
    const adoptedRef = useRef<string | null>(null)
    const latestRevisionRef = useRef<number | null>(draft?.revision ?? null)

    const adopt = (next: ReviewDraft) => {
        adoptedRef.current = `${next.id}:${next.revision}`
        latestRevisionRef.current = next.revision
        summaryRef.current = next.summary
        setSummary(next.summary)
        decisionRef.current = next.decision
        setDecision(next.decision)
    }

    // Take up a draft that changed elsewhere — another window's edit, or this
    // composer's own save — unless the form holds an edit of its own. A
    // revision conflict is then decided when that edit is attempted, from the
    // authoritative draft the refusal carries.
    useEffect(() => {
        if (!draft) return
        const key = `${draft.id}:${draft.revision}`
        if (adoptedRef.current === key) return
        adoptedRef.current = key
        if (dirtyRef.current) return
        latestRevisionRef.current = draft.revision
        summaryRef.current = draft.summary
        setSummary(draft.summary)
        decisionRef.current = draft.decision
        setDecision(draft.decision)
    }, [draft])

    /// Marks a local edit, and the revision it is based on when the form was
    /// clean. An edit during a save asks the save loop for another round.
    const markEdited = () => {
        editRef.current += 1
        if (!dirtyRef.current) {
            dirtyRef.current = true
            baseRevisionRef.current =
                latestRevisionRef.current ?? stateRef.current?.draft?.revision ?? null
        }
        if (savingRef.current) pendingRef.current = true
    }

    /// The form now agrees with `updated` (no newer edit landed meanwhile), or
    /// a newer edit is rebased on the revision just stored.
    const finish = (updated: ReviewDraft, generation: number) => {
        if (editRef.current === generation) {
            dirtyRef.current = false
            baseRevisionRef.current = null
            adopt(updated)
            setNotice(null)
        } else {
            dirtyRef.current = true
            baseRevisionRef.current = updated.revision
            latestRevisionRef.current = updated.revision
        }
    }

    /// A failed mutation: a revision conflict adopts the draft the service
    /// returned and re-reads the rest of the state; every other refusal keeps
    /// the form and words the reason.
    const fail = (err: unknown) => {
        const refusal = draftRefusal(err)
        if (!refusal) {
            setNotice({ text: `Couldn't save the draft — ${prettifyError(err)}`, failed: true })
            return
        }
        const current = conflictingDraft(refusal)
        if (current) {
            dirtyRef.current = false
            baseRevisionRef.current = null
            adopt(current)
            setNotice({
                text: "Another window changed this review. Its newer draft is shown; check it before submitting.",
                failed: true,
            })
            void resolveConflict(current)
            return
        }
        setNotice({ text: refusalText(refusal), failed: true })
        void reload()
    }

    /// Stores the form's current values. Creates the draft when there is none,
    /// so the first edit or an empty approval needs no separate "start" step.
    /// A conflict or failure answers null and leaves the form as it was.
    const persist = async (): Promise<ReviewDraft | null> => {
        const current = stateRef.current
        if (!current) return null
        const generation = editRef.current
        let stored = current.draft
        try {
            if (!stored) stored = await createReviewDraft(reference)
            const summaryValue = summaryRef.current.trim() === "" ? "" : summaryRef.current
            const nextDecision = decisionRef.current
            if (stored.summary === summaryValue && stored.decision === nextDecision) {
                finish(stored, generation)
                await reload()
                return stored
            }
            const items: DraftItemInput[] = stored.items.map((item) => ({ kind: "carried", item }))
            const updated = await editReviewDraft(
                reference,
                baseRevisionRef.current ?? stored.revision,
                nextDecision,
                summaryValue === "" ? null : summaryValue,
                items,
            )
            finish(updated, generation)
            await reload()
            return updated
        } catch (err) {
            fail(err)
            return null
        }
    }

    /// Saves until no edit arrived while a save was in flight. A clean form
    /// with a draft is already saved. A caller that arrives during a save
    /// waits for that save, asking it for one more round when it holds a newer
    /// edit — the case where clicking Submit blurs the summary field first.
    const save = async (): Promise<ReviewDraft | null> => {
        if (savingPromiseRef.current) return savingPromiseRef.current
        if (!dirtyRef.current && stateRef.current?.draft) return stateRef.current.draft
        savingRef.current = true
        setSaving(true)
        const work = (async () => {
            let saved: ReviewDraft | null = null
            try {
                do {
                    pendingRef.current = false
                    saved = await persist()
                    if (saved === null) break
                } while (pendingRef.current)
            } finally {
                savingRef.current = false
                setSaving(false)
            }
            return saved
        })()
        savingPromiseRef.current = work
        try {
            return await work
        } finally {
            savingPromiseRef.current = null
        }
    }

    /// Saves any pending form edit, then asks the native confirmation to
    /// freeze exactly the stored revision. Only an accepted confirmation
    /// starts an operation; a cancel writes nothing.
    const submit = async () => {
        if (submitting) return
        const current = stateRef.current
        const eligibility = submitEligibility(
            current,
            decisionRef.current,
            summaryRef.current,
            current?.draft?.items.length ?? 0,
        )
        if (!eligibility.eligible) return
        setSubmitting(true)
        try {
            let saved = await save()
            if (!saved) return
            // An edit that landed after the save loop closed is saved too,
            // so the frozen revision is the form the reviewer saw.
            while (dirtyRef.current) {
                const next = await save()
                if (!next) return
                saved = next
            }
            const outcome = await submitReview(reference, saved.revision)
            if (outcome === "accepted") {
                setNotice(null)
                await reload()
            }
        } catch (err) {
            fail(err)
        } finally {
            setSubmitting(false)
        }
    }

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
    const editable = state !== null && blocked === null
    const loading = state === null && error === null
    const reasonId = "pull-request-composer-submit-reason"

    return (
        <section className="pull-request-view-section pull-request-composer" aria-label="Review">
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
                            onChange={(event) => {
                                const value = event.currentTarget.value
                                setSummary(value)
                                summaryRef.current = value
                                markEdited()
                            }}
                            onBlur={() => {
                                if (dirtyRef.current) void save()
                            }}
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
                                    decisionRef.current = next
                                    markEdited()
                                    void save()
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
                    <div className="pull-request-composer-actions">
                        <button
                            type="button"
                            className="btn-primary pull-request-composer-submit"
                            disabled={!eligibility.eligible || saving || submitting}
                            aria-describedby={!eligibility.eligible ? reasonId : undefined}
                            onClick={() => void submit()}
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
