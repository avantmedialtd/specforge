/// The diff view's decisions about an image file's two versions (`diff-view`:
/// *Image Comparison*): what a read's reply decodes to, how a version is
/// captioned and named, what a side shows instead of its image, whether the
/// comparison modes are offered, and the queue that reads files as their
/// sections near the view. Pure, for the reason `diffLayout.ts` gives in its
/// header.

import type { ImageRefusal, ImageSide, ImageVersions } from "./types"

/// One version the view may decode: its type, sniffed by the service, its
/// display dimensions (including JPEG EXIF orientation), and its bytes.
export interface DecodedImage {
    mime: string
    width: number
    height: number
    bytes: Uint8Array<ArrayBuffer>
}

/// One side of an image file, as the view keeps it once read.
export type DecodedSide =
    | { kind: "image"; image: DecodedImage }
    | { kind: "absent" }
    | { kind: "refused"; reason: ImageRefusal }

/// An image file's two sides, as the view keeps them once read.
export interface DecodedVersions {
    old: DecodedSide
    new: DecodedSide
}

/// The bytes a base64 string holds.
export function decodeBase64(data: string): Uint8Array<ArrayBuffer> {
    const binary = atob(data)
    const bytes = new Uint8Array(binary.length)
    for (let i = 0; i < binary.length; i += 1) { bytes[i] = binary.charCodeAt(i) }
    return bytes
}

/// Read only the primary TIFF directory's scalar orientation, never a
/// thumbnail's directory. Every offset stays inside this APP1 payload.
function exifOrientation(tiff: DataView): number {
    if (tiff.byteLength < 8) { return 0 }
    const order = tiff.getUint16(0)
    if (order !== 0x4949 && order !== 0x4d4d) { return 0 }
    const little = order === 0x4949
    if (tiff.getUint16(2, little) !== 42) { return 0 }
    const directory = tiff.getUint32(4, little)
    if (directory < 8 || directory + 2 > tiff.byteLength) { return 0 }
    const count = tiff.getUint16(directory, little)
    if (directory + 2 + count * 12 > tiff.byteLength) { return 0 }
    for (let entry = directory + 2; entry < directory + 2 + count * 12; entry += 12) {
        if (tiff.getUint16(entry, little) !== 0x0112) { continue }
        if (tiff.getUint16(entry + 2, little) !== 3 || tiff.getUint32(entry + 4, little) !== 1) { return 0 }
        const orientation = tiff.getUint16(entry + 8, little)
        return orientation >= 1 && orientation <= 8 ? orientation : 0
    }
    return 0
}

/// Browsers orient a JPEG before drawing it to an <img> or Canvas, whereas
/// the service reports its raw SOF dimensions. Match that geometry without
/// another image decode, provider read, or change to image acceptance.
function jpegAxesSwapped(bytes: Uint8Array<ArrayBuffer>): boolean {
    if (bytes[0] !== 0xff || bytes[1] !== 0xd8) { return false }
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
    let at = 2
    while (at < bytes.length) {
        if (bytes[at++] !== 0xff) { return false }
        while (bytes[at] === 0xff) { at++ }
        const marker = bytes[at++]
        // Metadata ends at the first scan or end marker.
        if (marker === 0xda || marker === 0xd9) { return false }
        if (marker === 0x01 || marker === 0xd8 || (marker >= 0xd0 && marker <= 0xd7)) { continue }
        if (at + 2 > bytes.length) { return false }
        const length = view.getUint16(at)
        if (length < 2 || at + length > bytes.length) { return false }
        const start = at + 2
        const end = at + length
        if (marker === 0xe1 && length >= 16 &&
            view.getUint32(start) === 0x45786966 && view.getUint16(start + 4) === 0) {
            const tiff = new DataView(bytes.buffer, bytes.byteOffset + start + 6, end - start - 6)
            const orientation = exifOrientation(tiff)
            if (orientation !== 0) { return orientation >= 5 }
        }
        at = end
    }
    return false
}

/// A side as a read answered it, its bytes decoded once.
export function decodeSide(side: ImageSide): DecodedSide {
    switch (side.kind) {
        case "image": {
            const bytes = decodeBase64(side.data)
            const swapped = side.mime === "image/jpeg" && jpegAxesSwapped(bytes)
            return {
                kind: "image",
                image: {
                    mime: side.mime,
                    width: swapped ? side.height : side.width,
                    height: swapped ? side.width : side.height,
                    bytes,
                },
            }
        }
        case "absent":
            return { kind: "absent" }
        case "refused":
            return { kind: "refused", reason: side.reason }
    }
}

export function decodeVersions(versions: ImageVersions): DecodedVersions {
    return { old: decodeSide(versions.old), new: decodeSide(versions.new) }
}

/// Whether a read found no image at all: every side absent or not an image,
/// so the file shows the state row it would show were it no image file.
export function holdsNoImage(versions: DecodedVersions): boolean {
    const none = (side: DecodedSide) =>
        side.kind === "absent" || (side.kind === "refused" && side.reason === "notImage")
    return none(versions.old) && none(versions.new)
}

