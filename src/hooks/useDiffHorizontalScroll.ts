import { useCallback, useLayoutEffect, useRef, type UIEvent } from "react"
import { horizontalOverflow, synchronizeCodeScroll } from "../diffHorizontalScroll"
import type { DiffLayout } from "../diffLayout"
import type { DiffContent } from "../types"
import type { HunkSlots } from "../components/DiffView"

/** One native horizontal scroll range per file. The fixed table retains its
 * paired rows, gutters and comment slots; only the code viewports move. */
export function useDiffHorizontalScroll(
    layout: DiffLayout,
    content: DiffContent,
    collapsed: boolean,
    slots: HunkSlots | undefined,
    shown: ReadonlySet<number> | undefined,
) {
    const sectionRef = useRef<HTMLElement>(null)
    const barRef = useRef<HTMLDivElement>(null)
    const portsRef = useRef<HTMLElement[]>([])
    const maximum = useRef(0)
    const left = useRef(0)

    const synchronize = useCallback((requested: number) => {
        left.current = synchronizeCodeScroll(portsRef.current, requested, maximum.current)
    }, [])

    const onScrollCapture = useCallback((event: UIEvent<HTMLElement>) => {
        const target = event.target
        // Synchronizing queues a scroll event on every line. Those followers
        // already have this offset; do not scan the whole file for each one.
        if (!(target instanceof HTMLElement) || Math.abs(target.scrollLeft - left.current) <= 0.5) { return }
        if (sectionRef.current?.getBoundingClientRect().width === 0) { return }
        if (portsRef.current.includes(target)) {
            synchronize(target.scrollLeft)
        }
    }, [synchronize])

    useLayoutEffect(() => {
        const section = sectionRef.current
        const bar = barRef.current
        if (!section) { return }
        if (layout !== "split" || collapsed || !bar) {
            portsRef.current = []
            section.removeAttribute("data-horizontal-overflow")
            return
        }
        const measure = () => {
            // Retained, hidden tab panels have no geometry. Measuring them
            // would replace a real horizontal range with zero and lose place.
            if (section.getBoundingClientRect().width === 0) { return }
            const ports = Array.from(section.querySelectorAll<HTMLElement>("[data-code-scroll]"))
            const measurements = ports.map((port) => {
                const inner = port.querySelector<HTMLElement>("[data-code-scroll-content]")
                if (!inner) { return { content: 0, viewport: port.clientWidth } }
                const style = getComputedStyle(inner)
                let width = (parseFloat(style.paddingLeft) || 0) + (parseFloat(style.paddingRight) || 0)
                for (const child of Array.from(inner.children)) {
                    const margins = getComputedStyle(child)
                    width += child.getBoundingClientRect().width +
                        (parseFloat(margins.marginLeft) || 0) + (parseFloat(margins.marginRight) || 0)
                }
                return { content: width, viewport: port.clientWidth }
            })
            maximum.current = horizontalOverflow(measurements)
            section.style.setProperty("--diff-horizontal-overflow", `${maximum.current}px`)
            section.toggleAttribute("data-horizontal-overflow", maximum.current > 0)
            portsRef.current = [...ports, bar]
            synchronize(left.current)
        }
        measure()
        const observer = new ResizeObserver(measure)
        observer.observe(section)
        // Text's intrinsic width changes with font loading, theme and zoom,
        // independently of the common minimum width on its scrolling parent.
        const text = Array.from(section.querySelectorAll<HTMLElement>(".diff-text"))
            .find((element) => element.textContent !== "")
        if (text) { observer.observe(text) }
        return () => observer.disconnect()
    }, [layout, content, collapsed, slots, shown, synchronize])

    return { sectionRef, barRef, onScrollCapture }
}
