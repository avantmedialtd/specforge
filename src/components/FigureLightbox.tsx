import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react"
import { createPortal } from "react-dom"
import type { PointerEvent as ReactPointerEvent, ReactNode } from "react"
import { Close, FitToView, ZoomIn, ZoomOut } from "./icons"
import type { Extents, Point, ZoomState } from "./figureZoom"
import { actualSizeState, fitState, panBy, pinchFactor, zoomAt } from "./figureZoom"

/// Breathing room held around the figure at fit scale, in CSS pixels. Kept
/// numerically in step with `.figure-lightbox__viewport`'s padding in
/// App.css — the fit maths needs the number, the surface needs the rule, and
/// a mismatch would let the fitted figure graze the surface's edge.
const PADDING = 24

/// Wheel-delta-to-scale sensitivity. Exponential rather than linear so a
/// zoom step is proportional at every scale: one notch out always undoes one
/// notch in, whether the figure is at 20% or 400%.
const WHEEL_SENSITIVITY = 0.002

/// Step for the toolbar's zoom controls. Reciprocals, so the two buttons are
/// exact inverses of each other.
const BUTTON_STEP = 1.25

/// The figure's own intrinsic extents, read from whichever element the
/// caller handed us.
///
/// The two paths differ by construction (design.md: *Decision 3*): a
/// `mermaid` diagram is live SVG in the document, whose `viewBox` states its
/// user-unit extents; an `svg` fence is an `<img>` over a data URI, whose
/// `naturalWidth`/`naturalHeight` state its rasterizable size. Neither is
/// the element's *laid-out* size, which is what we are about to set.
///
/// Returns null when the figure cannot be measured yet — an `<img>` whose
/// data URI has not decoded — so the caller can wait for its load rather
/// than fit to a zero-area box.
function intrinsicExtents(root: HTMLElement): Extents | null {
    const img = root.querySelector("img")
    if (img !== null) {
        if (!(img.naturalWidth > 0) || !(img.naturalHeight > 0)) return null
        return { width: img.naturalWidth, height: img.naturalHeight }
    }

    const svg = root.querySelector("svg")
    if (svg !== null) {
        const box = svg.viewBox.baseVal
        if (box !== null && box.width > 0 && box.height > 0) {
            return { width: box.width, height: box.height }
        }
        // A diagram with no viewBox still has a laid-out size to fall back
        // on; it is only unusable before first layout.
        const rect = svg.getBoundingClientRect()
        if (rect.width > 0 && rect.height > 0) {
            return { width: rect.width, height: rect.height }
        }
    }

    return null
}

/// True when two extents describe the same box — used to keep a
/// re-measurement that found nothing new from re-rendering the surface.
function sameExtents(a: Extents | null, b: Extents): boolean {
    return a !== null && a.width === b.width && a.height === b.height
}

interface FigureLightboxProps {
    /// Accessible name for the surface, naming what has been maximized.
    label: string
    /// The figure itself. Rendered by the caller — and by the same component
    /// that renders it inline — so a scheme-driven re-render of a diagram
    /// flows straight through here rather than into a stale copy
    /// (design.md: *Decision 1*).
    children: ReactNode
    onClose: () => void
}

/**
 * The maximized figure view (spec-browser → *Maximized Figure View*): one
 * figure on a surface above the whole window, zoomable and pannable.
 *
 * A native `<dialog>` opened with `showModal()`, portalled to the body. The
 * element supplies the top layer, the backdrop, focus trapping, and inert
 * background content — all of which a hand-rolled overlay would have to
 * reproduce (design.md: *Decision 6*).
 *
 * Zoom is expressed as the figure's **layout width**, never as a CSS
 * transform: an SVG inside an `<img>` is rasterized at its used layout size,
 * so a transform would magnify that fixed raster permanently rather than
 * re-rendering it (design.md: *Decision 3*). Scroll offsets accordingly live
 * in the DOM, where the browser already maintains them, and React holds only
 * the scale; every calculation over the two comes from `figureZoom.ts`,
 * which is pure and tested.
 */