const KIB = 1024
const MIB = 1024 * 1024

/// A size as a caption writes it: bytes below 1 KB, and otherwise KB or MB of
/// 1,024 units, with one decimal below 10 and none from 10 up ("812 B",
/// "345 KB", "1.4 MB").
export function formatBytes(bytes: number): string {
    if (bytes < KIB) { return `${bytes} B` }
    const [value, unit] = bytes < MIB ? [bytes / KIB, "KB"] : [bytes / MIB, "MB"]
    const tenths = Math.round(value * 10) / 10
    return tenths < 10 ? `${tenths.toFixed(1)} ${unit}` : `${Math.round(value)} ${unit}`
}

/// The sign a change is written with: the minus sign, a plus, or none for no
/// change.
function signOf(value: number): string {
    if (value < 0) { return "−" }
    return value > 0 ? "+" : ""
}

/// The change from `oldBytes` to `newBytes`, as bytes and as a whole
/// percentage of the old size ("−47 KB (−14%)"). An empty old version has no
/// percentage.
export function sizeDelta(oldBytes: number, newBytes: number): string {
    const change = newBytes - oldBytes
    const bytes = `${signOf(change)}${formatBytes(Math.abs(change))}`
    if (oldBytes === 0) { return bytes }
    const percent = Math.round((change / oldBytes) * 100)
    return `${bytes} (${signOf(percent)}${Math.abs(percent)}%)`
}

/// A version's dimensions and size, as its caption gives them after its
/// side's name; the new version of a file whose sides are both images also
/// gives the change in size.
export function imageDetails(image: DecodedImage, before?: DecodedImage): string {
    const details = [`${image.width} × ${image.height}`, formatBytes(image.bytes.length)]
    if (before) { details.push(sizeDelta(before.bytes.length, image.bytes.length)) }
    return details.join(" · ")
}

/// Comparisons do not require matching dimensions: the versions share their
/// top-left origin and a canvas large enough to hold both, without stretching.
export function comparisonAvailable(versions: DecodedVersions): boolean {
    return versions.old.kind === "image" && versions.new.kind === "image"
}

export const IMAGE_MODE_OPTIONS = [
    { value: "diff", label: "Diff" },
    { value: "before", label: "Before" },
    { value: "after", label: "After" },
    { value: "both", label: "Side by side" },
    { value: "slider", label: "Slider" },
    { value: "blink", label: "Blink" },
] as const

export function comparisonName(mode: ImageMode, path: string, oldSide: string, newSide: string): string {
    const label = IMAGE_MODE_OPTIONS.find(option => option.value === mode)!.label
    return `${path}: ${label} between ${oldSide} and ${newSide}`
}

export function clampReveal(value: number): number {
    return Number.isFinite(value) ? Math.max(0, Math.min(100, value)) : 50
}

export function revealAt(clientX: number, left: number, width: number): number {
    return width > 0 ? clampReveal((clientX - left) / width * 100) : 50
}

/// Native-slider keys; all other keys belong to the enclosing view.
export function revealKey(key: string, current: number): number | null {
    switch (key) {
        case "Home": return 0
        case "End": return 100
        case "ArrowLeft":
        case "ArrowDown": return clampReveal(current - 1)
        case "ArrowRight":
        case "ArrowUp": return clampReveal(current + 1)
        case "PageDown": return clampReveal(current - 10)
        case "PageUp": return clampReveal(current + 10)
        default: return null
    }
}

export function pixelDiffSummary(changed: number, total: number): string {
    if (changed === 0) { return "No pixels changed (0%)" }
    const percent = total > 0 ? changed / total * 100 : 0
    const formatted = percent < 0.01 ? "<0.01" : Number(percent.toFixed(2)).toString()
    return `${changed.toLocaleString("en-GB")} ${changed === 1 ? "pixel" : "pixels"} changed (${formatted}%)`
}

/// What a refused side reads in place of its image.
export function refusalText(reason: ImageRefusal): string {
    switch (reason) {
        case "lfs":
            return "Stored in Git LFS"
        case "tooLarge":
            return "Too large to preview"
        case "notImage":
            return "Not an image this view can show"
        case "tooManyPixels":
            return "Too many pixels to preview"
    }
}

/// What a side reads when the web view cannot draw its image.
export const UNDRAWABLE_TEXT = "Can't be shown here"

/// A version's name for assistive technology: its file's path and its side's
/// name ("icons/app.png at abc1234").
export function versionName(path: string, side: string): string {
    return `${path} at ${side}`
}

/// How an image file's versions are shown, always local per-file view state.
export type ImageMode = (typeof IMAGE_MODE_OPTIONS)[number]["value"]

/// The near-view read queue (`diff-view`: *Image Comparison*). A file joins
/// it once its section comes within the margin, in the order sections come
/// near. At most `limit` files are read at once, each next waiting file
/// starting as a read ends. A file whose section leaves the margin before its
/// read starts is dropped, so a section never brought near reads nothing,
/// and a file read once, or being read, never joins again unless the view
/// asks to try it again.
export { NearViewQueue } from "./nearViewQueue"
