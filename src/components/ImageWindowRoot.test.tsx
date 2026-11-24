import { afterAll, beforeAll, describe, expect, test } from "bun:test"
import { act } from "react"
import { FakeElement, installFakeDom } from "../testing/fakeDom"
import { installFakeTransport } from "../testing/fakeTransport"
import { imageWindowAddress, imageWindowPath } from "../imageWindow"
import type { ImageVersions } from "../types"
import { ImageWindowRoot } from "./ImageWindowRoot"
import { exifJpeg } from "../testing/exifJpeg"

let dom: ReturnType<typeof installFakeDom>
let createRoot: typeof import("react-dom/client").createRoot
const g = globalThis as unknown as Record<string, unknown>
let resize: unknown
const saved = new Map<string, PropertyDescriptor | undefined>()

beforeAll(async () => {
    dom = installFakeDom()
    createRoot = (await import("react-dom/client")).createRoot
    resize = g.ResizeObserver
    g.ResizeObserver = class { observe() {}; disconnect() {} }
    for (const [key, value] of Object.entries({ clientWidth: 400, clientHeight: 300, scrollLeft: 0, scrollTop: 0 })) {
        saved.set(key, Object.getOwnPropertyDescriptor(FakeElement.prototype, key))
        Object.defineProperty(FakeElement.prototype, key, { configurable: true, writable: true, value })
    }
    ;(window as unknown as Record<string, unknown>).matchMedia = () => ({
        matches: true, addEventListener() {}, removeEventListener() {},
    })
})
afterAll(() => {
    if (resize === undefined) { delete g.ResizeObserver } else { g.ResizeObserver = resize }
    for (const [key, descriptor] of saved) {
        if (descriptor) { Object.defineProperty(FakeElement.prototype, key, descriptor) }
        else { Reflect.deleteProperty(FakeElement.prototype, key) }
    }
    dom.uninstall()
})

function props(element: FakeElement): Record<string, (event: unknown) => void> {
    const key = Reflect.ownKeys(element).find(key => typeof key === "string" && key.startsWith("__reactProps$"))!
    return (element as unknown as Record<PropertyKey, unknown>)[key] as Record<string, (event: unknown) => void>
}
async function press(element: FakeElement) {
    await act(async () => props(element).onClick({
        currentTarget: element, preventDefault() {}, stopPropagation() {},
    }))
}

const versions: ImageVersions = {
    old: { kind: "image", mime: "image/png", width: 1000, height: 700, data: "AQID" },
    new: { kind: "image", mime: "image/png", width: 1200, height: 800, data: "BAUG" },
}

async function mount(images = versions) {
    const transport = installFakeTransport(command => {
        if (command === "get_appearance") { return { theme: "forge", scheme: "light" } }
        if (command === "get_pull_request_file_image") { return { kind: "images", ...images } }
        throw new Error(`unexpected command ${command}`)
    })
    window.location.search = imageWindowPath(imageWindowAddress({
        kind: "pullRequest", reference: { provider: "github", owner: "org", repo: "repo", number: 7 },
        path: "screen.png", head: "head", base: "base", sides: { old: "base", new: "head" },
    })).slice(1)
    const container = dom.document.createElement("div")
    dom.document.body.appendChild(container)
    const root = createRoot(container as unknown as Element)
    await act(async () => root.render(<ImageWindowRoot />))
    return {
        container, calls: transport.calls,
        async unmount() {
            await act(async () => root.unmount())
            dom.document.body.removeChild(container)
            transport.uninstall()
        },
    }
}

