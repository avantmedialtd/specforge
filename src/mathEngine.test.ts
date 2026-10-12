import { describe, expect, test } from "bun:test"
import { mayContainMath } from "./mathEngine"

// `spec-browser`: *Surfaces and Rendering Engines Load on Demand*; design D7.
// `loadMath` itself is exercised in `MarkdownView.test.tsx`, which orders its
// before-and-after cases around the module's loaded flag.

describe("mayContainMath", () => {
    test("double-dollar maths, inline or display, may be maths", () => {
        expect(mayContainMath("Energy is $$E=mc^2$$ in prose.")).toBe(true)
        expect(mayContainMath("$$\n\\sum_i x_i\n$$")).toBe(true)
        expect(mayContainMath("$$x$$")).toBe(true)
    })

    test("a backtick or tilde math fence may be maths", () => {
        expect(mayContainMath("```math\nx^2\n```")).toBe(true)
        expect(mayContainMath("~~~math\nx^2\n~~~")).toBe(true)
        expect(mayContainMath("````math\nx^2\n````")).toBe(true)
    })

    test("a fence with a space before its info string may be maths", () => {
        expect(mayContainMath("``` math\nx^2\n```")).toBe(true)
        expect(mayContainMath("~~~ \tmath\nx^2\n~~~")).toBe(true)
    })

    test("a fence opened inside a list item or a blockquote may be maths", () => {
        expect(mayContainMath("- item\n\n  ```math\n  x^2\n  ```")).toBe(true)
        expect(mayContainMath("- ```math\n  x^2\n  ```")).toBe(true)
        expect(mayContainMath("> ```math\n> x^2\n> ```")).toBe(true)
    })

    test("single-dollar prose is not maths", () => {
        expect(mayContainMath("It costs $50 per seat and $60 with add-ons.")).toBe(false)
        expect(mayContainMath("$x$ stays text")).toBe(false)
    })

    test("another fence, a short run and a plain document are not maths", () => {
        expect(mayContainMath("```js\nconst math = 1\n```")).toBe(false)
        expect(mayContainMath("``math`` is inline code")).toBe(false)
        expect(mayContainMath("# Title\n\nSome text about mathematics.")).toBe(false)
        expect(mayContainMath("")).toBe(false)
    })
})

describe("resetMathEngineForTest", () => {
    test("a load settling after a reset leaves the engine unloaded", async () => {
        const { loadMath, loadedMath, resetMathEngineForTest } = await import("./mathEngine")
        let resolve!: (plugin: never) => void
        resetMathEngineForTest(
            () =>
                new Promise((res) => {
                    resolve = res
                }),
        )
        const late = loadMath()
        resetMathEngineForTest()
        resolve((() => {}) as never)
        await late
        expect(loadedMath()).toBeNull()
    })
})
