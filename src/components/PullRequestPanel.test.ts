import { describe, expect, test } from "bun:test"
import {
    checksLabel,
    COLLAPSED_KEYS,
    conversationsLabel,
    headerCounts,
    nextRelabelDelayMs,
    PANEL_MESSAGES,
    PANEL_TITLES,
    panelBodyState,
    panelHeaderTitle,
    panelRows,
    panelSections,
    paneOf,
    paneTakesReserve,
    relativeUpdated,
    reviewCellText,
    reviewCellTitle,
    type PanelSnapshot,
} from "./PullRequestPanel"
import type {
    BitbucketPullRequestsState,
    GithubPullRequestsState,
    PullRequestSummary,
} from "../types"

const NOW_MS = Date.UTC(2026, 8, 22, 12, 0, 0)
const NOW_S = NOW_MS / 1000

function row(overrides: Partial<PullRequestSummary> = {}): PullRequestSummary {
    return {
        id: 1,
        title: "Add the panel",
        repoFullName: "acme/app",
        sourceBranch: "feature/panel",
        destinationBranch: "main",
        url: "https://bitbucket.org/acme/app/pull-requests/1",
        draft: false,
        updatedAtUnix: NOW_S - 120,
        review: { approvals: 2, changesRequested: 0, pending: 1 },
        openTasks: 3,
        author: null,
        checks: null,
        conflicting: false,
        unresolvedThreads: 0,
        ...overrides,
    }
}

function ghRow(overrides: Partial<PullRequestSummary> = {}): PullRequestSummary {
    return row({
        url: "https://github.com/acme/api/pull/7",
        repoFullName: "acme/api",
        openTasks: 0,
        author: "ada",
        checks: "passing",
        ...overrides,
    })
}

function bitbucket(overrides: Partial<BitbucketPullRequestsState> = {}): PanelSnapshot {
    return {
        provider: "bitbucket",
        snapshot: {
            status: "ok",
            stale: false,
            fetchedAtUnix: NOW_S,
            pullRequests: [row()],
            skippedWorkspaces: [],
            ...overrides,
        },
    }
}

function github(overrides: Partial<GithubPullRequestsState> = {}): PanelSnapshot {
    return {
        provider: "github",
        snapshot: {
            status: "ok",
            stale: false,
            fetchedAtUnix: NOW_S,
            authored: [ghRow({ id: 1, url: "https://github.com/acme/api/pull/1" })],
            reviewRequested: [
                ghRow({
                    id: 2,
                    url: "https://github.com/acme/api/pull/2",
                    author: "copilot-swe-agent",
                }),
            ],
            withheld: 0,
            ...overrides,
        },
    }
}

describe("panelBodyState", () => {
    test("a disabled feature renders no panel at all", () => {
        expect(panelBodyState(bitbucket({ status: "disabled", pullRequests: [] }))).toBeNull()
        expect(
            panelBodyState(github({ status: "disabled", authored: [], reviewRequested: [] })),
        ).toBeNull()
    })

    test("each non-ok status is its own one-line body", () => {
        expect(panelBodyState(bitbucket({ status: "unauthenticated", pullRequests: [] }))).toBe(
            "unauthenticated",
        )
        expect(panelBodyState(bitbucket({ status: "unavailable", pullRequests: [] }))).toBe(
            "unavailable",
        )
        expect(
            panelBodyState(github({ status: "unauthenticated", authored: [], reviewRequested: [] })),
        ).toBe("unauthenticated")
    })

    test("an ok snapshot is rows, or empty when it holds none", () => {
        expect(panelBodyState(bitbucket())).toBe("rows")
        expect(panelBodyState(bitbucket({ pullRequests: [] }))).toBe("empty")
        expect(panelBodyState(github())).toBe("rows")
        expect(panelBodyState(github({ authored: [] }))).toBe("rows")
        expect(panelBodyState(github({ authored: [], reviewRequested: [] }))).toBe("empty")
    })

    test("a stale snapshot still shows its rows", () => {
        expect(panelBodyState(bitbucket({ stale: true }))).toBe("rows")
        expect(panelBodyState(github({ stale: true }))).toBe("rows")
    })

    test("the unauthenticated line points at Settings, per provider", () => {
        expect(PANEL_MESSAGES.bitbucket.unauthenticated).toContain("Settings")
        expect(PANEL_MESSAGES.github.unauthenticated).toContain("Settings")
        expect(PANEL_MESSAGES.github.unauthenticated).toContain("GitHub")
        expect(PANEL_MESSAGES.bitbucket.empty).toBe("No open pull requests.")
    })

    test("GitHub's empty line says there is nothing open and nothing to review", () => {
        expect(PANEL_MESSAGES.github.empty).toBe("No open pull requests and nothing to review.")
    })
})

