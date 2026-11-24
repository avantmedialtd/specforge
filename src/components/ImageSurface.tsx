import { useEffect, useRef, useState } from "react"
import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react"
import { pixelDiffSummary, revealAt, revealKey, type DecodedImage, type ImageMode } from "../diffImage"
import { computeImageDiff } from "../imageDiff"
import {
    canHighlight, comparisonSize, IMAGE_DIFF_FAILED_TEXT, IMAGE_DIFF_LIMIT_TEXT, type ImageSize,
} from "../imageDiffPixels"
import type { Side } from "../diffLayout"
import { contentIsVisible } from "../prContentLoading"
import { ObjectImage } from "./ObjectImage"

export type ImageSurfaceMode = Exclude<ImageMode, "both">

/// Fits one version into a common union box, never stretching it to the
/// other's dimensions. Both inline and maximized frames use this geometry.
export function AlignedImage({ image, size, alt, onError }: {
    image: DecodedImage
    size: ImageSize
    alt: string
    onError?: () => void
}) {
    return (
        <div className="image-surface__version" style={{
            width: `${image.width / size.width * 100}%`,
            height: `${image.height / size.height * 100}%`,
        }}>
            <ObjectImage image={image} alt={alt} onError={onError} />
        </div>
    )
}

export function imageSurfaceStyle(size: ImageSize): CSSProperties & { "--image-frame-ratio": string } {
    // A pseudo-element reserves the height even on the supported Safari 14
    // floor, which predates CSS aspect-ratio.
    return { width: `${size.width}px`, maxWidth: "100%", "--image-frame-ratio": `${size.height * 100 / size.width}%` }
}

/// A single-frame comparison, shared between DiffView and ImageZoom. The
/// parent supplies an unscaled fit box or the zoom window's scaled union box.
export function ImageSurface({ before, after, mode, name, onError, onStatus }: {
    before: DecodedImage
    after: DecodedImage
    mode: ImageSurfaceMode
    name: string
    onError?: (side: Side) => void
    /// Inline captions can carry the readout below even a tiny natural-size
    /// icon; zoom frames use the in-frame readout instead.
    onStatus?: (message: string) => void
}) {
    const size = comparisonSize(before, after)
    const [reveal, setReveal] = useState(50)
    const surface = useRef<HTMLDivElement>(null)
    const move = (event: ReactPointerEvent<HTMLDivElement>) => {
        const rect = surface.current?.getBoundingClientRect()
        if (rect) { setReveal(revealAt(event.clientX, rect.left, rect.width)) }
    }
    const layer = (side: Side) => (
        <AlignedImage image={side === "old" ? before : after} size={size}
            alt="" onError={() => onError?.(side)} />
    )
    return (
        <div className="image-surface" ref={surface} data-image-mode={mode}>
            {mode === "diff" ? (
                <PixelDiff before={before} after={after} name={name} onStatus={onStatus} />
            ) : mode === "blink" ? (
                <Blink name={name} before={layer("old")} after={layer("new")} />
            ) : (
                <>
                    <div className="image-surface__layers" role="img" aria-label={name}>
                        <div className="image-surface__layer">
                            {layer(mode === "after" ? "new" : "old")}
                        </div>
                        {mode === "slider" && (
                            <div className="image-surface__layer"
                                style={{ clipPath: `inset(0 ${100 - reveal}% 0 0)` }}>
                                {layer("new")}
                            </div>
                        )}
                    </div>
                    {mode === "slider" && (
                        <div className="image-surface__divider" role="slider" tabIndex={0}
                            aria-label={`Reveal After in ${name}`} aria-orientation="horizontal"
                            aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(reveal)}
                            aria-valuetext={`${Math.round(reveal)}% After`}
                            style={{ left: `${reveal}%` }} data-copy="skip"
                            onClick={event => event.stopPropagation()}
                            onKeyDown={event => {
                                if (event.metaKey || event.ctrlKey || event.altKey) { return }
                                const next = revealKey(event.key, reveal)
                                if (next === null) { return }
                                event.preventDefault()
                                event.stopPropagation()
                                setReveal(next)
                            }}
                            onPointerDown={event => {
                                if (event.button !== 0) { return }
                                event.preventDefault()
                                event.stopPropagation()
                                event.currentTarget.focus({ preventScroll: true })
                                event.currentTarget.setPointerCapture(event.pointerId)
                                move(event)
                            }}
                            onPointerMove={event => {
                                if (!event.currentTarget.hasPointerCapture(event.pointerId)) { return }
                                event.stopPropagation()
                                move(event)
                            }}
                            onPointerUp={event => {
                                event.stopPropagation()
                                if (event.currentTarget.hasPointerCapture(event.pointerId)) {
                                    event.currentTarget.releasePointerCapture(event.pointerId)
                                }
                            }}
                            onPointerCancel={event => {
                                event.stopPropagation()
                                if (event.currentTarget.hasPointerCapture(event.pointerId)) {
                                    event.currentTarget.releasePointerCapture(event.pointerId)
                                }
                            }}
                        >
                            <span className="image-surface__grip" aria-hidden="true">↔</span>
                        </div>
                    )}
                </>
            )}
        </div>
    )
}

