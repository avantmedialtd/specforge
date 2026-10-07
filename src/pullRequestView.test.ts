import { describe, expect, test } from "bun:test"
import {
    announcementCallsForRead,
    answeredRead,
    askedRead,
    checkLink,
    checkStateLabel,
    deferralText,
    fileFailureText,
    hostFileLink,
    KEYED_BY_HEAD_REASON,
    linkedChange,
    minimisedText,
    NO_READ,
    noticeOffersRefresh,
    noticeWords,
    popOutAddress,
    progressByPath,
    progressWords,
    providerPage,
    readFor,
    referenceKey,
    resolutionNotice,
    reviewStateLabel,
    shownRow,
    splitThreads,
    threadPlace,
    unlistedFilesText,
    viewedMark,
    type PullRequestNotice,
    type ViewRead,
} from "./pullRequestView"
import type {
    ArtifactStatus,
    ChangeData,
    ChangeInstance,
    DiffFile,
    PullRequestDetail,
    PullRequestLinks,
    PullRequestSummary,
    ReviewProgress,
    ReviewThread,
    WorkspaceView,
} from "./types"

// ---- Fixture builders --------------------------------------------------

function row(overrides: Partial<PullRequestSummary> = {}): PullRequestSummary {
    return {
        id: 42,
        title: "Add rate limits",
        repoFullName: "acme/api",
        sourceRepoFullName: "acme/api",
        sourceBranch: "rate-limits",
        destinationBranch: "main",
        url: "https://github.com/acme/api/pull/42",
        draft: false,
        updatedAtUnix: 1_000,
        review: null,
        openTasks: 0,
        author: "ada",
        checks: "pending",
        conflicting: false,
        unresolvedThreads: 1,
        ...overrides,
    }
}

function detail(overrides: Partial<PullRequestDetail> = {}): PullRequestDetail {
    return {
        reference: { provider: "github", owner: "Acme", repo: "API", number: 42 },
        row: row({ repoFullName: "Acme/API", url: "https://github.com/Acme/API/pull/42" }),
        headBranch: "rate-limits",
        baseBranch: "main",
        headCommit: "head1",
        baseCommit: "base1",
        author: "ada",
        description: "",
        conversation: [],
        checks: [],
        threads: [],
        files: [],
        unlistedFiles: 0,
        readAtUnix: 900,
        noLongerListed: false,
        ...overrides,
    }
}

function thread(overrides: Partial<ReviewThread> = {}): ReviewThread {
    return {
        id: "t1",
        path: "src/api.ts",
        side: "new",
        line: 12,
        startSide: null,
        startLine: null,
        originalLine: null,
        originalStartLine: null,
        resolved: false,
        outdated: false,
        comments: [],
        ...overrides,
    }
}

function file(newPath: string | null, oldPath: string | null = newPath): DiffFile {
    return {
        oldPath,
        newPath,
        oldMode: null,
        newMode: null,
        status: { kind: "modified" },
        additions: 1,
        deletions: 1,
        content: { kind: "withheld" },
    }
}

/// A read state as the view holds it once it has asked under `asked`.
function readAfterAsking(asked: PullRequestSummary): ViewRead {
    return askedRead(NO_READ, asked)
}

// ---- Freshness ---------------------------------------------------------

