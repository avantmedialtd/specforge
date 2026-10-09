/// How the Settings view edits the review skip patterns (`pull-request-viewer`:
/// *Review Skip Patterns*, In Settings): one committed field per pattern and an
/// empty one that adds, where every commit and removal stores the whole list,
/// the empty one included. Whether a list is accepted is the service's to
/// say, never this module's — patterns are matched and validated in Rust
/// alone (`review-skip-patterns` design D4) — so what lives here is only how a
/// list is edited and how a refusal is worded on a field. Kept pure for the
/// reason `settingsFields.ts` gives: these tests are the only coverage it gets.

import type { Parsed } from "./settingsFields"
import type { PatternError } from "./types"

/// A pattern field's draft as the list takes it: without its surrounding
/// whitespace, as the service stores it. An emptied field reads as no
/// pattern, which removes the one the field held.
export function parsePattern(raw: string): Parsed<string> {
    return { ok: true, value: raw.trim() }
}

/// `list` with the pattern at `index` replaced by `pattern`, or removed when
/// `pattern` is empty: committing a field emptied of its pattern removes it.
export function withPattern(list: readonly string[], index: number, pattern: string): string[] {
    if (pattern === "") return withoutPattern(list, index)
    return list.map((held, at) => (at === index ? pattern : held))
}

/// `list` without the pattern at `index`, and no other, a duplicate of it
/// included.
export function withoutPattern(list: readonly string[], index: number): string[] {
    return list.filter((_, at) => at !== index)
}

/// `list` with `pattern` added after its last.
export function withAddedPattern(list: readonly string[], pattern: string): string[] {
    return [...list, pattern]
}

/// What a field says when the list holding its change was refused: why its
/// own pattern was refused, when the one it sent at `index` was, and then
/// each other refused pattern and why, which its change cannot be stored
/// beside — a pattern a hand-edited settings file holds, which its own field
/// reports too. `index` is null for a removal, which sends no pattern of its
/// own.
export function refusalText(refused: readonly PatternError[], index: number | null): string {
    const own = refused.find((error) => error.index === index)
    const others = refused
        .filter((error) => error !== own)
        .map((error) => `${error.pattern}: ${error.reason}`)
    return (own ? [own.reason, ...others] : others).join("; ")
}

/// Each pattern's stored error, by its place in the list: why a pattern a
/// hand-edited settings file holds is left out of matching, for its own field
/// to report.
export function errorsByIndex(errors: readonly PatternError[]): ReadonlyMap<number, string> {
    return new Map(errors.map((error) => [error.index, error.reason]))
}
