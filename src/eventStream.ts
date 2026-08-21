// -------------------------------------------------------------------------
// Web event-stream lifecycle
// -------------------------------------------------------------------------
//
// Owns the single `EventSource` the web transport listens on, and its recovery
// after the document has been suspended. Split out of `api.ts` because it is
// pure transport lifecycle with no command surface and no Tauri imports, which
// keeps it directly testable.
//
// In a long-lived browser tab none of this matters: the tab stays alive and
// EventSource retries transient drops itself. An installed standalone app is
// different — the OS suspends the whole document, and it can come back with the
// stream in CLOSED, where nothing will ever retry it. The UI would then keep
// rendering pre-suspension state while looking live.
//
// Reconnecting alone is not recovery. The server sets no event ids and keeps no
// history (a lagging receiver is deliberately skipped, not replayed), so a
// reconnected client cannot learn what it missed. It has to re-read.

import { EVENT_CACHE_UPDATED } from "./types"

const ENDPOINT = "/api/events"

type StreamListener = { event: string; listener: EventListener }

// Every listener attached to the stream, kept so a replacement can be
// repopulated. An EventSource cannot be reopened and loses its listeners when
// replaced, while the app subscribes once at mount and never again.
const listeners = new Set<StreamListener>()

let source: EventSource | null = null
let recoveryInstalled = false
let resumeRefreshPending = false

function openStream(): EventSource {
    if (!source) {
        source = new EventSource(ENDPOINT)
        installStreamRecovery()
    }
    return source
}

/// Attach a listener to the shared stream, returning its detach function.
export function subscribeToEventStream(
    event: string,
    listener: EventListener,
): () => void {
    const stream = openStream()
    const entry: StreamListener = { event, listener }
    listeners.add(entry)
    stream.addEventListener(event, entry.listener)
    return () => {
        listeners.delete(entry)
        // Detach from whichever stream is current, which may be a replacement.
        source?.removeEventListener(event, entry.listener)
    }
}

function installStreamRecovery(): void {
    if (recoveryInstalled) return
    if (typeof document === "undefined" || typeof window === "undefined") return
    recoveryInstalled = true
    document.addEventListener("visibilitychange", () => {
        if (document.visibilityState === "visible") restoreEventStream()
    })
    // Returning from the back/forward cache does not always raise a visibility
    // change, so cover the restore explicitly.
    window.addEventListener("pageshow", (e) => {
        if ((e as PageTransitionEvent).persisted) restoreEventStream()
    })
}

/// Replace the stream only when it is genuinely dead. A CONNECTING stream is
/// EventSource's own retry already in flight, and an OPEN one needs nothing —
/// tearing either down would churn connections on every tab switch.
export function restoreEventStream(): void {
    const dead = source
    if (!dead || dead.readyState !== 2 /* CLOSED */) return

    dead.close()
    const replacement = new EventSource(ENDPOINT)
    for (const { event, listener } of listeners) {
        replacement.addEventListener(event, listener)
    }
    source = replacement
    scheduleResumeRefresh()
}

// Re-read current state by replaying the event every refreshing surface already
// subscribes to. Consumers ignore the payload (`onCacheUpdated(() =>
// scheduleLoad())`) and debounce their own loads, so this reuses the existing
// refresh path rather than introducing a second one.
function scheduleResumeRefresh(): void {
    // Single-flight: overlapping restorations collapse into one re-read.
    if (resumeRefreshPending) return
    resumeRefreshPending = true
    queueMicrotask(() => {
        resumeRefreshPending = false
        for (const entry of [...listeners]) {
            if (entry.event !== EVENT_CACHE_UPDATED) continue
            entry.listener(new MessageEvent(entry.event, { data: "" }))
        }
    })
}

/// Test seam: drop all module state so each case starts from a cold stream.
export function __resetEventStreamForTests(): void {
    source = null
    listeners.clear()
    recoveryInstalled = false
    resumeRefreshPending = false
}
