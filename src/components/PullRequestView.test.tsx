import { describe, expect, test } from "bun:test"
import { renderToStaticMarkup } from "react-dom/server"
import type { PullRequestAddress } from "../routing/address"
import type { PullRequestResolution } from "../routing/resolve"
import type {
    ArtifactStatus,
    ChangeData,
    ChangeInstance,
    PullRequestLinks,
    PullRequestSummary,
    WorkspaceView,
} from "../types"
import { PullRequestAtAddress, type PaneNavigation } from "./PullRequestView"

// The view's presentations, server-rendered. Effects do not run on the server,
// so nothing here sends a command: what is pinned is what each outcome renders
// in the center pane and in the pull-request window before any read has
// answered (`pull-request-viewer`: *Pull-Request View*, *Pull-Request Window*,
// *Linked Change in the Pull-Request View*; `view-routing`: *Pull-Request
// Addresses*). The decisions themselves are tested in `pullRequestView.test.ts`.
// The browser skin is the host here, as `isWeb()` reads it without a window.

const ADDRESS: PullRequestAddress = {
    kind: "pullRequest",
    provider: "github",
    owner: "acme",
    repo: "api",
    number: 42,
}

const URL_42 = "https://github.com/acme/api/pull/42"

function row(overrides: Partial<PullRequestSummary> = {}): PullRequestSummary {
    return {
        id: 42,
        title: "Add rate limits",
        repoFullName: "acme/api",
        sourceRepoFullName: "acme/api",
        sourceBranch: "rate-limits",
        destinationBranch: "main",
        url: URL_42,
        draft: false,
        updatedAtUnix: 1_000,
        review: null,
        openTasks: 0,
        author: "ada",
        checks: null,
        conflicting: false,
        unresolvedThreads: 0,
        ...overrides,
    }
}

function listed(pr: PullRequestSummary = row()): PullRequestResolution {
    return { status: "listed", row: pr, address: ADDRESS }
}

const PANE: PaneNavigation = { openWorktree: () => {}, openSettings: () => {} }

function render(
    resolution: PullRequestResolution,
    options: { pane?: PaneNavigation; views?: WorkspaceView[]; links?: PullRequestLinks } = {},
): string {
    return renderToStaticMarkup(
        <PullRequestAtAddress
            address={ADDRESS}
            resolution={resolution}
            views={options.views ?? []}
            links={options.links ?? null}
            pane={options.pane}
        />,
    )
}

describe("an address that resolves to no pull request", () => {
    test("pending shows Loading… and nothing else", () => {
        expect(render({ status: "pending" })).toBe('<div class="detail-pane-status">Loading…</div>')
    })

    test("the provider-off notice names its provider, with a way to Settings in the center pane", () => {
        const html = render({ status: "providerOff" }, { pane: PANE })
        expect(html).toContain("GitHub is switched off")
        expect(html).toContain("<button")
        expect(html).toContain("Open Settings › Integrations")
    })

    // *The window's notices do not link to Settings*.
    test("the window names Settings › Integrations as text, with no control that navigates", () => {
        for (const resolution of [
            { status: "providerOff" },
            { status: "unavailable", reason: "unauthenticated" },
        ] as PullRequestResolution[]) {
            const html = render(resolution)
            expect(html).toContain("Settings › Integrations")
            expect(html).not.toContain("<button")
            expect(html).not.toContain("<a")
        }
    })

    test("an unavailable list says so, and points nowhere", () => {
        const html = render({ status: "unavailable", reason: "unavailable" }, { pane: PANE })
        expect(html).toContain("GitHub is unavailable")
        expect(html).not.toContain("Settings")
    })
})

describe("a listed pull request, before its first read answers", () => {
    test("the header names it and its branches, and Loading… holds the detail's place", () => {
        const html = render(listed(), { pane: PANE })
        expect(html).toContain('<span class="pull-request-view-number">#42</span>')
        expect(html).toContain("Add rate limits")
        expect(html).toContain("rate-limits")
        expect(html).toContain('<div class="detail-pane-status">Loading…</div>')
    })

    // *The header carries the row's signals*.
    test("the header carries the row's failing checks and conflict marker", () => {
        const html = render(listed(row({ checks: "failing", conflicting: true })), { pane: PANE })
        expect(html).toContain("pull-request-checks--failing")
        expect(html).toContain("Conflicts")
    })

    // *Hidden characters in the header are visible*.
    test("a zero-width space in a branch name shows as a marked escape", () => {
        const html = render(listed(row({ sourceBranch: "rate​limits" })), { pane: PANE })
        expect(html).not.toContain("​")
        expect(html).toContain('data-code-point="200b"')
        expect(html).toContain("U+200B")
    })

    // *Only the center pane offers the pop-out control*.
    test("the center pane offers the pop-out control, and the window does not", () => {
        expect(render(listed(), { pane: PANE })).toContain('aria-label="Open in its own window"')
        expect(render(listed())).not.toContain("Open in its own window")
    })

    // *The provider's page in the browser skin*.
    test("both carry the provider's page, as an opener-isolated link in the browser skin", () => {
        for (const html of [render(listed(), { pane: PANE }), render(listed())]) {
            expect(html).toContain(
                `<a class="pull-request-view-action" href="${URL_42}" target="_blank" rel="noopener noreferrer">Open on GitHub</a>`,
            )
        }
    })

    // *Nothing can be posted*.
    test("no control comments, approves, requests changes or merges", () => {
        const html = render(listed(), { pane: PANE }).toLowerCase()
        for (const action of ["approve", "merge", "request changes", "reply", "comment"]) {
            expect(html).not.toContain(`>${action}`)
        }
    })
})

describe("the linked change", () => {
    const REPO = "/code/api/.git"
    const WT = "/code/api-wt"
    const PROPOSAL_ONLY: ArtifactStatus = { proposal: true, design: false, tasks: false, specs: [] }
    const views = [
        {
            kind: "repo",
            repoId: REPO,
            mainWorktree: "/code/api",
            name: "api",
            defaultBranch: "main",
            active: [
                {
                    name: "add-rate-limits",
                    instances: [
                        {
                            worktreePath: WT,
                            branch: "rate-limits",
                            change: {
                                artifacts: PROPOSAL_ONLY,
                                completedTasks: 0,
                                totalTasks: 0,
                            } as unknown as ChangeData,
                            modifiedAt: 100,
                        } as unknown as ChangeInstance,
                    ],
                },
            ],
            displayName: null,
            color: null,
            worktrees: ["/code/api", WT],
        } as unknown as WorkspaceView,
    ]
    const links: PullRequestLinks = {
        worktrees: [],
        pullRequests: [
            { url: URL_42, worktrees: [{ repoId: REPO, worktreePath: WT, branch: "rate-limits" }] },
        ],
    }

    // *The center pane leads to the linked change*.
    test("is a control in the center pane", () => {
        expect(render(listed(), { pane: PANE, views, links })).toContain(
            '<button type="button" class="pull-request-view-linked-name pull-request-view-linked-name--control">add-rate-limits</button>',
        )
    })

    // *The window names the change passively*.
    test("is passive text in the window", () => {
        const html = render(listed(), { views, links })
        expect(html).toContain('<span class="pull-request-view-linked-name">add-rate-limits</span>')
        expect(html).not.toContain("pull-request-view-linked-name--control")
    })

    test("is named for no unlinked pull request", () => {
        expect(render(listed(), { pane: PANE, views })).not.toContain("pull-request-view-linked")
    })
})
