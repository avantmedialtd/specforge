import { afterAll, beforeAll, describe, expect, test } from "bun:test"
import { act } from "react"
import { installFakeDom, type FakeDocument } from "../testing/fakeDom"
import { FakeEventSource, installFakeTransport, settle } from "../testing/fakeTransport"
import type { PullRequestLinks, WorkspaceView } from "../types"
import { usePullRequestLinks } from "./usePullRequestLinks"
import { useWorkspaces } from "./useWorkspaces"

// `pull-request-worktree-links`: *The Pull-Request Links Snapshot*, *An
// Unchanged Links Snapshot Is Not Re-applied*. The two real hooks, wired as
// `App` and the pull-request window wire them, rendered through
// `react-dom/client` on the fake DOM against a fake web transport: `fetch` for
// `POST /api/invoke` and an `EventSource` for the event stream. Dropping
// either side — `useWorkspaces` firing `onViewsRead`, or the links hook
// subscribing to it — fails the first case.

/// The views every read answers: equal each time, new objects each time.
function views(): WorkspaceView[] {
    return [
        {
            kind: "flat",
            workspace: { uri: "/ws", name: "ws" },
            changes: [],
            displayName: null,
            color: null,
            hasOpenSpec: true,
        } as unknown as WorkspaceView,
    ]
}

let linksReply: () => PullRequestLinks = () => ({ worktrees: [], pullRequests: [] })

let dom: { document: FakeDocument; uninstall: () => void }
let transport: ReturnType<typeof installFakeTransport>
let createRoot: typeof import("react-dom/client").createRoot

beforeAll(async () => {
    dom = installFakeDom()
    transport = installFakeTransport((command) =>
        command === "get_workspace_views"
            ? views()
            : command === "list_workspaces"
              ? []
              : command === "get_pull_request_links"
                ? linksReply()
                : null,
    )
    createRoot = (await import("react-dom/client")).createRoot
})

afterAll(() => {
    transport.uninstall()
    dom.uninstall()
})

describe("the links re-read, through the real hooks", () => {
    test("every views read leads to a links read, and an equal reply changes nothing", async () => {
        const seenViews: WorkspaceView[][] = []
        const seenLinks: PullRequestLinks[] = []
        function Probe() {
            const { views: held, onViewsRead } = useWorkspaces()
            const links = usePullRequestLinks(onViewsRead)
            seenViews.push(held)
            seenLinks.push(links)
            return null
        }
        const root = createRoot(dom.document.createElement("div") as unknown as Element)
        try {
            await act(async () => root.render(<Probe />))
            await settle()
            const count = (command: string) => transport.calls.filter((c) => c === command).length
            expect(count("get_workspace_views")).toBe(1)
            const linkReadsAfterMount = count("get_pull_request_links")
            expect(linkReadsAfterMount).toBeGreaterThanOrEqual(1)
            const viewsHeld = seenViews[seenViews.length - 1]
            const linksHeld = seenLinks[seenLinks.length - 1]
            const renders = seenViews.length

            // A cache notification: a views read that returns equal views.
            await act(async () => FakeEventSource.current?.emit("cache-updated"))
            await settle()
            expect(count("get_workspace_views")).toBe(2)
            // The links were re-read after it, though the views were equal…
            expect(count("get_pull_request_links")).toBe(linkReadsAfterMount + 1)
            // …and neither equal reply rendered anything.
            expect(seenViews.length).toBe(renders)
            expect(seenViews[seenViews.length - 1]).toBe(viewsHeld)
            expect(seenLinks[seenLinks.length - 1]).toBe(linksHeld)

            // A remote change: equal views, different links.
            linksReply = () => ({
                worktrees: [],
                pullRequests: [{ provider: "github", id: 7 } as unknown as PullRequestLinks["pullRequests"][number]],
            })
            await act(async () => FakeEventSource.current?.emit("cache-updated"))
            await settle()
            expect(seenViews[seenViews.length - 1]).toBe(viewsHeld)
            expect(seenLinks[seenLinks.length - 1]?.pullRequests).toHaveLength(1)

        } finally {
            // Whatever failed, no timer of this root outlives the test.
            await act(async () => root.unmount())
        }
    })
})
