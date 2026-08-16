// How long ago something happened, in the compact register the sidebar, the
// Dashboard and the detail pane's identity header all render.
//
// ONE definition, deliberately. Before this module there were two private
// copies — `formatRelativeTime` in `WorkspaceTree.tsx` and `relativeTime` in
// `DashboardView.tsx` — which had already drifted into different vocabularies
// ("12s ago" vs "just now", weeks and months in one and not the other). Adding
// a third for the identity header would have put two spellings of the same kind
// of value on screen simultaneously, which is the defect
// `tint-identity-branch-chip` exists as a warning about: a hand-copy that fell
// out of step, invisible because nothing fails when it does.
//
// Pure functions with tests, for the same reason `changeIdentity.ts` is: JSX
// cannot be exercised by `bun test`, and a frontend-only diff short-circuits the
// mutation gate (`.cargo/mutants.toml` scopes it to the two library crates), so
// logic computed inline in a component has no coverage of any kind.

const SECOND_MS = 1_000
const MINUTE_MS = 60 * SECOND_MS
const HOUR_MS = 60 * MINUTE_MS
const DAY_MS = 24 * HOUR_MS
const WEEK_MS = 7 * DAY_MS
/// A "month" here is a flat 30 days.
///
/// Every label is computed from an elapsed *duration*, never from two calendar
/// dates, so this vocabulary has no notion of which month it is in. Defining the
/// coarsest unit as a fixed duration keeps `nextTickDelayMs` exactly aligned
/// with the value on screen: the text changes precisely when the elapsed time
/// crosses a multiple of its own unit.
const MONTH_MS = 30 * DAY_MS

/// The largest delay `setTimeout` can be given.
///
/// Its delay argument is a WebIDL `long`, so anything above 2^31−1 ms (~24.8
/// days) is coerced to a **negative** number and the timer fires immediately —
/// `2_592_000_000 | 0` is `-1702967296`. A caller that reschedules from inside
/// its own callback then never sleeps at all: it becomes a render loop clamped
/// only by the 4 ms nested-timeout floor.
///
/// The months tier reaches a 30-day delay, so this is not hypothetical: roughly
/// six days out of every thirty land above the ceiling. `nextTickDelayMs`
/// therefore never returns more than this. A clamped wakeup recomputes the same
/// text once every ~24.8 days, which is the one place the "no wakeup recomputes
/// the value already shown" property below is knowingly traded — for not
/// spinning a core.
const MAX_TIMEOUT_MS = 2_147_483_647

/// The widest text `formatRelativeTime` can produce for anything younger than
/// about 83 years.
///
/// The identity header reserves a box this wide so its label cannot change size
/// as it advances (`spec-browser`: *Change Identity Header in the Detail Pane* —
/// "The advancing label never moves the change name"). The surfaces that render
/// it are monospace, so character count is an exact rendered width.
///
/// The months tier is open-ended, so this is a practical bound rather than a
/// mathematical one. It is applied as `min-width`, never a fixed width — an
/// implausibly old artifact widens its own box by a character instead of having
/// its label clipped.
export const RELATIVE_TIME_WIDEST = "999mo ago"

/// Milliseconds elapsed since `unixSeconds`, floored at zero.
///
/// Clock skew, restored archives, and network filesystems all produce files
/// stamped in the future. `in 4 minutes` reads as a bug in the application
/// rather than a fact about the file, so a negative interval is reported as the
/// present moment instead (`spec-browser`: *…* — "A modification time in the
/// future is not shown as future").
function elapsedMs(unixSeconds: number, nowMs: number): number {
    return Math.max(0, nowMs - unixSeconds * SECOND_MS)
}

/// The duration unit the label is currently expressed in. Within a tier, the
/// text changes each time the elapsed time crosses a multiple of this.
function unitMs(elapsed: number): number {
    if (elapsed < HOUR_MS) return MINUTE_MS
    if (elapsed < DAY_MS) return HOUR_MS
    if (elapsed < WEEK_MS) return DAY_MS
    if (elapsed < MONTH_MS) return WEEK_MS
    return MONTH_MS
}

