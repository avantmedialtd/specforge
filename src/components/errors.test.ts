import { describe, expect, test } from "bun:test"
import { prettifyError } from "./errors"

describe("prettifyError", () => {
    test("unwraps a Tauri IPC error shell", () => {
        expect(prettifyError('IpcMessage:Error("presentation store is poisoned")')).toBe(
            "presentation store is poisoned",
        )
    })

    test("strips a leading module path from a bare error string", () => {
        expect(prettifyError("io::PermissionDenied")).toBe("PermissionDenied")
    })

    test("passes a plain Error through readably", () => {
        expect(prettifyError(new Error("boom"))).toBe("Error: boom")
    })
})
