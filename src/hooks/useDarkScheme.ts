import { useEffect, useState } from "react"

/**
 * Tracks whether `query` currently matches, live: subscribes to matchMedia's
 * `change` event and re-renders on toggle. The shared subscribe pattern
 * behind `useDarkScheme` below and DashboardView's `usePrefersReducedMotion`
 * — any future consumer that needs to re-key a render off an OS-level media
 * feature should call this rather than reimplementing the listener.
 */
export function useMediaQuery(query: string): boolean {
    const [matches, setMatches] = useState(() => window.matchMedia(query).matches)

    useEffect(() => {
        const mql = window.matchMedia(query)
        const onChange = (event: MediaQueryListEvent) => setMatches(event.matches)
        mql.addEventListener("change", onChange)
        return () => mql.removeEventListener("change", onChange)
    }, [query])

    return matches
}

const DARK_SCHEME = "(prefers-color-scheme: dark)"

/**
 * Tracks whether the OS is currently in dark mode, live. Shared by
 * MermaidBlock and SvgBlock: both bake design tokens into a render that
 * won't otherwise follow a scheme change on its own (Mermaid stamps colours
 * into its output SVG; SvgBlock's data URI is static once built), so each
 * re-keys its memo/effect off this value to redo that render on toggle.
 */
export function useDarkScheme(): boolean {
    return useMediaQuery(DARK_SCHEME)
}
