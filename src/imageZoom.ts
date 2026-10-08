/// The maximized image comparison's own decisions (`diff-view`: *Image
/// Comparison*, Zooming; design D11), over `figureZoom.ts`'s arithmetic. The
/// two frames move in lockstep, so the model is exactly the figure view's:
/// one viewport over one content box, here the box that holds both versions
/// aligned at their top-left corners. Pure, for the reason `diffLayout.ts`
/// gives in its header.

import type { Extents, ZoomState } from "./components/figureZoom"
import { clampScale, fitScale } from "./components/figureZoom"

/// How far a maximized comparison enlarges: a 16-pixel icon at 32 times is
/// 512 pixels, where the figure view's 8 would leave it at 128.
export const IMAGE_MAX_SCALE = 32

/// The box that holds every version, aligned at their top-left corners.
export function unionExtents(versions: readonly Extents[]): Extents {
    return {
        width: Math.max(0, ...versions.map((version) => version.width)),
        height: Math.max(0, ...versions.map((version) => version.height)),
    }
}

/// Where the maximized comparison opens: every version wholly visible, the
/// scale held within its bounds, so a tiny icon opens at the ceiling rather
/// than past it.
export function openingState(viewport: Extents, content: Extents, padding: number): ZoomState {
    const fit = fitScale(viewport, content, padding)
    return { scale: clampScale(fit, fit, IMAGE_MAX_SCALE), left: 0, top: 0 }
}

/// Whether a version is drawn with its pixels kept square and sharp: above
/// actual size, where smoothing would blur exactly what a reviewer checks.
export function sharpPixels(scale: number): boolean {
    return scale > 1
}

/// How strongly a pinch, as a wheel event, zooms per pixel of `deltaY`.
const PINCH_SENSITIVITY = 0.01
/// The most one wheel event moves the scale, in pixels of `deltaY`, so a
/// mouse wheel's notch with Control steps rather than leaps.
const PINCH_STEP_PIXELS = 50
/// What a line and a page of `deltaY` count as, in pixels.
const LINE_PIXELS = 16
const PAGE_PIXELS = 800

/// The zoom factor a wheel event with Control asks for: how Chromium and
/// Firefox deliver a trackpad pinch, and what a mouse wheel with Control
/// gives. A positive `deltaY` zooms out. `deltaMode` is the event's own: 0
/// pixels, 1 lines, 2 pages.
export function pinchWheelFactor(deltaY: number, deltaMode: number): number {
    const unit = deltaMode === 1 ? LINE_PIXELS : deltaMode === 2 ? PAGE_PIXELS : 1
    const pixels = Math.max(-PINCH_STEP_PIXELS, Math.min(PINCH_STEP_PIXELS, deltaY * unit))
    return Number.isFinite(pixels) ? Math.exp(-pixels * PINCH_SENSITIVITY) : 1
}

/// What a zoom key asks for, as Preview binds them: zoom in, zoom out,
/// actual size, or fit.
export type ZoomKey = "in" | "out" | "actual" | "fit"

/// The zoom a key press asks for, or null: Command on macOS, Control
/// elsewhere, with `+` or `=` to zoom in, `-` to zoom out, `0` for actual size
/// and `9` to fit. Option or Alt makes it no zoom key.
export function zoomKeyOf(
    event: { key: string; metaKey: boolean; ctrlKey: boolean; altKey: boolean },
    userAgent: string,
): ZoomKey | null {
    const command = /Mac/i.test(userAgent) ? event.metaKey : event.ctrlKey
    if (!command || event.altKey) return null
    switch (event.key) {
        case "+":
        case "=":
            return "in"
        case "-":
        case "_":
            return "out"
        case "0":
            return "actual"
        case "9":
            return "fit"
        default:
            return null
    }
}
