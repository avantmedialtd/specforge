import { comparePixels, type PixelDiffReply, type PixelDiffRequest } from "./imageDiffPixels"

// DOM and worker globals have different postMessage signatures.
const worker = globalThis as unknown as {
    onmessage: (event: MessageEvent<PixelDiffRequest>) => void
    postMessage: (reply: PixelDiffReply, transfer: ArrayBuffer[]) => void
}

worker.onmessage = ({ data }) => {
    try {
        const result = comparePixels(data)
        worker.postMessage({ kind: "diff", result }, [result.pixels.buffer])
    } catch {
        worker.postMessage({ kind: "failed" }, [])
    }
}
