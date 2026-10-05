import { describe, expect, test } from "bun:test"
import {
    handlePullRequestClick,
    isActivationSpace,
    PULL_REQUEST_TITLE_CAP,
    pullRequestAddressFor,
    pullRequestGesture,
    pullRequestTitle,
    pullRequestWindowName,
    pullRequestWindowPath,
    type ClickModifiers,
    type PullRequestOpeners,
} from "./pullRequestOpen"
import type { PullRequestAddress } from "./routing/address"
import { decodeAddress, encodeAddress } from "./routing/codec"
import { shortHash } from "./routing/slug"
import type { LinkedPullRequest, PullRequestReference, PullRequestSummary } from "./types"
import { windowKind } from "./windowKind"

const MAC =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15"
const WINDOWS =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
const LINUX =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"

const PLAIN: ClickModifiers = { metaKey: false, ctrlKey: false, shiftKey: false, altKey: false }

function click(held: Partial<ClickModifiers> = {}): ClickModifiers {
    return { ...PLAIN, ...held }
}

describe("pullRequestGesture", () => {
    test("a plain click navigates on every platform", () => {
        for (const ua of [MAC, WINDOWS, LINUX]) {
            expect(pullRequestGesture(click(), ua)).toBe("navigate")
        }
    })

    // Enter and Space reach the handler as the click they become: natively on
    // a button and for a link's Enter, through `isActivationSpace` for a
    // link's Space. Unmodified, that click navigates; held with the platform's
    // modifier, it opens the window as a click would.
    test("Enter and Space open as a click does", () => {
        for (const ua of [MAC, WINDOWS, LINUX]) {
            expect(pullRequestGesture(click(), ua)).toBe("navigate")
        }
        expect(pullRequestGesture(click({ metaKey: true }), MAC)).toBe("window")
        expect(pullRequestGesture(click({ ctrlKey: true }), WINDOWS)).toBe("window")
    })

    test("Cmd-click opens the window on macOS", () => {
        expect(pullRequestGesture(click({ metaKey: true }), MAC)).toBe("window")
        expect(pullRequestGesture(click({ metaKey: true, shiftKey: true }), MAC)).toBe("window")
    })

    test("Ctrl-click opens the window on Windows and Linux", () => {
        for (const ua of [WINDOWS, LINUX]) {
            expect(pullRequestGesture(click({ ctrlKey: true }), ua)).toBe("window")
            expect(pullRequestGesture(click({ ctrlKey: true, shiftKey: true }), ua)).toBe("window")
        }
    })

    test("the secondary click opens nothing on macOS", () => {
        expect(pullRequestGesture(click({ ctrlKey: true }), MAC)).toBe("nothing")
        // Cmd held as well is still the secondary click, not the gesture.
        expect(pullRequestGesture(click({ ctrlKey: true, metaKey: true }), MAC)).toBe("nothing")
        expect(pullRequestGesture(click({ ctrlKey: true, shiftKey: true }), MAC)).toBe("nothing")
    })

    test("any other modified click is left to the element on every platform", () => {
        for (const ua of [MAC, WINDOWS, LINUX]) {
            expect(pullRequestGesture(click({ shiftKey: true }), ua)).toBe("default")
            expect(pullRequestGesture(click({ altKey: true }), ua)).toBe("default")
        }
        // The Windows and Super keys are not the modifier there.
        expect(pullRequestGesture(click({ metaKey: true }), WINDOWS)).toBe("default")
        expect(pullRequestGesture(click({ metaKey: true }), LINUX)).toBe("default")
    })
})

describe("isActivationSpace", () => {
    test("Space activates a browser-skin row or chip; Enter is the link's own", () => {
        expect(isActivationSpace(" ")).toBe(true)
        expect(isActivationSpace("Spacebar")).toBe(true)
        expect(isActivationSpace("Enter")).toBe(false)
        expect(isActivationSpace("a")).toBe(false)
    })
})