describe("the image zoom window", () => {
    test("oriented JPEG dimensions reach zoom frames and their common extent", async () => {
        const image = { kind: "image" as const, mime: "image/jpeg", width: 200, height: 100,
            data: btoa(String.fromCharCode(...exifJpeg(6))) }
        const view = await mount({ old: image, new: image })
        try {
            const actual = view.container.findAll("button").find(element =>
                element.getAttribute("aria-label") === "Actual size")!
            await press(actual)
            for (const img of view.container.findAll("img")) {
                expect(img.getAttribute("width")).toBe("100")
                expect(img.getAttribute("height")).toBe("200")
            }
            const contents = view.container.findAll("div").filter(element =>
                element.getAttribute("class") === "image-zoom__content")
            expect(contents).toHaveLength(2)
            for (const content of contents) {
                expect(content.style.width).toBe("100px")
                expect(content.style.height).toBe("200px")
            }
        } finally { await view.unmount() }
    })

    test("all six modes reuse one governed read and keep zoom and pan across modes", async () => {
        const view = await mount()
        try {
            const button = (label: string) => view.container.findAll("button")
                .find(element => element.textContent === label)!
            const named = (label: string) => view.container.findAll("button")
                .find(element => element.getAttribute("aria-label") === label)!
            const ports = () => view.container.findAll("div").filter(element =>
                element.getAttribute("class")?.includes("image-zoom__viewport"))
            const first = ports()[0] as unknown as HTMLDivElement
            expect(ports()).toHaveLength(2)
            expect(view.container.findAll("button").filter(element => element.getAttribute("role") === "radio")
                .map(element => element.textContent)).toEqual(["Diff", "Before", "After", "Side by side", "Slider", "Blink"])
            await press(named("Actual size"))
            const scale = () => view.container.findAll("span").find(element =>
                element.getAttribute("class") === "figure-lightbox__scale")!.textContent
            expect(scale()).toBe("100%")
            first.scrollLeft = 123
            first.scrollTop = 87
            for (const mode of ["Slider", "Blink", "Before", "After", "Diff", "Side by side"]) {
                await press(button(mode))
                expect(ports()[0] as unknown).toBe(first)
                expect(first.scrollLeft).toBe(123)
                expect(first.scrollTop).toBe(87)
                expect(scale()).toBe("100%")
                if (mode === "Before" || mode === "After") {
                    expect(view.container.findAll("img")).toHaveLength(1)
                    const content = view.container.findAll("div").find(element =>
                        element.getAttribute("class") === "image-zoom__content")!
                    expect(content.style.width).toBe("1200px")
                    expect(content.style.height).toBe("800px")
                }
            }
            const second = ports()[1] as unknown as HTMLDivElement
            expect(second.scrollLeft).toBe(123)
            expect(second.scrollTop).toBe(87)
            second.scrollLeft = 200
            await act(async () => props(ports()[1]).onScroll({ currentTarget: second }))
            expect(first.scrollLeft).toBe(200)
            expect(view.calls.filter(command => command === "get_pull_request_file_image")).toHaveLength(1)

            const fakePort = ports()[0]
            let prevented = false
            await act(async () => fakePort.dispatch("wheel", {
                ctrlKey: false, deltaY: 40, deltaMode: 0, clientX: 100, clientY: 100,
                preventDefault() { prevented = true },
            }))
            expect(prevented).toBe(false)
            expect(scale()).toBe("100%")
            await act(async () => fakePort.dispatch("wheel", {
                ctrlKey: true, deltaY: -20, deltaMode: 0, clientX: 100, clientY: 100,
                preventDefault() { prevented = true },
            }))
            expect(prevented).toBe(true)
            expect(scale()).not.toBe("100%")
        } finally { await view.unmount() }
    })

    test("an unavailable side retains its note and does not offer comparison modes", async () => {
        const view = await mount({ old: versions.old, new: { kind: "refused", reason: "tooLarge" } })
        try {
            expect(view.container.textContent).toContain("Too large to preview")
            expect(view.container.findAll("img")).toHaveLength(1)
            expect(view.container.findAll("button").some(element => element.getAttribute("role") === "radio")).toBe(false)
        } finally { await view.unmount() }
    })

    test("mode-induced clamping preserves the intended position until the reader pans again", async () => {
        const view = await mount()
        try {
            const button = (label: string) => view.container.findAll("button").find(element =>
                element.textContent === label || element.getAttribute("aria-label") === label)!
            const ports = () => view.container.findAll("div").filter(element =>
                element.getAttribute("class")?.includes("image-zoom__viewport"))
            await press(button("Actual size"))
            const first = ports()[0]
            // Model the browser's irreversible clamping when one full-width
            // frame replaces two columns, including clamping on layout reads.
            for (const [key, splitLimit, singleLimit] of [
                ["scrollLeft", 800, 400], ["scrollTop", 500, 450],
            ] as const) {
                let offset = 0
                const clamp = (value: number) => Math.max(0, Math.min(value, ports().length === 1 ? singleLimit : splitLimit))
                Object.defineProperty(first, key, {
                    configurable: true,
                    get() { return (offset = clamp(offset)) },
                    set(value: number) { offset = clamp(value) },
                })
            }
            const port = first as unknown as HTMLDivElement
            const scroll = async () => act(async () => props(first).onScroll({ currentTarget: port }))
            port.scrollLeft = 799
            port.scrollTop = 499
            await scroll()
            for (const mode of ["Slider", "Blink", "Before", "After", "Diff"]) {
                await press(button(mode))
                expect(port.scrollLeft).toBe(400)
                expect(port.scrollTop).toBe(450)
                // Resize/programmatic scroll events must not be mistaken for
                // user input and overwrite the unclamped retained position.
                await scroll()
            }
            await press(button("Side by side"))
            expect(ports()).toHaveLength(2)
            expect(port.scrollLeft).toBe(799)
            expect(port.scrollTop).toBe(499)
            expect((ports()[1] as unknown as HTMLDivElement).scrollLeft).toBe(799)
            expect((ports()[1] as unknown as HTMLDivElement).scrollTop).toBe(499)

            await press(button("Slider"))
            port.scrollLeft = 180
            port.scrollTop = 90
            await scroll()
            await press(button("Side by side"))
            expect(port.scrollLeft).toBe(180)
            expect(port.scrollTop).toBe(90)
            expect((ports()[1] as unknown as HTMLDivElement).scrollLeft).toBe(180)
            expect((ports()[1] as unknown as HTMLDivElement).scrollTop).toBe(90)
        } finally { await view.unmount() }
    })
})