describe("whether an announcement calls for a read", () => {
    test("a checks change calls for a read, and a title-only change does not", () => {
        const read = readAfterAsking(row())
        expect(announcementCallsForRead(read, row({ checks: "failing" }), 2_000)).toBe(true)
        expect(announcementCallsForRead(read, row({ title: "Add rate limits, take two" }), 2_000)).toBe(
            false,
        )
    })

    test("an updated time or an unresolved-conversation count that changed calls for a read", () => {
        const read = readAfterAsking(row())
        expect(announcementCallsForRead(read, row({ updatedAtUnix: 1_001 }), 2_000)).toBe(true)
        expect(announcementCallsForRead(read, row({ unresolvedThreads: 2 }), 2_000)).toBe(true)
    })

    test("an unchanged row calls for nothing, whatever else the row says", () => {
        const read = readAfterAsking(row())
        expect(announcementCallsForRead(read, row(), 2_000)).toBe(false)
        expect(
            announcementCallsForRead(read, row({ draft: true, conflicting: true, openTasks: 3 }), 2_000),
        ).toBe(false)
    })

    // `pull-request-viewer`: *A deferred view reads at the first announcement
    // after its time*.
    test("after a deferral whose time has passed, an unchanged announcement reads, once", () => {
        const deferred = answeredRead(readAfterAsking(row()), {
            kind: "deferred",
            untilUnix: 1_600,
            detail: null,
        })
        // Before the time has passed, an unchanged announcement asks nothing,
        // and neither does a changed one: the view sends nothing meanwhile.
        expect(announcementCallsForRead(deferred, row(), 1_599)).toBe(false)
        expect(announcementCallsForRead(deferred, row({ checks: "failing" }), 1_599)).toBe(false)
        // At the time and after it, the first announcement reads.
        expect(announcementCallsForRead(deferred, row(), 1_600)).toBe(true)
        expect(announcementCallsForRead(deferred, row(), 1_700)).toBe(true)
        // Once that read is sent, a further unchanged announcement does not.
        const asked = askedRead(deferred, row())
        expect(asked.deferredUntilUnix).toBeNull()
        expect(announcementCallsForRead(asked, row(), 1_701)).toBe(false)
    })

    test("a view that has not asked under any row reads at the first announcement", () => {
        expect(announcementCallsForRead(NO_READ, row(), 2_000)).toBe(true)
        expect(announcementCallsForRead(askedRead(NO_READ, null), row(), 2_000)).toBe(true)
    })
})

describe("readFor", () => {
    test("opening, a refused withheld file and a manual refresh each ask, and only the refresh is manual", () => {
        const read = readAfterAsking(row())
        expect(readFor(read, { kind: "open" })).toEqual({ manual: false })
        expect(readFor(read, { kind: "fileRefused" })).toEqual({ manual: false })
        expect(readFor(read, { kind: "manual" })).toEqual({ manual: true })
    })

    test("an announcement asks by the freshness rule", () => {
        const read = readAfterAsking(row())
        expect(readFor(read, { kind: "announcement", row: row(), nowUnix: 2_000 })).toBeNull()
        expect(
            readFor(read, { kind: "announcement", row: row({ checks: "failing" }), nowUnix: 2_000 }),
        ).toEqual({ manual: false })
    })
})

// ---- Answers -----------------------------------------------------------