function PixelDiff({ before, after, name, onStatus }: {
    before: DecodedImage
    after: DecodedImage
    name: string
    onStatus?: (message: string) => void
}) {
    const canvas = useRef<HTMLCanvasElement>(null)
    const [message, setMessage] = useState("Computing pixel diff…")
    const allowed = canHighlight(before, after)
    useEffect(() => { onStatus?.(allowed ? message : IMAGE_DIFF_LIMIT_TEXT) }, [onStatus, allowed, message])
    useEffect(() => {
        if (!allowed) { return }
        const target = canvas.current
        if (!target) { return }
        const controller = new AbortController()
        setMessage("Computing pixel diff…")
        computeImageDiff(before, after, controller.signal).then(result => {
            if (controller.signal.aborted) { return }
            try {
                target.width = result.width
                target.height = result.height
                const context = target.getContext("2d")
                if (!context) { throw new Error(IMAGE_DIFF_FAILED_TEXT) }
                context.putImageData(new ImageData(result.pixels, result.width, result.height), 0, 0)
                setMessage(`${pixelDiffSummary(result.changed, result.width * result.height)} · changed pixels are red`)
            } catch { setMessage(IMAGE_DIFF_FAILED_TEXT) }
        }, () => {
            if (!controller.signal.aborted) { setMessage(IMAGE_DIFF_FAILED_TEXT) }
        })
        return () => {
            controller.abort()
            target.width = 0
            target.height = 0
        }
    }, [before, after, allowed])
    return (
        <>
            {allowed && <canvas ref={canvas} className="image-surface__pixels" role="img" aria-label={name} />}
            {!onStatus && <div className="image-surface__status" role="status" data-copy-line="">
                {allowed ? message : IMAGE_DIFF_LIMIT_TEXT}
            </div>}
        </>
    )
}

/// Unlike CSS animation, an explicit timer can be stopped for retained hidden
/// PR sections, reduced motion, an explicit Pause, and unmount.
function Blink({ name, before, after }: {
    name: string
    before: React.ReactNode
    after: React.ReactNode
}) {
    const root = useRef<HTMLDivElement>(null)
    const [showAfter, setShowAfter] = useState(false)
    const [paused, setPaused] = useState(() =>
        typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches === true,
    )
    const [visible, setVisible] = useState(typeof IntersectionObserver === "undefined")
    const [documentVisible, setDocumentVisible] = useState(() =>
        typeof document === "undefined" || document.visibilityState !== "hidden",
    )
    useEffect(() => {
        const element = root.current
        const observer = typeof IntersectionObserver === "undefined" ? null : new IntersectionObserver(entries => {
            setVisible(entries.some(entry => entry.isIntersecting))
        })
        if (element) { observer?.observe(element) }
        const changed = () => setDocumentVisible(document.visibilityState !== "hidden")
        const motion = window.matchMedia?.("(prefers-reduced-motion: reduce)")
        const reduced = () => { if (motion?.matches) { setPaused(true) } }
        document.addEventListener("visibilitychange", changed)
        motion?.addEventListener("change", reduced)
        return () => {
            observer?.disconnect()
            document.removeEventListener("visibilitychange", changed)
            motion?.removeEventListener("change", reduced)
        }
    }, [])
    useEffect(() => {
        if (paused || !visible || !documentVisible) { return }
        const timer = setInterval(() => {
            if (root.current && contentIsVisible(root.current)) { setShowAfter(value => !value) }
        }, 500)
        return () => clearInterval(timer)
    }, [paused, visible, documentVisible])
    return (
        <div ref={root} className="image-surface__layers">
            <div className="image-surface__layers" role="img" aria-label={`${name}: ${showAfter ? "After" : "Before"}`}>
                <div className="image-surface__layer" style={{ visibility: showAfter ? "hidden" : "visible" }}>{before}</div>
                <div className="image-surface__layer" style={{ visibility: showAfter ? "visible" : "hidden" }}>{after}</div>
            </div>
            <div className="image-surface__blink-controls" data-copy="skip"
                onPointerDown={event => event.stopPropagation()} onClick={event => event.stopPropagation()}>
                <span>{showAfter ? "After" : "Before"}</span>
                <button type="button" className="diff-image-zoom" onClick={() => setPaused(value => !value)}>
                    {paused ? "Resume" : "Pause"}
                </button>
            </div>
        </div>
    )
}
