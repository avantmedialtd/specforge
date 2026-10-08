import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react"
import type { PointerEvent as ReactPointerEvent, ReactNode } from "react"
import type { DecodedImage, ImageMode } from "../diffImage"
import {
    IMAGE_MAX_SCALE,
    openingState,
    pinchWheelFactor,
    sharpPixels,
    unionExtents,
    zoomKeyOf,
} from "../imageZoom"
import { ChoiceGroup, type ChoiceOption } from "./ChoiceGroup"
import type { Extents, Point, ZoomState } from "./figureZoom"
import { actualSizeState, panBy, pinchFactor, zoomAt } from "./figureZoom"
import { Close, FitToView, ZoomIn, ZoomOut } from "./icons"
import { ObjectImage } from "./ObjectImage"

/// Breathing room around the versions at fit scale, as the figure view keeps.
const PADDING = 24
/// The toolbar's and the keys' zoom step; in and out are exact inverses.
const BUTTON_STEP = 1.25

/// One frame of the zoom window: a version, what a version shows instead of
/// its image, or the Difference of two.
export type ZoomFrame =
    | {
          kind: "image"
          image: DecodedImage
          name: string
          caption: ReactNode
          onError?: () => void
      }
    | { kind: "note"; text: string; caption: ReactNode }
    | {
          kind: "difference"
          old: DecodedImage
          next: DecodedImage
          name: string
          caption: ReactNode
      }

interface ImageZoomProps {
    /// The file's path, the toolbar's title.
    label: string
    /// Old first, then new; or the one version; or the Difference frame.
    frames: readonly ZoomFrame[]
    /// The window's Both/Difference choice, when Difference is offered.
    mode: {
        value: ImageMode
        options: readonly ChoiceOption<ImageMode>[]
        onChange: (mode: ImageMode) => void
    } | null
    onClose: () => void
}

/// The images a frame draws.
function imagesOf(frame: ZoomFrame): DecodedImage[] {
    switch (frame.kind) {
        case "image":
            return [frame.image]
        case "difference":
            return [frame.old, frame.next]
        case "note":
            return []
    }
}

/// WebKit's pinch, as the desktop's web view and Safari deliver it: a scale
/// running from 1 since the gesture began, at a point in the window.
interface GestureLike extends UIEvent {
    scale: number
    clientX: number
    clientY: number
}

/**
 * An image file's versions in the zoom window (`diff-view`: *Image
 * Comparison*, Zooming; design D11), zoomed and panned together under the
 * platform's gestures.
 *
 * Zoom is one scale applied as layout size, and offsets are scroll offsets,
 * through `figureZoom.ts`, as in the figure view. What it adds is lockstep:
 * every frame scrolls over the same content box, the one that holds every
 * version aligned at its top-left corner, so one scale and one pair of offsets
 * describe them all, each offset is written to every frame, and a frame
 * scrolled by any other means is followed by the others.
 *
 * Gestures are the platform's, as in Preview. Two-finger scrolling and the
 * wheel are left to the frames' native scrolling, momentum and all. A pinch
 * zooms at the pointer: WebKit delivers it as `gesturechange`, Chromium and
 * Firefox as a wheel with `ctrlKey`. Dragging pans, two contacts pinch, and
 * Command (Control elsewhere) with `+`, `-`, `0` and `9` zooms in, out, to
 * actual size and to fit.
 */