describe("what an answer leaves on screen", () => {
    const shown = detail()
    const showing: ViewRead = { ...readAfterAsking(row()), detail: shown }

    test("a detail replaces what was shown and clears the notice", () => {
        const fresh = detail({ headCommit: "head2" })
        const read = answeredRead(
            { ...showing, notice: { kind: "transient" } },
            { kind: "detail", detail: fresh },
        )
        expect(read.detail).toBe(fresh)
        expect(read.notice).toBeNull()
    })

    test("a deferral shows the detail it carries, else keeps the one shown, and names its time", () => {
        const cached = detail({ headCommit: "cached" })
        expect(
            answeredRead(showing, { kind: "deferred", untilUnix: 1_600, detail: cached }),
        ).toMatchObject({
            detail: cached,
            notice: { kind: "deferred", untilUnix: 1_600 },
            deferredUntilUnix: 1_600,
        })
        expect(answeredRead(showing, { kind: "deferred", untilUnix: 1_600, detail: null }).detail).toBe(
            shown,
        )
    })

    // So a re-read keeps the withheld files the reader loaded in the diff view,
    // which drops them with the array they were loaded for.
    test("a detail read at the same commits keeps the files on screen, and one after a push brings its own", () => {
        const again = detail({ files: [file("src/api.ts")], conversation: [] })
        const kept = answeredRead(showing, { kind: "detail", detail: again })
        expect(kept.detail?.files).toBe(shown.files)
        expect(kept.detail?.conversation).toBe(again.conversation)
        const deferredAgain = answeredRead(showing, {
            kind: "deferred",
            untilUnix: 1_600,
            detail: again,
        })
        expect(deferredAgain.detail?.files).toBe(shown.files)

        const pushed = detail({ headCommit: "head2", files: [file("src/api.ts")] })
        expect(answeredRead(showing, { kind: "detail", detail: pushed }).detail?.files).toBe(
            pushed.files,
        )
        const retargeted = detail({ baseCommit: "base2", files: [file("src/api.ts")] })
        expect(answeredRead(showing, { kind: "detail", detail: retargeted }).detail?.files).toBe(
            retargeted.files,
        )
    })

    test("a cache-only call with nothing cached paints nothing and changes nothing", () => {
        expect(answeredRead(NO_READ, { kind: "notCached" })).toBe(NO_READ)
        expect(answeredRead(showing, { kind: "notCached" })).toBe(showing)
    })

    test("a transient failure keeps the detail shown", () => {
        const read = answeredRead(showing, { kind: "transient" })
        expect(read.detail).toBe(shown)
        expect(read.notice).toEqual({ kind: "transient" })
    })

    test("not listed, refused, unauthenticated and unavailable leave only their notice", () => {
        for (const kind of ["notListed", "refused", "unauthenticated", "unavailable"] as const) {
            const read = answeredRead(showing, { kind })
            expect(read.detail).toBeNull()
            expect(read.notice).toEqual({ kind })
        }
    })

    test("asking keeps what is shown until the answer lands", () => {
        const deferred = answeredRead(showing, { kind: "deferred", untilUnix: 1_600, detail: null })
        const asked = askedRead(deferred, row({ checks: "failing" }))
        expect(asked.detail).toBe(shown)
        expect(asked.notice).toEqual({ kind: "deferred", untilUnix: 1_600 })
        expect(asked.askedWith?.checks).toBe("failing")
    })
})

// ---- Notices -----------------------------------------------------------

describe("the deferral's wording", () => {
    const clock = (unix: number) => `T${unix}`

    test("before its time it names when a read becomes possible", () => {
        expect(deferralText("github", 1_600, 1_000, false, clock)).toBe(
            "Reading from GitHub is paused until T1600, to stay within its rate limits. Refresh after then to read this pull request.",
        )
    })

    test("once its time has passed it says a refresh reads", () => {
        expect(deferralText("bitbucket", 1_600, 1_600, false, clock)).toBe(
            "Reading from BitBucket was paused until T1600. Refresh to read this pull request now.",
        )
    })

    test("it says when the last read stays on screen", () => {
        expect(deferralText("github", 1_600, 1_000, true, clock)).toStartWith(
            "Showing the last read. Reading from GitHub is paused until T1600",
        )
    })

    test("the notice carries the same words", () => {
        expect(
            noticeWords(
                "github",
                { kind: "deferred", untilUnix: 1_600 },
                { kept: false, nowUnix: 1_000, clock },
            ),
        ).toEqual({
            title: "Reading paused",
            text: deferralText("github", 1_600, 1_000, false, clock),
            settings: false,
        })
    })
})

describe("resolutionNotice", () => {
    // `view-routing`: *Pull-Request Addresses*.
    test("provider off, and an unauthenticated or unavailable list, are notices of the address", () => {
        expect(resolutionNotice({ status: "providerOff" })).toEqual({ kind: "providerOff" })
        expect(resolutionNotice({ status: "unavailable", reason: "unauthenticated" })).toEqual({
            kind: "listUnauthenticated",
        })
        expect(resolutionNotice({ status: "unavailable", reason: "unavailable" })).toEqual({
            kind: "listUnavailable",
        })
    })

    test("pending, listed and not listed are not: the view or Loading… shows instead", () => {
        expect(resolutionNotice({ status: "pending" })).toBeNull()
        expect(resolutionNotice({ status: "notListed" })).toBeNull()
        expect(
            resolutionNotice({
                status: "listed",
                row: row(),
                address: { kind: "pullRequest", provider: "github", owner: "acme", repo: "api", number: 42 },
            }),
        ).toBeNull()
    })
})

