import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react"
import {
    abandonReviewOperation,
    createReviewDraft,
    editReviewDraft,
    recoverReviewDraft,
    resumeReviewOperation,
    startReviewDraftOver,
    submitReview,
} from "../api"
import {
    bodyBytes,
    conflictingDraft,
    draftRefusal,
    isTerminalPhase,
    MAX_BODY_BYTES,
    MAX_DRAFT_ITEMS,
    refusalText,
    resumeEligible,
    reviewCapability,
    submitEligibility,
} from "../reviewActions"
import { prettifyError } from "../components/errors"
import type { PullRequestActions } from "./usePullRequestActions"
import type {
    DiffSide,
    DraftItem,
    DraftItemInput,
    PlacedInlineItem,
    PlacedReplyItem,
    PullRequestReference,
    ReviewDecision,
    ReviewDraft,
    ReviewThread,
    UnplacedInlineItem,
    UnplacedReplyItem,
} from "../types"

// `pull-request-actions`: *Draft Items and Decisions Are Explicit and Valid*,
// *Stale Commit Recovery Never Remaps an Inline Comment*, *Desktop-Only
// Pull-Request Reviews*. One draft's form and item edits, shared by the
// composer and the diff's line and thread affordances so every control writes
// through one serialised whole-list mutation. Mounted only under Tauri; the
// browser skin has no provider and so renders no draft or draft control.

/// One line a reviewer may place an inline comment on.
export interface DraftAnchor {
    path: string
    side: DiffSide
    line: number
}

/// The unplaced item a reviewer is choosing a current line or thread for.
export type DraftPlacing =
    | { kind: "inline"; itemId: string }
    | { kind: "reply"; itemId: string }

export interface DraftItemError {
    itemId: string
    text: string
}

/// One explicit operation action the status block can be running.
export type ReviewAction = "recover" | "startOver" | "resume" | "abandon"

export interface Notice {
    text: string
    failed: boolean
}

/// What the composer and the diff affordances hold of one draft.
export interface ReviewDraftEditor {
    state: PullRequestActions["state"]
    error: string | null
    reload: PullRequestActions["reload"]
    draft: ReviewDraft | null
    summary: string
    decision: ReviewDecision
    setSummary: (value: string) => void
    setDecision: (value: ReviewDecision) => void
    saveForm: () => void
    notice: Notice | null
    saving: boolean
    submitting: boolean
    submit: () => void
    editable: boolean
    canInline: boolean
    canReply: boolean
    inlineUnavailable: string | null
    replyUnavailable: string | null
    atItemLimit: boolean
    placing: DraftPlacing | null
    beginPlacing: (placing: DraftPlacing) => void
    cancelPlacing: () => void
    itemBusy: string | null
    itemError: DraftItemError | null
    addInline: (anchor: DraftAnchor, body: string) => Promise<boolean>
    addReply: (thread: ReviewThread, body: string) => Promise<boolean>
    editItem: (item: DraftItem, body: string) => Promise<boolean>
    removeItem: (item: DraftItem) => Promise<boolean>
    moveInline: (
        item: PlacedInlineItem | UnplacedInlineItem,
        anchor: DraftAnchor,
    ) => Promise<boolean>
    moveReply: (item: PlacedReplyItem | UnplacedReplyItem, thread: ReviewThread) => Promise<boolean>
    actionBusy: ReviewAction | null
    recover: () => Promise<boolean>
    startOver: () => Promise<boolean>
    resume: () => Promise<boolean>
    abandon: () => Promise<boolean>
}

export const ReviewDraftEditorContext = createContext<ReviewDraftEditor | null>(null)

/// The enclosing pull-request view's draft editor, or null in a browser tab
/// where no draft exists. Controls that can render on either surface read
/// this and render nothing without it.
export function useReviewDraftEditorContext(): ReviewDraftEditor | null {
    return useContext(ReviewDraftEditorContext)
}

