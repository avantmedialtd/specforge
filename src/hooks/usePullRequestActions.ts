import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react"
import { getReviewActionState, isTauri, onReviewActionChanged } from "../api"
import { sameReference } from "../routing/address"
import { createRequestSequence } from "../refreshSignals"
import { shareStructure } from "../structuralShare"
import type { PullRequestReference, ReviewActionState, ReviewDraft } from "../types"

// `pull-request-actions`: *Review Action State Synchronises Only Native
// Windows*, *Desktop-Only Pull-Request Reviews*. One pull request's action
// state, read on mount, re-read after an action invalidation for that
// pull request, and never read or subscribed to in a browser tab.

/// What one pull request's presentations hold of its review actions.
export interface PullRequestActions {
    /// The state the service last gave, or null until a read answers. It is
    /// the same in the center pane and in a detached window, both reading
    /// through this hook.
    state: ReviewActionState | null
    /// Why the last read failed, as the transport worded it; the state stays
    /// as it was. Cleared by a read that answers.
    error: string | null
    /// Reads the state again, and resolves with what the service said, or
    /// null when it could not be read or a newer read superseded this one.
    reload: () => Promise<ReviewActionState | null>
    /// A revision conflict's authoritative draft: adopt it now, so the next
    /// edit names the revision the service holds, and re-read the rest of the
    /// state a conflict cannot carry — capabilities, the blocking reason and
    /// any operation.
    resolveConflict: (current: ReviewDraft) => Promise<ReviewActionState | null>
}

/// The action state one pull request's presentations share. `PullRequestView`
/// provides the one hook's value, so its center-pane and detached-window
/// presentations — and the composer inside either — read one state, one
/// revision and one blocking reason.
export const PullRequestActionsContext = createContext<PullRequestActions | null>(null)

/// The enclosing pull request's action state. Throws outside a
/// `PullRequestActionsContext`: every action control renders inside a
/// pull-request view, and no other surface has an action state.
export function useReviewActions(): PullRequestActions {
    const actions = useContext(PullRequestActionsContext)
    if (!actions) throw new Error("review actions are read only inside a pull-request view")
    return actions
}

/// One pull request's current review-action state, kept current by the native
/// action broadcast (`pull-request-actions`: *Review Action State Synchronises
/// Only Native Windows*).
///
/// The first read is the mount read, which is how a window that missed an
/// event obtains current state. After that a read follows an invalidation for
/// this reference alone: another pull request's edit never re-reads this one.
/// Replies are newest-first-wins, and an equal reply keeps the state's
/// identity, so nothing derived from it renders again.
///
/// In a browser tab there is no action state to read and no action event to
/// hear, so the hook neither sends a command nor subscribes: `state` stays
/// null and `reload` answers null without a request. That is the frontend half
/// of the boundary whose other half is the absent web dispatch arm.
export function usePullRequestActions(reference: PullRequestReference): PullRequestActions {
    const [state, setState] = useState<ReviewActionState | null>(null)
    const [error, setError] = useState<string | null>(null)
    const [sequence] = useState(createRequestSequence)
    // Read at the time a read starts or an event arrives, so a caller that
    // rebuilds an equal reference does not re-subscribe and re-read.
    const referenceRef = useRef(reference)
    referenceRef.current = reference

    const reload = useCallback((): Promise<ReviewActionState | null> => {
        if (!isTauri()) return Promise.resolve(null)
        const request = sequence.next()
        return getReviewActionState(referenceRef.current).then(
            (next) => {
                if (!sequence.isLatest(request)) return null
                setError(null)
                setState((previous) => shareStructure(previous, next))
                return next
            },
            (err: unknown) => {
                if (sequence.isLatest(request)) setError(String(err))
                return null
            },
        )
    }, [sequence])

    const resolveConflict = useCallback(
        (current: ReviewDraft): Promise<ReviewActionState | null> => {
            setState((previous) => (previous ? { ...previous, draft: current } : previous))
            return reload()
        },
        [reload],
    )

    useEffect(() => {
        if (!isTauri()) return
        void reload()
        const subscription = onReviewActionChanged((invalidation) => {
            // An unparseable frame arrives as `undefined`.
            if (invalidation && sameReference(invalidation.reference, referenceRef.current)) {
                void reload()
            }
        })
        return () => {
            // Nothing still in flight lands after the view lets go.
            sequence.cancel()
            void subscription.then(
                (unlisten) => unlisten(),
                () => {},
            )
        }
    }, [reload, sequence])

    return useMemo(
        () => ({ state, error, reload, resolveConflict }),
        [state, error, reload, resolveConflict],
    )
}
