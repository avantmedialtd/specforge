import { useEffect, useState } from "react"
import { PANEL_SOURCES, type PanelSnapshot } from "../components/PullRequestPanel"
import type { PullRequestProvider } from "../types"

/// One provider's pull-request snapshot — read on mount and re-read on each of
/// that provider's `*-pull-requests-updated` events (`PANEL_SOURCES` pins which
/// getter and which event). `null` until the first read lands.
///
/// Read here, in `App`, rather than inside `PullRequestPanel`: whether a panel
/// is present decides whether the rail exists at all (`spec-browser`: *Rail
/// Exists Only While Occupied*), and a panel that only learned its snapshot
/// once mounted could never make an absent rail appear, nor retire a hidden
/// rail's restore chevron once disabled (design D3).
export function usePullRequestSnapshot(provider: PullRequestProvider): PanelSnapshot | null {
    const [panel, setPanel] = useState<PanelSnapshot | null>(null)

    useEffect(() => {
        let mounted = true
        const source = PANEL_SOURCES[provider]
        const refresh = () =>
            source
                .fetch()
                .then((next) => {
                    if (mounted) setPanel(next)
                })
                .catch(() => {})
        refresh()
        let unlisten: (() => void) | undefined
        source
            .onUpdated(() => refresh())
            .then((u) => {
                if (mounted) unlisten = u
                else u()
            })
        return () => {
            mounted = false
            unlisten?.()
        }
    }, [provider])

    return panel
}
