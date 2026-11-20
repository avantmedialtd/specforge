import { afterAll, beforeAll, describe, expect, test } from "bun:test"
import { act, createElement } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { installFakeDom, type FakeElement } from "../testing/fakeDom"
import { installFakeTransport } from "../testing/fakeTransport"
import {
    getBitbucketPullRequests,
    getGithubPullRequests,
    onBitbucketPullRequestsUpdated,
    onGithubPullRequestsUpdated,
} from "../api"
import {
    CERTIFICATE_HINT,
    checksLabel,
    COLLAPSED_KEYS,
    conversationsLabel,
    headerCounts,
    nextRelabelDelayMs,
    PANEL_MESSAGES,
    PANEL_SOURCES,
    PANEL_TITLES,
    panelBodyState,
    panelHeaderTitle,
    panelMessage,
    panelPresent,
    panelRows,
    panelSections,
    paneOf,
    paneTakesReserve,
    relativeUpdated,
    reviewCellText,
    reviewCellTitle,
    routePanelMove,
    type PanelSnapshot,
    PullRequestPanel,
} from "./PullRequestPanel"
import type {
    BitbucketPullRequestsState,
    GithubPullRequestsState,
    PullRequestSummary,
    UnavailableReason,
} from "../types"

const NOW_MS = Date.UTC(2026, 8, 22, 12, 0, 0)
const NOW_S = NOW_MS / 1000

test("the active PR is marked canonically and rows expose their PR number", () => {
    const html = renderToStaticMarkup(createElement(PullRequestPanel, {
        provider: "github", panel: github({ authored: [ghRow({ id: 7 })] }),
        active: { kind: "pullRequest", provider: "github", owner: "ACME", repo: "API", number: 7 },
        onOpenPullRequest() {},
    }))
    expect(html).toContain('aria-current="page"')
    expect(html).toContain("acme/api #7")
    expect(html).toContain('value="requested"')
    const bb = renderToStaticMarkup(createElement(PullRequestPanel, {
        provider: "bitbucket", panel: bitbucket(), onOpenPullRequest() {},
    }))
    expect(bb).not.toContain('value="requested"')
})

test("both providers offer status filters without a search field or empty search row", () => {
    for (const panel of [bitbucket(), github()]) {
        const html = renderToStaticMarkup(createElement(PullRequestPanel, {
            provider: panel.provider, panel, onOpenPullRequest() {},
        }))
        expect(html).not.toContain("<input")
        expect(html).not.toContain("Search pull requests")
        expect(html).toContain('<div class="pr-panel-controls"><select aria-label="Filter pull requests"')
        for (const value of ["all", "failing", "conflicts", "drafts"]) {
            expect(html).toContain(`value="${value}"`)
        }
        expect(html).not.toContain(" matching")
        expect(html).not.toContain(">Clear</button>")
    }
})

