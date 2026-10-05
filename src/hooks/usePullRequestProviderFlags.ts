import { useEffect, useState } from "react"
import { getBitbucketConfig, getGithubConfig, onPullRequestProviderChanged } from "../api"
import {
    UNREAD_PROVIDER_FLAGS,
    withConfigRead,
    withProviderChange,
} from "../pullRequestProviderFlags"
import type { PullRequestProviderFlags } from "../routing/resolve"
import type { PullRequestProvider } from "../types"

/// Each pull-request provider's enabled flag, for a root that resolves
/// pull-request addresses with `resolvePullRequestAddress`
/// (`pull-request-viewer`: *Provider Enabled Flags Stay Current*).
///
/// A flag is `null` until its configuration has been read, so an address pends
/// rather than resolving against a guess, and is then kept current from the
/// `pull-request-provider-changed` notice: switching a provider off anywhere —
/// this window's Settings, another window, a browser tab the service serves —
/// re-resolves every address this root shows, with no reload. A read that
/// fails leaves its flag unread until a notice sets it.
export function usePullRequestProviderFlags(): PullRequestProviderFlags {
    const [flags, setFlags] = useState<PullRequestProviderFlags>(UNREAD_PROVIDER_FLAGS)

    useEffect(() => {
        let mounted = true
        // Subscribed before the reads, so a flag set while they are in flight
        // is not missed. In whichever order a read and a notice land, the
        // notice's word is kept (`withConfigRead`).
        const unlisten = onPullRequestProviderChanged((payload) => {
            if (mounted) setFlags((current) => withProviderChange(current, payload))
        })
        const read = (provider: PullRequestProvider, config: Promise<{ enabled: boolean }>) => {
            config
                .then(({ enabled }) => {
                    if (mounted) setFlags((current) => withConfigRead(current, provider, enabled))
                })
                .catch((err) => {
                    console.warn(`failed to read whether ${provider} is enabled`, err)
                })
        }
        read("github", getGithubConfig())
        read("bitbucket", getBitbucketConfig())
        return () => {
            mounted = false
            void unlisten.then((off) => off())
        }
    }, [])

    return flags
}
