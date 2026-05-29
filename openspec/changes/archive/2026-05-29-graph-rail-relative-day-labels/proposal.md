# Relative day labels in the commit-graph rail

## Why

The commit-graph rail groups commits into calendar-day sections, but every separator is labelled with an absolute date (`Fri, May 29`). For the days a user reads most — today and yesterday — an absolute date forces a small mental subtraction to answer "is this today?". Relative wording for recent days reads instantly and matches how people talk about recent commits.

## What Changes

- The day-separator label SHALL read `Today` for the viewer's current calendar day and `Yesterday` for the day before, in the viewer's local time zone.
- For commits 2–6 calendar days back, the separator SHALL read the plain weekday name (`Wednesday`). This window is exactly safe: today plus the prior six days cover all seven weekday names once each, so a bare weekday is never ambiguous with today, with yesterday, or with the same weekday one week earlier (which falls into the absolute case).
- For commits 7 or more days back, the separator SHALL keep the existing absolute format (`Mon, May 25`).
- Relative wording SHALL be locale-aware, consistent with the rail's existing locale-respecting date formatting (no hard-coded English).
- Day *grouping* is unchanged — only the rendered text of a separator changes. No backend, IPC, or lane-layout changes.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `commit-graph`: the day-separator labelling rule under **Faithful Commit Graph Rendering** changes from always-absolute to relative wording for recent days (today / yesterday / weekday-within-the-week) with absolute dates beyond a week.

## Impact

- **Code:** `src/components/GraphRail.tsx` only — the `dayLabel` helper that formats separator text. `dayKey` (grouping) is untouched.
- **APIs / IPC:** none. The ISO-8601 author date already crosses the boundary; no Rust changes.
- **Behavioural tradeoff:** `dayLabel` becomes time-dependent (it reads the current date). A `Today` label on a window left open across midnight is accepted as stale for this change; it self-corrects on the next re-render (any watcher event or scroll). A live midnight rollover is explicitly out of scope.
