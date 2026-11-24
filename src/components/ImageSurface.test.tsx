import { afterAll, beforeAll, describe, expect, test } from "bun:test"
import { act } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { readFileSync } from "node:fs"
import { installFakeDom, type FakeElement } from "../testing/fakeDom"
import type { DecodedImage } from "../diffImage"
import { IMAGE_DIFF_LIMIT_TEXT } from "../imageDiffPixels"
import { AlignedImage, ImageSurface, imageSurfaceStyle } from "./ImageSurface"
import { parseBlocks, resolveValue, themeTokens } from "../themeContrast"

const before: DecodedImage = {
    width: 200, height: 100, mime: "image/png", bytes: new Uint8Array(),
}
const after: DecodedImage = { ...before, width: 400, height: 300 }

test("versions reserve their own fraction of the union without distortion", () => {
    const html = renderToStaticMarkup(<AlignedImage image={before} size={after} alt="Before" />)
    expect(html).toContain("width:50%")
    expect(html).toContain("height:33.33333333333333%")
    expect(html).toContain('alt="Before"')
})

test("inline boxes reserve space without requiring newer WebKit aspect-ratio support", () => {
    expect(imageSurfaceStyle({ width: 16, height: 16 })).toEqual({
        width: "16px", maxWidth: "100%", "--image-frame-ratio": "100%",
    })
    expect(imageSurfaceStyle({ width: 800, height: 460 })["--image-frame-ratio"]).toBe("57.5%")
    const css = readFileSync(new URL("../styles/base.css", import.meta.url), "utf8")
    const reservation = parseBlocks(css).find(block => block.selector === ".diff-image-frame::before")
    expect(reservation?.body).toMatch(/padding-bottom:\s*var\(--image-frame-ratio\)/)
})

test("an oversized Diff explains its processing ceiling without a canvas", () => {
    const html = renderToStaticMarkup(<ImageSurface before={{ ...before, width: 4000001 }}
        after={after} mode="diff" name="screen diff" />)
    expect(html).toContain(IMAGE_DIFF_LIMIT_TEXT)
    expect(html).not.toContain("<canvas")
})

test("image controls override the frame's zero line-height with a token every theme defines", () => {
    const css = readFileSync(new URL("../styles/base.css", import.meta.url), "utf8")
    const block = parseBlocks(css).find(block =>
        block.selector.includes(".image-surface__blink-controls") && /line-height:/.test(block.body),
    )
    const leading = block?.body.match(/line-height:\s*([^;]+);/)?.[1]
    expect(leading).toBeDefined()
    for (const id of ["forge", "blueprint", "folio", "mycelium", "avant"]) {
        const theme = themeTokens(readFileSync(new URL(`../styles/themes/${id}.css`, import.meta.url), "utf8"), id)
        for (const scheme of ["light", "dark"] as const) {
            expect(Number(resolveValue(leading!, theme[scheme]))).toBeGreaterThan(0)
        }
    }
})

let dom: ReturnType<typeof installFakeDom>
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

function props(element: FakeElement): Record<string, (...args: never[]) => unknown> {
    const key = Reflect.ownKeys(element).find(key => typeof key === "string" && key.startsWith("__reactProps$"))
    return (element as unknown as Record<PropertyKey, unknown>)[key!] as Record<string, (...args: never[]) => unknown>
}
async function fire(element: FakeElement, name: string, event: Record<string, unknown> = {}) {
    const state = { stopped: false, prevented: false }
    await act(async () => {
        props(element)[name]?.({
            currentTarget: element, target: element,
            stopPropagation() { state.stopped = true }, preventDefault() { state.prevented = true },
            ...event,
        } as never)
    })
    return state
}

async function mount(mode: "slider" | "blink" | "before") {
    const container = dom.document.createElement("div")
    dom.document.body.appendChild(container)
    const root = createRoot(container as unknown as Element)
    const render = (value: "slider" | "blink" | "before") =>
        root.render(<ImageSurface before={before} after={after} mode={value} name="screen.png: comparison" />)
    await act(async () => render(mode))
    return {
        container,
        async mode(value: "slider" | "blink" | "before") { await act(async () => render(value)) },
        async unmount() {
            await act(async () => root.unmount())
            dom.document.body.removeChild(container)
        },
    }
}

