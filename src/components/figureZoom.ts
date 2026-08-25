/// Zoom and pan arithmetic for the maximized figure view (*Maximized Figure
/// View*), kept out of FigureLightbox.tsx so it is unit-testable without a
/// DOM — this repository has no component-test infrastructure and a
/// `src/`-only diff short-circuits the mutation gate, so these functions and
/// their tests are the change's only automated coverage (design.md:
/// *Decision 4*). Every export below is a total function of its arguments
/// and touches no DOM.
///
/// The model throughout: a fixed-size **viewport** (the lightbox's content
/// area) scrolled over **content** (the figure) displayed at `scale`. Zoom
/// is expressed as the figure's rendered size rather than a CSS transform,
/// because an SVG inside an `<img>` is rasterized at its used layout size
/// and a transform would magnify that fixed raster permanently (design.md:
/// *Decision 3*). Offsets are therefore ordinary scroll offsets.

/// Extents of a box in CSS pixels.
export interface Extents {
    width: number
    height: number
}

/// A position in CSS pixels — a pointer within the viewport, or a movement
/// delta, depending on the parameter it is passed as.
export interface Point {
    x: number
    y: number
}

/// Scale plus both scroll offsets. `left`/`top` are measured in rendered
/// (post-scale) pixels, exactly like `scrollLeft`/`scrollTop`.
export interface ZoomState {
    scale: number
    left: number
    top: number
}

/// The ceiling in *Maximized Figure View*'s bounded-scale obligation.
/// Zooming in stops here rather than continuing without limit.
export const MAX_SCALE = 8

/// Fallback scale for a degenerate box — zero-area content, a viewport
/// smaller than its own padding, or a non-finite dimension. Actual size is
/// the honest answer when "fit" is not computable; the figure is then
/// governed by the ordinary offset clamps like any other.
const DEGENERATE_SCALE = 1

/// The scale at which `content` is wholly visible inside `viewport` with
/// `padding` on every side — the smaller of the two axis ratios, so neither
/// dimension overflows:
///
///   s_fit = min((W_v - 2p) / W_c, (H_v - 2p) / H_c)
///
/// Returns `DEGENERATE_SCALE` when that is not a positive finite number.
export function fitScale(viewport: Extents, content: Extents, padding: number): number {
    if (!(content.width > 0) || !(content.height > 0)) return DEGENERATE_SCALE

    const byWidth = (viewport.width - 2 * padding) / content.width
    const byHeight = (viewport.height - 2 * padding) / content.height
    const fit = Math.min(byWidth, byHeight)

    return Number.isFinite(fit) && fit > 0 ? fit : DEGENERATE_SCALE
}

/// Bounds `scale` to $[\min(s_{fit}, 1), MAX\_SCALE]$.
///
/// The floor is `min(fit, 1)` rather than `fit` alone: a figure smaller than
/// the viewport has `fit > 1`, and clamping *up* to it would forbid viewing
/// such a figure at its actual size — which the *Maximized Figure View*
/// requirement offers as an explicit control. A non-finite input collapses
/// to the floor rather than propagating NaN into a layout width.
export function clampScale(scale: number, fit: number): number {
    const min = Math.min(fit, 1)
    if (!Number.isFinite(scale)) return min
    return Math.min(Math.max(scale, min), MAX_SCALE)
}

/// The largest scroll offset an axis permits: how far the rendered content
/// extends past the viewport, or zero when it fits inside it.
function maxOffset(contentExtent: number, viewportExtent: number, scale: number): number {
    const overflow = contentExtent * scale - viewportExtent
    return Number.isFinite(overflow) && overflow > 0 ? overflow : 0
}

/// One axis's offset, held inside `[0, maxOffset]`. A non-finite offset
/// collapses to 0 rather than escaping the range.
function clampOffset(
    offset: number,
    contentExtent: number,
    viewportExtent: number,
    scale: number,
): number {
    if (!Number.isFinite(offset)) return 0
    return Math.min(Math.max(offset, 0), maxOffset(contentExtent, viewportExtent, scale))
}

