import { useRef, useState, type ReactNode } from "react"

interface SplitPaneProps {
    left: ReactNode
    /** Center pane — flexes to fill the space between the side panes. */
    right: ReactNode
    /** Optional far-right pane (the commit-graph rail). When present a second
     * divider lets the user resize it; the center pane absorbs the slack. */
    far?: ReactNode
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

    const containerWidth = () => containerRef.current?.clientWidth ?? 800

    // Shared clamp limits — the keyboard path resizes through the same
    // bounds as a pointer drag. The center pane always keeps minRightWidth.
    const maxLeftWidth = () =>
        Math.max(
            minLeftWidth,
            containerWidth() - minRightWidth - (hasFar ? farWidth : 0),
        )
    const maxFarWidth = () =>
        Math.max(minFarWidth, containerWidth() - leftWidth - minRightWidth)

    const startLeftDrag = (downEvent: React.MouseEvent) => {
        downEvent.preventDefault()
        const startX = downEvent.clientX
        const startWidth = leftWidth

        const onMove = (ev: MouseEvent) => {
            const proposed = startWidth + (ev.clientX - startX)
            const max = maxLeftWidth()
            setLeftWidth(Math.min(max, Math.max(minLeftWidth, proposed)))
        }
        const onUp = () => {
            document.removeEventListener("mousemove", onMove)
            document.removeEventListener("mouseup", onUp)
            document.body.style.cursor = ""
        }
        document.addEventListener("mousemove", onMove)
        document.addEventListener("mouseup", onUp)
        document.body.style.cursor = "col-resize"
    }

    const startFarDrag = (downEvent: React.MouseEvent) => {
        downEvent.preventDefault()
        const startX = downEvent.clientX
        const startWidth = farWidth
        let latest = startWidth

        const onMove = (ev: MouseEvent) => {
            // Dragging the divider left (negative delta) grows the rail.
            const proposed = startWidth + (startX - ev.clientX)
            latest = Math.min(maxFarWidth(), Math.max(minFarWidth, proposed))
            setFarWidth(latest)
        }
        const onUp = () => {
            document.removeEventListener("mousemove", onMove)
            document.removeEventListener("mouseup", onUp)
            document.body.style.cursor = ""
            onFarWidthChange?.(latest)
        }
        document.addEventListener("mousemove", onMove)
        document.addEventListener("mouseup", onUp)
        document.body.style.cursor = "col-resize"
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
        <div ref={containerRef} className="split-pane">
            <div className="split-pane-left" style={{ width: leftWidth }}>
                {left}
            </div>
            <div
                className="split-pane-divider"
                onMouseDown={startLeftDrag}
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
            <div className="split-pane-right">{right}</div>
            {hasFar && (
                <>
                    <div
                        className="split-pane-divider"
                        onMouseDown={startFarDrag}
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
                    </div>
                </>
            )}
        </div>
    )
}
