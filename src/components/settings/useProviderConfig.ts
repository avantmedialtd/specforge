import { useEffect, useState, type Dispatch, type SetStateAction } from "react"
import { getBitbucketConfig, getGithubConfig, onPullRequestPanelMoved } from "../../api"
import type {
    BitbucketConfigView,
    GithubConfigView,
    PanelPosition,
    PullRequestProvider,
} from "../../types"

/// What a provider-configuration hook hands its row or card.
export interface ProviderConfigState<C> {
    /// `null` until read.
    config: C | null
    setConfig: Dispatch<SetStateAction<C | null>>
    loadFailed: boolean
    reload: () => Promise<C>
    /// Runs one configuration write and then re-reads the configuration. A
    /// failed write is re-read too, so a card reports the failure while
    /// showing the stored state rather than the attempted one, and the write's
    /// failure is rethrown after that read.
    transact: (write: () => Promise<unknown>) => Promise<C>
}

/// A pull-request provider's configuration as Settings shows it, kept in step
/// with panel moves made anywhere else — another window, a connected browser
/// tab — so the Layout group's slot choice and an enabled card's slot line
/// never show a slot the panel has since left (`settings-view`: *The Layout
/// Group Gathers the Side Panes' Occupants*).
///
/// Every mount reads afresh. A group is mounted only while it is shown, so
/// moving from Integrations to Layout re-reads the switch just flipped there.
function useProviderConfig<C extends { panelPosition: PanelPosition }>(
    provider: PullRequestProvider,
    load: () => Promise<C>,
): ProviderConfigState<C> {
    const [config, setConfig] = useState<C | null>(null)
    const [loadFailed, setLoadFailed] = useState(false)

    useEffect(() => {
        let cancelled = false
        load()
            .then((c) => {
                if (!cancelled) setConfig(c)
            })
            .catch(() => {
                if (!cancelled) setLoadFailed(true)
            })
        // The event announces both providers' moves; only this provider's
        // concerns this configuration.
        let unlisten: (() => void) | undefined
        void onPullRequestPanelMoved((payload) => {
            if (payload?.provider !== provider) return
            const { position } = payload
            setConfig((c) => (c && c.panelPosition !== position ? { ...c, panelPosition: position } : c))
        }).then((u) => {
            if (cancelled) u()
            else unlisten = u
        })
        return () => {
            cancelled = true
            unlisten?.()
        }
        // `provider` and `load` are fixed per hook below.
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [])

    /// Re-read rather than assume — after a credential save the configuration
    /// is the only place that says whether a token is now stored.
    const reload = async (): Promise<C> => {
        const next = await load()
        setConfig(next)
        return next
    }

    /// One credential transaction: the write, then the authoritative read.
    /// A failure re-reads before rethrowing so the card cannot keep a
    /// half-applied source state (`settings-view`: *Settings Persist by One
    /// Rule*).
    const transact = async (write: () => Promise<unknown>): Promise<C> => {
        try {
            await write()
        } catch (err) {
            try {
                await reload()
            } catch {
                // The write's own failure is the one the card reports.
            }
            throw err
        }
        return reload()
    }

    return { config, setConfig, loadFailed, reload, transact }
}

export function useBitbucketConfig() {
    return useProviderConfig<BitbucketConfigView>("bitbucket", getBitbucketConfig)
}

export function useGithubConfig() {
    return useProviderConfig<GithubConfigView>("github", getGithubConfig)
}