export function ImageZoom({ label, frames, mode, onClose }: ImageZoomProps) {
    const viewportRefs = useRef<(HTMLDivElement | null)[]>([])

    const union = unionExtents(frames.flatMap(imagesOf))
    // One object per size, so a re-render re-registers nothing.
    const content = useMemo<Extents>(
        () => ({ width: union.width, height: union.height }),
        [union.width, union.height],
    )
    const [viewport, setViewport] = useState<Extents | null>(null)
    /// Null until the frames have been measured and fitted.
    const [scale, setScale] = useState<number | null>(null)
    /// Offsets to write once a scale change has been laid out.
    const pendingScroll = useRef<{ left: number; top: number } | null>(null)

    const contacts = useRef(new Map<number, Point>())
    const pinch = useRef<[Point, Point] | null>(null)

    // ---- Measuring ------------------------------------------------------

    // Every frame is the same size, so the first one measures them all. It
    // keeps its element across a change of mode, being the first either way.
    useLayoutEffect(() => {
        const element = viewportRefs.current[0]
        if (!element) return
        const update = () => {
            const next = { width: element.clientWidth, height: element.clientHeight }
            setViewport((previous) =>
                previous !== null &&
                previous.width === next.width &&
                previous.height === next.height
                    ? previous
                    : next,
            )
        }
        update()
        const observer = new ResizeObserver(update)
        observer.observe(element)
        return () => observer.disconnect()
    }, [frames.length])

    // The first fit, once both boxes are known; a later change of mode or
    // size keeps the reader's scale.
    useLayoutEffect(() => {
        if (scale !== null || viewport === null || content.width === 0) return
        setScale(openingState(viewport, content, PADDING).scale)
    }, [scale, viewport, content])

    // The first frame takes the focus, so the arrow keys, Space and the page
    // keys scroll the versions from the start.
    useEffect(() => {
        viewportRefs.current[0]?.focus({ preventScroll: true })
    }, [])

    // ---- Lockstep -------------------------------------------------------

    const writeOffsets = useCallback((left: number, top: number) => {
        for (const element of viewportRefs.current) {
            if (element === null) continue
            element.scrollLeft = left
            element.scrollTop = top
        }
    }, [])

    useLayoutEffect(() => {
        const pending = pendingScroll.current
        if (pending === null) return
        pendingScroll.current = null
        writeOffsets(pending.left, pending.top)
    }, [scale, writeOffsets])

    /// A frame scrolled by the trackpad, the wheel, a scrollbar or the
    /// keyboard: every other frame follows. Setting an equal offset fires no
    /// scroll, so the frames cannot echo one another.
    const follow = useCallback((source: HTMLDivElement) => {
        for (const element of viewportRefs.current) {
            if (element === null || element === source) continue
            if (element.scrollLeft !== source.scrollLeft) element.scrollLeft = source.scrollLeft
            if (element.scrollTop !== source.scrollTop) element.scrollTop = source.scrollTop
        }
    }, [])

    const current = useCallback((): ZoomState | null => {
        const element = viewportRefs.current[0]
        if (!element || scale === null) return null
        return { scale, left: element.scrollLeft, top: element.scrollTop }
    }, [scale])

    const applyState = useCallback(
        (next: ZoomState) => {
            if (next.scale === scale) {
                writeOffsets(next.left, next.top)
                return
            }
            pendingScroll.current = { left: next.left, top: next.top }
            setScale(next.scale)
        },
        [scale, writeOffsets],
    )

    const applyZoom = useCallback(
        (factor: number, pointer: Point) => {
            const state = current()
            if (state === null || viewport === null) return
            applyState(zoomAt(state, factor, pointer, viewport, content, PADDING, IMAGE_MAX_SCALE))
        },
        [applyState, current, viewport, content],
    )

    const centre = useCallback(
        (): Point => ({ x: (viewport?.width ?? 0) / 2, y: (viewport?.height ?? 0) / 2 }),
        [viewport],
    )

    const fit = useCallback(() => {
        if (viewport === null) return
        applyState(openingState(viewport, content, PADDING))
    }, [applyState, viewport, content])

    const actualSize = useCallback(() => {
        const state = current()
        if (state === null || viewport === null) return
        applyState(actualSizeState(state, viewport, content, PADDING, IMAGE_MAX_SCALE))
    }, [applyState, current, viewport, content])

    // The listeners below are registered once and reach the latest scale
    // through these, so a pinch in progress keeps its running scale across
    // the renders its own steps cause.
    const zoomRef = useRef(applyZoom)
    zoomRef.current = applyZoom
    const keysRef = useRef({ zoomIn: () => {}, zoomOut: () => {}, fit, actualSize })
    keysRef.current = {
        zoomIn: () => applyZoom(BUTTON_STEP, centre()),
        zoomOut: () => applyZoom(1 / BUTTON_STEP, centre()),
        fit,
        actualSize,
    }

    /// The frame under a point in the window, and the point in that frame.
    const frameAt = useCallback((x: number, y: number): Point | null => {
        const frames = viewportRefs.current.filter((element) => element !== null)
        const under =
            frames.find((element) => {
                const rect = element.getBoundingClientRect()
                return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom
            }) ?? frames[0]
        if (!under) return null
        const rect = under.getBoundingClientRect()
        return { x: x - rect.left, y: y - rect.top }
    }, [])

    // ---- Gestures -------------------------------------------------------

    // A pinch as Chromium and Firefox deliver it, and the wheel with Control:
    // zoom at the pointer. Any other wheel is left alone, so two-finger
    // scrolling and the mouse wheel pan the frames natively, with momentum.
    // WebKit's pinch arrives as gesture events instead, and while one is in
    // progress a wheel with Control is not counted twice.
    const gesturing = useRef(false)
    useEffect(() => {
        const removers = viewportRefs.current.flatMap((element) => {
            if (element === null) return []
            const onWheel = (event: WheelEvent) => {
                if (!event.ctrlKey) return
                event.preventDefault()
                if (gesturing.current) return
                const rect = element.getBoundingClientRect()
                zoomRef.current(pinchWheelFactor(event.deltaY, event.deltaMode), {
                    x: event.clientX - rect.left,
                    y: event.clientY - rect.top,
                })
            }
            element.addEventListener("wheel", onWheel, { passive: false })
            return [() => element.removeEventListener("wheel", onWheel)]
        })
        return () => removers.forEach((remove) => remove())
    }, [frames.length])

    // WebKit's pinch: each change's scale runs from 1 since the gesture
    // began, so its step is the ratio to the last. Its default, the web
    // view's own magnification, is prevented, so only the versions zoom.
    useEffect(() => {
        let last = 1
        const onStart = (event: Event) => {
            event.preventDefault()
            gesturing.current = true
            last = 1
        }
        const onChange = (event: Event) => {
            event.preventDefault()
            const gesture = event as GestureLike
            if (!(gesture.scale > 0) || !(last > 0)) return
            const point = frameAt(gesture.clientX, gesture.clientY)
            if (point !== null) zoomRef.current(gesture.scale / last, point)
            last = gesture.scale
        }
        const onEnd = (event: Event) => {
            event.preventDefault()
            gesturing.current = false
        }
        const options = { passive: false }
        document.addEventListener("gesturestart", onStart, options)
        document.addEventListener("gesturechange", onChange, options)
        document.addEventListener("gestureend", onEnd, options)
        return () => {
            document.removeEventListener("gesturestart", onStart)
            document.removeEventListener("gesturechange", onChange)
            document.removeEventListener("gestureend", onEnd)
        }
    }, [frameAt])

    // The zoom keys, as Preview binds them. Escape and Command-W close the
    // window through the detached window's own handler.
    useEffect(() => {
        const onKeyDown = (event: KeyboardEvent) => {
            const key = zoomKeyOf(event, navigator.userAgent)
            if (key === null) return
            event.preventDefault()
            const keys = keysRef.current
            if (key === "in") keys.zoomIn()
            else if (key === "out") keys.zoomOut()
            else if (key === "actual") keys.actualSize()
            else keys.fit()
        }
        window.addEventListener("keydown", onKeyDown)
        return () => window.removeEventListener("keydown", onKeyDown)
    }, [])

    // ---- Pointer pan and touch pinch, from any frame ---------------------

    const pointAt = (event: ReactPointerEvent<HTMLDivElement>): Point => {
        const rect = event.currentTarget.getBoundingClientRect()
        return { x: event.clientX - rect.left, y: event.clientY - rect.top }
    }

    const onPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
        if (event.pointerType === "mouse" && event.button !== 0) return
        event.currentTarget.setPointerCapture(event.pointerId)
        contacts.current.set(event.pointerId, pointAt(event))
        pinch.current = null
    }

    const onPointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
        if (!contacts.current.has(event.pointerId)) return
        const previous = contacts.current.get(event.pointerId)
        const point = pointAt(event)
        contacts.current.set(event.pointerId, point)
        const live = Array.from(contacts.current.values())
        if (live.length >= 2) {
            const now: [Point, Point] = [live[0], live[1]]
            if (pinch.current !== null) {
                applyZoom(pinchFactor(pinch.current, now), {
                    x: (now[0].x + now[1].x) / 2,
                    y: (now[0].y + now[1].y) / 2,
                })
            }
            pinch.current = now
            return
        }
        const element = event.currentTarget
        if (previous === undefined || scale === null || viewport === null) return
        const next = panBy(
            { scale, left: element.scrollLeft, top: element.scrollTop },
            { x: point.x - previous.x, y: point.y - previous.y },
            viewport,
            content,
        )
        writeOffsets(next.left, next.top)
    }

    const onPointerUp = (event: ReactPointerEvent<HTMLDivElement>) => {
        contacts.current.delete(event.pointerId)
        if (contacts.current.size < 2) pinch.current = null
        if (event.currentTarget.hasPointerCapture(event.pointerId)) {
            event.currentTarget.releasePointerCapture(event.pointerId)
        }
    }

    // ---- Rendering ------------------------------------------------------

    const sized = (image: DecodedImage) =>
        scale === null
            ? undefined
            : { width: `${image.width * scale}px`, height: `${image.height * scale}px` }

    const sharp = scale !== null && sharpPixels(scale)
    // The zoom keys' modifier as the platform writes it, for the titles.
    const command = /Mac/i.test(navigator.userAgent) ? "⌘" : "Ctrl+"

    return (
        <div className="image-zoom">
            <div className="figure-lightbox__toolbar image-zoom__toolbar">
                <span className="image-zoom__title">{label}</span>
                {mode && (
                    <ChoiceGroup
                        options={mode.options}
                        value={mode.value}
                        onChange={mode.onChange}
                        label={`Compare ${label}`}
                    />
                )}
                <button
                    type="button"
                    className="figure-lightbox__control"
                    onClick={() => keysRef.current.zoomOut()}
                    aria-label="Zoom out"
                    title={`Zoom out (${command}−)`}
                >
                    <ZoomOut width={16} height={16} />
                </button>
                <span className="figure-lightbox__scale" aria-live="polite">
                    {scale === null ? "—" : `${Math.round(scale * 100)}%`}
                </span>
                <button
                    type="button"
                    className="figure-lightbox__control"
                    onClick={() => keysRef.current.zoomIn()}
                    aria-label="Zoom in"
                    title={`Zoom in (${command}+)`}
                >
                    <ZoomIn width={16} height={16} />
                </button>
                <button
                    type="button"
                    className="figure-lightbox__control"
                    onClick={fit}
                    aria-label="Fit to window"
                    title={`Fit to window (${command}9)`}
                >
                    <FitToView width={16} height={16} />
                </button>
                <button
                    type="button"
                    className="figure-lightbox__control figure-lightbox__control--ratio"
                    onClick={actualSize}
                    aria-label="Actual size"
                    title={`Actual size (${command}0)`}
                >
                    1:1
                </button>
                <button
                    type="button"
                    className="figure-lightbox__control"
                    onClick={onClose}
                    aria-label="Close"
                    title="Close (Esc)"
                >
                    <Close width={16} height={16} />
                </button>
            </div>
            <div
                className={sharp ? "image-zoom__frames image-zoom__frames--sharp" : "image-zoom__frames"}
                style={{ gridTemplateColumns: `repeat(${frames.length}, minmax(0, 1fr))` }}
            >
                {frames.map((frame, index) => (
                    <div className="image-zoom__frame" key={index}>
                        <div className="image-zoom__caption">{frame.caption}</div>
                        <div
                            ref={(element) => {
                                viewportRefs.current[index] = element
                            }}
                            className="figure-lightbox__viewport image-zoom__viewport"
                            // Focusable, so the keyboard scrolls the versions.
                            tabIndex={0}
                            onPointerDown={onPointerDown}
                            onPointerMove={onPointerMove}
                            onPointerUp={onPointerUp}
                            onPointerCancel={onPointerUp}
                            onScroll={(event) => follow(event.currentTarget)}
                        >
                            {frame.kind === "note" ? (
                                <p className="image-zoom__note">{frame.text}</p>
                            ) : (
                                scale !== null && (
                                    <div
                                        className="image-zoom__content"
                                        style={{
                                            width: `${content.width * scale}px`,
                                            height: `${content.height * scale}px`,
                                        }}
                                    >
                                        {frame.kind === "image" ? (
                                            <div className="image-zoom__version" style={sized(frame.image)}>
                                                <ObjectImage
                                                    image={frame.image}
                                                    alt={frame.name}
                                                    onError={frame.onError}
                                                />
                                            </div>
                                        ) : (
                                            <div
                                                className="image-zoom__stack"
                                                role="img"
                                                aria-label={frame.name}
                                                style={sized(frame.old)}
                                            >
                                                <ObjectImage image={frame.old} alt="" />
                                                <ObjectImage
                                                    image={frame.next}
                                                    alt=""
                                                    className="image-zoom__blend"
                                                />
                                            </div>
                                        )}
                                    </div>
                                )
                            )}
                        </div>
                    </div>
                ))}
            </div>
        </div>
    )
}
