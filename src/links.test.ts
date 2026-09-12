import { describe, expect, test } from "bun:test"
import { classifyHref, fragmentTarget, hrefScheme } from "./links"

const HEADINGS = new Set(["what-changes", "context", "scenario", "scenario-1"])

describe("hrefScheme", () => {
    test("reads a scheme, lower-cased", () => {
        expect(hrefScheme("HTTPS://example.com")).toBe("https")
        expect(hrefScheme("mailto:a@b.c")).toBe("mailto")
    })

    test("a relative reference has no scheme", () => {
        expect(hrefScheme("./notes.md")).toBeNull()
        expect(hrefScheme("#what-changes")).toBeNull()
        expect(hrefScheme("a/b:c")).toBeNull()
    })
})

describe("fragmentTarget", () => {
    test("reads the identifier a fragment-only href names", () => {
        expect(fragmentTarget("#what-changes")).toBe("what-changes")
    })

    test("percent-decodes, so a fragment written by a markdown host resolves", () => {
        expect(fragmentTarget("#risks%20and%20trade-offs")).toBe("risks and trade-offs")
    })

    test("a path-bearing href and a bare hash name nothing", () => {
        expect(fragmentTarget("./a.md#x")).toBeNull()
        expect(fragmentTarget("#")).toBeNull()
    })
})

describe("classifyHref", () => {
    test("http, https, mailto and tel are external", () => {
        for (const href of ["http://a", "https://a", "mailto:a@b.c", "tel:+1"]) {
            expect(classifyHref(href, HEADINGS)).toBe("external")
        }
    })

    test("other schemes are inert", () => {
        for (const href of ["javascript:alert(1)", "file:///etc/passwd", "data:text/html,x"]) {
            expect(classifyHref(href, HEADINGS)).toBe("inert")
        }
    })

    test("a relative markdown link stays inert, whatever the extension's casing", () => {
        expect(classifyHref("./notes.md", HEADINGS)).toBe("inert")
        expect(classifyHref("./NOTES.MD", HEADINGS)).toBe("inert")
    })

    test("a relative non-markdown link is a file", () => {
        expect(classifyHref("./mockups/login.html", HEADINGS)).toBe("file")
        expect(classifyHref("../images/a.png", HEADINGS)).toBe("file")
    })

    // The three cases task 3.7 names.
    test("a fragment matching a rendered heading is a live fragment link", () => {
        expect(classifyHref("#what-changes", HEADINGS)).toBe("fragment")
        expect(classifyHref("#scenario-1", HEADINGS)).toBe("fragment")
    })

    test("a fragment matching no heading is a dangling link, not a live one", () => {
        expect(classifyHref("#nope", HEADINGS)).toBe("danglingFragment")
        expect(classifyHref("#", HEADINGS)).toBe("danglingFragment")
    })

    test("a path carrying a fragment keeps its path's class, unchanged", () => {
        expect(classifyHref("./mockups/login.html#hero", HEADINGS)).toBe("file")
        expect(classifyHref("./notes.md#what-changes", HEADINGS)).toBe("inert")
        expect(classifyHref("https://a/b#what-changes", HEADINGS)).toBe("external")
    })

    test("with no headings known, every fragment dangles rather than silently scrolling", () => {
        expect(classifyHref("#what-changes")).toBe("danglingFragment")
    })

    test("an empty href is inert", () => {
        expect(classifyHref("", HEADINGS)).toBe("inert")
    })
})