describe("noticeOffersRefresh", () => {
    test("a notice a later read may lift offers a refresh; not listed and refused do not", () => {
        expect(noticeOffersRefresh({ kind: "transient" })).toBe(true)
        expect(noticeOffersRefresh({ kind: "deferred", untilUnix: 1_600 })).toBe(true)
        expect(noticeOffersRefresh({ kind: "unauthenticated" })).toBe(true)
        expect(noticeOffersRefresh({ kind: "unavailable" })).toBe(true)
        expect(noticeOffersRefresh({ kind: "notListed" })).toBe(false)
        expect(noticeOffersRefresh({ kind: "refused" })).toBe(false)
    })
})

describe("noticeWords", () => {
    const context = { kept: false, nowUnix: 1_000 }

    test("only the notices a credential or a switch can fix point to Settings › Integrations", () => {
        const pointing: PullRequestNotice["kind"][] = [
            "providerOff",
            "refused",
            "listUnauthenticated",
            "unauthenticated",
        ]
        const notices: PullRequestNotice[] = [
            { kind: "providerOff" },
            { kind: "refused" },
            { kind: "listUnauthenticated" },
            { kind: "unauthenticated" },
            { kind: "listUnavailable" },
            { kind: "unavailable" },
            { kind: "notListed" },
            { kind: "transient" },
            { kind: "deferred", untilUnix: 2_000 },
        ]
        for (const provider of ["github", "bitbucket"] as const) {
            for (const notice of notices) {
                const words = noticeWords(provider, notice, context)
                expect(words.settings).toBe(pointing.includes(notice.kind))
                // The window shows the same words with no link, so the text
                // itself names where to go.
                if (words.settings) expect(words.text).toContain("Settings › Integrations")
            }
        }
    })

    test("the provider-off notice names its provider", () => {
        expect(noticeWords("bitbucket", { kind: "providerOff" }, context).title).toBe(
            "BitBucket is switched off",
        )
        expect(noticeWords("github", { kind: "providerOff" }, context).title).toBe(
            "GitHub is switched off",
        )
    })

    test("not listed says what each provider's list holds", () => {
        expect(noticeWords("github", { kind: "notListed" }, context).text).toContain(
            "you authored or were asked to review",
        )
        expect(noticeWords("bitbucket", { kind: "notListed" }, context).text).toContain(
            "you authored, and",
        )
    })

    test("a transient failure says whether the last read stays", () => {
        expect(noticeWords("github", { kind: "transient" }, { ...context, kept: true }).text).toContain(
            "this is the last read",
        )
        expect(noticeWords("github", { kind: "transient" }, context).text).not.toContain("last read")
    })
})

// ---- The header ----------------------------------------------------------

describe("the header's row, provider page and pop-out address", () => {
    const listed = row()
    const cached = detail()

    test("the live row while listed, else the row the cached detail was read through", () => {
        expect(shownRow(listed, cached)).toBe(listed)
        expect(shownRow(null, cached)).toBe(cached.row)
        expect(shownRow(null, null)).toBeNull()
    })

    test("the provider's page opens through the snapshot while listed, and through the link opener after", () => {
        expect(providerPage(listed, cached)).toEqual({ url: listed.url, through: "snapshot" })
        expect(providerPage(null, cached)).toEqual({ url: cached.row.url, through: "link" })
        expect(providerPage(null, null)).toBeNull()
    })

    test("the pop-out control encodes the matched row's spelling, else the cached detail's", () => {
        expect(popOutAddress("github", row({ repoFullName: "Acme/API" }), null)).toEqual({
            kind: "pullRequest",
            provider: "github",
            owner: "Acme",
            repo: "API",
            number: 42,
        })
        expect(popOutAddress("github", null, cached)).toEqual({
            kind: "pullRequest",
            ...cached.reference,
        })
        expect(popOutAddress("github", null, null)).toBeNull()
    })

    test("the key ignores ASCII case, so a respelt address keeps its view", () => {
        expect(referenceKey({ provider: "github", owner: "Acme", repo: "API", number: 42 })).toBe(
            referenceKey({ provider: "github", owner: "acme", repo: "api", number: 42 }),
        )
        expect(referenceKey({ provider: "github", owner: "acme", repo: "api", number: 42 })).not.toBe(
            referenceKey({ provider: "bitbucket", owner: "acme", repo: "api", number: 42 }),
        )
        expect(referenceKey({ provider: "github", owner: "acme", repo: "api", number: 42 })).not.toBe(
            referenceKey({ provider: "github", owner: "acme", repo: "api", number: 43 }),
        )
    })
})

