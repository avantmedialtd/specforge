import { describe, expect, test } from "bun:test"
import { getEventListeners } from "node:events"
import { computeImageDiff, readImagePixels } from "./imageDiff"
import { comparisonSize, IMAGE_DIFF_FAILED_TEXT, IMAGE_DIFF_LIMIT_TEXT, type PixelDiffResult } from "./imageDiffPixels"
import { decodeSide, type DecodedImage } from "./diffImage"
import { exifJpeg } from "./testing/exifJpeg"

const image: DecodedImage = {
    width: 2, height: 2, mime: "image/png", bytes: new Uint8Array([1, 2, 3]),
}

function environment() {
    const g = globalThis as unknown as Record<string, unknown>
    const saved = ["Image", "Worker", "document"].map(key => [key, key in g, g[key]] as const)
    const create = Object.getOwnPropertyDescriptor(URL, "createObjectURL")!
    const revoke = Object.getOwnPropertyDescriptor(URL, "revokeObjectURL")!
    const urls: string[] = []
    const revoked: string[] = []
    const images: LocalImage[] = []
    const canvases: LocalCanvas[] = []
    const workers: LocalWorker[] = []
    const draws: number[][] = []
    const flags = { canvasFails: false, postFails: false }
    class LocalImage {
        onload: (() => void) | null = null
        onerror: (() => void) | null = null
        src = ""
        constructor() { images.push(this) }
        removeAttribute() { this.src = "" }
    }
    class LocalCanvas {
        width = 0
        height = 0
        getContext() {
            if (flags.canvasFails) { return null }
            return {
                drawImage(_image: unknown, x: number, y: number, width: number, height: number) {
                    draws.push([x, y, width, height])
                },
                getImageData: () => ({ data: new Uint8ClampedArray(this.width * this.height * 4) }),
            }
        }
    }
    class LocalWorker {
        onmessage: ((event: unknown) => void) | null = null
        onerror: (() => void) | null = null
        onmessageerror: (() => void) | null = null
        terminated = false
        messages: unknown[] = []
        constructor() { workers.push(this) }
        terminate() { this.terminated = true }
        postMessage(message: unknown, transfer: unknown) {
            if (flags.postFails) { throw new Error("worker can't receive") }
            this.messages.push([message, transfer])
        }
    }
    g.Image = LocalImage
    g.Worker = LocalWorker
    g.document = {
        createElement(tag: string) {
            expect(tag).toBe("canvas")
            const canvas = new LocalCanvas()
            canvases.push(canvas)
            return canvas
        },
    }
    URL.createObjectURL = () => {
        const url = `blob:comparison-${urls.length}`
        urls.push(url)
        return url
    }
    URL.revokeObjectURL = url => revoked.push(url)
    return {
        images, canvases, workers, urls, revoked, flags, draws,
        restore() {
            Object.defineProperty(URL, "createObjectURL", create)
            Object.defineProperty(URL, "revokeObjectURL", revoke)
            for (const [key, existed, value] of saved) {
                if (existed) { g[key] = value } else { delete g[key] }
            }
        },
    }
}

const settle = async () => { for (let i = 0; i < 8; i += 1) { await Promise.resolve() } }

