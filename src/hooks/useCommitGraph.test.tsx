import { afterAll, beforeAll, describe, expect, test } from "bun:test"
import { act } from "react"
import { installFakeDom, type FakeDocument } from "../testing/fakeDom"
import { FakeEventSource, installFakeTransport, settle } from "../testing/fakeTransport"
import type { CommitGraph } from "../types"
import { useCommitGraph, type UseCommitGraphResult } from "./useCommitGraph"

// `commit-graph`: *Overlapping Graph Refreshes Settle on the Newest*, through
// the real hook on the fake DOM and transport.

/// The graph replies, answered in the order the test resolves them.
const pending: Array<{ repoId: string; resolve: (graph: CommitGraph) => void }> = []

function graph(label: string): CommitGraph {
    return { commits: [{ id: label }], edges: [], laneCount: 1 } as unknown as CommitGraph
}

let dom: { document: FakeDocument; uninstall: () => void }
let transport: ReturnType<typeof installFakeTransport>
let createRoot: typeof import("react-dom/client").createRoot

beforeAll(async () => {
    dom = installFakeDom()
    transport = installFakeTransport((command, args) =>
        command === "get_commit_graph"
            ? new Promise<CommitGraph>((resolve) => {
                  pending.push({ repoId: String(args.repoId), resolve })
              })
            : null,
    )
    createRoot = (await import("react-dom/client")).createRoot
})

afterAll(() => {
    transport.uninstall()
    dom.uninstall()
})

describe("useCommitGraph", () => {
    test("a repository with no graph yet shows loading from its first frame, and refreshes do not", async () => {
        const seen: Array<UseCommitGraphResult & { repoId: string | null }> = []
        function Probe({ repoId }: { repoId: string | null }) {
            const result = useCommitGraph(repoId, 200)
            seen.push({ ...result, repoId })
            return null
        }
        const root = createRoot(dom.document.createElement("div") as unknown as Element)
        try {
            await act(async () => root.render(<Probe repoId={null} />))
            await act(async () => root.render(<Probe repoId="r1" />))
            // In the commit that shows the repository, before the scheduled read
            // has even started: the rail never draws a graph-less, loading-less
            // frame ("No commits") for longer than that one commit.
            expect(seen[seen.length - 1]).toMatchObject({ repoId: "r1", graph: null, loading: true })
            await settle()

            expect(pending).toHaveLength(1)
            await act(async () => pending.shift()!.resolve(graph("first")))
            await settle()
            expect(seen[seen.length - 1]!.graph).toEqual(graph("first"))
            expect(seen[seen.length - 1]!.loading).toBe(false)

            // A burst of ref changes during a read costs one more read, and no
            // frame shows the loading state while a graph is held.
            const before = seen.length
            await act(async () => FakeEventSource.current?.emit("graph-changed", { repoId: "r1" }))
            await settle()
            expect(pending).toHaveLength(1)
            await act(async () => {
                for (let i = 0; i < 3; i += 1) {
                    FakeEventSource.current?.emit("graph-changed", { repoId: "r1" })
                }
            })
            await settle()
            expect(pending).toHaveLength(1)
            await act(async () => pending.shift()!.resolve(graph("second")))
            await settle()
            expect(pending).toHaveLength(1)
            await act(async () => pending.shift()!.resolve(graph("third")))
            await settle()
            expect(seen.slice(before).some((frame) => frame.loading)).toBe(false)
            expect(seen[seen.length - 1]!.graph).toEqual(graph("third"))
            expect(transport.calls.filter((c) => c === "get_commit_graph")).toHaveLength(3)

        } finally {
            // Whatever failed, no timer of this root outlives the test.
            await act(async () => root.unmount())
        }
    })

    test("a reply for a repository no longer shown is discarded", async () => {
        pending.length = 0
        const seen: UseCommitGraphResult[] = []
        function Probe({ repoId }: { repoId: string | null }) {
            const result = useCommitGraph(repoId, 200)
            seen.push(result)
            return null
        }
        const root = createRoot(dom.document.createElement("div") as unknown as Element)
        try {
            await act(async () => root.render(<Probe repoId="a" />))
            await settle()
            expect(pending.map((p) => p.repoId)).toEqual(["a"])
            await act(async () => root.render(<Probe repoId="b" />))
            await settle()
            // The read for `a` is still in flight; `b`'s waits behind it.
            await act(async () => pending.shift()!.resolve(graph("a")))
            await settle()
            expect(seen.some((frame) => frame.graph !== null)).toBe(false)
            expect(pending.map((p) => p.repoId)).toEqual(["b"])
            await act(async () => pending.shift()!.resolve(graph("b")))
            await settle()
            expect(seen[seen.length - 1]!.graph).toEqual(graph("b"))
        } finally {
            // Whatever failed, no timer of this root outlives the test.
            await act(async () => root.unmount())
        }
    })
})