describe("progressWords", () => {
    function progress(overrides: Partial<ReviewProgress>): ReviewProgress {
        return {
            files: [],
            viewed: 0,
            changedSinceViewed: 0,
            total: 0,
            lastMarkedHead: null,
            headCommit: "head",
            baseCommit: "base",
            ...overrides,
        }
    }

    // `pull-request-viewer`: *The header counts progress*.
    test("ten files with four viewed and one changed", () => {
        expect(
            progressWords(
                progress({
                    viewed: 4,
                    changedSinceViewed: 1,
                    total: 10,
                    lastMarkedHead: "abc1234def567890",
                }),
            ),
        ).toEqual({
            viewed: "4 of 10 files viewed",
            changed: "1 changed since viewed",
            since: "since you last marked, at abc1234",
        })
    })

    test("nothing changed since viewed says only the viewed count", () => {
        expect(progressWords(progress({ viewed: 1, total: 1, lastMarkedHead: "abc1234" }))).toEqual({
            viewed: "1 of 1 file viewed",
            changed: null,
            since: null,
        })
    })

    test("no progress, or no files, says nothing", () => {
        expect(progressWords(null)).toBeNull()
        expect(progressWords(progress({}))).toBeNull()
    })
})

// ---- Linked change -----------------------------------------------------

const PROPOSAL_ONLY: ArtifactStatus = { proposal: true, design: false, tasks: false, specs: [] }
const NOTHING: ArtifactStatus = { proposal: false, design: false, tasks: false, specs: [] }
const REPO = "/code/api/.git"
const WT = "/code/api-wt"
const URL_42 = "https://github.com/acme/api/pull/42"

function instance(modifiedAt: number, artifacts: ArtifactStatus = PROPOSAL_ONLY): ChangeInstance {
    return {
        worktreePath: WT,
        branch: "rate-limits",
        isMainWorktree: false,
        isDefaultBranch: false,
        isArchivedHere: false,
        change: { artifacts, completedTasks: 0, totalTasks: 0 } as unknown as ChangeData,
        modifiedAt,
        divergence: null,
        specCommitState: "committed",
    } as unknown as ChangeInstance
}

function repoView(logicals: { name: string; instances: ChangeInstance[] }[]): WorkspaceView {
    return {
        kind: "repo",
        repoId: REPO,
        mainWorktree: "/code/api",
        name: "api",
        defaultBranch: "main",
        active: logicals,
        displayName: null,
        color: null,
        dirty: false,
        dirtyWorktrees: [],
        hasUncommittedSpecs: false,
        worktrees: ["/code/api", WT],
    } as unknown as WorkspaceView
}

const LINKS: PullRequestLinks = {
    worktrees: [],
    pullRequests: [{ url: URL_42, worktrees: [{ repoId: REPO, worktreePath: WT, branch: "rate-limits" }] }],
}

