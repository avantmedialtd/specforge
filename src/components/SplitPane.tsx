import { useRef, useState, type ReactNode } from "react"

interface SplitPaneProps {
    left: ReactNode
    right: ReactNode
    initialLeftWidth?: number
    minLeftWidth?: number
    minRightWidth?: number
}

export function SplitPane({
    left,
    right,
    initialLeftWidth = 340,
    minLeftWidth = 180,
    minRightWidth = 320,
}: SplitPaneProps) {
    const [leftWidth, setLeftWidth] = useState(initialLeftWidth)
    const containerRef = useRef<HTMLDivElement>(null)

    const startDrag = (downEvent: React.MouseEvent) => {
        downEvent.preventDefault()
        const startX = downEvent.clientX
        const startWidth = leftWidth

        const onMove = (ev: MouseEvent) => {
            const delta = ev.clientX - startX
            const proposed = startWidth + delta
            const containerWidth = containerRef.current?.clientWidth ?? 800
            const max = Math.max(minLeftWidth, containerWidth - minRightWidth)
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

    return (
        <div ref={containerRef} className="split-pane">
            <div className="split-pane-left" style={{ width: leftWidth }}>
                {left}
            </div>
            <div
                className="split-pane-divider"
                onMouseDown={startDrag}
                role="separator"
                aria-orientation="vertical"
            />
            <div className="split-pane-right">{right}</div>
        </div>
    )
}
