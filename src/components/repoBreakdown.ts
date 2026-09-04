import type { RepoBreakdown } from "../types"

/// How many breakdown entries the Dashboard's per-repository card presents.
///
/// The cap is what keeps the card's height independent of the registry's size:
/// a two-repository registry and a forty-repository one produce the same block
/// of rows. It is deliberately a presentation constant — the payload carries
/// every entry, because the Dashboard's closing footnote sums it for the
/// registry-wide archived total (`dashboard`: *Per-Repository Breakdown*).
export const BREAKDOWN_LIMIT = 5

/// The breakdown as rendered: the entries to show, and what the cap withheld.
export interface CappedBreakdown {
    /// The entries to present, in payload order. The payload arrives sorted by
    /// active count descending, then archived descending, then label — so
    /// slicing the front takes the highest-ranked entries without re-sorting.
    shown: RepoBreakdown[]
    /// How many entries the cap withheld; `0` when everything is shown.
    hiddenCount: number
    /// How many of the withheld entries carry at least one active change. Zero
    /// when the cap hid nothing in flight — which is the common case, and the
    /// thing the remainder line exists to report.
    hiddenActiveCount: number
}

/// Take the first [`BREAKDOWN_LIMIT`] entries and describe what was left out.
///
/// The ordering is the payload's (`repo_breakdowns` sorts it in `openspec-core`,
/// where `cargo test` and the mutation gate can see the comparator); this
/// function only bounds the list. It never re-sorts, so a payload that arrives
/// unordered is rendered unordered rather than silently repaired here.
export function capBreakdown(
    repos: RepoBreakdown[],
    limit: number = BREAKDOWN_LIMIT,
): CappedBreakdown {
    const shown = repos.slice(0, limit)
    const hidden = repos.slice(limit)
    return {
        shown,
        hiddenCount: hidden.length,
        hiddenActiveCount: hidden.filter((r) => r.activeCount > 0).length,
    }
}

/// The remainder line's text, or `null` when nothing was withheld and the line
/// should not render at all.
///
/// It deliberately does NOT restate the registry-wide archived total: that
/// already appears in the Dashboard's footnote a few elements below, so
/// repeating it says nothing new. What a truncated list owes its reader is
/// whether any *active* work was hidden — so that is what the line reports.
export function remainderLabel(capped: CappedBreakdown): string | null {
    if (capped.hiddenCount === 0) return null
    const more = `+ ${capped.hiddenCount} more`
    return capped.hiddenActiveCount === 0
        ? `${more} · none active`
        : `${more} · ${capped.hiddenActiveCount} active`
}

/// The bar's length as a percentage of the track, for one entry.
///
/// Normalised against the largest active count among the *presented* entries,
/// so the top row always fills the track and the rest read against it. An entry
/// with no active changes returns `0` and renders no bar at all — the caller
/// checks `activeCount` rather than relying on a zero-width element, so that
/// every bar on screen encodes a non-zero quantity and the visual order agrees
/// with the sort order.
export function barPercent(activeCount: number, maxActive: number): number {
    if (activeCount <= 0 || maxActive <= 0) return 0
    return Math.round((activeCount / maxActive) * 100)
}