describe("linkedChange", () => {
    // `pull-request-viewer`: *The center pane leads to the linked change*.
    test("names a worktree's single change", () => {
        const views = [repoView([{ name: "add-rate-limits", instances: [instance(100)] }])]
        expect(linkedChange(views, LINKS, URL_42)).toEqual({
            kind: "change",
            name: "add-rate-limits",
            repoId: REPO,
            worktreePath: WT,
        })
    })

    // *Several changes name the most recently modified*.
    test("names the more recently modified of two changes", () => {
        const views = [
            repoView([
                { name: "add-rate-limits", instances: [instance(100)] },
                { name: "fix-cache", instances: [instance(200)] },
            ]),
        ]
        expect(linkedChange(views, LINKS, URL_42)).toMatchObject({ kind: "change", name: "fix-cache" })
    })

    // *A worktree without a change shows its branch*.
    test("names the branch alone for a worktree with no change to open", () => {
        expect(linkedChange([repoView([])], LINKS, URL_42)).toEqual({
            kind: "branch",
            name: "rate-limits",
        })
        const nothingToOpen = [repoView([{ name: "empty", instances: [instance(100, NOTHING)] }])]
        expect(linkedChange(nothingToOpen, LINKS, URL_42)).toEqual({
            kind: "branch",
            name: "rate-limits",
        })
    })

    test("names nothing for an unlinked pull request", () => {
        const views = [repoView([{ name: "add-rate-limits", instances: [instance(100)] }])]
        expect(linkedChange(views, LINKS, "https://github.com/acme/api/pull/43")).toBeNull()
        expect(linkedChange(views, null, URL_42)).toBeNull()
        expect(linkedChange(views, LINKS, "")).toBeNull()
    })
})

// ---- Conversation and checks ---------------------------------------------

describe("checkLink", () => {
    // `pull-request-viewer`: *A check link with another scheme opens nothing*.
    test("a file:, data: or custom-scheme URL earns no link, while an https one does", () => {
        expect(checkLink("file:///etc/passwd")).toBeNull()
        expect(checkLink("data:text/html,<p>hi</p>")).toBeNull()
        expect(checkLink("vscode://ci/run/7")).toBeNull()
        expect(checkLink("javascript:alert(1)")).toBeNull()
        expect(checkLink("https://ci.example/run/7")).toBe("https://ci.example/run/7")
        expect(checkLink("http://ci.example/run/7")).toBe("http://ci.example/run/7")
    })

    test("a relative, hostless, empty or missing URL earns no link", () => {
        expect(checkLink("/runs/7")).toBeNull()
        expect(checkLink("https://")).toBeNull()
        expect(checkLink("")).toBeNull()
        expect(checkLink(null)).toBeNull()
    })
})

describe("the conversation's and the checks' words", () => {
    test("a check's state", () => {
        expect(checkStateLabel("failing")).toBe("Failed")
        expect(checkStateLabel("passing")).toBe("Passed")
        expect(checkStateLabel("unknown")).toBe("Unknown")
    })

    test("a review summary's state", () => {
        expect(reviewStateLabel("approved")).toBe("Approved")
        expect(reviewStateLabel("changesRequested")).toBe("Changes requested")
    })

    // *A minimised comment stays collapsed*.
    test("a minimised comment shows its reason in place of its body", () => {
        expect(minimisedText("outdated")).toBe("This comment was marked as outdated.")
        expect(minimisedText("")).toBe("This comment was hidden.")
    })
})

// ---- Threads -----------------------------------------------------------

describe("threadPlace", () => {
    // `pull-request-viewer`: *A thread names its file, side and line*.
    test("a LEFT-side line 12 is old line 12", () => {
        expect(threadPlace(thread({ side: "old", line: 12 }))).toBe("old line 12")
    })

    // *A BitBucket inline comment takes its side from its anchor*.
    test("an inline.to 7 is new line 7", () => {
        expect(threadPlace(thread({ path: "README.md", side: "new", line: 7 }))).toBe("new line 7")
    })

    test("a range on one side, and a range across sides", () => {
        expect(threadPlace(thread({ startLine: 10, line: 12 }))).toBe("new lines 10–12")
        expect(threadPlace(thread({ startSide: "old", startLine: 10, line: 12 }))).toBe(
            "old line 10 to new line 12",
        )
        expect(threadPlace(thread({ startSide: "new", startLine: 12, line: 12 }))).toBe("new line 12")
    })

    test("an outdated thread names the line it was written on", () => {
        expect(threadPlace(thread({ outdated: true, line: null, originalLine: 30 }))).toBe(
            "originally new line 30",
        )
        expect(
            threadPlace(
                thread({ outdated: true, line: null, originalLine: 30, originalStartLine: 28 }),
            ),
        ).toBe("originally new lines 28–30")
    })

    test("a thread with no line is on the whole file", () => {
        expect(threadPlace(thread({ line: null }))).toBe("the whole file")
        expect(threadPlace(thread({ outdated: true, line: null, originalLine: null }))).toBe(
            "the whole file",
        )
    })
})

