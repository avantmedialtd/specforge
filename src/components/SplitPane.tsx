import { useRef, useState, type ReactNode } from "react"
import { ChevronLeft, ChevronRight } from "./icons"

/** One in-flight divider drag, keyed to the pointer that started it. */
interface DragState {
    pointerId: number
    startX: number
    startWidth: number
    /** Latest clamped width, so the end handler can persist it without
     * waiting on a state update. */
    latest: number
}

interface SplitPaneProps {
    left: ReactNode
    /** Center pane — flexes to fill the space between the side panes. */
    right: ReactNode
    /** Optional far-right pane (the commit-graph rail). When present a second
     * divider lets the user resize it; the center pane absorbs the slack. */
    far?: ReactNode
    /** Hide the left pane. The pane and its divider unmount entirely; the
     * width state stays here, so restoring returns the remembered width
     * through the usual clamps. */
    leftHidden?: boolean
    /** Hide the far pane — same contract as `leftHidden`. */
    farHidden?: boolean
    /** Toggle the left pane's visibility. When provided, the visible pane
     * gets a collapse chevron and the hidden pane a floating restore chevron
     * in the center pane's top-left corner. */
    onToggleLeft?: () => void
    /** Toggle the far pane's visibility — restore chevron sits top-right. */
    onToggleFar?: () => void
    initialLeftWidth?: number
    minLeftWidth?: number
    minRightWidth?: number
    initialFarWidth?: number
    minFarWidth?: number
    /** Called with the rail's new width on resize, so the caller can persist
     * it across sessions. */
    onFarWidthChange?: (width: number) => void
}