describe("filter-only panel interactions", () => {
    let dom: ReturnType<typeof installFakeDom>
    let transport: ReturnType<typeof installFakeTransport>
    let createRoot: typeof import("react-dom/client").createRoot

    beforeAll(async () => {
        dom = installFakeDom()
        transport = installFakeTransport(() => null)
        createRoot = (await import("react-dom/client")).createRoot
    })
    afterAll(() => {
        transport.uninstall()
        dom.uninstall()
    })

    function props(element: FakeElement): Record<string, unknown> {
        const key = Object.keys(element).find((name) => name.startsWith("__reactProps$"))
        if (!key) { throw new Error("the element carries no React props") }
        return (element as unknown as Record<string, Record<string, unknown>>)[key]!
    }

    async function selectFilter(container: FakeElement, value: string) {
        const select = container.findAll("select")[0]!
        await act(async () => {
            ;(props(select).onChange as (event: unknown) => void)({ currentTarget: { value } })
        })
    }

    async function clearFilter(container: FakeElement) {
        const clear = container.findAll("button").find((button) => button.textContent === "Clear")!
        await act(async () => {
            ;(props(clear).onClick as () => void)()
        })
        expect(props(container.findAll("select")[0]!).value).toBe("all")
        expect(container.textContent).not.toContain(" matching")
        expect(container.findAll("button").some((button) => button.textContent === "Clear")).toBe(false)
    }

    function listedIds(container: FakeElement) {
        return container.findAll("span")
            .filter((span) => span.getAttribute("class") === "pull-request-repo")
            .map((span) => span.textContent.match(/#(\d+)$/)?.[1])
    }

    for (const provider of ["github", "bitbucket"] as const) {
        test(`${provider} retains counts, local status filtering, no matches and Clear`, async () => {
            const makeRow = provider === "github" ? ghRow : row
            const rows = [
                makeRow({ id: 1, title: "First", checks: "passing" }),
                makeRow({ id: 2, title: "Second", checks: "failing" }),
                makeRow({ id: 3, title: "Third", draft: true, conflicting: true }),
            ]
            const panel = provider === "github"
                ? github({ authored: rows.slice(0, 2), reviewRequested: rows.slice(2) })
                : bitbucket({ pullRequests: rows })
            const container = dom.document.createElement("div")
            const root = createRoot(container as unknown as Element)
            const readsBefore = transport.calls.length
            try {
                await act(async () => root.render(createElement(PullRequestPanel, {
                    provider, panel, onOpenPullRequest() {},
                })))
                const count = container.findAll("span").find((span) =>
                    span.getAttribute("class") === "pull-request-panel-count",
                )!
                const headerTotal = count.textContent
                expect(listedIds(container)).toEqual(["1", "2", "3"])
                expect(container.findAll("input")).toHaveLength(0)
                const focusedRow = container.findAll("a")[0]!
                focusedRow.focus()
                const urgent = { ...rows[0]!, checks: "failing" as const }
                const refreshed = provider === "github"
                    ? github({ authored: [rows[1]!, urgent], reviewRequested: rows.slice(2) })
                    : bitbucket({ pullRequests: [rows[2]!, rows[1]!, urgent] })
                await act(async () => root.render(createElement(PullRequestPanel, {
                    provider, panel: refreshed, onOpenPullRequest() {},
                })))
                expect(listedIds(container)).toEqual(["1", "2", "3"])
                expect(container.findAll("a")[0]).toBe(focusedRow)
                expect(dom.document.activeElement).toBe(focusedRow)
                expect(focusedRow.findAll("span").some((span) =>
                    span.getAttribute("aria-label") === "Checks failing",
                )).toBe(true)
                await act(async () => root.render(createElement(PullRequestPanel, {
                    provider, panel, onOpenPullRequest() {},
                })))
                for (const [filter, ids] of [
                    ["failing", ["2"]], ["conflicts", ["3"]], ["drafts", ["3"]],
                    ...(provider === "github" ? [["requested", ["3"]]] as const : []),
                ] as const) {
                    await selectFilter(container, filter)
                    expect(listedIds(container)).toEqual([...ids])
                    expect(container.textContent).toContain("1 / 3 matching")
                    expect(count.textContent).toBe(headerTotal)
                    await clearFilter(container)
                    expect(listedIds(container)).toEqual(["1", "2", "3"])
                }
                const updatedRows = rows.map((pr) => ({ ...pr, checks: "passing" as const }))
                const updated = provider === "github"
                    ? github({ authored: updatedRows.slice(0, 2), reviewRequested: updatedRows.slice(2) })
                    : bitbucket({ pullRequests: updatedRows })
                await selectFilter(container, "failing")
                await act(async () => root.render(createElement(PullRequestPanel, {
                    provider, panel: updated, onOpenPullRequest() {},
                })))
                expect(listedIds(container)).toEqual([])
                expect(container.textContent).toContain("0 / 3 matching")
                expect(container.textContent).toContain("No matching pull requests.")
                expect(container.textContent).not.toContain(PANEL_MESSAGES[provider].empty)
                expect(container.textContent).not.toContain(PANEL_MESSAGES[provider].unauthenticated)
                expect(count.textContent).toBe(headerTotal)
                await clearFilter(container)
                expect(listedIds(container)).toEqual(["1", "2", "3"])
                expect(transport.calls).toHaveLength(readsBefore)
            } finally {
                await act(async () => root.unmount())
            }
        })
    }
})

function row(overrides: Partial<PullRequestSummary> = {}): PullRequestSummary {
    return {
        id: 1,
        title: "Add the panel",
        repoFullName: "acme/app",
        sourceRepoFullName: "acme/app",
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
            reason: null,
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

// Presence decides whether the rail exists at all (`spec-browser`: *Rail
// Exists Only While Occupied*), so it must be exactly "renders something":
// a panel showing only a quiet line still holds its slot open.
describe("panelPresent", () => {
    test("not present before the first snapshot has been read", () => {
        expect(panelPresent(null)).toBe(false)
    })

    test("not present while the feature is disabled", () => {
        expect(panelPresent(bitbucket({ status: "disabled", pullRequests: [] }))).toBe(false)
        expect(
            panelPresent(github({ status: "disabled", authored: [], reviewRequested: [] })),
        ).toBe(false)
    })

    test("present whenever it renders a body, rows or a one-line message", () => {
        expect(panelPresent(bitbucket())).toBe(true)
        expect(panelPresent(github())).toBe(true)
        expect(panelPresent(bitbucket({ pullRequests: [] }))).toBe(true)
        expect(panelPresent(github({ authored: [], reviewRequested: [] }))).toBe(true)
        expect(panelPresent(bitbucket({ status: "unauthenticated", pullRequests: [] }))).toBe(
            true,
        )
        expect(panelPresent(github({ status: "unavailable", authored: [], reviewRequested: [] }))).toBe(
            true,
        )
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

// `bitbucket-pull-requests`: *Unavailable and Stale Snapshots Carry a Reason*.
describe("unavailable reasons", () => {
    function unavailable(reason: UnavailableReason | null): PanelSnapshot {
        return bitbucket({ status: "unavailable", fetchedAtUnix: null, pullRequests: [], reason })
    }

    test("an untrusted certificate says this machine does not trust it, and where a root goes", () => {
        expect(panelMessage(unavailable({ kind: "certificateUntrusted" }), "unavailable")).toBe(
            "BitBucket is unavailable: this machine does not trust the server's certificate. " +
                "Add your organisation's root CA to the system store or SSL_CERT_FILE.",
        )
    })

    test("each other reason has its own words", () => {
        const cases: [UnavailableReason, string][] = [
            [{ kind: "tls" }, "the secure connection could not be set up."],
            [{ kind: "dns" }, "the host name api.bitbucket.org did not resolve."],
            [{ kind: "connect" }, "the connection was refused, reset or unreachable."],
            [{ kind: "timeout" }, "the request timed out."],
            [{ kind: "httpStatus", status: 503 }, "it answered HTTP 503."],
            [{ kind: "unexpectedResponse" }, "its reply could not be read."],
            [{ kind: "rateLimited" }, "too many requests, so SpecForge waits before asking again."],
        ]
        for (const [reason, words] of cases) {
            expect(panelMessage(unavailable(reason), "unavailable")).toBe(
                `BitBucket is unavailable: ${words}`,
            )
        }
    })

    test("an unavailable snapshot without a reason keeps today's line", () => {
        expect(panelMessage(unavailable(null), "unavailable")).toBe(
            "BitBucket is unavailable right now.",
        )
    })

    test("GitHub's lines, and BitBucket's other lines, are unchanged", () => {
        expect(
            panelMessage(github({ status: "unavailable", authored: [], reviewRequested: [] }), "unavailable"),
        ).toBe("GitHub is unavailable right now.")
        expect(
            panelMessage(bitbucket({ status: "unauthenticated", pullRequests: [] }), "unauthenticated"),
        ).toBe(PANEL_MESSAGES.bitbucket.unauthenticated)
        expect(panelMessage(bitbucket({ pullRequests: [] }), "empty")).toBe("No open pull requests.")
    })

    test("the unavailable panel renders the reasoned line", () => {
        const html = renderToStaticMarkup(
            createElement(PullRequestPanel, {
                provider: "bitbucket",
                panel: unavailable({ kind: "timeout" }),
                onOpenPullRequest() {},
            }),
        )
        expect(html).toContain("BitBucket is unavailable: the request timed out.")
        expect(html).not.toContain("right now")
    })

    test("a stale snapshot keeps its rows and shows its reason on request", () => {
        const stale = bitbucket({ stale: true, reason: { kind: "timeout" } })
        expect(panelBodyState(stale)).toBe("rows")
        const title = panelHeaderTitle(stale)
        expect(title).toContain(
            "Showing the last list — BitBucket is unavailable: the request timed out.",
        )
        expect(title).not.toContain("could not be reached")
        expect(title).not.toContain("update-ca-certificates")
    })

    test("an untrusted certificate's tooltip says how a root CA is added", () => {
        const stale = panelHeaderTitle(bitbucket({ stale: true, reason: { kind: "certificateUntrusted" } }))
        expect(stale).toContain("does not trust the server's certificate")
        expect(stale).toContain(CERTIFICATE_HINT)
        expect(CERTIFICATE_HINT).toContain("update-ca-certificates")
        expect(panelHeaderTitle(unavailable({ kind: "certificateUntrusted" }))).toContain(
            CERTIFICATE_HINT,
        )
        expect(panelHeaderTitle(unavailable({ kind: "dns" }))).not.toContain(CERTIFICATE_HINT)
    })
})

describe("provider routing", () => {
    test("each panel listens to its own provider's update event", () => {
        expect(PANEL_SOURCES.github.onUpdated).toBe(onGithubPullRequestsUpdated)
        expect(PANEL_SOURCES.bitbucket.onUpdated).toBe(onBitbucketPullRequestsUpdated)
        expect(PANEL_SOURCES.github.onUpdated).not.toBe(PANEL_SOURCES.bitbucket.onUpdated)
    })

    test("each panel fetches its own provider's snapshot", () => {
        // The fetchers wrap the getters, so pin them by what they call.
        expect(PANEL_SOURCES.github.fetch.toString()).toContain(getGithubPullRequests.name)
        expect(PANEL_SOURCES.bitbucket.fetch.toString()).toContain(getBitbucketPullRequests.name)
    })

    test("a move re-seats only the panel it names", () => {
        expect(routePanelMove({ provider: "github", position: "right-top" })).toEqual({
            provider: "github",
            position: "right-top",
        })
        expect(routePanelMove({ provider: "bitbucket", position: "left-top" })).toEqual({
            provider: "bitbucket",
            position: "left-top",
        })
    })

    test("a frame naming no known provider or no position routes nowhere", () => {
        expect(routePanelMove(undefined)).toBeNull()
        expect(routePanelMove(null)).toBeNull()
        expect(
            routePanelMove({ provider: "gitlab", position: "left-top" } as unknown as Parameters<
                typeof routePanelMove
            >[0]),
        ).toBeNull()
        expect(
            routePanelMove({ provider: "github" } as unknown as Parameters<typeof routePanelMove>[0]),
        ).toBeNull()
    })
})
