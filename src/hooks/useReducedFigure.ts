import { useEffect, useState } from "react"
import type { RefObject } from "react"

/// Sub-pixel slack. A figure laid out at exactly its natural width can measure
/// a hair under it (fractional layout, a scrollbar's rounding), and treating
/// that as "reduced" would light the affordance on every figure.
const SLACK_PX = 1

/// The figure element inside `frame`, scoped to the two containers that hold
/// one. Deliberately NOT a bare `svg, img` query on the frame: the maximize
/// button's own icon is an `<svg>`, and in the image path it is the FIRST svg
/// in document order — so the loose query measures a 24px icon and concludes
/// every SVG figure is enormous and unreduced.
function figureIn(frame: HTMLElement): SVGSVGElement | HTMLImageElement | null {
    return frame.querySelector(".mermaid-block > svg, .svg-block > img")
}

/// The natural (authored) width of `figure`, or null when it isn't knowable
/// yet. An `<svg>` carries it in its viewBox — the coordinate system the
/// diagram was laid out in — and an `<img>` in `naturalWidth`, which is 0
/// until the image has actually decoded.
function naturalWidthOf(
    figure: SVGSVGElement | HTMLImageElement,
): number | null {
    if (figure instanceof HTMLImageElement) {
        return figure.naturalWidth > 0 ? figure.naturalWidth : null
    }
    const vb = figure.viewBox.baseVal
    return vb && vb.width > 0 ? vb.width : null
}

/**
 * Tracks whether the figure inside `frame` is being displayed smaller than the
 * size it was authored at — scaled down to fit the pane, or held at the
 * mermaid legibility floor and scrolling inside its block.
 *
 * A reduced figure is exactly the one whose reader needs the escape to full
 * size, so `spec-browser`'s *Maximized Figure View* requires its maximize
 * control to be visible at rest rather than on hover. This hook is shared by
 * both figure paths (`MermaidBlock`, `SvgBlock`) so a shrunken SVG image earns
 * that treatment the same way a shrunken diagram does.
 *
 * Measurement, not arithmetic: the pane can be resized, the sidebar toggled,
 * or the image decoded late, and each changes the answer — hence the
 * ResizeObserver rather than a one-shot read after render. `contentKey` is the
 * identity of what the frame currently holds (the diagram source, the image
 * src): re-measuring is keyed to it, because a new figure inside an
 * unchanged-size frame resizes nothing and would otherwise keep the previous
 * figure's verdict.
 */
export function useReducedFigure(
    frameRef: RefObject<HTMLElement | null>,
    contentKey: string,
): boolean {
    const [reduced, setReduced] = useState(false)

    useEffect(() => {
        const frame = frameRef.current
        if (!frame) {
            setReduced(false)
            return
        }

        // React bails out of the re-render when the value is unchanged, so
        // this can be called on every observer tick without guarding.
        const measure = () => {
            const figure = figureIn(frame)
            // Clear rather than bail: a frame with no figure in it is not a
            // reduced figure, and a bare return would latch the last verdict.
            if (!figure) {
                setReduced(false)
                return
            }
            const natural = naturalWidthOf(figure)
            const rendered = figure.getBoundingClientRect().width
            setReduced(
                natural !== null &&
                    rendered > 0 &&
                    rendered < natural - SLACK_PX,
            )
        }

        // Measured once synchronously, then again on every resize. The
        // synchronous pass is the load-bearing one: a diagram held at the
        // legibility floor overflows INSIDE its block, so the frame's own box
        // never changes size and a purely observer-driven verdict would wait
        // for a resize that never comes.
        measure()
        const observer = new ResizeObserver(measure)
        observer.observe(frame)
        // The <img> path decodes asynchronously: its natural width is 0 at
        // mount and no resize of the frame follows the decode. Measuring again
        // on load is what lets a reduced SVG image resolve at all.
        const img = frame.querySelector("img")
        img?.addEventListener("load", measure)

        return () => {
            observer.disconnect()
            img?.removeEventListener("load", measure)
        }
    }, [frameRef, contentKey])

    return reduced
}