export function FigureLightbox({ label, children, onClose }: FigureLightboxProps) {
    const dialogRef = useRef<HTMLDialogElement>(null)
    const viewportRef = useRef<HTMLDivElement>(null)
    const figureRef = useRef<HTMLDivElement>(null)

    const [content, setContent] = useState<Extents | null>(null)
    const [viewport, setViewport] = useState<Extents | null>(null)
    /// Null until the figure has been measured and fitted. Deliberately not
    /// reset when `children` change, so a colour-scheme flip or a live file
    /// edit re-renders the figure in place at the reader's current scale.
    const [scale, setScale] = useState<number | null>(null)

    /// Scroll offsets to apply once a scale change has been laid out. The
    /// width must land first, or the browser clamps the offsets against the
    /// old scroll range.
    const pendingScroll = useRef<{ left: number; top: number } | null>(null)

    /// Live pointer contacts, by `pointerId`, in viewport coordinates. One
    /// contact pans, two pinch — a mouse, a touch contact, and a pen all
    /// arrive here identically (`touch-input`: *Drag Interactions Accept
    /// Pointer Input*).
    const contacts = useRef(new Map<number, Point>())
    const pinch = useRef<[Point, Point] | null>(null)
    /// Set by any pointer movement, so releasing a drag over the surface
    /// background does not read as a click-to-dismiss.
    const dragged = useRef(false)
    /// Whether the gesture began on the surface background rather than on
    /// the figure. `setPointerCapture` retargets the click that follows to
    /// the capturing element, so the click's own `target` cannot be trusted
    /// to say what was pressed — this records it at pointerdown, before
    /// capture is taken.
    const downOnBackground = useRef(false)

    // ---- Open and close the native dialog -----------------------------

    useLayoutEffect(() => {
        const dialog = dialogRef.current
        if (dialog === null) return
        if (!dialog.open) dialog.showModal()
        return () => {
            if (dialog.open) dialog.close()
        }
    }, [])

    // ---- Escape ------------------------------------------------------

    useEffect(() => {
        const dialog = dialogRef.current
        if (dialog === null) return

        const onKeyDown = (event: KeyboardEvent) => {
            if (event.key !== "Escape") return
            // `App.tsx` installs an outermost document-level Escape handler
            // that dismisses the Settings and Archive panes, guarded on
            // `defaultPrevented`. Consume the key here so one Escape closes
            // only this surface and a pane open behind it stays open — the
            // same contract the settings rename input already relies on.
            // preventDefault also suppresses the dialog's own cancel, so
            // closing stays on the single path below.
            event.preventDefault()
            event.stopPropagation()
            onClose()
        }

        dialog.addEventListener("keydown", onKeyDown)
        return () => dialog.removeEventListener("keydown", onKeyDown)
    }, [onClose])

    // ---- Measurement --------------------------------------------------

    const measure = useCallback(() => {
        const root = figureRef.current
        if (root === null) return
        const next = intrinsicExtents(root)
        if (next === null) return
        setContent((previous) => (sameExtents(previous, next) ? previous : next))
    }, [])

    // Re-measure whenever the figure itself changes — a diagram re-rendered
    // for the active scheme, or a fence reparsed after a file edit.
    useLayoutEffect(() => {
        measure()
    }, [measure, children])

    // An `<img>` over a data URI usually decodes before this mounts, since
    // the inline block already loaded the same URI. When it has not, `load`
    // is the signal — captured rather than bubbled, because `load` on an
    // element does not bubble.
    useEffect(() => {
        const root = figureRef.current
        if (root === null) return
        const onLoad = () => measure()
        root.addEventListener("load", onLoad, true)
        return () => root.removeEventListener("load", onLoad, true)
    }, [measure])

    useLayoutEffect(() => {
        const element = viewportRef.current
        if (element === null) return

        const update = () => {
            const next = { width: element.clientWidth, height: element.clientHeight }
            setViewport((previous) => (sameExtents(previous, next) ? previous : next))
        }

        update()
        const observer = new ResizeObserver(update)
        observer.observe(element)
        return () => observer.disconnect()
    }, [])

    // First fit, once both boxes are known. Guarded on `scale === null` so a
    // later re-measure never yanks the reader back to fit.
    useLayoutEffect(() => {
        if (scale !== null || content === null || viewport === null) return
        setScale(fitState(viewport, content, PADDING).scale)
    }, [scale, content, viewport])

    // Apply the offsets a scale change computed, after its width has landed.
    useLayoutEffect(() => {
        const element = viewportRef.current
        const pending = pendingScroll.current
        if (element === null || pending === null) return
        element.scrollLeft = pending.left
        element.scrollTop = pending.top
        pendingScroll.current = null
    }, [scale])

    // ---- Applying zoom and pan ----------------------------------------

    /// Moves to `next`. When the scale is unchanged there is no re-render to
    /// wait for, so the offsets are written straight through — otherwise
    /// they would sit in `pendingScroll` until some later scale change
    /// happened to flush them.
    const applyState = useCallback(
        (next: ZoomState) => {
            const element = viewportRef.current
            if (element === null) return
            if (next.scale === scale) {
                element.scrollLeft = next.left
                element.scrollTop = next.top
                return
            }
            pendingScroll.current = { left: next.left, top: next.top }
            setScale(next.scale)
        },
        [scale],
    )

    const applyZoom = useCallback(
        (factor: number, pointer: Point) => {
            const element = viewportRef.current
            if (element === null || scale === null || content === null || viewport === null) return
            const current: ZoomState = {
                scale,
                left: element.scrollLeft,
                top: element.scrollTop,
            }
            applyState(zoomAt(current, factor, pointer, viewport, content, PADDING))
        },
        [applyState, scale, content, viewport],
    )

    /// The viewport's centre, for zoom that did not come from a pointer.
    const centre = useCallback(
        (): Point => ({
            x: (viewport?.width ?? 0) / 2,
            y: (viewport?.height ?? 0) / 2,
        }),
        [viewport],
    )

    // Wheel is registered natively and non-passively: React attaches its own
    // wheel listener passively, where preventDefault is a no-op and the page
    // would scroll behind the gesture.
    useEffect(() => {
        const element = viewportRef.current
        if (element === null) return

        const onWheel = (event: WheelEvent) => {
            event.preventDefault()
            const rect = element.getBoundingClientRect()
            applyZoom(Math.exp(-event.deltaY * WHEEL_SENSITIVITY), {
                x: event.clientX - rect.left,
                y: event.clientY - rect.top,
            })
        }

        element.addEventListener("wheel", onWheel, { passive: false })
        return () => element.removeEventListener("wheel", onWheel)
    }, [applyZoom])

    // ---- Pointer pan and pinch ----------------------------------------

    const pointAt = useCallback((event: ReactPointerEvent<HTMLDivElement>): Point => {
        const element = viewportRef.current
        if (element === null) return { x: 0, y: 0 }
        const rect = element.getBoundingClientRect()
        return { x: event.clientX - rect.left, y: event.clientY - rect.top }
    }, [])

    const onPointerDown = useCallback(
        (event: ReactPointerEvent<HTMLDivElement>) => {
            // Recorded before capture is taken — afterwards the click's
            // target is this container regardless of what was pressed.
            downOnBackground.current = event.target === event.currentTarget
            event.currentTarget.setPointerCapture(event.pointerId)
            contacts.current.set(event.pointerId, pointAt(event))
            dragged.current = false
            pinch.current = null
        },
        [pointAt],
    )

    const onPointerMove = useCallback(
        (event: ReactPointerEvent<HTMLDivElement>) => {
            if (!contacts.current.has(event.pointerId)) return

            const previous = contacts.current.get(event.pointerId)
            const point = pointAt(event)
            contacts.current.set(event.pointerId, point)

            const live = Array.from(contacts.current.values())

            if (live.length >= 2) {
                const current: [Point, Point] = [live[0], live[1]]
                if (pinch.current !== null) {
                    applyZoom(pinchFactor(pinch.current, current), {
                        x: (current[0].x + current[1].x) / 2,
                        y: (current[0].y + current[1].y) / 2,
                    })
                }
                pinch.current = current
                dragged.current = true
                return
            }

            const element = viewportRef.current
            if (
                element === null ||
                previous === undefined ||
                scale === null ||
                content === null ||
                viewport === null
            ) {
                return
            }

            const next = panBy(
                { scale, left: element.scrollLeft, top: element.scrollTop },
                { x: point.x - previous.x, y: point.y - previous.y },
                viewport,
                content,
            )
            element.scrollLeft = next.left
            element.scrollTop = next.top
            dragged.current = true
        },
        [applyZoom, pointAt, scale, content, viewport],
    )

    const onPointerUp = useCallback((event: ReactPointerEvent<HTMLDivElement>) => {
        contacts.current.delete(event.pointerId)
        if (contacts.current.size < 2) pinch.current = null
        if (event.currentTarget.hasPointerCapture(event.pointerId)) {
            event.currentTarget.releasePointerCapture(event.pointerId)
        }
    }, [])

    // ---- Toolbar ------------------------------------------------------

    const onFit = useCallback(() => {
        if (content === null || viewport === null) return
        applyState(fitState(viewport, content, PADDING))
    }, [applyState, content, viewport])

    const onActualSize = useCallback(() => {
        const element = viewportRef.current
        if (element === null || scale === null || content === null || viewport === null) return
        applyState(
            actualSizeState(
                { scale, left: element.scrollLeft, top: element.scrollTop },
                viewport,
                content,
                PADDING,
            ),
        )
    }, [applyState, scale, content, viewport])

    const figureStyle =
        content !== null && scale !== null ? { width: `${content.width * scale}px` } : undefined

    return createPortal(
        <dialog
            ref={dialogRef}
            className="figure-lightbox"
            aria-label={label}
            // Belt and braces: the keydown handler above already consumes
            // Escape, but a cancel arriving by any other route must not
            // close the dialog behind React's back and desynchronise the
            // caller's state.
            onCancel={(event) => {
                event.preventDefault()
                onClose()
            }}
            onClick={(event) => {
                if (event.target === dialogRef.current) onClose()
            }}
        >
            <div className="figure-lightbox__toolbar">
                <button
                    type="button"
                    className="figure-lightbox__control"
                    onClick={() => applyZoom(1 / BUTTON_STEP, centre())}
                    aria-label="Zoom out"
                    title="Zoom out"
                >
                    <ZoomOut width={16} height={16} />
                </button>
                <span className="figure-lightbox__scale" aria-live="polite">
                    {scale === null ? "—" : `${Math.round(scale * 100)}%`}
                </span>
                <button
                    type="button"
                    className="figure-lightbox__control"
                    onClick={() => applyZoom(BUTTON_STEP, centre())}
                    aria-label="Zoom in"
                    title="Zoom in"
                >
                    <ZoomIn width={16} height={16} />
                </button>
                <button
                    type="button"
                    className="figure-lightbox__control"
                    onClick={onFit}
                    aria-label="Fit to window"
                    title="Fit to window"
                >
                    <FitToView width={16} height={16} />
                </button>
                <button
                    type="button"
                    className="figure-lightbox__control figure-lightbox__control--ratio"
                    onClick={onActualSize}
                    aria-label="Actual size"
                    title="Actual size"
                >
                    1:1
                </button>
                <button
                    type="button"
                    className="figure-lightbox__control"
                    onClick={onClose}
                    aria-label="Close"
                    title="Close"
                >
                    <Close width={16} height={16} />
                </button>
            </div>

            <div
                ref={viewportRef}
                className="figure-lightbox__viewport"
                onPointerDown={onPointerDown}
                onPointerMove={onPointerMove}
                onPointerUp={onPointerUp}
                onPointerCancel={onPointerUp}
                onClick={() => {
                    // Dismiss only a genuine click on the surface around the
                    // figure: it must have started there, and it must not
                    // have been the tail of a pan.
                    if (downOnBackground.current && !dragged.current) onClose()
                }}
            >
                <div
                    ref={figureRef}
                    className={
                        scale === null
                            ? "figure-lightbox__figure figure-lightbox__figure--measuring"
                            : "figure-lightbox__figure"
                    }
                    style={figureStyle}
                >
                    {children}
                </div>
            </div>
        </dialog>,
        document.body,
    )
}
