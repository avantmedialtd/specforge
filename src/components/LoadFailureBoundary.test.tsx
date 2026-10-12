import { afterAll, beforeAll, describe, expect, test } from "bun:test"
import { act, lazy, Suspense, type ComponentType } from "react"
import { installFakeDom, type FakeDocument, type FakeElement } from "../testing/fakeDom"
import { settle } from "../testing/fakeTransport"
import { isChunkLoadError, LoadFailureBoundary, surfaceResetKey } from "./LoadFailureBoundary"

// `spec-browser`: *Surfaces and Rendering Engines Load on Demand*: a surface
// whose code cannot be loaded says so in place, rather than React unmounting
// the whole root. Rendered for real, through `react-dom/client` on the fake
// DOM, since an error boundary does nothing in a server render.

let dom: { document: FakeDocument; uninstall: () => void }
let createRoot: typeof import("react-dom/client").createRoot
const g = globalThis as unknown as Record<string, unknown>

beforeAll(async () => {
    dom = installFakeDom()
    g.IS_REACT_ACT_ENVIRONMENT = true
    createRoot = (await import("react-dom/client")).createRoot
})

afterAll(() => {
    delete g.IS_REACT_ACT_ENVIRONMENT
    dom.uninstall()
})

/// A lazy surface whose chunk cannot be read, as Chromium words it.
function unloadable(): ComponentType {
    return lazy(() =>
        Promise.reject(new TypeError("Failed to fetch dynamically imported module: /assets/x.js")),
    )
}

/// Renders `run`'s trees in a fresh root with React's error logging quiet,
/// unmounting it whatever happens.
async function withRoot(run: (root: ReturnType<typeof createRoot>, container: FakeElement) => Promise<void>) {
    const container = dom.document.createElement("div")
    const root = createRoot(container as unknown as Element)
    const { warn, error } = console
    console.warn = () => {}
    console.error = () => {}
    try {
        await run(root, container)
    } finally {
        await act(async () => root.unmount())
        console.warn = warn
        console.error = error
    }
}

describe("LoadFailureBoundary", () => {
    test("a lazy surface that fails to load shows the notice in place", async () => {
        const Broken = unloadable()
        await withRoot(async (root, container) => {
            await act(async () =>
                root.render(
                    <div className="pane">
                        <span className="sibling">Sidebar</span>
                        <LoadFailureBoundary onReload={() => {}}>
                            <Suspense fallback={<span>Loading…</span>}>
                                <Broken />
                            </Suspense>
                        </LoadFailureBoundary>
                    </div>,
                ),
            )
            await settle()
            const text = container.textContent
            expect(text).toContain("Sidebar")
            expect(text).toContain("Couldn’t load this view")
            expect(text).not.toContain("Loading…")
            expect(container.findAll("button").map((button) => button.textContent)).toEqual([
                "Reload",
            ])
        })
    })

    test("another render error is worded neutrally, and a new reset key tries again", async () => {
        let fail = true
        const Flaky = () => {
            if (fail) throw new Error("render failed")
            return <span>Shown</span>
        }
        await withRoot(async (root, container) => {
            await act(async () =>
                root.render(
                    <LoadFailureBoundary resetKey="/a" onReload={() => {}}>
                        <Flaky />
                    </LoadFailureBoundary>,
                ),
            )
            expect(container.textContent).toContain("Something went wrong showing this view")
            expect(container.textContent).not.toContain("Couldn’t load")
            fail = false
            await act(async () =>
                root.render(
                    <LoadFailureBoundary resetKey="/b" onReload={() => {}}>
                        <Flaky />
                    </LoadFailureBoundary>,
                ),
            )
            expect(container.textContent).toBe("Shown")
        })
    })
})

describe("isChunkLoadError", () => {
    test("knows each engine's failed dynamic import and Vite's stylesheet preload", () => {
        expect(isChunkLoadError(new TypeError("Failed to fetch dynamically imported module: /a.js"))).toBe(true)
        expect(isChunkLoadError(new TypeError("error loading dynamically imported module: /a.js"))).toBe(true)
        expect(isChunkLoadError(new TypeError("Importing a module script failed."))).toBe(true)
        expect(isChunkLoadError(new Error("Unable to preload CSS for /assets/a.css"))).toBe(true)
        expect(isChunkLoadError(new Error("render failed"))).toBe(false)
        expect(isChunkLoadError("nope")).toBe(false)
    })
})

describe("surfaceResetKey", () => {
    test("changes with the address and with the rail's commit", () => {
        const home = surfaceResetKey({ kind: "home" }, null)
        expect(surfaceResetKey({ kind: "home" }, null)).toBe(home)
        expect(surfaceResetKey({ kind: "home" }, "abc")).not.toBe(home)
        expect(surfaceResetKey({ kind: "home" }, "abc")).not.toBe(surfaceResetKey({ kind: "home" }, "def"))
        expect(surfaceResetKey({ kind: "settings", group: "workspaces" }, null)).not.toBe(home)
        expect(surfaceResetKey({ kind: "unresolvable" }, null)).toBe("\u0000")
    })
})
