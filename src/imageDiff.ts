import type { DecodedImage } from "./diffImage"
import {
    canHighlight, comparisonSize, IMAGE_DIFF_FAILED_TEXT, IMAGE_DIFF_LIMIT_TEXT,
    type ImageSize, type PixelDiffReply, type PixelDiffResult,
} from "./imageDiffPixels"

function aborted(): DOMException {
    return new DOMException("Image comparison cancelled", "AbortError")
}

/// Decode only service-accepted bytes. URLs and canvas storage belong to this
/// one decode, including error and cancellation paths; no remote URL is used.
export function readImagePixels(
    image: DecodedImage, size: ImageSize, signal: AbortSignal,
): Promise<Uint8ClampedArray<ArrayBuffer>> {
    if (signal.aborted) { return Promise.reject(aborted()) }
    return new Promise((resolve, reject) => {
        const element = new Image()
        const url = URL.createObjectURL(new Blob([image.bytes], { type: image.mime }))
        const canvas = document.createElement("canvas")
        const cleanup = () => {
            signal.removeEventListener("abort", cancel)
            element.onload = null
            element.onerror = null
            element.removeAttribute("src")
            URL.revokeObjectURL(url)
            canvas.width = 0
            canvas.height = 0
        }
        const cancel = () => { cleanup(); reject(aborted()) }
        element.onload = () => {
            try {
                canvas.width = size.width
                canvas.height = size.height
                const context = canvas.getContext("2d", { willReadFrequently: true })
                if (!context) { throw new Error(IMAGE_DIFF_FAILED_TEXT) }
                context.drawImage(element, 0, 0, image.width, image.height)
                const pixels = context.getImageData(0, 0, size.width, size.height).data
                cleanup()
                resolve(pixels)
            } catch {
                cleanup()
                reject(new Error(IMAGE_DIFF_FAILED_TEXT))
            }
        }
        element.onerror = () => { cleanup(); reject(new Error(IMAGE_DIFF_FAILED_TEXT)) }
        signal.addEventListener("abort", cancel, { once: true })
        element.src = url
    })
}

/// Only the pixel loop runs off-thread: Image + Canvas keeps decoding usable
/// in WebKit too. Cancelling a decode also cancels its peer, and a worker never
/// outlives this request. No worker or pixel buffers are made over the ceiling.
export async function computeImageDiff(
    before: DecodedImage, after: DecodedImage, signal: AbortSignal,
): Promise<PixelDiffResult> {
    if (!canHighlight(before, after)) { throw new Error(IMAGE_DIFF_LIMIT_TEXT) }
    if (signal.aborted) { throw aborted() }
    const decoding = new AbortController()
    const cancel = () => decoding.abort()
    signal.addEventListener("abort", cancel, { once: true })
    let worker: Worker | null = null
    let stopWaiting = () => {}
    try {
        const size = comparisonSize(before, after)
        const [oldPixels, newPixels] = await Promise.all([
            readImagePixels(before, size, decoding.signal),
            readImagePixels(after, size, decoding.signal),
        ])
        if (signal.aborted) { throw aborted() }
        const running = new Worker(new URL("./imageDiff.worker.ts", import.meta.url), { type: "module" })
        worker = running
        return await new Promise<PixelDiffResult>((resolve, reject) => {
            const stop = () => reject(aborted())
            stopWaiting = () => signal.removeEventListener("abort", stop)
            const finish = (result?: PixelDiffResult) => {
                stopWaiting()
                if (result) { resolve(result) }
                else { reject(new Error(IMAGE_DIFF_FAILED_TEXT)) }
            }
            running.onmessage = (event: MessageEvent<PixelDiffReply>) => {
                finish(event.data.kind === "diff" ? event.data.result : undefined)
            }
            running.onerror = () => finish()
            running.onmessageerror = () => finish()
            signal.addEventListener("abort", stop, { once: true })
            running.postMessage(
                { before: { width: before.width, height: before.height },
                    after: { width: after.width, height: after.height }, oldPixels, newPixels },
                [oldPixels.buffer, newPixels.buffer],
            )
        })
    } finally {
        stopWaiting()
        signal.removeEventListener("abort", cancel)
        decoding.abort()
        if (worker) {
            worker.onmessage = null
            worker.onerror = null
            worker.onmessageerror = null
            worker.terminate()
        }
    }
}