describe("handlePullRequestClick", () => {
    const address: PullRequestAddress = {
        kind: "pullRequest",
        provider: "github",
        owner: "acme",
        repo: "api",
        number: 42,
    }

    /// A click as a handler receives it, and every effect handling it had.
    function handle(held: Partial<ClickModifiers>, ua: string, title: string | null = "Add rate limits") {
        const effects: string[] = []
        const openers: PullRequestOpeners = {
            navigate: (to) => effects.push(`navigate ${JSON.stringify(to)}`),
            openWindow: (path, windowTitle) => effects.push(`window ${path} | ${windowTitle}`),
        }
        let prevented = false
        handlePullRequestClick(
            { ...click(held), preventDefault: () => (prevented = true) },
            address,
            title,
            openers,
            ua,
        )
        return { effects, prevented }
    }

    test("a click shows the pull request in the center pane, at the row's address", () => {
        for (const ua of [MAC, WINDOWS, LINUX]) {
            expect(handle({}, ua)).toEqual({
                effects: [`navigate ${JSON.stringify(address)}`],
                prevented: true,
            })
        }
    })

    test("the new-window gesture opens the pull request's own window and nothing else", () => {
        const opened = {
            effects: ["window /pr/github/acme/api/42 | #42 Add rate limits — acme/api"],
            prevented: true,
        }
        expect(handle({ metaKey: true }, MAC)).toEqual(opened)
        expect(handle({ ctrlKey: true }, WINDOWS)).toEqual(opened)
        expect(handle({ ctrlKey: true }, LINUX)).toEqual(opened)
    })

    // Opened before the handler returns, so inside the click, where no popup
    // blocker intervenes.
    test("the window opens synchronously, inside the click", () => {
        let openedDuringClick = false
        handlePullRequestClick(
            { ...click({ metaKey: true }), preventDefault: () => {} },
            address,
            null,
            {
                navigate: () => {},
                openWindow: () => {
                    openedDuringClick = true
                },
            },
            MAC,
        )
        expect(openedDuringClick).toBe(true)
    })

    test("a window opened before the title is known is named without one", () => {
        expect(handle({ metaKey: true }, MAC, null).effects).toEqual([
            "window /pr/github/acme/api/42 | #42 — acme/api",
        ])
    })

    test("the secondary click on macOS opens nothing and follows nothing", () => {
        expect(handle({ ctrlKey: true }, MAC)).toEqual({ effects: [], prevented: true })
    })

    test("any other modified click is left to the element", () => {
        for (const ua of [MAC, WINDOWS, LINUX]) {
            expect(handle({ shiftKey: true }, ua)).toEqual({ effects: [], prevented: false })
            expect(handle({ altKey: true }, ua)).toEqual({ effects: [], prevented: false })
        }
    })
})

describe("pullRequestAddressFor", () => {
    const row = { id: 42, repoFullName: "acme/api", url: "https://github.com/acme/api/pull/42" }

    test("a row opens its own address", () => {
        expect(pullRequestAddressFor("github", row)).toEqual({
            kind: "pullRequest",
            provider: "github",
            owner: "acme",
            repo: "api",
            number: 42,
        })
        expect(encodeAddress(pullRequestAddressFor("github", row)!)).toBe("/pr/github/acme/api/42")
    })

    test("a pull request has an address, of identifiers only", () => {
        const fullRow: PullRequestSummary = {
            id: 42,
            title: "Add rate limits",
            repoFullName: "acme/api",
            sourceRepoFullName: "acme/api",
            sourceBranch: "rate-limits",
            destinationBranch: "main",
            url: "https://github.com/acme/api/pull/42",
            draft: false,
            updatedAtUnix: 1_700_000_000,
            review: null,
            openTasks: 0,
            author: "ada",
            checks: "passing",
            conflicting: false,
            unresolvedThreads: 0,
        }
        const address = pullRequestAddressFor("github", fullRow)!
        expect(Object.keys(address).sort()).toEqual(["kind", "number", "owner", "provider", "repo"])
        expect(JSON.stringify(address)).not.toContain("Add rate limits")
        expect(JSON.stringify(address)).not.toContain("https://")
    })

    test("a linked pull request's address carries no worktree path", () => {
        // A chip's pull request, linked to the worktree at
        // /Users/ada/src/api-feature: the address names the provider's
        // repository, never the worktree.
        const linked: LinkedPullRequest = {
            provider: "github",
            role: "authored",
            id: 42,
            title: "Add rate limits",
            url: "https://github.com/acme/api/pull/42",
            repoFullName: "acme/api",
            draft: false,
            checks: null,
            conflicting: false,
            review: null,
        }
        const path = encodeAddress(pullRequestAddressFor(linked.provider, linked)!)
        expect(path).toBe("/pr/github/acme/api/42")
        expect(path).not.toContain("/Users/ada/src/api-feature")
    })

    test("the address is spelt as the row spells it", () => {
        expect(pullRequestAddressFor("github", { ...row, repoFullName: "Acme/API" })).toMatchObject({
            owner: "Acme",
            repo: "API",
        })
    })

    test("a BitBucket row names its workspace as the owner", () => {
        expect(
            pullRequestAddressFor("bitbucket", {
                id: 7,
                repoFullName: "acme/api",
                url: "https://bitbucket.org/acme/api/pull-requests/7",
            }),
        ).toEqual({ kind: "pullRequest", provider: "bitbucket", owner: "acme", repo: "api", number: 7 })
    })

    test("a row without a provider URL opens nothing", () => {
        expect(pullRequestAddressFor("github", { ...row, url: "" })).toBeNull()
    })

    test("a row no path could name opens nothing", () => {
        for (const repoFullName of ["", "acme", "/api", "acme/"]) {
            expect(pullRequestAddressFor("github", { ...row, repoFullName })).toBeNull()
        }
        for (const id of [0, -1, 4.2, Number.NaN, Number.MAX_SAFE_INTEGER + 1]) {
            expect(pullRequestAddressFor("github", { ...row, id })).toBeNull()
        }
    })

    test("every address a row opens round-trips through the codec", () => {
        for (const sample of [
            row,
            { ...row, repoFullName: "Acme/API" },
            { ...row, repoFullName: "acme/a b" },
            { ...row, id: 1 },
            { ...row, id: Number.MAX_SAFE_INTEGER },
        ]) {
            const address = pullRequestAddressFor("github", sample)!
            expect(decodeAddress(encodeAddress(address))).toEqual(address)
        }
    })
})

