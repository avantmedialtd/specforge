import { describe, expect, test } from "bun:test"
import {
    nextRelabelDelayMs,
    PANEL_MESSAGES,
    panelBodyState,
    panelHeaderTitle,
    relativeUpdated,
    reviewCellText,
    reviewCellTitle,
} from "./PullRequestPanel"
import type { PullRequestSummary, PullRequestsState } from "../types"

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
        ...overrides,
    }
}

function snapshot(overrides: Partial<PullRequestsState> = {}): PullRequestsState {
    return {
        status: "ok",
        stale: false,
        fetchedAtUnix: NOW_S,
        pullRequests: [row()],
        skippedWorkspaces: [],
        ...overrides,
    }
}

describe("panelBodyState", () => {
    test("a disabled feature renders no panel at all", () => {
        expect(panelBodyState(snapshot({ status: "disabled", pullRequests: [] }))).toBeNull()
    })

    test("each non-ok status is its own one-line body", () => {
        expect(panelBodyState(snapshot({ status: "unauthenticated", pullRequests: [] }))).toBe(
            "unauthenticated",
        )
        expect(panelBodyState(snapshot({ status: "unavailable", pullRequests: [] }))).toBe(
            "unavailable",
        )
    })

    test("an ok snapshot is rows, or empty when it holds none", () => {
        expect(panelBodyState(snapshot())).toBe("rows")
        expect(panelBodyState(snapshot({ pullRequests: [] }))).toBe("empty")
    })

    test("a stale snapshot still shows its rows", () => {
        expect(panelBodyState(snapshot({ stale: true }))).toBe("rows")
    })

    test("the unauthenticated line points at Settings", () => {
        expect(PANEL_MESSAGES.unauthenticated).toContain("Settings")
        expect(PANEL_MESSAGES.empty).toBe("No open pull requests.")
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
        const title = panelHeaderTitle(snapshot({ skippedWorkspaces: ["locked", "gone"] }))
        expect(title).toContain("Skipped (no access): locked, gone")
    })

    test("says nothing about skips when there are none", () => {
        expect(panelHeaderTitle(snapshot())).toBe("Your open BitBucket pull requests")
    })

    test("explains a stale list", () => {
        expect(panelHeaderTitle(snapshot({ stale: true }))).toContain(
            "BitBucket could not be reached",
        )
    })
})
