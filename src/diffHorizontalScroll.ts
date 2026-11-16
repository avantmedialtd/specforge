/** All code-line viewports get the same range, even when the two gutters
 * leave their code columns a pixel apart in width. */
export function horizontalOverflow(
    measurements: readonly { content: number; viewport: number }[],
): number {
    return measurements.reduce((maximum, { content, viewport }) =>
        Math.max(maximum, Math.ceil(content - viewport)), 0)
}

export interface HorizontalPort {
    scrollLeft: number
}

/** Native scrolling moves DOM viewports, not React state or source text.
 * Avoid redundant writes so the resulting scroll events settle immediately. */
export function synchronizeCodeScroll(
    ports: readonly HorizontalPort[],
    requested: number,
    maximum: number,
): number {
    const left = Math.max(0, Math.min(maximum, requested))
    for (const port of ports) {
        if (Math.abs(port.scrollLeft - left) > 0.5) { port.scrollLeft = left }
    }
    return left
}