describe("splitThreads", () => {
    // *A thread on an unlisted file follows the files*.
    test("a thread on a listed file sits with it, and one on any other path follows the files", () => {
        const onApi = thread({ id: "a", path: "src/api.ts" })
        const pastTheThousandth = thread({ id: "b", path: "src/file-1001.ts" })
        const split = splitThreads([onApi, pastTheThousandth], [file("src/api.ts"), file("README.md")])
        expect(split.byFile.get("src/api.ts")).toEqual([onApi])
        expect(split.byFile.has("README.md")).toBe(false)
        expect(split.unlisted).toEqual([pastTheThousandth])
    })

    test("threads keep their order within a file", () => {
        const first = thread({ id: "1" })
        const second = thread({ id: "2" })
        expect(splitThreads([first, second], [file("src/api.ts")]).byFile.get("src/api.ts")).toEqual([
            first,
            second,
        ])
    })

    test("a thread on a renamed or deleted file's old path sits with that file", () => {
        const onOldName = thread({ id: "r", path: "src/old.ts", side: "old" })
        const onDeleted = thread({ id: "d", path: "src/gone.ts", side: "old" })
        const split = splitThreads(
            [onOldName, onDeleted],
            [file("src/new.ts", "src/old.ts"), file(null, "src/gone.ts")],
        )
        expect(split.byFile.get("src/new.ts")).toEqual([onOldName])
        expect(split.byFile.get("src/gone.ts")).toEqual([onDeleted])
        expect(split.unlisted).toEqual([])
    })

    test("a path one file was renamed to keeps its threads when another was renamed from it", () => {
        const onA = thread({ path: "a.ts" })
        // `c.ts` became `a.ts`, and `a.ts` became `b.ts`: a thread on `a.ts`
        // is on the file now called `a.ts`.
        const split = splitThreads([onA], [file("b.ts", "a.ts"), file("a.ts", "c.ts")])
        expect(split.byFile.get("a.ts")).toEqual([onA])
        expect(split.byFile.has("b.ts")).toBe(false)
    })

    test("no threads, no files", () => {
        expect(splitThreads([], [])).toEqual({ byFile: new Map(), unlisted: [] })
    })
})

describe("unlistedFilesText", () => {
    test("says how many files are not listed and where every file is", () => {
        expect(unlistedFilesText(200, "github")).toBe(
            "200 more changed files aren't listed here. Open the pull request on GitHub to see every file.",
        )
        expect(unlistedFilesText(1, "bitbucket")).toStartWith("1 more changed file isn't listed here.")
        expect(unlistedFilesText(0, "github")).toBeNull()
    })
})

// ---- Viewed marks --------------------------------------------------------