describe("local pixel-diff resources", () => {
    test("decode uses only owned blob URLs and releases canvas storage", async () => {
        const env = environment()
        try {
            const decoded = readImagePixels(image, { width: 3, height: 2 }, new AbortController().signal)
            expect(env.images[0].src).toBe("blob:comparison-0")
            env.images[0].onload?.()
            expect((await decoded).length).toBe(24)
            expect(env.revoked).toEqual(env.urls)
            expect(env.images[0].src).toBe("")
            expect(env.canvases[0].width).toBe(0)
        } finally { env.restore() }
    })

    test("pixel decoding uses the same oriented geometry as the version frames", async () => {
        const env = environment()
        try {
            const side = decodeSide({
                kind: "image", mime: "image/jpeg", width: 200, height: 100,
                data: btoa(String.fromCharCode(...exifJpeg(6))),
            })
            if (side.kind !== "image") { throw new Error("missing JPEG") }
            const size = comparisonSize(side.image, { width: 100, height: 200 })
            const decoded = readImagePixels(side.image, size, new AbortController().signal)
            env.images[0].onload?.()
            expect((await decoded).length).toBe(100 * 200 * 4)
            expect(env.draws).toEqual([[0, 0, 100, 200]])
            expect(env.revoked).toEqual(env.urls)
        } finally { env.restore() }
    })

    test("failed decode and missing canvas context release their URL", async () => {
        const env = environment()
        try {
            const failed = readImagePixels(image, image, new AbortController().signal)
            env.images[0].onerror?.()
            await expect(failed).rejects.toThrow(IMAGE_DIFF_FAILED_TEXT)
            env.flags.canvasFails = true
            const noCanvas = readImagePixels(image, image, new AbortController().signal)
            env.images[1].onload?.()
            await expect(noCanvas).rejects.toThrow(IMAGE_DIFF_FAILED_TEXT)
            expect(env.revoked).toEqual(env.urls)
        } finally { env.restore() }
    })

    test("cancelling during decode cancels both sides before any worker starts", async () => {
        const env = environment()
        const controller = new AbortController()
        try {
            const request = computeImageDiff(image, image, controller.signal)
            controller.abort()
            await expect(request).rejects.toMatchObject({ name: "AbortError" })
            expect(env.revoked).toEqual(env.urls)
            expect(env.images.every(side => side.onload === null && side.src === "")).toBe(true)
            expect(env.workers).toHaveLength(0)
        } finally { env.restore() }
    })

    test("a failed side cancels the other pending decode", async () => {
        const env = environment()
        try {
            const request = computeImageDiff(image, image, new AbortController().signal)
            env.images[0].onerror?.()
            await expect(request).rejects.toThrow(IMAGE_DIFF_FAILED_TEXT)
            expect(env.revoked).toEqual(env.urls)
            expect(env.workers).toHaveLength(0)
        } finally { env.restore() }
    })

    test("a successful worker receives transferable buffers and is terminated", async () => {
        const env = environment()
        try {
            const request = computeImageDiff(image, image, new AbortController().signal)
            for (const side of env.images) { side.onload?.() }
            await settle()
            const worker = env.workers[0]
            const [message, transfer] = worker.messages[0] as [
                { oldPixels: Uint8ClampedArray<ArrayBuffer>; newPixels: Uint8ClampedArray<ArrayBuffer> }, ArrayBuffer[],
            ]
            expect(transfer).toEqual([message.oldPixels.buffer, message.newPixels.buffer])
            const result: PixelDiffResult = { width: 2, height: 2, changed: 0, pixels: new Uint8ClampedArray(16) }
            worker.onmessage?.({ data: { kind: "diff", result } })
            expect(await request).toBe(result)
            expect(worker.terminated).toBe(true)
            expect(worker.onmessage).toBeNull()
        } finally { env.restore() }
    })

    test("cancellation discards a late worker result", async () => {
        const env = environment()
        const controller = new AbortController()
        try {
            const request = computeImageDiff(image, image, controller.signal)
            for (const side of env.images) { side.onload?.() }
            await settle()
            const worker = env.workers[0]
            const late = worker.onmessage
            controller.abort()
            late?.({ data: { kind: "diff", result: { changed: 999 } } })
            await expect(request).rejects.toMatchObject({ name: "AbortError" })
            expect(worker.terminated).toBe(true)
            expect(worker.onmessage).toBeNull()
        } finally { env.restore() }
    })

    test("worker errors and transfer failures terminate work", async () => {
        for (const postFails of [false, true]) {
            const env = environment()
            try {
                env.flags.postFails = postFails
                const controller = new AbortController()
                const request = computeImageDiff(image, image, controller.signal)
                for (const side of env.images) { side.onload?.() }
                await settle()
                if (!postFails) { env.workers[0].onerror?.() }
                await expect(request).rejects.toThrow()
                expect(env.workers[0].terminated).toBe(true)
                expect(env.revoked).toEqual(env.urls)
                expect(getEventListeners(controller.signal, "abort")).toHaveLength(0)
            } finally { env.restore() }
        }
    })

    test("over-limit and already-cancelled requests allocate no decoder or worker", async () => {
        const env = environment()
        try {
            await expect(computeImageDiff({ ...image, width: 4000001 }, image,
                new AbortController().signal)).rejects.toThrow(IMAGE_DIFF_LIMIT_TEXT)
            const controller = new AbortController()
            controller.abort()
            await expect(computeImageDiff(image, image, controller.signal)).rejects.toMatchObject({ name: "AbortError" })
            expect(env.images).toHaveLength(0)
            expect(env.canvases).toHaveLength(0)
            expect(env.workers).toHaveLength(0)
        } finally { env.restore() }
    })
})
