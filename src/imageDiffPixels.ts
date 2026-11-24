import pixelmatch from "pixelmatch"

export interface ImageSize {
    width: number
    height: number
}

/// A processing bound, NOT the service's accepted-image preview ceiling.
export const IMAGE_DIFF_PIXEL_LIMIT = 4_000_000
export const IMAGE_DIFF_LIMIT_TEXT =
    "Too large to highlight: the comparison exceeds 4 million pixels. Use another comparison mode."
export const IMAGE_DIFF_FAILED_TEXT =
    "Can't compute a pixel diff here. Use another comparison mode."

export function comparisonSize(before: ImageSize, after: ImageSize): ImageSize {
    return { width: Math.max(before.width, after.width), height: Math.max(before.height, after.height) }
}

export function canHighlight(before: ImageSize, after: ImageSize): boolean {
    const valid = (size: ImageSize) => Number.isSafeInteger(size.width) && size.width > 0
        && Number.isSafeInteger(size.height) && size.height > 0
    const size = comparisonSize(before, after)
    return valid(before) && valid(after) && size.width * size.height <= IMAGE_DIFF_PIXEL_LIMIT
}

export interface PixelDiffRequest {
    before: ImageSize
    after: ImageSize
    /// Both buffers cover the union canvas, padded with transparent pixels.
    oldPixels: Uint8ClampedArray<ArrayBuffer>
    newPixels: Uint8ClampedArray<ArrayBuffer>
}

export interface PixelDiffResult extends ImageSize {
    pixels: Uint8ClampedArray<ArrayBuffer>
    changed: number
}

export type PixelDiffReply = { kind: "diff"; result: PixelDiffResult } | { kind: "failed" }

/// Runs in a worker; kept pure for pixel-level regression tests.
export function comparePixels({ before, after, oldPixels, newPixels }: PixelDiffRequest): PixelDiffResult {
    if (!canHighlight(before, after)) { throw new Error(IMAGE_DIFF_LIMIT_TEXT) }
    const { width, height } = comparisonSize(before, after)
    if (oldPixels.length !== width * height * 4 || newPixels.length !== oldPixels.length) {
        throw new Error("Invalid pixel buffer dimensions")
    }
    const pixels = new Uint8ClampedArray(oldPixels.length)
    pixelmatch(oldPixels, newPixels, pixels, width, height, {
        // Pixelmatch 8's OKLab metric uses 0.1 by default. The 0.2 value
        // familiar from older screenshot comparators hides visible recolors.
        threshold: 0.1,
        includeAA: false,
        diffMask: true,
    })
    let changed = 0
    for (let y = 0; y < height; y += 1) {
        for (let x = 0; x < width; x += 1) {
            const offset = (y * width + x) * 4
            const inOld = x < before.width && y < before.height
            const inNew = x < after.width && y < after.height
            // A resized transparent border is a real geometry change, even
            // though the padded pixels have identical RGBA values.
            if (pixels[offset + 3] !== 0 || inOld !== inNew) {
                pixels[offset] = 255
                pixels[offset + 1] = 0
                pixels[offset + 2] = 0
                changed += 1
            } else {
                const source = inOld ? oldPixels : newPixels
                const luminance = source[offset] * 0.299 + source[offset + 1] * 0.587 + source[offset + 2] * 0.114
                const gray = 255 + (luminance - 255) * 0.1 * source[offset + 3] / 255
                pixels[offset] = gray
                pixels[offset + 1] = gray
                pixels[offset + 2] = gray
            }
            pixels[offset + 3] = 255
        }
    }
    return { width, height, pixels, changed }
}
