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

    const startLeftDrag = (downEvent: React.MouseEvent) => {
        downEvent.preventDefault()
        const startX = downEvent.clientX
        const startWidth = leftWidth

        const onMove = (ev: MouseEvent) => {
            const proposed = startWidth + (ev.clientX - startX)
            // Center keeps at least minRightWidth; the far pane (if any) keeps
            // its current width.
            const reserved = minRightWidth + (hasFar ? farWidth : 0)
            const max = Math.max(minLeftWidth, containerWidth() - reserved)
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
            const max = Math.max(minFarWidth, containerWidth() - leftWidth - minRightWidth)
            latest = Math.min(max, Math.max(minFarWidth, proposed))
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

    return (
        <div ref={containerRef} className="split-pane">
            <div className="split-pane-left" style={{ width: leftWidth }}>
                {left}
            </div>
            <div
                className="split-pane-divider"
                onMouseDown={startLeftDrag}
                role="separator"
                aria-orientation="vertical"
            />
            <div className="split-pane-right">{right}</div>
            {hasFar && (
                <>
                    <div
                        className="split-pane-divider"
                        onMouseDown={startFarDrag}
                        role="separator"
                        aria-orientation="vertical"
                    />
                    <div className="split-pane-far" style={{ width: farWidth }}>
                        {far}
                    </div>
                </>
            )}
        </div>
    )
}
