import { describe, expect, test } from "bun:test"
import type { PullRequestReference } from "../types"
import { referenceOf, sameReference } from "./address"

const ACME_API_42: PullRequestReference = { provider: "github", owner: "acme", repo: "api", number: 42 }

describe("sameReference", () => {
    test("owners and repositories compare ignoring ASCII case", () => {
        expect(sameReference(ACME_API_42, { ...ACME_API_42, owner: "ACME", repo: "Api" })).toBe(true)
        expect(sameReference({ ...ACME_API_42, owner: "Acme", repo: "API" }, ACME_API_42)).toBe(true)
    })

    test("a different provider or number is a different pull request", () => {
        expect(sameReference(ACME_API_42, { ...ACME_API_42, provider: "bitbucket" })).toBe(false)
        expect(sameReference(ACME_API_42, { ...ACME_API_42, number: 43 })).toBe(false)
    })

    test("a different owner or repository is a different pull request", () => {
        expect(sameReference(ACME_API_42, { ...ACME_API_42, owner: "acme-corp" })).toBe(false)
        expect(sameReference(ACME_API_42, { ...ACME_API_42, repo: "web" })).toBe(false)
        // The owner and the repository are compared apart, not joined.
        expect(
            sameReference(
                { ...ACME_API_42, owner: "ac", repo: "meapi" },
                { ...ACME_API_42, owner: "acme", repo: "api" },
            ),
        ).toBe(false)
    })

    test("only A to Z fold: other letters keep their case", () => {
        const owned = (owner: string): PullRequestReference => ({ ...ACME_API_42, owner })
        expect(sameReference(owned("Ärger"), owned("ärger"))).toBe(false)
        expect(sameReference(owned("ÄRGER"), owned("Ärger"))).toBe(true)
    })
})

describe("referenceOf", () => {
    test("is the address without its kind", () => {
        const reference = referenceOf({ kind: "pullRequest", ...ACME_API_42 })
        expect(reference).toEqual(ACME_API_42)
        expect(Object.keys(reference).sort()).toEqual(["number", "owner", "provider", "repo"])
    })
})