describe("panelSections", () => {
    test("BitBucket has one untitled section with every row", () => {
        const sections = panelSections(bitbucket({ pullRequests: [row({ id: 1 }), row({ id: 2 })] }))
        expect(sections).toHaveLength(1)
        expect(sections[0].title).toBeNull()
        expect(sections[0].rows.map((r) => r.id)).toEqual([1, 2])
        expect(sections[0].showAuthor).toBe(false)
    })

    test("GitHub has Yours then To review, and only To review shows authors", () => {
        const sections = panelSections(github())
        expect(sections.map((s) => s.title)).toEqual(["Yours", "To review"])
        expect(sections.map((s) => s.showAuthor)).toEqual([false, true])
        expect(sections[1].rows[0].author).toBe("copilot-swe-agent")
    })

    test("an empty GitHub section is not rendered", () => {
        expect(panelSections(github({ reviewRequested: [] })).map((s) => s.title)).toEqual([
            "Yours",
        ])
        expect(panelSections(github({ authored: [] })).map((s) => s.title)).toEqual([
            "To review",
        ])
        expect(panelSections(github({ authored: [], reviewRequested: [] }))).toEqual([])
    })

    test("panelRows lists every row in body order", () => {
        expect(panelRows(github()).map((r) => r.id)).toEqual([1, 2])
    })
})

describe("headerCounts", () => {
    test("BitBucket shows its row count", () => {
        expect(headerCounts(bitbucket({ pullRequests: [row(), row({ id: 2 })] }))).toEqual({
            text: "2",
            label: "2 open",
        })
    })

    test("GitHub shows yours · to review", () => {
        expect(
            headerCounts(
                github({
                    authored: [ghRow({ id: 1 }), ghRow({ id: 3 })],
                    reviewRequested: [ghRow({ id: 2 })],
                }),
            ),
        ).toEqual({ text: "2 · 1", label: "2 yours, 1 to review" })
    })

    test("both counts read zero when nothing is open", () => {
        expect(headerCounts(github({ authored: [], reviewRequested: [] }))?.text).toBe("0 · 0")
    })

    test("a stale list keeps its counts; a status without a list has none", () => {
        expect(headerCounts(github({ stale: true }))?.text).toBe("1 · 1")
        expect(
            headerCounts(github({ status: "unauthenticated", authored: [], reviewRequested: [] })),
        ).toBeNull()
        expect(headerCounts(bitbucket({ status: "unavailable", pullRequests: [] }))).toBeNull()
    })
})

describe("checksLabel", () => {
    test("says each checks state in words", () => {
        expect(checksLabel("passing")).toBe("Checks passing")
        expect(checksLabel("failing")).toBe("Checks failing")
        expect(checksLabel("pending")).toBe("Checks pending")
    })
})

describe("conversationsLabel", () => {
    test("counts unresolved conversations in words", () => {
        expect(conversationsLabel(1)).toBe("1 unresolved conversation")
        expect(conversationsLabel(2)).toBe("2 unresolved conversations")
    })
})

describe("titles and keys", () => {
    test("each panel names its provider", () => {
        expect(PANEL_TITLES.bitbucket).toContain("BitBucket")
        expect(PANEL_TITLES.github).toContain("GitHub")
    })

    test("BitBucket keeps its shipped collapse key; GitHub has its own", () => {
        expect(COLLAPSED_KEYS.bitbucket).toBe("specforge.pullRequestsCollapsed")
        expect(COLLAPSED_KEYS.github).toBe("specforge.githubPullRequestsCollapsed")
    })
})

