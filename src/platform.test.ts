import { describe, expect, test } from "bun:test"
import { usesMacTitlebarChrome } from "./platform"

// Real user-agent strings. Note that `macDesktop` and `ipadDefault` are
// byte-for-byte identical: since iPadOS 13, Safari on iPad requests desktop
// sites by default and reports the Macintosh string verbatim. There is no
// user-agent test that separates them, which is exactly why the host check
// has to carry the decision.
const MAC_DESKTOP =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15"
const APPLE_UAS = {
    "macOS Safari": MAC_DESKTOP,
    "iPadOS Safari (desktop-class default)": MAC_DESKTOP,
    "iPad, Request Mobile Website":
        "Mozilla/5.0 (iPad; CPU OS 17_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Mobile/15E148 Safari/604.1",
    iPhone: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Mobile/15E148 Safari/604.1",
}
const WINDOWS =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
const LINUX =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"

describe("usesMacTitlebarChrome", () => {
    test("the native macOS window gets the titlebar layout", () => {
        expect(usesMacTitlebarChrome(true, MAC_DESKTOP)).toBe(true)
    })

    // The regression this change exists to prevent: a browser must never get
    // the layout, however Mac-like its user-agent claims to be.
    for (const [label, ua] of Object.entries(APPLE_UAS)) {
        test(`the served web UI never gets it — ${label}`, () => {
            expect(usesMacTitlebarChrome(false, ua)).toBe(false)
        })
    }

    test("every Apple user-agent really does carry a Mac token", () => {
        // Guards the premise above: if this ever stops holding, the host
        // check is still correct but the comments explaining it are stale.
        for (const ua of Object.values(APPLE_UAS)) {
            expect(/Mac/i.test(ua)).toBe(true)
        }
    })

    test("native Windows and Linux windows do not get it", () => {
        expect(usesMacTitlebarChrome(true, WINDOWS)).toBe(false)
        expect(usesMacTitlebarChrome(true, LINUX)).toBe(false)
    })

    test("a non-Apple browser does not get it either", () => {
        expect(usesMacTitlebarChrome(false, WINDOWS)).toBe(false)
        expect(usesMacTitlebarChrome(false, LINUX)).toBe(false)
    })
})