describe("viewedMark", () => {
    test("a viewed file shows viewed, with no flag", () => {
        expect(viewedMark({ path: "a.ts", state: "viewed", keyedByHead: false, hunks: null })).toEqual({
            viewed: true,
            changed: false,
            reason: null,
        })
    })

    // *A push that changes a file flags it*.
    test("a file changed since viewed is flagged and shows unviewed", () => {
        expect(viewedMark({ path: "a.ts", state: "changedSinceViewed", keyedByHead: false, hunks: null })).toEqual(
            { viewed: false, changed: true, reason: null },
        )
    })

    // *A file keyed by the head commit says why it changed*.
    test("a file keyed by the head commit says why it changed", () => {
        expect(
            viewedMark({ path: "logo.png", state: "changedSinceViewed", keyedByHead: true, hunks: null }),
        ).toEqual({ viewed: false, changed: true, reason: KEYED_BY_HEAD_REASON })
        // Viewed, it has nothing to explain.
        expect(viewedMark({ path: "logo.png", state: "viewed", keyedByHead: true, hunks: null }).reason).toBeNull()
    })

    test("an unviewed file, and one progress has not been read for, show unviewed", () => {
        expect(viewedMark({ path: "a.ts", state: "unviewed", keyedByHead: false, hunks: null })).toEqual({
            viewed: false,
            changed: false,
            reason: null,
        })
        expect(viewedMark(undefined)).toEqual({ viewed: false, changed: false, reason: null })
    })

    // *Each file carries its viewed mark*.
    test("progress is looked up by each file's key path", () => {
        const byPath = progressByPath({
            files: [
                { path: "src/api.ts", state: "viewed", keyedByHead: false, hunks: null },
                { path: "README.md", state: "unviewed", keyedByHead: false, hunks: null },
            ],
            viewed: 1,
            changedSinceViewed: 0,
            total: 2,
            lastMarkedHead: "abc1234",
            headCommit: "head",
            baseCommit: "base",
        })
        expect(viewedMark(byPath.get("src/api.ts")).viewed).toBe(true)
        expect(viewedMark(byPath.get("README.md")).viewed).toBe(false)
        expect(progressByPath(null).size).toBe(0)
    })
})

describe("fileFailureText", () => {
    const clock = (unix: number) => `at ${unix}`

    test("words each reason a load could not complete, naming the provider", () => {
        expect(fileFailureText("github", "deferred", 1_700, clock)).toBe(
            "GitHub's rate limit holds until at 1700. Try again after then.",
        )
        expect(fileFailureText("github", "deferred", null, clock)).toBe(
            "GitHub's rate limit holds. Try again later.",
        )
        expect(fileFailureText("bitbucket", "unauthenticated", null, clock)).toBe(
            "BitBucket refused the credential. Check Settings › Integrations.",
        )
        expect(fileFailureText("github", "unavailable", null, clock)).toBe(
            "GitHub no longer has this file at these commits.",
        )
        expect(fileFailureText("github", "refused", null, clock)).toBe("GitHub is switched off.")
        expect(fileFailureText("github", "transient", null, clock)).toBe(
            "GitHub did not answer. Try again.",
        )
    })
})

describe("hostFileLink", () => {
    const page = "https://github.com/acme/api/pull/42"

    test("anchors a GitHub file at the SHA-256 of its path in the files tab", () => {
        expect(
            hostFileLink("github", page, { oldPath: "src/huge.json", newPath: "src/huge.json" }),
        ).toBe(
            "https://github.com/acme/api/pull/42/files#diff-dd89c4cf549b7418f9dde6cfa5beea228450b2df2f433c9d98d2c949c9c3fc2e",
        )
    })

    test("takes the new path, or a deleted file's old one, and ignores a trailing slash", () => {
        const renamed = hostFileLink("github", `${page}/`, {
            oldPath: "a.json",
            newPath: "src/huge.json",
        })
        expect(renamed).toBe(hostFileLink("github", page, { oldPath: null, newPath: "src/huge.json" }))
        expect(hostFileLink("github", page, { oldPath: "src/huge.json", newPath: null })).toBe(renamed)
    })

    test("anchors a BitBucket file at its encoded path on the diff page", () => {
        expect(
            hostFileLink("bitbucket", "https://bitbucket.org/acme/api/pull-requests/7", {
                oldPath: null,
                newPath: "docs/read me.md",
            }),
        ).toBe("https://bitbucket.org/acme/api/pull-requests/7/diff#chg-docs/read%20me.md")
    })

    test("gives no link without a page or a path", () => {
        expect(hostFileLink("github", "", { oldPath: null, newPath: "a" })).toBeNull()
        expect(hostFileLink("github", page, { oldPath: null, newPath: null })).toBeNull()
    })
})
