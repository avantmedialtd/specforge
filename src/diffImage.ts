/// The diff view's decisions about an image file's two versions (`diff-view`:
/// *Image Comparison*): what a read's reply decodes to, how a version is
/// captioned and named, what a side shows instead of its image, whether the
/// Difference mode is offered, and the queue that reads files as their
/// sections near the view. Pure, for the reason `diffLayout.ts` gives in its
/// header.

import type { ImageRefusal, ImageSide, ImageVersions } from "./types"

/// One version the view may decode: its type, sniffed by the service, the
/// dimensions its header declares, and its bytes.
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
    for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i)
    return bytes
}

/// A side as a read answered it, its bytes decoded once.
export function decodeSide(side: ImageSide): DecodedSide {
    switch (side.kind) {
        case "image":
            return {
                kind: "image",
                image: {
                    mime: side.mime,
                    width: side.width,
                    height: side.height,
                    bytes: decodeBase64(side.data),
                },
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
    if (bytes < KIB) return `${bytes} B`
    const [value, unit] = bytes < MIB ? [bytes / KIB, "KB"] : [bytes / MIB, "MB"]
    const tenths = Math.round(value * 10) / 10
    return tenths < 10 ? `${tenths.toFixed(1)} ${unit}` : `${Math.round(value)} ${unit}`
}

/// The sign a change is written with: the minus sign, a plus, or none for no
/// change.
function signOf(value: number): string {
    if (value < 0) return "−"
    return value > 0 ? "+" : ""
}

/// The change from `oldBytes` to `newBytes`, as bytes and as a whole
/// percentage of the old size ("−47 KB (−14%)"). An empty old version has no
/// percentage.
export function sizeDelta(oldBytes: number, newBytes: number): string {
    const change = newBytes - oldBytes
    const bytes = `${signOf(change)}${formatBytes(Math.abs(change))}`
    if (oldBytes === 0) return bytes
    const percent = Math.round((change / oldBytes) * 100)
    return `${bytes} (${signOf(percent)}${Math.abs(percent)}%)`
}

/// A version's dimensions and size, as its caption gives them after its
/// side's name; the new version of a file whose sides are both images also
/// gives the change in size.
export function imageDetails(image: DecodedImage, before?: DecodedImage): string {
    const details = [`${image.width} × ${image.height}`, formatBytes(image.bytes.length)]
    if (before) details.push(sizeDelta(before.bytes.length, image.bytes.length))
    return details.join(" · ")
}

/// Whether the Difference mode is offered: both sides are images of equal
/// width and equal height.
export function differenceAvailable(versions: DecodedVersions): boolean {
    const { old, new: next } = versions
    return (
        old.kind === "image" &&
        next.kind === "image" &&
        old.image.width === next.image.width &&
        old.image.height === next.image.height
    )
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

/// What the Difference frame's caption says it shows.
export const DIFFERENCE_CAPTION = "Difference: unchanged pixels are black"

/// A version's name for assistive technology: its file's path and its side's
/// name ("icons/app.png at abc1234").
export function versionName(path: string, side: string): string {
    return `${path} at ${side}`
}

/// The Difference frame's name: its file's path and both sides' names.
export function differenceName(path: string, oldSide: string, newSide: string): string {
    return `${path}: difference between ${oldSide} and ${newSide}`
}

/// How an image file's versions are shown: both, or their difference.
export type ImageMode = "both" | "difference"

/// The near-view read queue (`diff-view`: *Image Comparison*). A file joins
/// it once its section comes within the margin, in the order sections come
/// near. At most `limit` files are read at once, each next waiting file
/// starting as a read ends. A file whose section leaves the margin before its
/// read starts is dropped, so a section never brought near reads nothing,
/// and a file read once, or being read, never joins again unless the view
/// asks to try it again.
export class NearViewQueue {
    private waiting: string[] = []
    private readonly reading = new Set<string>()
    private readonly done = new Set<string>()

    constructor(
        private readonly read: (key: string) => Promise<unknown>,
        private readonly limit = 2,
    ) {}

    /// A file's section came within the margin.
    near(key: string): void {
        if (this.waiting.includes(key) || this.reading.has(key) || this.done.has(key)) return
        this.waiting.push(key)
        this.pump()
    }

    /// A file's section left the margin: dropped while it still waits.
    left(key: string): void {
        this.waiting = this.waiting.filter((waiting) => waiting !== key)
    }

    /// Reads a file again, as "Try again" asks, whether or not its section is
    /// near.
    retry(key: string): void {
        this.done.delete(key)
        this.left(key)
        this.waiting.unshift(key)
        this.pump()
    }

    /// The files waiting, in order, and those being read.
    snapshot(): { waiting: string[]; reading: string[] } {
        return { waiting: [...this.waiting], reading: [...this.reading] }
    }

    private pump(): void {
        while (this.reading.size < this.limit && this.waiting.length > 0) {
            const key = this.waiting.shift() as string
            this.reading.add(key)
            void this.read(key)
                .catch(() => undefined)
                .then(() => {
                    this.reading.delete(key)
                    this.done.add(key)
                    this.pump()
                })
        }
    }
}
