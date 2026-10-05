import { describe, expect, test } from "bun:test"
import {
    installHeadPolicies,
    PULL_REQUEST_CONTENT_SECURITY_POLICY,
    windowKind,
    type HeadDocument,
    type WindowKind,
} from "./windowKind"

/// A `<meta>` as the stand-in document makes one: its attributes, by name.
class FakeMeta {
    readonly attributes = new Map<string, string>()
    setAttribute(name: string, value: string): void {
        this.attributes.set(name, value)
    }
}

/// A document with a `head`, the one part the installer touches. It also
/// records every element made, so a test can tell an element left out of
/// `head`, which no engine would honour, from one appended to it.
function fakeDocument() {
    const head: FakeMeta[] = []
    const created: FakeMeta[] = []
    const doc = {
        createElement(_tagName: string): FakeMeta {
            const meta = new FakeMeta()
            created.push(meta)
            return meta
        },
        head: { appendChild: (node: FakeMeta) => head.push(node) },
    }
    return { doc: doc as unknown as HeadDocument, head, created }
}

/// The `<meta>` elements `kind`'s root ends up with in `head`, as attribute
/// records.
function headAfterInstall(kind: WindowKind): Record<string, string>[] {
    const { doc, head, created } = fakeDocument()
    installHeadPolicies(kind, doc)
    expect(created).toEqual(head)
    return head.map((meta) => Object.fromEntries(meta.attributes))
}

const DNS_PREFETCH_OFF = { "http-equiv": "x-dns-prefetch-control", content: "off" }

/// The content-security policies among `metas`, matching the header name as
/// engines do, without regard to case.
function policiesIn(metas: Record<string, string>[]): string[] {
    return metas
        .filter((meta) => meta["http-equiv"]?.toLowerCase() === "content-security-policy")
        .map((meta) => meta.content ?? "")
}

describe("windowKind", () => {
    test("the application is the root without a flag", () => {
        expect(windowKind("")).toBe("application")
        expect(windowKind("?at=%2Fpr%2Fgithub%2Facme%2Fapi%2F42")).toBe("application")
    })

    test("the reader flag selects the reader", () => {
        expect(windowKind("?reader=1")).toBe("reader")
        expect(windowKind("?reader=1&at=%2Fw%2Fnotes%2Ffile%2Fa.md")).toBe("reader")
    })

    // Both hosts' spellings: the desktop puts the address in `at`, while the
    // browser skin keeps it in the path and carries the flag alone.
    test("the pull-request flag selects the pull-request window on either host", () => {
        expect(windowKind("?pullRequest=1&at=%2Fpr%2Fgithub%2Facme%2Fapi%2F42")).toBe(
            "pullRequest",
        )
        expect(windowKind("?pullRequest=1")).toBe("pullRequest")
    })

    test("only the value 1, under the flag's exact name, counts", () => {
        expect(windowKind("?pullRequest=0")).toBe("application")
        expect(windowKind("?pullRequest=true")).toBe("application")
        expect(windowKind("?pullRequest=")).toBe("application")
        expect(windowKind("?pullrequest=1")).toBe("application")
        expect(windowKind("?reader=0")).toBe("application")
    })

    test("a page carrying both flags is a reader", () => {
        expect(windowKind("?pullRequest=1&reader=1")).toBe("reader")
    })
})

describe("installHeadPolicies", () => {
    test("the pull-request window's head carries the policy", () => {
        expect(policiesIn(headAfterInstall("pullRequest"))).toEqual([
            PULL_REQUEST_CONTENT_SECURITY_POLICY,
        ])
    })

    test("the policy is exactly the one the spec gives", () => {
        expect(PULL_REQUEST_CONTENT_SECURITY_POLICY).toBe(
            "img-src 'self' data: blob:; font-src 'self' data:; media-src 'none'; object-src 'none'",
        )
    })

    test("the application's and a reader's heads carry no policy", () => {
        expect(policiesIn(headAfterInstall("application"))).toEqual([])
        expect(policiesIn(headAfterInstall("reader"))).toEqual([])
    })

    test("every root turns DNS prefetching off", () => {
        expect(headAfterInstall("application")).toEqual([DNS_PREFETCH_OFF])
        expect(headAfterInstall("reader")).toEqual([DNS_PREFETCH_OFF])
        const pullRequest = headAfterInstall("pullRequest")
        expect(pullRequest).toContainEqual(DNS_PREFETCH_OFF)
        expect(pullRequest).toHaveLength(2)
    })
})
