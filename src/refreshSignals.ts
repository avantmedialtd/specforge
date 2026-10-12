// The small pure pieces the refresh hooks share (`pull-request-worktree-links`:
// *An Unchanged Links Snapshot Is Not Re-applied*; `commit-graph`:
// *Overlapping Graph Refreshes Settle on the Newest*; design D4 and D6).
//
// - `createSignal`: a payload-less emitter. `useWorkspaces` fires one after
//   every successful views read, so the links are re-read on each views read
//   even when structural sharing kept the views' identity.
// - `createRequestSequence`: last request wins. A read takes a number; its
//   reply is applied only while that number is the newest, so an older reply
//   that arrives late never overwrites a newer one, and a cancelled sequence
//   (an unmount, another repository) applies nothing still in flight.
// - `readLatest`: the two read paths in one, as the hooks use it.
//
// No React here, so `bun test` covers them without a DOM.

/// A signal's subscription: subscribe a listener, and get back its
/// unsubscribe. Stable for the signal's life.
export type Subscribe = (listener: () => void) => () => void

export interface Signal {
    subscribe: Subscribe
    /// Calls every listener subscribed now. A listener that unsubscribes
    /// during the dispatch does not shift it.
    fire: () => void
}

export function createSignal(): Signal {
    const listeners = new Set<() => void>()
    return {
        subscribe(listener) {
            listeners.add(listener)
            return () => {
                listeners.delete(listener)
            }
        },
        fire() {
            for (const listener of [...listeners]) listener()
        },
    }
}

export interface RequestSequence {
    /// Starts a request: its number, which is now the newest.
    next: () => number
    /// Whether `request` is still the newest one started.
    isLatest: (request: number) => boolean
    /// Makes every request started so far stale.
    cancel: () => void
}

export function createRequestSequence(): RequestSequence {
    let latest = 0
    return {
        next: () => ++latest,
        isLatest: (request) => request === latest,
        cancel: () => {
            latest += 1
        },
    }
}

/// Runs `read` as the newest request of `sequence`, and hands its reply to
/// `apply` only if no request started after it, and the sequence was not
/// cancelled, by the time it answers. A rejection goes to `fail` under the
/// same rule; without one, a failed read changes nothing.
export async function readLatest<T>(
    sequence: RequestSequence,
    read: () => Promise<T>,
    apply: (reply: T) => void,
    fail?: (err: unknown) => void,
): Promise<void> {
    const request = sequence.next()
    let reply: T
    try {
        reply = await read()
    } catch (err) {
        if (sequence.isLatest(request)) fail?.(err)
        return
    }
    if (sequence.isLatest(request)) apply(reply)
}
