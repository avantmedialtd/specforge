import { describe, expect, test } from "bun:test"
import {
    UNREAD_PROVIDER_FLAGS,
    withConfigRead,
    withProviderChange,
} from "./pullRequestProviderFlags"
import { resolvePullRequestAddress } from "./routing/resolve"
import type { PullRequestProviderChangedPayload } from "./types"

// `pull-request-viewer`: *Provider Enabled Flags Stay Current*.

describe("withConfigRead", () => {
    test("before any read, every flag is unread", () => {
        expect(UNREAD_PROVIDER_FLAGS).toEqual({ github: null, bitbucket: null })
    })

    test("a read fills in its own provider's flag only", () => {
        expect(withConfigRead(UNREAD_PROVIDER_FLAGS, "github", true)).toEqual({
            github: true,
            bitbucket: null,
        })
        expect(withConfigRead(UNREAD_PROVIDER_FLAGS, "bitbucket", false)).toEqual({
            github: null,
            bitbucket: false,
        })
    })

    test("a read that lands after a notice keeps the notice's word", () => {
        // The notice switched GitHub off while the read, answered before it,
        // was on its way back saying on.
        const noticed = withProviderChange(UNREAD_PROVIDER_FLAGS, {
            provider: "github",
            enabled: false,
        })
        expect(withConfigRead(noticed, "github", true)).toBe(noticed)
    })
})

describe("withProviderChange", () => {
    test("a notice sets its provider's flag, read or not", () => {
        const read = withConfigRead(UNREAD_PROVIDER_FLAGS, "bitbucket", true)
        expect(withProviderChange(read, { provider: "bitbucket", enabled: false })).toEqual({
            github: null,
            bitbucket: false,
        })
        expect(
            withProviderChange(UNREAD_PROVIDER_FLAGS, { provider: "github", enabled: true }),
        ).toEqual({ github: true, bitbucket: null })
    })

    test("a notice after a read keeps the flag current", () => {
        const read = withConfigRead(UNREAD_PROVIDER_FLAGS, "github", true)
        const off = withProviderChange(read, { provider: "github", enabled: false })
        expect(off.github).toBe(false)
        expect(withProviderChange(off, { provider: "github", enabled: true }).github).toBe(true)
    })

    test("a notice of the flag already held changes nothing", () => {
        const read = withConfigRead(UNREAD_PROVIDER_FLAGS, "github", true)
        expect(withProviderChange(read, { provider: "github", enabled: true })).toBe(read)
    })

    test("a frame naming no known provider or no flag changes nothing", () => {
        const read = withConfigRead(UNREAD_PROVIDER_FLAGS, "github", true)
        for (const payload of [
            undefined,
            null,
            { provider: "gitlab", enabled: false },
            { provider: "github" },
            { provider: "github", enabled: "false" },
        ]) {
            expect(
                withProviderChange(read, payload as PullRequestProviderChangedPayload | undefined),
            ).toBe(read)
        }
    })

    // The scenario the flags exist for: the center pane shows a BitBucket pull
    // request, and BitBucket is switched off in Settings.
    test("switching a provider off re-resolves its address as off", () => {
        const address = {
            kind: "pullRequest" as const,
            provider: "bitbucket" as const,
            owner: "acme",
            repo: "api",
            number: 7,
        }
        const snapshots = {
            github: null,
            bitbucket: {
                provider: "bitbucket" as const,
                snapshot: {
                    status: "disabled" as const,
                    stale: false,
                    fetchedAtUnix: null,
                    pullRequests: [],
                    skippedWorkspaces: [],
                },
            },
        }
        const on = withConfigRead(UNREAD_PROVIDER_FLAGS, "bitbucket", true)
        expect(resolvePullRequestAddress(address, UNREAD_PROVIDER_FLAGS, snapshots).status).toBe(
            "pending",
        )
        expect(resolvePullRequestAddress(address, on, snapshots).status).toBe("pending")
        const off = withProviderChange(on, { provider: "bitbucket", enabled: false })
        expect(resolvePullRequestAddress(address, off, snapshots).status).toBe("providerOff")
    })
})
