import { describe, expect, test } from "bun:test"
import {
    imageWindowAddress,
    imageWindowName,
    imageWindowPath,
    imageWindowTitle,
    parseImageWindowAddress,
    type ImageWindowSource,
} from "./imageWindow"
import { shortHash } from "./routing/slug"

const commit: ImageWindowSource = {
    kind: "commit",
    repoId: "/Users/ada/specforge/.git",
    sha: "c7701f25a562119cf53d15be726404dc26f08353",
    path: "icons/new.png",
    oldPath: "icons/old.png",
    sides: { old: "4c5774c", new: "c7701f2" },
}

const pullRequest: ImageWindowSource = {
    kind: "pullRequest",
    reference: { provider: "github", owner: "acme", repo: "web", number: 19 },
    path: "e2e/baselines/home.png",
    head: "77f3ae9cc6d1c0edf09c0947c5fc9a0a14efdb9e",
    base: "7b1cb0b3c80624dbe25b0c68cc293eaad519b6b8",
    sides: { old: "master", new: "feature/home" },
}

describe("the zoom window's address", () => {
    test("names a commit's or a pull request's file, and reads back as the same source", () => {
        for (const source of [commit, pullRequest, { ...commit, oldPath: null }]) {
            expect(parseImageWindowAddress(imageWindowAddress(source))).toEqual(source)
        }
    })

    test("is one string per file, whatever order the source was built in", () => {
        const reordered: ImageWindowSource = {
            sides: { new: "feature/home", old: "master" },
            base: pullRequest.base,
            head: pullRequest.head,
            path: pullRequest.path,
            reference: { number: 19, repo: "web", owner: "acme", provider: "github" },
            kind: "pullRequest",
        }
        expect(imageWindowAddress(reordered)).toBe(imageWindowAddress(pullRequest))
        expect(imageWindowAddress(commit)).not.toBe(
            imageWindowAddress({ ...commit, path: "icons/other.png" }),
        )
    })

    test("refuses anything that is not one", () => {
        const spoiled = (change: (value: Record<string, unknown>) => void) => {
            const value = JSON.parse(imageWindowAddress(commit)) as Record<string, unknown>
            change(value)
            return parseImageWindowAddress(JSON.stringify(value))
        }
        expect(parseImageWindowAddress(null)).toBeNull()
        expect(parseImageWindowAddress("")).toBeNull()
        expect(parseImageWindowAddress("not json")).toBeNull()
        expect(parseImageWindowAddress("[]")).toBeNull()
        expect(spoiled((v) => (v.kind = "file"))).toBeNull()
        expect(spoiled((v) => (v.sha = "HEAD"))).toBeNull()
        expect(spoiled((v) => (v.sha = "--output=x"))).toBeNull()
        expect(spoiled((v) => (v.repoId = ""))).toBeNull()
        expect(spoiled((v) => (v.path = ""))).toBeNull()
        expect(spoiled((v) => (v.oldPath = ""))).toBeNull()
        expect(spoiled((v) => (v.oldPath = 7))).toBeNull()
        expect(spoiled((v) => (v.sides = { old: "a" }))).toBeNull()
        expect(spoiled((v) => delete v.sides)).toBeNull()

        const spoiledPullRequest = (change: (reference: Record<string, unknown>) => void) => {
            const value = JSON.parse(imageWindowAddress(pullRequest)) as {
                reference: Record<string, unknown>
            }
            change(value.reference)
            return parseImageWindowAddress(JSON.stringify(value))
        }
        expect(spoiledPullRequest((r) => (r.provider = "gitlab"))).toBeNull()
        expect(spoiledPullRequest((r) => (r.number = 0))).toBeNull()
        expect(spoiledPullRequest((r) => (r.number = 1.5))).toBeNull()
        expect(spoiledPullRequest((r) => (r.number = "19"))).toBeNull()
        expect(spoiledPullRequest((r) => (r.owner = 7))).toBeNull()
        expect(
            parseImageWindowAddress(
                JSON.stringify({ ...JSON.parse(imageWindowAddress(pullRequest)), head: null }),
            ),
        ).toBeNull()
    })
})

describe("the zoom window's tab and title", () => {
    test("the tab is the app's own document with the flag and the address", () => {
        const address = imageWindowAddress(commit)
        const path = imageWindowPath(address)
        expect(path.startsWith("/?imageWindow=1&at=")).toBe(true)
        const params = new URLSearchParams(path.slice(path.indexOf("?")))
        expect(params.get("imageWindow")).toBe("1")
        expect(params.get("at")).toBe(address)
        expect(imageWindowName(address)).toBe(`specforge-image:${shortHash(address)}`)
    })

    test("the title names the file", () => {
        expect(imageWindowTitle(commit)).toBe("icons/new.png — zoom")
    })
})
