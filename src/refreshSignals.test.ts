import { describe, expect, test } from "bun:test"
import { createRequestSequence, createSignal, readLatest } from "./refreshSignals"
import { shareStructure } from "./structuralShare"

// `pull-request-worktree-links`: *An Unchanged Links Snapshot Is Not
// Re-applied*; `commit-graph`: *Overlapping Graph Refreshes Settle on the
// Newest*; design D4 and D6.

/// A promise and the functions that settle it, so a test decides the order
/// replies arrive in.
function deferred<T>() {
    let resolve!: (value: T) => void
    let reject!: (err: unknown) => void
    const promise = new Promise<T>((res, rej) => {
        resolve = res
        reject = rej
    })
    return { promise, resolve, reject }
}

describe("createSignal", () => {
    test("each fire reaches every listener, until it unsubscribes", () => {
        const signal = createSignal()
        const calls: string[] = []
        const offA = signal.subscribe(() => calls.push("a"))
        signal.subscribe(() => calls.push("b"))
        signal.fire()
        offA()
        signal.fire()
        expect(calls).toEqual(["a", "b", "b"])
    })

    test("a listener that unsubscribes during a fire does not shift it", () => {
        const signal = createSignal()
        const calls: string[] = []
        const off = signal.subscribe(() => {
            calls.push("a")
            off()
        })
        signal.subscribe(() => calls.push("b"))
        signal.fire()
        signal.fire()
        expect(calls).toEqual(["a", "b", "b"])
    })
})

describe("the links re-read, as pure pieces", () => {
    /// The pieces the two hooks wire, without React (the hooks themselves are
    /// rendered in `hooks/usePullRequestLinks.test.tsx`): a views read that stores its reply
    /// through `shareStructure` and fires the signal, and a links reader
    /// subscribed to it.
    function harness() {
        const viewsRead = createSignal()
        let views: unknown[] = []
        let links: Record<string, unknown> = {}
        let linkReads = 0
        const replies: Array<ReturnType<typeof deferred<Record<string, unknown>>>> = []
        const sequence = createRequestSequence()
        const readLinks = () =>
            readLatest(
                sequence,
                () => {
                    linkReads += 1
                    const reply = deferred<Record<string, unknown>>()
                    replies.push(reply)
                    return reply.promise
                },
                (next) => {
                    links = shareStructure(links, next)
                },
            )
        const pending: Promise<void>[] = []
        viewsRead.subscribe(() => pending.push(readLinks()))
        return {
            readViews(next: unknown[]) {
                views = shareStructure(views, next)
                viewsRead.fire()
                return views
            },
            readLinks: () => pending.push(readLinks()),
            replies,
            settled: () => Promise.all(pending),
            links: () => links,
            linkReads: () => linkReads,
            sequence,
        }
    }

    test("every views read leads to a links read, even when the views are identical", async () => {
        const h = harness()
        const first = h.readViews([{ repoId: "a" }])
        const second = h.readViews([{ repoId: "a" }])
        expect(second).toBe(first)
        expect(h.linkReads()).toBe(2)
        h.replies[0]!.resolve({})
        h.replies[1]!.resolve({ byPullRequest: { "#1": "/wt" } })
        await h.settled()
        // The read after the equal views brought what changed remotely.
        expect(h.links()).toEqual({ byPullRequest: { "#1": "/wt" } })
    })

    test("an older reply arriving after a newer one is discarded", async () => {
        const h = harness()
        h.readLinks()
        h.readLinks()
        h.replies[1]!.resolve({ value: "newer" })
        await Promise.resolve()
        h.replies[0]!.resolve({ value: "older" })
        await h.settled()
        expect(h.links()).toEqual({ value: "newer" })
    })

    test("an equal reply keeps the held snapshot's identity", async () => {
        const h = harness()
        h.readLinks()
        h.replies[0]!.resolve({ byPullRequest: { "#1": ["/wt"] } })
        await h.settled()
        const held = h.links()
        h.readLinks()
        h.replies[1]!.resolve({ byPullRequest: { "#1": ["/wt"] } })
        await h.settled()
        expect(h.links()).toBe(held)
    })

    test("a failed read keeps the held snapshot", async () => {
        const h = harness()
        h.readLinks()
        h.replies[0]!.resolve({ value: 1 })
        await h.settled()
        const held = h.links()
        h.readLinks()
        h.replies[1]!.reject(new Error("offline"))
        await h.settled()
        expect(h.links()).toBe(held)
    })
})

describe("readLatest", () => {
    test("two overlapping loads resolving in reverse order apply the newer graph", async () => {
        const sequence = createRequestSequence()
        const older = deferred<string>()
        const newer = deferred<string>()
        const applied: string[] = []
        const first = readLatest(sequence, () => older.promise, (graph) => applied.push(graph))
        const second = readLatest(sequence, () => newer.promise, (graph) => applied.push(graph))
        newer.resolve("newer graph")
        await second
        older.resolve("older graph")
        await first
        expect(applied).toEqual(["newer graph"])
    })

    test("a cancelled sequence applies nothing still in flight", async () => {
        const sequence = createRequestSequence()
        const reply = deferred<string>()
        const applied: string[] = []
        const failed: unknown[] = []
        const read = readLatest(
            sequence,
            () => reply.promise,
            (graph) => applied.push(graph),
            (err) => failed.push(err),
        )
        sequence.cancel()
        reply.resolve("for another repository")
        await read
        expect(applied).toEqual([])
        expect(failed).toEqual([])
    })

    test("only the newest read's failure is reported", async () => {
        const sequence = createRequestSequence()
        const older = deferred<string>()
        const newer = deferred<string>()
        const failed: unknown[] = []
        const applied: string[] = []
        const first = readLatest(
            sequence,
            () => older.promise,
            (graph) => applied.push(graph),
            (err) => failed.push(err),
        )
        const second = readLatest(
            sequence,
            () => newer.promise,
            (graph) => applied.push(graph),
            (err) => failed.push(err),
        )
        older.reject("stale failure")
        await first
        newer.reject("current failure")
        await second
        expect(failed).toEqual(["current failure"])
        expect(applied).toEqual([])
    })
})