/// Provides one draft editor to a native pull-request view's composer, files
/// and threads.
export function ReviewDraftEditorProvider({
    reference,
    actions,
    children,
}: {
    reference: PullRequestReference
    actions: PullRequestActions
    children: ReactNode
}) {
    const editor = useReviewDraftEditor(reference, actions)
    return (
        <ReviewDraftEditorContext.Provider value={editor}>
            {children}
        </ReviewDraftEditorContext.Provider>
    )
}

/// The body stored for a field: empty when it is empty after whitespace is
/// ignored, exactly as typed otherwise.
function storedBody(value: string): string {
    return value.trim() === "" ? "" : value
}

/// One item as the whole-list edit carries it back unchanged.
function carried(items: readonly DraftItem[]): DraftItemInput[] {
    return items.map((item) => ({ kind: "carried", item }))
}

/// One pull request's draft form and item edits, serialised so no two
/// whole-list writes are in flight from this window at once.
export function useReviewDraftEditor(
    reference: PullRequestReference,
    actions: PullRequestActions,
): ReviewDraftEditor {
    const { state, error, reload, resolveConflict } = actions
    const draft = state?.draft ?? null
    const blocked = state?.blockedReason ?? null

    const [summary, setSummary] = useState(() => draft?.summary ?? "")
    const [decision, setDecision] = useState<ReviewDecision>(() => draft?.decision ?? "comment")
    const [notice, setNotice] = useState<Notice | null>(null)
    const [saving, setSaving] = useState(false)
    const [submitting, setSubmitting] = useState(false)
    const [placing, setPlacing] = useState<DraftPlacing | null>(null)
    const [itemBusy, setItemBusy] = useState<string | null>(null)
    const [itemError, setItemError] = useState<DraftItemError | null>(null)
    const [actionBusy, setActionBusy] = useState<ReviewAction | null>(null)
    const actionBusyRef = useRef<ReviewAction | null>(null)

    const stateRef = useRef(state)
    stateRef.current = state
    const summaryRef = useRef(summary)
    const decisionRef = useRef(decision)
    // The latest draft the service is known to hold, which is what an item
    // mutation builds its whole list from even when a re-read has not reached
    // this render yet.
    const latestDraftRef = useRef<ReviewDraft | null>(draft)

    const dirtyRef = useRef(false)
    const baseRevisionRef = useRef<number | null>(null)
    const editRef = useRef(0)
    const savingRef = useRef(false)
    const pendingRef = useRef(false)
    const adoptedRef = useRef<string | null>(null)
    const latestRevisionRef = useRef<number | null>(draft?.revision ?? null)
    // One whole-list write at a time, in the order the reviewer asked for
    // them: a save, an item edit and a submission never interleave revisions.
    const queueRef = useRef<Promise<unknown>>(Promise.resolve())

    const runExclusive = <T,>(work: () => Promise<T>): Promise<T> => {
        const next = queueRef.current.then(work, work)
        queueRef.current = next.then(
            () => undefined,
            () => undefined,
        )
        return next
    }

    const adopt = (next: ReviewDraft) => {
        adoptedRef.current = `${next.id}:${next.revision}`
        latestRevisionRef.current = next.revision
        latestDraftRef.current = next
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
        latestDraftRef.current = draft
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

    // A placing choice names an item that another window may remove.
    useEffect(() => {
        if (!placing) return
        if (!draft?.items.some((item) => item.id === placing.itemId)) setPlacing(null)
    }, [draft, placing])

    const markEdited = () => {
        editRef.current += 1
        if (!dirtyRef.current) {
            dirtyRef.current = true
            baseRevisionRef.current =
                latestRevisionRef.current ?? stateRef.current?.draft?.revision ?? null
        }
        if (savingRef.current) pendingRef.current = true
    }

    const setSummaryValue = (value: string) => {
        setSummary(value)
        summaryRef.current = value
        markEdited()
    }

    const setDecisionValue = (next: ReviewDecision) => {
        setDecision(next)
        decisionRef.current = next
        markEdited()
    }

    /// The form now agrees with `updated` (no newer edit landed meanwhile), or
    /// a newer edit is rebased on the revision just stored.
    const finish = (updated: ReviewDraft, generation: number) => {
        latestDraftRef.current = updated
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
        let stored = latestDraftRef.current ?? current.draft
        try {
            if (!stored) stored = await createReviewDraft(reference)
            latestDraftRef.current = stored
            const summaryValue = storedBody(summaryRef.current)
            const nextDecision = decisionRef.current
            if (stored.summary === summaryValue && stored.decision === nextDecision) {
                finish(stored, generation)
                await reload()
                return stored
            }
            const updated = await editReviewDraft(
                reference,
                baseRevisionRef.current ?? stored.revision,
                nextDecision,
                summaryValue === "" ? null : summaryValue,
                carried(stored.items),
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
    /// with a draft is already saved.
    const saveNow = async (): Promise<ReviewDraft | null> => {
        if (!dirtyRef.current && latestDraftRef.current) return latestDraftRef.current
        savingRef.current = true
        setSaving(true)
        try {
            let saved: ReviewDraft | null = null
            do {
                pendingRef.current = false
                saved = await persist()
                if (saved === null) break
            } while (pendingRef.current)
            return saved
        } finally {
            savingRef.current = false
            setSaving(false)
        }
    }

    const save = () => runExclusive(saveNow)

    /// Saves any pending form edit, then asks the native confirmation to
    /// freeze exactly the stored revision. Only an accepted confirmation
    /// starts an operation; a cancel writes nothing.
    const submit = () => {
        if (submitting || actionBusyRef.current !== null) return
        const current = stateRef.current
        const eligibility = submitEligibility(
            current,
            decisionRef.current,
            summaryRef.current,
            current?.draft?.items.length ?? 0,
        )
        if (!eligibility.eligible) return
        setSubmitting(true)
        void runExclusive(async () => {
            try {
                let saved = await saveNow()
                if (!saved) return
                // An edit that landed after the save loop closed is saved too,
                // so the frozen revision is the form the reviewer saw.
                while (dirtyRef.current) {
                    const next = await saveNow()
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
        })
    }

    /// Writes one whole-list item mutation, creating the draft first when the
    /// reviewer has not started one. `build` reads the draft the service
    /// holds now and answers the complete list to store, or null to write
    /// nothing. A conflict adopts the authoritative draft; every other
    /// refusal is worded beside the item and in the composer.
    const mutateItems = (
        itemId: string | null,
        build: (stored: ReviewDraft) => DraftItemInput[] | null,
    ): Promise<boolean> =>
        runExclusive(async () => {
            setItemBusy(itemId ?? "new")
            setItemError(null)
            try {
                let stored = latestDraftRef.current ?? stateRef.current?.draft ?? null
                if (!stored) {
                    try {
                        stored = await createReviewDraft(reference)
                    } catch (err) {
                        fail(err)
                        return false
                    }
                    latestDraftRef.current = stored
                }
                const items = build(stored)
                if (items === null) return false
                const generation = editRef.current
                const summaryValue = storedBody(summaryRef.current)
                const updated = await editReviewDraft(
                    reference,
                    baseRevisionRef.current ?? stored.revision,
                    decisionRef.current,
                    summaryValue === "" ? null : summaryValue,
                    items,
                )
                finish(updated, generation)
                const next = await reload()
                if (
                    next?.draft &&
                    next.draft.id === latestDraftRef.current?.id &&
                    next.draft.revision >= (latestDraftRef.current?.revision ?? 0)
                ) {
                    latestDraftRef.current = next.draft
                }
                if (itemId !== null) {
                    setPlacing((current) => (current?.itemId === itemId ? null : current))
                }
                return true
            } catch (err) {
                const refusal = draftRefusal(err)
                setItemError({
                    itemId: itemId ?? "new",
                    text: refusal
                        ? refusalText(refusal)
                        : `Couldn't save the draft — ${prettifyError(err)}`,
                })
                fail(err)
                return false
            } finally {
                setItemBusy(null)
            }
        })

    const itemCount = draft?.items.length ?? 0
    const inlineCapability = state ? reviewCapability(state, "inlineComment") : null
    const replyCapability = state ? reviewCapability(state, "reply") : null
    const editable = state !== null && blocked === null
    const canInline = editable && inlineCapability?.available === true
    const canReply = editable && replyCapability?.available === true
    const atItemLimit = itemCount >= MAX_DRAFT_ITEMS

    // A blocked or closed draft offers no placing target; drop one a state
    // change made stale rather than leaving inert buttons in the diff.
    useEffect(() => {
        if (!editable) setPlacing(null)
    }, [editable])

    const addInline = (anchor: DraftAnchor, body: string): Promise<boolean> => {
        if (!canInline || atItemLimit || body.trim() === "" || bodyBytes(body) > MAX_BODY_BYTES) {
            return Promise.resolve(false)
        }
        return mutateItems("new", (stored) => [
            ...carried(stored.items),
            {
                kind: "newInline",
                body,
                path: anchor.path,
                side: anchor.side,
                line: anchor.line,
            },
        ])
    }

    const addReply = (thread: ReviewThread, body: string): Promise<boolean> => {
        if (
            !canReply ||
            atItemLimit ||
            !thread.replyable ||
            body.trim() === "" ||
            bodyBytes(body) > MAX_BODY_BYTES
        ) {
            return Promise.resolve(false)
        }
        return mutateItems("new", (stored) => [
            ...carried(stored.items),
            { kind: "newReply", body, target: thread.id },
        ])
    }

    const editItem = (item: DraftItem, body: string): Promise<boolean> => {
        if (!editable || body === item.body) return Promise.resolve(true)
        if (body.trim() === "" || bodyBytes(body) > MAX_BODY_BYTES) return Promise.resolve(false)
        return mutateItems(item.id, (stored) => {
            if (!stored.items.some((current) => current.id === item.id)) return null
            return carried(
                stored.items.map((current) =>
                    current.id === item.id ? { ...current, body } : current,
                ),
            )
        })
    }

    const removeItem = (item: DraftItem): Promise<boolean> => {
        if (!editable) return Promise.resolve(false)
        return mutateItems(item.id, (stored) => {
            if (!stored.items.some((current) => current.id === item.id)) return null
            return carried(stored.items.filter((current) => current.id !== item.id))
        })
    }

    const moveInline = (
        item: PlacedInlineItem | UnplacedInlineItem,
        anchor: DraftAnchor,
    ): Promise<boolean> => {
        if (!editable) return Promise.resolve(false)
        return mutateItems(item.id, (stored) => {
            if (!stored.items.some((current) => current.id === item.id)) return null
            const placed: PlacedInlineItem = {
                kind: "placedInline",
                id: item.id,
                body: item.body,
                path: anchor.path,
                side: anchor.side,
                line: anchor.line,
            }
            return carried(
                stored.items.map((current) => (current.id === item.id ? placed : current)),
            )
        })
    }

    const moveReply = (
        item: PlacedReplyItem | UnplacedReplyItem,
        thread: ReviewThread,
    ): Promise<boolean> => {
        if (!editable || !thread.replyable) return Promise.resolve(false)
        return mutateItems(item.id, (stored) => {
            if (!stored.items.some((current) => current.id === item.id)) return null
            const placed: PlacedReplyItem = {
                kind: "placedReply",
                id: item.id,
                body: item.body,
                target: thread.id,
            }
            return carried(
                stored.items.map((current) => (current.id === item.id ? placed : current)),
            )
        })
    }

    /// Runs one explicit status action through the same queue as every draft
    /// write, so a recovery, start-over, resume or abandonment never interleaves
    /// with a save or item mutation.
    const perform = (
        action: ReviewAction,
        work: () => Promise<boolean>,
    ): Promise<boolean> => {
        if (actionBusyRef.current !== null) return Promise.resolve(false)
        actionBusyRef.current = action
        setActionBusy(action)
        return runExclusive(work).finally(() => {
            actionBusyRef.current = null
            setActionBusy(null)
        })
    }

    /// Explicit stale recovery: save the form's exact prose first, then move the
    /// draft onto the cached detail's current commits. The service preserves the
    /// summary and every still-valid reply, and unplaces each inline comment;
    /// the adopted draft is what the composer and diff then show.
    const recover = (): Promise<boolean> =>
        perform("recover", async () => {
            const current = stateRef.current
            if (!current || !current.staleBasis) return false
            try {
                const saved = await saveNow()
                if (!saved) return false
                const recovered = await recoverReviewDraft(reference, saved.revision)
                dirtyRef.current = false
                baseRevisionRef.current = null
                pendingRef.current = false
                editRef.current += 1
                adopt(recovered)
                setPlacing(null)
                setItemError(null)
                setNotice(null)
                await reload()
                return true
            } catch (err) {
                fail(err)
                return false
            }
        })

    /// Explicit local Start over: a fresh empty draft under current state, with
    /// no old summary, item, decision or provider-effect state. Offered only
    /// while no operation is unfinished; nothing here sends a provider write.
    const startOver = (): Promise<boolean> =>
        perform("startOver", async () => {
            const current = stateRef.current
            if (!current) return false
            if (current.operation !== null && !isTerminalPhase(current.operation.phase)) {
                return false
            }
            try {
                const fresh = await startReviewDraftOver(reference)
                dirtyRef.current = false
                baseRevisionRef.current = null
                pendingRef.current = false
                editRef.current += 1
                adopt(fresh)
                setPlacing(null)
                setItemError(null)
                setNotice(null)
                await reload()
                return true
            } catch (err) {
                fail(err)
                return false
            }
        })

    /// Explicit Resume of the paused operation, skipping every confirmed step.
    /// An uncertain or failed first unresolved step, or a blocked state, keeps
    /// it unavailable here as it is at the service.
    const resume = (): Promise<boolean> =>
        perform("resume", async () => {
            const current = stateRef.current
            if (!current?.operation || current.blockedReason !== null) return false
            if (!resumeEligible(current.operation)) return false
            try {
                await resumeReviewOperation(reference)
                await reload()
                return true
            } catch (err) {
                fail(err)
                return false
            }
        })

    /// Explicit Abandon: always a local terminal transition, permitted even
    /// when the frozen operation's credential or actor no longer authorises a
    /// provider cleanup. The receipt keeps every confirmed or uncertain effect.
    const abandon = (): Promise<boolean> =>
        perform("abandon", async () => {
            const current = stateRef.current
            if (!current?.operation || current.operation.phase !== "paused") return false
            try {
                await abandonReviewOperation(reference)
                await reload()
                return true
            } catch (err) {
                fail(err)
                return false
            }
        })

    const beginPlacing = (next: DraftPlacing) => {
        if (!editable) return
        if (!draft?.items.some((item) => item.id === next.itemId)) return
        setItemError(null)
        setPlacing(next)
    }

    const inlineReason =
        inlineCapability && !inlineCapability.available
            ? inlineCapability.reason || "not offered for this pull request"
            : null
    const replyReason =
        replyCapability && !replyCapability.available
            ? replyCapability.reason || "not offered for this pull request"
            : null

    return {
        state,
        error,
        reload,
        draft,
        summary,
        decision,
        setSummary: setSummaryValue,
        setDecision: setDecisionValue,
        saveForm: () => {
            if (dirtyRef.current) void save()
        },
        notice,
        saving,
        submitting,
        submit,
        editable,
        canInline,
        canReply,
        inlineUnavailable: inlineReason,
        replyUnavailable: replyReason,
        atItemLimit,
        placing,
        beginPlacing,
        cancelPlacing: () => setPlacing(null),
        itemBusy,
        itemError,
        addInline,
        addReply,
        editItem,
        removeItem,
        moveInline,
        moveReply,
        actionBusy,
        recover,
        startOver,
        resume,
        abandon,
    }
}
