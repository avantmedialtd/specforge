/// The legibility-floor arithmetic for rendered mermaid diagrams
/// (*Mermaid Diagram Rendering*), kept out of MermaidBlock.tsx so it is
/// unit-testable without a DOM — the same reasoning as `figureZoom.ts`: this
/// repository has no component-test infrastructure and a `src/`-only diff
/// short-circuits the mutation gate, so this module and its tests are the
/// change's only automated coverage. Every export is a total function of its
/// arguments and touches no DOM.

/// The smallest label text a rendered diagram may show, in CSS pixels.
///
/// Fitting a very wide diagram to the pane is only a courtesy while the result
/// stays readable. A ten-node flowchart is ~2580px wide naturally; fitting it
/// into the default rung's 880px column scales it to 0.34, which renders its
/// 15px labels at about 5px — present, but not reading material. Below this
/// floor the diagram stops shrinking and scrolls inside its own block instead.
///
/// The arithmetic below is column-agnostic — `floorWidth` is a function of
/// natural width and label size, not of the column — so the reading-width
/// ladder needs nothing from it. What the ladder changes is how often the
/// floor is reached: at `full` the column is unbounded, so on a wide pane this
/// same diagram is fitted to the surface rather than to 880px and the floor
/// stops being the thing the reader meets.
export const MIN_LABEL_PX = 10

/// The label size assumed when a diagram's own is unreadable. Matches the
/// `fontSize` MermaidBlock hands the engine (`--text-md`), which is what the
/// diagram would have been laid out with.
export const FALLBACK_LABEL_PX = 15

/**
 * The floor width for a diagram of natural width `naturalWidth` whose labels
 * are authored at `labelPx`, in CSS pixels — the width below which it must not
 * be scaled, so that
 *
 *     s_render = max(s_fit, MIN_LABEL_PX / labelPx)
 *
 * holds. Expressed as a width rather than a scale because width is the lever
 * CSS gives us: mermaid stamps `max-width: <natural>px` and the stylesheet
 * supplies the fit, so a `min-width` beneath both is what stops the fit going
 * too far.
 *
 * Returns null when `naturalWidth` is not a usable measurement (a diagram with
 * no viewBox), which is the one state where no floor can be computed and the
 * caller must apply none.
 *
 * The result is clamped to `naturalWidth`: a diagram whose labels are ALREADY
 * below the floor at full size cannot be rescued by scaling, and enlarging it
 * past what its author drew would be a worse answer than leaving it be.
 * Rounded up so the floor never lands a fraction of a pixel short of the
 * label size it is there to guarantee.
 */
export function floorWidth(
    naturalWidth: number,
    labelPx: number,
    minLabelPx: number = MIN_LABEL_PX,
): number | null {
    if (!Number.isFinite(naturalWidth) || naturalWidth <= 0) return null

    // A non-finite or non-positive label size means the measurement failed
    // (an empty diagram, a computed style of "" parsed to NaN); assume the
    // size the engine was told to use rather than propagating the failure.
    const label =
        Number.isFinite(labelPx) && labelPx > 0 ? labelPx : FALLBACK_LABEL_PX

    // Ceil BEFORE the clamp, never after: mermaid viewBox widths are routinely
    // fractional (2579.5 in the audit's own fixture), so rounding the clamped
    // result up would return 2580 for a 2579.5px diagram — past natural width,
    // which is the one thing this function promises not to do. It would also
    // beat mermaid's inline `max-width` in the used-value algorithm and, being
    // wider than natural, would read as "not reduced" to `useReducedFigure`.
    const floor = naturalWidth * (minLabelPx / label)
    return Math.min(naturalWidth, Math.ceil(floor))
}
