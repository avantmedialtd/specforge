import { beforeEach, describe, expect, test } from "bun:test"
import { EVENT_CACHE_UPDATED, EVENT_CHANGE_ADDED } from "./types"

// A stand-in for the browser's EventSource that records what was attached and
// lets a test kill the connection the way an OS suspension does.
class FakeEventSource {
    static readonly CLOSED = 2
    static instances: FakeEventSource[] = []
    readyState = 1 // OPEN
    readonly url: string
    readonly listeners = new Map<string, Set<EventListener>>()

    constructor(url: string) {
        this.url = url
        FakeEventSource.instances.push(this)
    }
    addEventListener(type: string, listener: EventListener): void {
        let set = this.listeners.get(type)
        if (!set) {
            set = new Set()
            this.listeners.set(type, set)
        }
        set.add(listener)
    }
    removeEventListener(type: string, listener: EventListener): void {
        this.listeners.get(type)?.delete(listener)
    }
    close(): void {
        this.readyState = FakeEventSource.CLOSED
    }
    countFor(type: string): number {
        return this.listeners.get(type)?.size ?? 0
    }
}

class FakeTarget {
    handlers = new Map<string, ((e: unknown) => void)[]>()
    addEventListener(type: string, handler: (e: unknown) => void): void {
        const list = this.handlers.get(type) ?? []
        list.push(handler)
        this.handlers.set(type, list)
    }
    dispatch(type: string, event: unknown = {}): void {
        for (const h of [...(this.handlers.get(type) ?? [])]) h(event)
    }
}

const fakeDocument = new FakeTarget() as FakeTarget & { visibilityState: string }
fakeDocument.visibilityState = "visible"
const fakeWindow = new FakeTarget()

const g = globalThis as unknown as Record<string, unknown>
g.EventSource = FakeEventSource
g.document = fakeDocument
g.window = fakeWindow
if (typeof g.MessageEvent === "undefined") {
    g.MessageEvent = class {
        type: string
        data: unknown
        constructor(type: string, init: { data?: unknown } = {}) {
            this.type = type
            this.data = init.data
        }
    }
}

const { subscribeToEventStream, __resetEventStreamForTests } = await import("./eventStream")

/** Let queued microtasks (the single-flight re-read) run. */
const flush = () => new Promise((r) => setTimeout(r, 0))

const latest = () => FakeEventSource.instances[FakeEventSource.instances.length - 1]!

beforeEach(() => {
    FakeEventSource.instances = []
    fakeDocument.handlers.clear()
    fakeWindow.handlers.clear()
    fakeDocument.visibilityState = "visible"
    __resetEventStreamForTests()
})

describe("event stream recovery after document suspension", () => {
    test("a healthy stream is not replaced", async () => {
        let refreshes = 0
        subscribeToEventStream(EVENT_CACHE_UPDATED, () => refreshes++)
        expect(FakeEventSource.instances).toHaveLength(1)

        // Ordinary tab switching, stream still OPEN.
        fakeDocument.dispatch("visibilitychange")
        await flush()

        expect(FakeEventSource.instances).toHaveLength(1)
        expect(refreshes).toBe(0)
    })

    test("a stream still retrying is left to its own reconnect", async () => {
        subscribeToEventStream(EVENT_CACHE_UPDATED, () => {})
        latest().readyState = 0 // CONNECTING — EventSource is already retrying
        fakeDocument.dispatch("visibilitychange")
        await flush()
        expect(FakeEventSource.instances).toHaveLength(1)
    })

    test("a closed stream is replaced and every listener re-attached", async () => {
        subscribeToEventStream(EVENT_CACHE_UPDATED, () => {})
        subscribeToEventStream(EVENT_CHANGE_ADDED, () => {})
        const original = latest()
        expect(original.countFor(EVENT_CHANGE_ADDED)).toBe(1)

        original.close() // the OS suspended the app; nothing will retry this
        fakeDocument.dispatch("visibilitychange")
        await flush()

        expect(FakeEventSource.instances).toHaveLength(2)
        const replacement = latest()
        expect(replacement).not.toBe(original)
        expect(replacement.url).toBe("/api/events")
        expect(replacement.countFor(EVENT_CACHE_UPDATED)).toBe(1)
        expect(replacement.countFor(EVENT_CHANGE_ADDED)).toBe(1)
    })

    test("resuming re-reads state, because the stream cannot replay", async () => {
        let refreshes = 0
        subscribeToEventStream(EVENT_CACHE_UPDATED, () => refreshes++)
        latest().close()

        fakeDocument.dispatch("visibilitychange")
        await flush()

        expect(refreshes).toBe(1)
    })

    test("overlapping restorations collapse into one re-read", async () => {
        let refreshes = 0
        subscribeToEventStream(EVENT_CACHE_UPDATED, () => refreshes++)
        latest().close()

        // Two restorations before the queued re-read has run.
        fakeDocument.dispatch("visibilitychange")
        fakeDocument.dispatch("visibilitychange")
        await flush()

        expect(refreshes).toBe(1)
    })

    test("a hidden document does not restore", async () => {
        subscribeToEventStream(EVENT_CACHE_UPDATED, () => {})
        latest().close()
        fakeDocument.visibilityState = "hidden"

        fakeDocument.dispatch("visibilitychange")
        await flush()

        expect(FakeEventSource.instances).toHaveLength(1)
    })

    test("returning from the back/forward cache restores", async () => {
        subscribeToEventStream(EVENT_CACHE_UPDATED, () => {})
        latest().close()

        fakeWindow.dispatch("pageshow", { persisted: true })
        await flush()

        expect(FakeEventSource.instances).toHaveLength(2)
    })

    test("a non-persisted pageshow does not restore", async () => {
        subscribeToEventStream(EVENT_CACHE_UPDATED, () => {})
        latest().close()

        fakeWindow.dispatch("pageshow", { persisted: false })
        await flush()

        expect(FakeEventSource.instances).toHaveLength(1)
    })

    test("unsubscribing detaches from the replacement, not the dead stream", async () => {
        const off = subscribeToEventStream(EVENT_CACHE_UPDATED, () => {})
        latest().close()
        fakeDocument.dispatch("visibilitychange")
        await flush()

        const replacement = latest()
        expect(replacement.countFor(EVENT_CACHE_UPDATED)).toBe(1)

        off()
        expect(replacement.countFor(EVENT_CACHE_UPDATED)).toBe(0)
    })

    test("a listener removed before suspension is not resurrected", async () => {
        const off = subscribeToEventStream(EVENT_CACHE_UPDATED, () => {})
        subscribeToEventStream(EVENT_CHANGE_ADDED, () => {})
        off()

        latest().close()
        fakeDocument.dispatch("visibilitychange")
        await flush()

        const replacement = latest()
        expect(replacement.countFor(EVENT_CACHE_UPDATED)).toBe(0)
        expect(replacement.countFor(EVENT_CHANGE_ADDED)).toBe(1)
    })
})
