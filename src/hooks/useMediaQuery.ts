import { useEffect, useState } from "react"

/**
 * Tracks whether `query` currently matches, live: subscribes to matchMedia's
 * `change` event and re-renders on toggle. DashboardView's
 * `usePrefersReducedMotion` is built on it — any future consumer that needs to
 * re-key a render off an OS-level media feature should call this rather than
 * reimplementing the listener.
 *
 * Not for the colour scheme: what the page renders in is the stored
 * appearance resolved against the operating system, which `useAppearance`
 * tracks (`theme-system`: *The Scheme Is a Selectable Preference*), so a raw
 * `prefers-color-scheme` read would disagree with the page under an explicit
 * Light or Dark.
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
