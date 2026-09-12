// The document outline's pure derivations: heading identifiers, which outline
// levels are listed, and the per-section task counts a tasks document shows.
//
// Kept out of the components that consume them so they are unit-testable
// without a DOM — the same reasoning `docWidth.ts` and `changeIdentity.ts`
// state in their own headers. JSX is not exercised by `bun test` and a
// frontend-only diff short-circuits the mutation gate, so these tests are the
// only automated coverage the outline gets.

import { stripInlineMarkdown } from "./markdown"
import type { Section } from "./types"

/// One heading of the rendered document, in document order.
export interface HeadingEntry {
    /// 1–6.
    level: number
    /// The heading's rendered text.
    text: string
    /// Source line, stamped on the element as `data-line` so the existing
    /// anchor path can scroll to it.
    line: number
    /// The heading's stable identifier, or `""` for a heading whose text
    /// carries no identifier characters at all (see `headingId`).
    id: string
}

/// The outline lists level-two and level-three headings only.
///
/// Level one is the document's own title, and level four and deeper are
/// excluded because a capability spec carries one heading per scenario — an
/// outline of scenarios is as long as the document (`document-outline`:
/// *Document Outline Surface*).
export function outlineEntries(headings: HeadingEntry[]): HeadingEntry[] {
    return headings.filter((h) => h.level === 2 || h.level === 3)
}

/// The identifier a heading carries, derived from its text: lower-cased, with
/// punctuation other than hyphens removed, whitespace replaced by single
/// hyphens, and a numeric suffix appended to later duplicates in document
/// order — the derivation common markdown hosts apply, so a fragment written
/// for a document on such a host resolves here to the same heading
/// (`document-outline`: *Fragment Links Resolve Within the Document*).
///
/// `seen` carries the duplicate counts across one document and is MUTATED, so
/// callers pass a fresh map per document and walk the headings in order.
///
/// A heading whose text survives the strip as nothing — one written entirely
/// in punctuation — gets the empty identifier and is NOT recorded as seen: no
/// fragment can name it, and minting `-1` for the next such heading would
/// invent an identifier for a heading that has none. Its outline entry still
/// works, because activating an entry scrolls by source line, not by id.
export function headingId(text: string, seen: Map<string, number>): string {
    const base = text
        .trim()
        .toLowerCase()
        // Keep letters, digits, whitespace (folded to hyphens next) and
        // hyphens; drop every other character.
        .replace(/[^\p{L}\p{N}\s-]/gu, "")
        .trim()
        .replace(/\s+/g, "-")
    if (base === "") return ""
    const count = seen.get(base) ?? 0
    seen.set(base, count + 1)
    return count === 0 ? base : `${base}-${count}`
}

/// Per-section task counts for one outline entry.
export interface SectionProgress {
    completed: number
    total: number
}

/// The parsed section whose title matches `text`, or `undefined` when none
/// does.
///
/// Matched on the heading's TEXT, which is what the rendered document shows,
/// against the section title the aggregation parsed. Inline markdown is
/// stripped from the parsed title first: the renderer has already resolved
/// `**bold**` to its text, so comparing the raw source title would miss a
/// section whose heading carries any emphasis at all.
export function sectionForHeading(
    text: string,
    sections: Section[],
): Section | undefined {
    const needle = text.trim()
    return sections.find(
        (section) => stripInlineMarkdown(section.title).trim() === needle,
    )
}

/// A section's completed and total task counts, taken from the parsed section
/// model rather than from the rendered document, so they agree with the
/// progress meter on the change row and on the Tasks tab, which read the same
/// model (`document-outline`: *Section Progress in a Tasks Outline*).
export function sectionProgress(section: Section): SectionProgress {
    return {
        completed: section.tasks.filter((task) => task.completed).length,
        total: section.tasks.length,
    }
}
