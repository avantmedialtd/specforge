import { useEffect, useState } from "react"

const DARK_SCHEME = "(prefers-color-scheme: dark)"

/**
 * Tracks whether the OS is currently in dark mode, live. Shared by
 * MermaidBlock and SvgBlock: both bake design tokens into a render that
 * won't otherwise follow a scheme change on its own (Mermaid stamps colours
 * into its output SVG; SvgBlock's data URI is static once built), so each
 * re-keys its memo/effect off this value to redo that render on toggle.
 */
export function useDarkScheme(): boolean {
    const [isDark, setIsDark] = useState(
        () => window.matchMedia(DARK_SCHEME).matches,
    )

    useEffect(() => {
        const query = window.matchMedia(DARK_SCHEME)
        const onChange = (event: MediaQueryListEvent) => setIsDark(event.matches)
        query.addEventListener("change", onChange)
        return () => query.removeEventListener("change", onChange)
    }, [])

    return isDark
}
