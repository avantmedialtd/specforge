/// How a settings field's typed text becomes a value, and whether committing it
/// writes (`settings-view`: *Settings Persist by One Rule*).
///
/// A field commits on Enter or when focus leaves it, never per keystroke, so
/// the only question at commit time is what to do with the whole draft: write
/// nothing (it says what is stored already), refuse it (the setting does not
/// accept it), or write the parsed value. Kept pure for the reason `docWidth.ts`
/// gives in its header — this module and its tests are the rule's only
/// automated coverage.

/// A parser's verdict on a draft.
export type Parsed<T> = { ok: true; value: T } | { ok: false; message: string }

/// What committing a draft does.
export type CommitDecision<T> =
    | { kind: "unchanged" }
    | { kind: "invalid"; message: string }
    | { kind: "write"; value: T }

const DIGITS = /^\d+$/

/// Digits only, then a whole number. A sign, a decimal point, an exponent or
/// an empty field is refused rather than coerced, so the user is told what to
/// fix instead of having a surprise written.
function wholeNumber(raw: string): number | null {
    const text = raw.trim()
    return DIGITS.test(text) ? Number(text) : null
}

/// A TCP port, 1 to 65535 inclusive.
export function parsePort(raw: string): Parsed<number> {
    const value = wholeNumber(raw)
    if (value === null || value < 1 || value > 65535) {
        return { ok: false, message: "The port must be a whole number from 1 to 65535." }
    }
    return { ok: true, value }
}

/// A poll interval in whole seconds, at least one.
export function parsePollSeconds(raw: string): Parsed<number> {
    const value = wholeNumber(raw)
    if (value === null || value < 1) {
        return { ok: false, message: "The interval must be a whole number of seconds, at least 1." }
    }
    return { ok: true, value }
}

/// Free text, trimmed, where an empty field means "no value" — the convention
/// every optional text setting already follows (a cleared display name falls
/// back to its default).
export function parseText(raw: string): Parsed<string | null> {
    const text = raw.trim()
    return { ok: true, value: text.length > 0 ? text : null }
}

/// A comma-separated list, each entry trimmed and empty entries dropped — how
/// the Tailscale login allow-list is typed.
export function parseList(raw: string): Parsed<string[]> {
    return {
        ok: true,
        value: raw
            .split(",")
            .map((entry) => entry.trim())
            .filter((entry) => entry.length > 0),
    }
}

/// Whether two lists hold the same entries in the same order.
export function sameList(a: readonly string[], b: readonly string[]): boolean {
    return a.length === b.length && a.every((entry, i) => entry === b[i])
}

/// Decide what committing `draft` does, given the value `stored` now. Parsed
/// values are compared — never the raw text — so a draft that differs from the
/// stored value only in surrounding whitespace, or a port only in leading
/// zeros, is unchanged and writes nothing.
export function commitDecision<T>(
    draft: string,
    stored: T,
    parse: (raw: string) => Parsed<T>,
    equals: (a: T, b: T) => boolean = Object.is,
): CommitDecision<T> {
    const parsed = parse(draft)
    if (!parsed.ok) return { kind: "invalid", message: parsed.message }
    if (equals(parsed.value, stored)) return { kind: "unchanged" }
    return { kind: "write", value: parsed.value }
}
