/// The detail pane's load lifecycle, as a pure transition over `(state,
/// event)`. Everything that makes a live pane pleasant rather than hostile
/// lives here — the equality guard that makes a redundant refresh
/// unobservable, the trigger-dependent failure policy, and the suppression of
/// the loading flag on refreshes the user did not ask for — so it is testable
/// without a DOM, a Tauri host, or a rendered component.
///
/// See the *Reactive Updates from Filesystem* requirement in the `spec-browser`
/// capability.

/// Why a read was issued. `select` is the user choosing an artifact; `watch` is
/// the filesystem watcher reporting that something changed.
export type LoadTrigger = "select" | "watch"

export interface DetailState {
    /// Markdown to render. Held across a `select` so the previous artifact
    /// stays on screen while the next one loads, and across a failed `watch`
    /// so a reader never loses their page to an event they did not cause.
    content: string | null
    /// Message for a failed user-initiated read. Non-null implies `content` is
    /// null — the two are never displayed together.
    error: string | null
    /// True only while a user-initiated read is outstanding. A watcher-driven
    /// read never raises it, so a live pane cannot flash a spinner.
    loading: boolean
}

export type DetailEvent =
    /// The pane has no target — nothing selected.
    | { kind: "cleared" }
    /// A user-initiated read has started.
    | { kind: "select" }
    /// A watcher-driven read has started.
    | { kind: "watch" }
    | { kind: "resolved"; trigger: LoadTrigger; content: string }
    | { kind: "failed"; trigger: LoadTrigger; error: string }

export const INITIAL: DetailState = {
    content: null,
    error: null,
    loading: false,
}

/// Advance the pane's state. Returns the *same object* whenever nothing
/// observable changed, so React skips the re-render and the reader's scroll
/// position is never disturbed by a refresh that carried no news.
export function reduce(state: DetailState, event: DetailEvent): DetailState {
    switch (event.kind) {
        case "cleared":
            if (
                state.content === null &&
                state.error === null &&
                !state.loading
            ) {
                return state
            }
            return INITIAL

        // Content is deliberately retained: the outgoing artifact stays
        // rendered until the incoming one arrives, which is why the pane's
        // "Loading…" branch is guarded on there being no content at all.
        case "select":
            return { content: state.content, error: null, loading: true }

        // Starting a watcher-driven read is not itself observable.
        case "watch":
            return state

        case "resolved":
            if (event.trigger === "watch") {
                // A read the user asked for is outstanding and will deliver
                // its own result; this one is superseded.
                if (state.loading) return state
                // The whole point of the unfiltered subscription: unchanged
                // bytes cost nothing.
                if (state.content === event.content && state.error === null) {
                    return state
                }
                return { content: event.content, error: null, loading: false }
            }
            return { content: event.content, error: null, loading: false }

        case "failed":
            // A refresh the user did not initiate must not destroy what they
            // are reading. The next batch will correct a transient failure.
            if (event.trigger === "watch") return state
            return { content: null, error: event.error, loading: false }
    }
}