/// Where the current tier ends and the next unit takes over — `Infinity` for the
/// open-ended months tier.
///
/// Needed because the label also changes when it crosses into the next *tier*,
/// and that boundary is not always a multiple of the current unit. An hour, a
/// day and a week are whole multiples of the units below them, so those
/// transitions fall out of the modulo for free — but a 30-day month is not a
/// whole number of weeks, so `4w ago` would otherwise sit until day 35 before
/// noticing it should have said `1mo ago` on day 30. Bounding by the tier end
/// makes every transition correct by construction rather than by coincidence.
function tierEndMs(elapsed: number): number {
    if (elapsed < HOUR_MS) return HOUR_MS
    if (elapsed < DAY_MS) return DAY_MS
    if (elapsed < WEEK_MS) return WEEK_MS
    if (elapsed < MONTH_MS) return MONTH_MS
    return Number.POSITIVE_INFINITY
}

/// How long ago `unixSeconds` was, as the sidebar, Dashboard and identity header
/// all say it.
///
/// Sub-minute reads "just now" rather than counting seconds. The seconds tier
/// the sidebar used to carry could only stay honest with a per-second timer —
/// and it never had one, so it displayed a frozen "12s ago" for up to a minute
/// anyway. One less tier removes both the wrong label and the timer that would
/// have been needed to fix it.
///
/// Callers own the "unknown" case, because it genuinely differs by surface: the
/// sidebar shows an em dash for an instance with no recorded time, the Dashboard
/// shows nothing, and the identity header renders no label at all.
export function formatRelativeTime(
    unixSeconds: number,
    nowMs: number,
): string {
    const elapsed = elapsedMs(unixSeconds, nowMs)
    if (elapsed < MINUTE_MS) return "just now"
    if (elapsed < HOUR_MS) return `${Math.floor(elapsed / MINUTE_MS)}m ago`
    if (elapsed < DAY_MS) return `${Math.floor(elapsed / HOUR_MS)}h ago`
    if (elapsed < WEEK_MS) return `${Math.floor(elapsed / DAY_MS)}d ago`
    if (elapsed < MONTH_MS) return `${Math.floor(elapsed / WEEK_MS)}w ago`
    return `${Math.floor(elapsed / MONTH_MS)}mo ago`
}

/// How long until `formatRelativeTime` would return something different.
///
/// A relative label is stale the instant it renders, and nothing needs to happen
/// for it to become wrong — the reader simply sits there. So every surface that
/// shows one advances it on a timer (`spec-browser`: *…* — "The label advances
/// while the reader stays on the artifact").
///
/// Exported beside the formatter rather than chosen at each call site, so the
/// tick and the text cannot disagree about which unit is on screen. The previous
/// arrangement — a fixed 60-second interval in `WorkspaceTree` — was both too
/// fast for a three-week-old row and too slow for a seconds-granularity one.
///
/// Always greater than zero, never finer than the unit displayed, never above
/// what `setTimeout` can hold, and otherwise landing exactly on the boundary
/// where the text changes — so no wakeup recomputes the value already shown.
///
/// Scheduled from the **unclamped** delta, unlike the label. A stamp in the
/// future reads "just now" (the formatter floors at zero), but it will go on
/// reading "just now" until the clock catches up and a further minute passes;
/// waking every 60 seconds until then would be pure churn for a file stamped a
/// year ahead.
export function nextTickDelayMs(unixSeconds: number, nowMs: number): number {
    const rawElapsed = nowMs - unixSeconds * SECOND_MS
    if (rawElapsed < 0) {
        // Sleep until the stamp becomes the present, then the usual minute.
        return Math.min(-rawElapsed + MINUTE_MS, MAX_TIMEOUT_MS)
    }
    const unit = unitMs(rawElapsed)
    const untilNextStep = unit - (rawElapsed % unit)
    const untilNextTier = tierEndMs(rawElapsed) - rawElapsed
    return Math.min(untilNextStep, untilNextTier, MAX_TIMEOUT_MS)
}