describe("the height reserve", () => {
    test("slots map onto their panes", () => {
        expect(paneOf("left-top")).toBe("sidebar")
        expect(paneOf("left-bottom")).toBe("sidebar")
        expect(paneOf("right-top")).toBe("rail")
        expect(paneOf("right-bottom")).toBe("rail")
        expect(paneOf(null)).toBeNull()
    })

    test("a default install — both off at left-bottom — takes no reserve", () => {
        const panels = [
            { position: "left-bottom" as const, present: false },
            { position: "left-bottom" as const, present: false },
        ]
        expect(paneTakesReserve("sidebar", panels)).toBe(false)
        expect(paneTakesReserve("rail", panels)).toBe(false)
    })

    test("only the pane holding a present panel takes the reserve", () => {
        const panels = [
            { position: "left-bottom" as const, present: false },
            { position: "right-top" as const, present: true },
        ]
        expect(paneTakesReserve("sidebar", panels)).toBe(false)
        expect(paneTakesReserve("rail", panels)).toBe(true)
    })

    test("a present panel with an unread position takes nothing", () => {
        expect(paneTakesReserve("sidebar", [{ position: null, present: true }])).toBe(false)
    })
})

describe("reviewCellText", () => {
    test("reads approvals, changes requested, pending — in that order", () => {
        // The spec's scenario: two approvals, no change requests, one pending.
        expect(reviewCellText({ approvals: 2, changesRequested: 0, pending: 1 })).toBe(
            "✓2 ✗0 ○1",
        )
        expect(reviewCellText({ approvals: 0, changesRequested: 3, pending: 5 })).toBe(
            "✓0 ✗3 ○5",
        )
    })

    test("an absent summary is unknown, not zero", () => {
        expect(reviewCellText(null)).toBe("?")
        expect(reviewCellText(null)).not.toBe(
            reviewCellText({ approvals: 0, changesRequested: 0, pending: 0 }),
        )
    })

    test("the tooltip says the same numbers in words", () => {
        expect(reviewCellTitle({ approvals: 2, changesRequested: 0, pending: 1 })).toBe(
            "2 approved · 0 changes requested · 1 pending",
        )
        expect(reviewCellTitle(null)).toBe("Review state unknown")
    })
})

describe("relativeUpdated", () => {
    test("uses the application's relative-time vocabulary", () => {
        expect(relativeUpdated(NOW_MS, NOW_S - 120)).toBe("2m ago")
        expect(relativeUpdated(NOW_MS, NOW_S - 3 * 86_400)).toBe("3d ago")
        expect(relativeUpdated(NOW_MS, NOW_S - 10)).toBe("just now")
    })

    test("an unreadable time is a dash, not the epoch", () => {
        expect(relativeUpdated(NOW_MS, 0)).toBe("—")
    })

    test("a time in the future reads as now", () => {
        expect(relativeUpdated(NOW_MS, NOW_S + 600)).toBe("just now")
    })
})

describe("nextRelabelDelayMs", () => {
    test("wakes for the soonest-changing label", () => {
        const rows = [
            row({ updatedAtUnix: NOW_S - 3 * 86_400 }), // "3d ago": next change in a day
            row({ updatedAtUnix: NOW_S - 90 }), // "1m ago": next change in 30 s
        ]
        expect(nextRelabelDelayMs(NOW_MS, rows)).toBe(30_000)
    })

    test("has nothing to wake for without a readable time", () => {
        expect(nextRelabelDelayMs(NOW_MS, [])).toBeNull()
        expect(nextRelabelDelayMs(NOW_MS, [row({ updatedAtUnix: 0 })])).toBeNull()
    })
})

describe("panelHeaderTitle", () => {
    test("names skipped workspaces on request, not as an error", () => {
        const title = panelHeaderTitle(bitbucket({ skippedWorkspaces: ["locked", "gone"] }))
        expect(title).toContain("Skipped (no access): locked, gone")
    })

    test("says nothing about skips when there are none", () => {
        expect(panelHeaderTitle(bitbucket())).toBe("Your open BitBucket pull requests")
    })

    test("explains a stale list", () => {
        expect(panelHeaderTitle(bitbucket({ stale: true }))).toContain(
            "BitBucket could not be reached",
        )
        expect(panelHeaderTitle(github({ stale: true }))).toContain(
            "GitHub could not be reached",
        )
    })

    test("GitHub's withheld results are counted and explained, not an error", () => {
        const title = panelHeaderTitle(github({ withheld: 3 }))
        expect(title).toContain("3 results withheld")
        expect(title).toContain("single sign-on")
        expect(title).toContain("fine-grained token")
    })

    test("says nothing about withholding when nothing was withheld", () => {
        const title = panelHeaderTitle(github())
        expect(title).not.toContain("withheld")
        expect(title).toContain("1 yours, 1 to review")
    })

    test("a single withheld result reads in the singular", () => {
        expect(panelHeaderTitle(github({ withheld: 1 }))).toContain("1 result withheld")
    })
})
