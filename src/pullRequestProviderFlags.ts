// Each pull-request provider's enabled flag, as a root that resolves
// pull-request addresses holds it: the main window's `App` and the
// pull-request window's root (`pull-request-viewer`: *Provider Enabled Flags
// Stay Current*). A flag is read from its provider's configuration when the
// root mounts, and kept current from the `pull-request-provider-changed`
// notice the service raises whenever a flag is set, in whichever window or tab
// of the service it is set. The two arrive in either order; how they combine is
// decided here, as pure functions, for the reason `pullRequestOpen.ts` gives,
// and `usePullRequestProviderFlags` only wires them.

import type { PullRequestProviderFlags } from "./routing/resolve"
import type { PullRequestProvider, PullRequestProviderChangedPayload } from "./types"

/// Before either configuration is read, so every pull-request address pends.
export const UNREAD_PROVIDER_FLAGS: PullRequestProviderFlags = { github: null, bitbucket: null }

/// `flags` with the flag a configuration read found. A read fills in only a
/// flag nothing has set yet: a notice that landed before it is the newer word,
/// since the read may have been answered before the flag changed.
export function withConfigRead(
    flags: PullRequestProviderFlags,
    provider: PullRequestProvider,
    enabled: boolean,
): PullRequestProviderFlags {
    if (flags[provider] !== null) return flags
    return { ...flags, [provider]: enabled }
}

/// `flags` with the flag a `pull-request-provider-changed` notice carries,
/// which is always the newest word on its provider. A frame naming no known
/// provider or carrying no boolean flag changes nothing — an unparseable SSE
/// frame arrives as `undefined` — and neither does a notice of the flag
/// already held, so it costs no render.
export function withProviderChange(
    flags: PullRequestProviderFlags,
    payload: PullRequestProviderChangedPayload | null | undefined,
): PullRequestProviderFlags {
    if (!payload || typeof payload.enabled !== "boolean") return flags
    if (payload.provider !== "github" && payload.provider !== "bitbucket") return flags
    if (flags[payload.provider] === payload.enabled) return flags
    return { ...flags, [payload.provider]: payload.enabled }
}
