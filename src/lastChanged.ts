// How long ago the artifact in the detail pane was last written, as the header
// says it (`spec-browser`: *Change Identity Header in the Detail Pane*, "Last
// changed").
//
// Pure functions with tests, for the same reason `changeIdentity.ts` is: JSX
// cannot be exercised by `bun test`, and a frontend-only diff short-circuits the
// mutation gate (`.cargo/mutants.toml` scopes it to the two library crates), so
// a component that computed this inline would have no coverage of any kind.

const SECOND_MS = 1_000
const MINUTE_MS = 60 * SECOND_MS
const HOUR_MS = 60 * MINUTE_MS
const DAY_MS = 24 * HOUR_MS
/// A "month" in this vocabulary is a flat 30 days, and a "year" a flat 365.
///
/// The label is computed from an elapsed *duration*, never from two calendar
/// dates, so it has no notion of which month it is in. Defining the coarse units
/// as fixed durations keeps `nextTickDelayMs` exactly aligned with the value
/// being displayed — the label changes precisely when the elapsed time crosses a
/// multiple of its own unit, with no calendar arithmetic and no boundary where a
/// scheduled tick would recompute to the same text.
const MONTH_MS = 30 * DAY_MS
const YEAR_MS = 365 * DAY_MS

/// The widest text `formatLastChanged` can produce.
///
/// The whole change-identity row renders in `var(--font-mono)`, so width is
/// character count exactly — which is what lets the header reserve a box that
/// cannot change size as the label advances (`spec-browser`: *…* — "The
/// advancing label never moves the change name"). The reserved width in
/// `App.css` is stated in `ch` and must equal this string's length; a test pins
/// the two together, so rewording a label here fails the suite rather than
/// silently shrinking the box.
///
/// It is `29 days ago` because the days range runs 1–29 before rolling into
/// months, and 11 characters beats "just now" (8), "59 min ago" (10),
/// "23 hr ago" (9) and "12 mo ago" (9). The years range is open-ended but stays
/// narrower until a file is a thousand years old.
export const LAST_CHANGED_WIDEST = "29 days ago"

/// Milliseconds elapsed since `modifiedAtSecs`, floored at zero.
///
/// Clock skew, restored archives, and network filesystems all produce files
/// stamped in the future. `in 4 minutes` reads as a bug in the application
/// rather than as a fact about the file, so a negative interval is reported as
/// the present moment instead (`spec-browser`: *…* — "A modification time in the
/// future is not shown as future").
function elapsedMs(modifiedAtSecs: number, nowMs: number): number {
    return Math.max(0, nowMs - modifiedAtSecs * SECOND_MS)
}

/// The duration unit the label is currently expressed in. Within a range, the
/// label changes each time the elapsed time crosses a multiple of this.
function unitMs(elapsed: number): number {
    if (elapsed < HOUR_MS) return MINUTE_MS
    if (elapsed < DAY_MS) return HOUR_MS
    if (elapsed < MONTH_MS) return DAY_MS
    if (elapsed < YEAR_MS) return MONTH_MS
    return YEAR_MS
}

/// Where the current range ends and the next unit takes over — `Infinity` for
/// the open-ended years range.
///
/// Needed because the label also changes when it crosses into the next *range*,
/// and that boundary is not always a multiple of the current unit. An hour, a
/// day and a month are whole multiples of the units below them, so those
/// transitions fall out of the modulo for free — but a year is 365 days and a
/// month here is 30, so `12 mo ago` would otherwise sit until day 390 before
/// noticing it should have said `1 yr ago` on day 365. Bounding by the range end
/// makes every transition correct by construction rather than by coincidence,
/// and keeps a future unit (weeks, say) from quietly reintroducing the same gap.
function rangeEndMs(elapsed: number): number {
    if (elapsed < HOUR_MS) return HOUR_MS
    if (elapsed < DAY_MS) return DAY_MS
    if (elapsed < MONTH_MS) return MONTH_MS
    if (elapsed < YEAR_MS) return YEAR_MS
    return Number.POSITIVE_INFINITY
}

/// How long ago the artifact was last written, in the compact register the rest
/// of the identity row uses.
///
/// Relative rather than absolute because the question the header answers is
/// "how long has this stood", and a clock face makes the reader do the
/// arithmetic against a number the header does not show. The pane is already
/// refreshed live, so this is not a staleness indicator.
export function formatLastChanged(
    modifiedAtSecs: number,
    nowMs: number,
): string {
    const elapsed = elapsedMs(modifiedAtSecs, nowMs)
    if (elapsed < MINUTE_MS) return "just now"
    if (elapsed < HOUR_MS) return `${Math.floor(elapsed / MINUTE_MS)} min ago`
    if (elapsed < DAY_MS) return `${Math.floor(elapsed / HOUR_MS)} hr ago`
    if (elapsed < MONTH_MS) {
        const days = Math.floor(elapsed / DAY_MS)
        return `${days} ${days === 1 ? "day" : "days"} ago`
    }
    if (elapsed < YEAR_MS) return `${Math.floor(elapsed / MONTH_MS)} mo ago`
    return `${Math.floor(elapsed / YEAR_MS)} yr ago`
}

/// How long until `formatLastChanged` would return something different.
///
/// A relative label is stale the instant it renders, and nothing on disk needs
/// to change for it to become wrong — so the header advances it on a timer
/// (`spec-browser`: *…* — "The label advances while the reader stays on the
/// artifact"). This is exported beside the formatter, rather than picked as a
/// constant at the call site, so the tick and the text cannot disagree about
/// which unit is on screen.
///
/// Always greater than zero and never finer than the unit displayed: a pane left
/// open on a twelve-day-old artifact wakes once a day, not once a second. The
/// returned delay lands exactly on the boundary where the text changes, so no
/// wakeup recomputes to the value already shown.
export function nextTickDelayMs(
    modifiedAtSecs: number,
    nowMs: number,
): number {
    const elapsed = elapsedMs(modifiedAtSecs, nowMs)
    const unit = unitMs(elapsed)
    const untilNextStep = unit - (elapsed % unit)
    const untilNextRange = rangeEndMs(elapsed) - elapsed
    return Math.min(untilNextStep, untilNextRange)
}