/// Scales by `factor` about `pointer`, holding the point of the figure under
/// the pointer stationary (*Maximized Figure View*: zoom is anchored at the
/// pointer).
///
/// The content coordinate under the pointer is $x = (\ell + c) / s$;
/// requiring it to still sit at `c` after scaling to $s'$ gives
///
///   ℓ' = (s' / s)(ℓ + c) - c
///
/// applied independently per axis. The result is then held inside the valid
/// offset range, so the anchor is exact everywhere except where the figure
/// has run out of travel — at which point holding it would require scrolling
/// past the content's edge.
export function zoomAt(
    state: ZoomState,
    factor: number,
    pointer: Point,
    viewport: Extents,
    content: Extents,
    padding: number,
): ZoomState {
    const fit = fitScale(viewport, content, padding)
    const scale = clampScale(state.scale * factor, fit)

    // A degenerate current scale carries no anchor information — there is no
    // ratio to project the offsets through — so re-fit from the origin
    // rather than dividing by it.
    if (!Number.isFinite(state.scale) || state.scale <= 0) {
        return { scale, left: 0, top: 0 }
    }

    const ratio = scale / state.scale
    return {
        scale,
        left: clampOffset(
            ratio * (state.left + pointer.x) - pointer.x,
            content.width,
            viewport.width,
            scale,
        ),
        top: clampOffset(
            ratio * (state.top + pointer.y) - pointer.y,
            content.height,
            viewport.height,
            scale,
        ),
    }
}

/// Moves the figure by a pointer movement of `delta`, holding both offsets
/// inside their valid ranges so a drag cannot push the figure entirely out
/// of the surface.
///
/// The offsets move *against* the pointer: dragging rightwards pulls the
/// figure rightwards, which is a smaller scroll offset, so `delta` is
/// subtracted rather than added.
export function panBy(
    state: ZoomState,
    delta: Point,
    viewport: Extents,
    content: Extents,
): ZoomState {
    return {
        scale: state.scale,
        left: clampOffset(state.left - delta.x, content.width, viewport.width, state.scale),
        top: clampOffset(state.top - delta.y, content.height, viewport.height, state.scale),
    }
}

/// Euclidean distance between two contacts.
function separation(a: Point, b: Point): number {
    return Math.hypot(a.x - b.x, a.y - b.y)
}

/// The scale factor a two-contact pinch implies between two frames — the
/// ratio of the contacts' separations:
///
///   f = ‖p₁' - p₀'‖ / ‖p₁ - p₀‖
///
/// Returns a neutral factor of 1 when either separation is degenerate
/// (coincident or non-finite contacts), so a pinch that begins with the two
/// contacts on the same pixel cannot divide by zero and drive the scale to a
/// non-finite value.
export function pinchFactor(
    previous: readonly [Point, Point],
    current: readonly [Point, Point],
): number {
    const before = separation(previous[0], previous[1])
    const after = separation(current[0], current[1])

    if (!(before > 0) || !(after > 0)) return 1
    return after / before
}

/// The state that shows `content` wholly inside `viewport`, centred by the
/// offset clamps (both collapse to 0, since nothing overflows at fit scale).
/// This is the maximized view's opening state and the target of its fit
/// control.
export function fitState(viewport: Extents, content: Extents, padding: number): ZoomState {
    return { scale: fitScale(viewport, content, padding), left: 0, top: 0 }
}

/// The state that shows `content` at actual size, centred on whatever the
/// current view was looking at — the target of the actual-size control.
/// Implemented as a zoom about the viewport's centre so the reader's focus
/// is preserved rather than jumping to the origin.
export function actualSizeState(
    state: ZoomState,
    viewport: Extents,
    content: Extents,
    padding: number,
): ZoomState {
    if (!Number.isFinite(state.scale) || state.scale <= 0) {
        return { scale: clampScale(1, fitScale(viewport, content, padding)), left: 0, top: 0 }
    }
    const centre = { x: viewport.width / 2, y: viewport.height / 2 }
    return zoomAt(state, 1 / state.scale, centre, viewport, content, padding)
}
