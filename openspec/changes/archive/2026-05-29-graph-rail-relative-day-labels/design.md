## Context

The commit-graph rail (`src/components/GraphRail.tsx`) groups commit rows into calendar-day sections. Two pure helpers drive this:

- `dayKey(iso)` → a viewer-local `YYYY-M-D` string used to detect day boundaries. **Unchanged by this work.**
- `dayLabel(iso)` → the human-readable separator text, currently always `d.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" })` → `Fri, May 29`.

`computeGeometry` calls `dayLabel` once per day-separator (not per row), so cost is negligible. The author date arrives as an ISO-8601 string from the Rust core (`%aI`) and crosses the IPC boundary unchanged; there is nothing to change on the backend. The separator label is rendered uppercased by CSS (`.graph-day-separator-label { text-transform: uppercase }`), so casing of the produced string does not matter.

## Goals / Non-Goals

**Goals:**

- Replace the always-absolute separator label with relative wording for recent days: `Today`, `Yesterday`, weekday name for 2–6 days back, absolute date for 7+ days back.
- Keep the wording locale-aware, matching the rail's existing `undefined`-locale formatting.
- Confine the change to `dayLabel`; leave grouping, geometry, lanes, and the backend untouched.

**Non-Goals:**

- Live rollover of a `Today` label at midnight for a window left open (accepted stale; self-heals on next re-render).
- Reformatting the per-commit hover tooltip or the commit-detail view (they keep full `toLocaleString` timestamps).
- Any change to commit ordering, lane assignment, or grouping boundaries.

## Decisions

**1. Compute the calendar-day delta, then branch on it.**
`dayLabel` computes an integer `diff` = (commit's local-midnight) − (today's local-midnight) in days, and branches: `0` → today, `-1` → yesterday, `-2…-6` → weekday name, else → existing absolute format.

**2. Locale-aware words via `Intl.RelativeTimeFormat`.** `new Intl.RelativeTimeFormat(undefined, { numeric: "auto" }).format(diff, "day")` yields `"today"` / `"yesterday"` (localized: `heute` / `gestern`, etc.). Weekday names use `toLocaleDateString(undefined, { weekday: "long" })`.
_Alternative — hard-coded English strings:_ rejected; it would make the rail inconsistent for non-English users, since the rest of its date formatting already respects the OS locale.

**3. DST-safe day math.** The delta is computed on local-midnight anchors — `new Date(y, m, d)` for both dates — divided by 86,400,000 ms and `Math.round`ed. Anchoring to local midnight makes month/year rollover correct, and rounding absorbs the 23h/25h days at DST transitions. _Alternative — subtracting 24h from the ISO instant:_ rejected; it lands a day off twice a year.

**4. The 2–6 day weekday window is the exact safe window.** Today plus the prior six days span all seven weekday names once each. So a bare weekday name is unambiguous for days −2 through −6: it can never collide with today (−0), yesterday (−1), or the same weekday one week earlier (−7), because −7 already falls into the absolute case. The scope boundary and the collision boundary are the same line, so no disambiguation logic is needed.

## Risks / Trade-offs

- **`dayLabel` becomes time-dependent.** It now reads the current date, so it is no longer a pure function of its argument and its output can go stale. → Accepted for this change; a stale `Today` corrects itself on the next re-render (any watcher event, scroll, or load-more). Live midnight rollover is explicitly out of scope.
- **Future-dated commit (clock skew, `diff === +1`).** → Falls through to the absolute format rather than printing `Tomorrow`; safe and acceptable. Only `0` and `-1` take the relative-word branch.
- **Unparseable date.** → Preserved: the existing `Number.isNaN` guard still returns the raw ISO string so a separator is never blank.
