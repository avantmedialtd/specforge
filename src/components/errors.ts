// Turning a rejected command into something worth showing a user. Shared by
// every surface that reports a failed `invoke` inline (Settings rows, the
// disabled-address notice) so one wrapper shape is unwrapped in one place.

/// A readable message for a rejected Tauri command.
export function prettifyError(err: unknown): string {
    const s = String(err)
    // Tauri wraps command errors in `IpcMessage:Error("...")` shells —
    // unwrap to the readable inner message where possible.
    const inner = s.match(/Error\("(.+)"\)/)?.[1] ?? s
    return inner.replace(/^[A-Za-z]+::/, "")
}
