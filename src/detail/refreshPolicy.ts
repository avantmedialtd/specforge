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
    /// When the file `content` came from was last written, in unix seconds, or
    /// null when the filesystem reported no usable time. Travels with `content`
    /// as one invariant: it describes the *same* read, so it is retained
    /// whenever the content is retained and null whenever the content is null.
    modifiedAt: number | null
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
    | {
          kind: "resolved"
          trigger: LoadTrigger
          content: string
          modifiedAt: number | null
      }
    | { kind: "failed"; trigger: LoadTrigger; error: string }

/// The trigger a newly-issued read should carry.
///
/// Issuing a read invalidates whatever was in flight. When a watcher-driven
/// read invalidates a user-initiated one, it inherits `select`: the user is
/// still waiting for the artifact they chose, so its result must land at the
/// top of the pane and a failure must be reported rather than silently leaving
/// the previous artifact's body on screen. Without this the newer, fresher read
/// would be presented as if nobody were waiting for it.
///
/// Mirrors `pending_trigger` in the terminal frontend, so both surfaces resolve
/// the same race the same way.
export function effectiveTrigger(
    requested: LoadTrigger,
    pending: LoadTrigger | null,
): LoadTrigger {
    return requested === "watch" && pending === "select" ? "select" : requested
}

export const INITIAL: DetailState = {
    content: null,
    modifiedAt: null,
    error: null,
    loading: false,
}

/// Advance the pane's state. Returns the *same object* whenever nothing
/// observable changed, so React skips the re-render and the reader's scroll
/// position is never disturbed by a refresh that carried no news.
///
/// "Nothing observable" means the bytes AND the time they were written. A file
/// rewritten with identical content — a branch switch, an idempotent write by
/// an agent, a formatter — leaves the markdown equal while genuinely changing
/// when it last changed, and the header reports that (`spec-browser`: *Change
/// Identity Header in the Detail Pane*, "Last changed"). Comparing content
/// alone would freeze the header's label on those writes; comparing the time
/// alone would repaint the document for them. Comparing both, and returning a
/// new object whose `content` is *referentially equal*, updates the header
/// while `memo(MarkdownView)` skips the document — which is why that memo is
/// a prerequisite of this guard and not an optimization beside it.
export function reduce(state: DetailState, event: DetailEvent): DetailState {
    switch (event.kind) {
        case "cleared":
            if (
                state.content === null &&
                state.modifiedAt === null &&
                state.error === null &&
                !state.loading
            ) {
                return state
            }
            return INITIAL

        // Content is deliberately retained: the outgoing artifact stays
        // rendered until the incoming one arrives, which is why the pane's
        // "Loading…" branch is guarded on there being no content at all. The
        // time is retained with it — it describes the content still on screen,
        // so dropping it would blank the header under a reader who can still
        // see the document it belongs to.
        case "select":
            return {
                content: state.content,
                modifiedAt: state.modifiedAt,
                error: null,
                loading: true,
            }

        // Starting a watcher-driven read is not itself observable.
        case "watch":
            return state

        case "resolved":
            // Reaching the `watch` branch means no user-initiated read is
            // waiting on us: one that this read superseded was re-labelled
            // `select` before dispatch (see `effectiveTrigger`). So a watcher
            // result is never dropped in favour of an older read — only its
            // *presentation* differs.
            if (
                event.trigger === "watch" &&
                state.content === event.content &&
                state.modifiedAt === event.modifiedAt &&
                state.error === null
            ) {
                // The whole point of the unfiltered subscription: a read that
                // carried no news at all costs nothing.
                return state
            }
            return {
                content: event.content,
                modifiedAt: event.modifiedAt,
                error: null,
                loading: false,
            }

        case "failed":
            // A refresh the user did not initiate must not destroy what they
            // are reading. The next batch will correct a transient failure.
            if (event.trigger === "watch") return state
            // A user-initiated failure clears the time along with the content:
            // nothing is displayed, so there is nothing for a header to date.
            return {
                content: null,
                modifiedAt: null,
                error: event.error,
                loading: false,
            }
    }
}
