import { describe, expect, test } from "bun:test"
import { closeKeyOf, SIZE_SAVE_DELAY_MS, type CloseKeyEvent } from "./detachedWindow"

// A reader window's keys, as the pull-request window now shares them
// (`reader-window`: *Dismissing a Reader Window Destroys It*;
// `pull-request-viewer`: *Pull-Request Window*).

function press(key: string, code: string, held: Partial<CloseKeyEvent> = {}): CloseKeyEvent {
    return {
        key,
        code,
        metaKey: false,
        ctrlKey: false,
        altKey: false,
        shiftKey: false,
        defaultPrevented: false,
        ...held,
    }
}

describe("closeKeyOf", () => {
    test("Escape closes the window", () => {
        expect(closeKeyOf(press("Escape", "Escape"))).toBe("escape")
    })

    test("Cmd-W and Ctrl-W close it by the shortcut", () => {
        expect(closeKeyOf(press("w", "KeyW", { metaKey: true }))).toBe("shortcut")
        expect(closeKeyOf(press("w", "KeyW", { ctrlKey: true }))).toBe("shortcut")
    })

    test("the shortcut is the key's position, on any layout", () => {
        // A Cyrillic layout's W key types "ц".
        expect(closeKeyOf(press("ц", "KeyW", { metaKey: true }))).toBe("shortcut")
        // Dvorak's "w" sits elsewhere; the position, not the letter, closes.
        expect(closeKeyOf(press("w", "Comma", { metaKey: true }))).toBeNull()
    })

    test("W with Shift or Alt held, or with no modifier, closes nothing", () => {
        expect(closeKeyOf(press("W", "KeyW", { metaKey: true, shiftKey: true }))).toBeNull()
        expect(closeKeyOf(press("∑", "KeyW", { metaKey: true, altKey: true }))).toBeNull()
        expect(closeKeyOf(press("w", "KeyW"))).toBeNull()
    })

    test("a key a control inside has claimed closes nothing", () => {
        // A maximized figure takes the first Escape.
        expect(closeKeyOf(press("Escape", "Escape", { defaultPrevented: true }))).toBeNull()
        expect(
            closeKeyOf(press("w", "KeyW", { metaKey: true, defaultPrevented: true })),
        ).toBeNull()
    })

    test("any other key closes nothing", () => {
        expect(closeKeyOf(press("Enter", "Enter"))).toBeNull()
        expect(closeKeyOf(press("q", "KeyQ", { metaKey: true }))).toBeNull()
    })
})

describe("the remembered size", () => {
    test("is saved 400 ms after a resize ends", () => {
        expect(SIZE_SAVE_DELAY_MS).toBe(400)
    })
})
