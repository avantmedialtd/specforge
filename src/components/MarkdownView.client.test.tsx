import { afterAll, beforeAll, describe, expect, test } from "bun:test"
import { act } from "react"
import { loadedMath, resetMathEngineForTest } from "../mathEngine"
import { installFakeDom, type FakeDocument } from "../testing/fakeDom"
import { settle } from "../testing/fakeTransport"
import { MarkdownView } from "./MarkdownView"

// `spec-browser`: *Surfaces and Rendering Engines Load on Demand*: a
// document whose maths engine cannot be loaded stops waiting for it and shows
// its maths as code, rather than staying blank. Mounted through
// `react-dom/client` on the fake DOM, since the wait and its end are effects.

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
    resetMathEngineForTest()
    dom.uninstall()
})

describe("MarkdownView maths", () => {
    test("a failed engine load renders the maths as code", async () => {
        let attempts = 0
        resetMathEngineForTest(() => {
            attempts += 1
            return Promise.reject(new Error("chunk unavailable"))
        })
        const container = dom.document.createElement("div")
        const root = createRoot(container as unknown as Element)
        const { warn } = console
        console.warn = () => {}
        try {
            await act(async () =>
                root.render(<MarkdownView content={"$$x^2$$"} root="/ws" basePath="a.md" />),
            )
            // While the load is pending, nothing: never the source.
            expect(container.textContent).not.toContain("$$")
            await settle()
            expect(attempts).toBe(1)
            expect(loadedMath()).toBeNull()
            const codes = container.findAll("code")
            expect(codes).toHaveLength(1)
            expect(codes[0]!.getAttribute("class")).toContain("language-math")
            expect(codes[0]!.textContent).toBe("x^2")
            expect(container.textContent).not.toContain("$$")
        } finally {
            await act(async () => root.unmount())
            console.warn = warn
        }
    })
})