describe("shared image interactions", () => {
    test("the divider supports keys, pointer capture, clamping and isolated gestures", async () => {
        const view = await mount("slider")
        try {
            const slider = view.container.findAll("div").find(element => element.getAttribute("role") === "slider")!
            const surface = slider.parentElement!
            surface.getBoundingClientRect = () => ({
                x: 0, y: 0, left: 0, top: 0, width: 200, height: 100, right: 200, bottom: 100,
            })
            let captured = false
            const target = slider as unknown as Record<string, unknown>
            target.setPointerCapture = () => { captured = true }
            target.hasPointerCapture = () => captured
            target.releasePointerCapture = () => { captured = false }
            expect(slider.getAttribute("aria-valuenow")).toBe("50")
            expect(await fire(slider, "onKeyDown", { key: "End" })).toEqual({ stopped: true, prevented: true })
            expect(slider.getAttribute("aria-valuenow")).toBe("100")
            await fire(slider, "onKeyDown", { key: "Home" })
            expect(slider.getAttribute("aria-valuenow")).toBe("0")
            await fire(slider, "onKeyDown", { key: "ArrowRight" })
            expect(slider.getAttribute("aria-valuenow")).toBe("1")
            expect(await fire(slider, "onKeyDown", { key: "+" })).toEqual({ stopped: false, prevented: false })
            expect(await fire(slider, "onPointerDown", { button: 0, pointerId: 7, clientX: 150 })).toEqual({
                stopped: true, prevented: true,
            })
            expect(captured).toBe(true)
            expect(slider.getAttribute("aria-valuenow")).toBe("75")
            await fire(slider, "onPointerMove", { pointerId: 7, clientX: -10 })
            expect(slider.getAttribute("aria-valuenow")).toBe("0")
            await fire(slider, "onPointerUp", { pointerId: 7 })
            expect(captured).toBe(false)
            expect((await fire(slider, "onClick")).stopped).toBe(true)
        } finally { await view.unmount() }
    })

    test("Blink pauses for reduced motion, visibility, explicit pause and cleanup", async () => {
        const originalSet = globalThis.setInterval
        const originalClear = globalThis.clearInterval
        const originalObserver = g.IntersectionObserver
        const timers = new Map<number, () => void>()
        let tick = 0
        let intersect: (entries: { isIntersecting: boolean }[]) => void = () => {}
        let reduced = () => {}
        const media = { matches: true, addEventListener: (_: string, listener: () => void) => { reduced = listener },
            removeEventListener() {} }
        ;(window as unknown as Record<string, unknown>).matchMedia = () => media
        g.IntersectionObserver = class {
            constructor(callback: typeof intersect) { intersect = callback }
            observe() {}
            disconnect() {}
        }
        globalThis.setInterval = ((callback: () => void, delay: number) => {
            expect(delay).toBe(500)
            timers.set(++tick, callback)
            return tick
        }) as unknown as typeof setInterval
        globalThis.clearInterval = ((id: number) => timers.delete(id)) as unknown as typeof clearInterval
        const view = await mount("blink")
        try {
            const button = () => view.container.findAll("button")[0]
            const root = view.container.findAll("div").find(element => element.getAttribute("class") === "image-surface__layers")!
            root.getBoundingClientRect = () => ({
                x: 0, y: 0, left: 0, top: 0, width: 200, height: 100, right: 200, bottom: 100,
            })
            ;(window as unknown as Record<string, unknown>).innerHeight = 800
            expect(button().textContent).toBe("Resume")
            await act(async () => intersect([{ isIntersecting: true }]))
            expect(timers.size).toBe(0)
            await fire(button(), "onClick")
            expect(timers.size).toBe(1)
            await act(async () => { for (const timer of timers.values()) { timer() } })
            expect(view.container.textContent).toContain("After")
            await fire(button(), "onClick")
            expect(timers.size).toBe(0)
            await fire(button(), "onClick")
            expect(timers.size).toBe(1)
            await act(async () => {
                dom.document.visibilityState = "hidden"
                dom.document.dispatch("visibilitychange", {})
            })
            expect(timers.size).toBe(0)
            await act(async () => {
                dom.document.visibilityState = "visible"
                dom.document.dispatch("visibilitychange", {})
            })
            expect(timers.size).toBe(1)
            await act(async () => intersect([{ isIntersecting: false }]))
            expect(timers.size).toBe(0)
            await act(async () => intersect([{ isIntersecting: true }]))
            await act(async () => reduced())
            expect(timers.size).toBe(0)
            await fire(button(), "onClick")
            await view.mode("before")
            expect(timers.size).toBe(0)
        } finally {
            await view.unmount()
            expect(timers.size).toBe(0)
            globalThis.setInterval = originalSet
            globalThis.clearInterval = originalClear
            if (originalObserver === undefined) { delete g.IntersectionObserver } else { g.IntersectionObserver = originalObserver }
            delete (window as unknown as Record<string, unknown>).matchMedia
        }
    })
})