describe("the pull request's own window in the browser skin", () => {
    const path = "/pr/github/acme/api/42"

    test("its tab sits at the address's path, with the flag outside it", () => {
        expect(pullRequestWindowPath(path)).toBe("/pr/github/acme/api/42?pullRequest=1")
    })

    test("the page it opens is read back as the pull-request window", () => {
        const opened = new URL(pullRequestWindowPath(path), "http://127.0.0.1:4317")
        expect(opened.pathname).toBe(path)
        expect(windowKind(opened.search)).toBe("pullRequest")
    })

    test("its tab is named from the hash of the path", () => {
        expect(pullRequestWindowName(path)).toBe(`specforge-pull-request:${shortHash(path)}`)
    })

    test("one pull request has one tab name, and two have two", () => {
        expect(pullRequestWindowName(path)).toBe(pullRequestWindowName("/pr/github/acme/api/42"))
        expect(pullRequestWindowName(path)).not.toBe(pullRequestWindowName("/pr/github/acme/api/43"))
        expect(pullRequestWindowName(path)).not.toBe(
            pullRequestWindowName("/pr/bitbucket/acme/api/42"),
        )
    })
})

describe("pullRequestTitle", () => {
    const github42: PullRequestReference = { provider: "github", owner: "acme", repo: "api", number: 42 }

    test("the title names the pull request", () => {
        expect(pullRequestTitle(github42, "Add rate limits")).toBe("#42 Add rate limits — acme/api")
    })

    test("the title before the detail arrives", () => {
        expect(pullRequestTitle(github42)).toBe("#42 — acme/api")
        expect(pullRequestTitle(github42, null)).toBe("#42 — acme/api")
        expect(pullRequestTitle(github42, "")).toBe("#42 — acme/api")
    })

    test("a BitBucket pull request's owner is its workspace", () => {
        expect(
            pullRequestTitle({ provider: "bitbucket", owner: "acme", repo: "api", number: 7 }, "Fix"),
        ).toBe("#7 Fix — acme/api")
    })

    test("hidden and control characters never reach the title", () => {
        const title = pullRequestTitle(github42, "Add‮ rate​ limits\n")
        expect(title).toBe("#42 Add rate limits — acme/api")
        for (const hidden of ["‮", "​", "\n"]) {
            expect(title).not.toContain(hidden)
        }
    })

    test("the names are cleaned as the title is", () => {
        expect(
            pullRequestTitle({ ...github42, owner: "ac​me", repo: "a⁦pi\u0007" }, "Fix"),
        ).toBe("#42 Fix — acme/api")
    })

    test("a title that is all hidden characters reads as no title", () => {
        expect(pullRequestTitle(github42, "​‮ \n")).toBe("#42 — acme/api")
    })

    test("an overlong title is capped", () => {
        const title = pullRequestTitle(github42, "x".repeat(5000))
        expect(PULL_REQUEST_TITLE_CAP).toBe(200)
        expect([...title]).toHaveLength(PULL_REQUEST_TITLE_CAP)
        expect(title.startsWith("#42 xxx")).toBe(true)
    })

    test("a title at the cap is kept whole, and one past it is cut", () => {
        // "#42 " is four code points, " — acme/api" eleven.
        const fits = "x".repeat(PULL_REQUEST_TITLE_CAP - 4 - 11)
        expect(pullRequestTitle(github42, fits)).toBe(`#42 ${fits} — acme/api`)
        expect(pullRequestTitle(github42, `${fits}y`)).toBe(`#42 ${fits}y — acme/ap`)
    })

    test("the cut falls between code points, never inside a surrogate pair", () => {
        // After "#42 a", five code points, a UTF-16 cut at 200 would land in
        // the middle of the 98th emoji.
        const title = pullRequestTitle(github42, `a${"\u{1F600}".repeat(3000)}`)
        expect([...title]).toHaveLength(PULL_REQUEST_TITLE_CAP)
        expect(title.length).toBe(5 + 2 * (PULL_REQUEST_TITLE_CAP - 5))
        expect(title).not.toMatch(/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/)
        expect(title.endsWith("\u{1F600}")).toBe(true)
    })
})