export function SplitPane({
    left,
    right,
    far,
    leftHidden = false,
    farHidden = false,
    onToggleLeft,
    onToggleFar,
    initialLeftWidth = 340,
    minLeftWidth = 180,
    minRightWidth = 320,
    initialFarWidth = 260,
    minFarWidth = 140,
    onFarWidthChange,
}: SplitPaneProps) {
    const [leftWidth, setLeftWidth] = useState(initialLeftWidth)
    const [farWidth, setFarWidth] = useState(initialFarWidth)
    const containerRef = useRef<HTMLDivElement>(null)
    const hasFar = far != null
    const showLeft = !leftHidden
    const showFar = hasFar && !farHidden

    const containerWidth = () => containerRef.current?.clientWidth ?? 800

    // Shared clamp limits — the keyboard path resizes through the same
    // bounds as a pointer drag. The center pane always keeps minRightWidth.
    // Hidden panes contribute no width, so the remaining visible pane may
    // grow into their space.
    const maxLeftWidth = () =>
        Math.max(
            minLeftWidth,
            containerWidth() - minRightWidth - (showFar ? farWidth : 0),
        )
    const maxFarWidth = () =>
        Math.max(
            minFarWidth,
            containerWidth() - (showLeft ? leftWidth : 0) - minRightWidth,
        )

    // Divider drags run on pointer events, so a mouse, a touch contact, and a
    // pen all drive the same path through the same clamps. The divider
    // captures the pointer on down, which is what lets the move/up handlers
    // live on the divider itself rather than on `document`: once captured,
    // events for that pointer are routed to the divider even when the contact
    // travels outside it. `pointercancel` gives the interrupted-gesture path
    // the old mouse implementation never had.
    const leftDrag = useRef<DragState | null>(null)
    const farDrag = useRef<DragState | null>(null)
    // Drives the resize cursor. Tied to the capture lifecycle via React state
    // rather than mutating document.body.style, so an interrupted gesture
    // cannot strand a col-resize cursor over the whole app.
    const [resizing, setResizing] = useState(false)

    const beginDrag = (
        slot: React.MutableRefObject<DragState | null>,
        startWidth: number,
        e: React.PointerEvent<HTMLDivElement>,
    ) => {
        // Secondary mouse buttons never start a drag; touch and pen report 0.
        if (e.button !== 0) return
        e.preventDefault()
        // Capture can throw when the pointer is no longer active. A drag
        // without it still tracks within the divider, so a failure here must
        // never abort the handler and leave the divider dead.
        try {
            e.currentTarget.setPointerCapture(e.pointerId)
        } catch {
            /* proceed uncaptured */
        }
        slot.current = {
            pointerId: e.pointerId,
            startX: e.clientX,
            startWidth,
            latest: startWidth,
        }
        setResizing(true)
    }

    /** Returns the live drag when this event belongs to it, else null. */
    const activeDrag = (
        slot: React.MutableRefObject<DragState | null>,
        e: React.PointerEvent<HTMLDivElement>,
    ) => {
        const drag = slot.current
        return drag && drag.pointerId === e.pointerId ? drag : null
    }

    const endDrag = (
        slot: React.MutableRefObject<DragState | null>,
        e: React.PointerEvent<HTMLDivElement>,
    ) => {
        const drag = activeDrag(slot, e)
        if (!drag) return null
        slot.current = null
        setResizing(false)
        if (e.currentTarget.hasPointerCapture(e.pointerId)) {
            e.currentTarget.releasePointerCapture(e.pointerId)
        }
        return drag
    }

    const onLeftPointerDown = (e: React.PointerEvent<HTMLDivElement>) =>
        beginDrag(leftDrag, leftWidth, e)

    const onLeftPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
        const drag = activeDrag(leftDrag, e)
        if (!drag) return
        const proposed = drag.startWidth + (e.clientX - drag.startX)
        const max = maxLeftWidth()
        setLeftWidth(Math.min(max, Math.max(minLeftWidth, proposed)))
    }

    const onLeftPointerEnd = (e: React.PointerEvent<HTMLDivElement>) => {
        endDrag(leftDrag, e)
    }

    const onFarPointerDown = (e: React.PointerEvent<HTMLDivElement>) =>
        beginDrag(farDrag, farWidth, e)

    const onFarPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
        const drag = activeDrag(farDrag, e)
        if (!drag) return
        // Dragging the divider left (negative delta) grows the rail.
        const proposed = drag.startWidth + (drag.startX - e.clientX)
        const next = Math.min(maxFarWidth(), Math.max(minFarWidth, proposed))
        drag.latest = next
        setFarWidth(next)
    }

    const onFarPointerEnd = (e: React.PointerEvent<HTMLDivElement>) => {
        // Persist on cancel as well as on up: the rail is already at this
        // width, so dropping the notification would desync the stored value.
        const drag = endDrag(farDrag, e)
        if (drag) onFarWidthChange?.(drag.latest)
    }

    // Keyboard resize: a focused divider moves by a fixed step per arrow
    // press (Shift for the coarse step), through the same clamps as a drag.
    const KEY_STEP = 16
    const KEY_STEP_COARSE = 64

    const handleLeftKeyDown = (e: React.KeyboardEvent) => {
        const step = e.shiftKey ? KEY_STEP_COARSE : KEY_STEP
        const delta =
            e.key === "ArrowRight" ? step : e.key === "ArrowLeft" ? -step : 0
        if (delta === 0) return
        e.preventDefault()
        const next = Math.min(
            maxLeftWidth(),
            Math.max(minLeftWidth, leftWidth + delta),
        )
        // Apply only when the clamped result moves in the pressed direction.
        // When the live max sits below the current width (narrow windows),
        // a "grow" press must be a no-op — not a surprise jump the wrong way.
        if ((next - leftWidth) * delta > 0) setLeftWidth(next)
    }

    const handleFarKeyDown = (e: React.KeyboardEvent) => {
        const step = e.shiftKey ? KEY_STEP_COARSE : KEY_STEP
        // The far divider sits left of the rail, so ArrowLeft grows it —
        // matching the drag direction.
        const delta =
            e.key === "ArrowLeft" ? step : e.key === "ArrowRight" ? -step : 0
        if (delta === 0) return
        e.preventDefault()
        const next = Math.min(
            maxFarWidth(),
            Math.max(minFarWidth, farWidth + delta),
        )
        if ((next - farWidth) * delta > 0) {
            setFarWidth(next)
            onFarWidthChange?.(next)
        }
    }

    return (
        <div
            ref={containerRef}
            className={
                resizing ? "split-pane split-pane--resizing" : "split-pane"
            }
        >
            {showLeft && (
                <div className="split-pane-left" style={{ width: leftWidth }}>
                    {left}
                    {onToggleLeft && (
                        <button
                            className="pane-toggle pane-collapse-left"
                            onClick={onToggleLeft}
                            aria-label="Hide sidebar"
                            title="Hide sidebar"
                        >
                            <ChevronLeft width={16} height={16} />
                        </button>
                    )}
                </div>
            )}
            {showLeft && (
                <div
                    className="split-pane-divider"
                    onPointerDown={onLeftPointerDown}
                    onPointerMove={onLeftPointerMove}
                    onPointerUp={onLeftPointerEnd}
                    onPointerCancel={onLeftPointerEnd}
                    onKeyDown={handleLeftKeyDown}
                    role="separator"
                    aria-orientation="vertical"
                    aria-label="Resize sidebar"
                    aria-valuenow={Math.round(leftWidth)}
                    aria-valuemin={minLeftWidth}
                    // Floored at the current width: before the container ref
                    // mounts (and at narrow windows) the computed max can sit
                    // below the actual width, and valuenow > valuemax is invalid
                    // ARIA. Live re-clamping on window resize is a follow-up.
                    aria-valuemax={Math.round(Math.max(leftWidth, maxLeftWidth()))}
                    tabIndex={0}
                />
            )}
            <div className="split-pane-right">
                {!showLeft && onToggleLeft && (
                    <button
                        className="pane-toggle pane-restore-left"
                        onClick={onToggleLeft}
                        aria-label="Show sidebar"
                        title="Show sidebar"
                    >
                        <ChevronRight width={16} height={16} />
                    </button>
                )}
                {hasFar && !showFar && onToggleFar && (
                    <button
                        className="pane-toggle pane-restore-far"
                        onClick={onToggleFar}
                        aria-label="Show commit rail"
                        title="Show commit rail"
                    >
                        <ChevronLeft width={16} height={16} />
                    </button>
                )}
                {right}
            </div>
            {showFar && (
                <>
                    <div
                        className="split-pane-divider"
                        onPointerDown={onFarPointerDown}
                        onPointerMove={onFarPointerMove}
                        onPointerUp={onFarPointerEnd}
                        onPointerCancel={onFarPointerEnd}
                        onKeyDown={handleFarKeyDown}
                        role="separator"
                        aria-orientation="vertical"
                        aria-label="Resize commit rail"
                        aria-valuenow={Math.round(farWidth)}
                        aria-valuemin={minFarWidth}
                        aria-valuemax={Math.round(
                            Math.max(farWidth, maxFarWidth()),
                        )}
                        tabIndex={0}
                    />
                    <div className="split-pane-far" style={{ width: farWidth }}>
                        {far}
                        {onToggleFar && (
                            <button
                                className="pane-toggle pane-collapse-far"
                                onClick={onToggleFar}
                                aria-label="Hide commit rail"
                                title="Hide commit rail"
                            >
                                <ChevronRight width={16} height={16} />
                            </button>
                        )}
                    </div>
                </>
            )}
        </div>
    )
}
