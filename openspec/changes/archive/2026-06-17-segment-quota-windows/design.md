## Context

The Claude quota gauge renders one utilization number per window in both
frontends. The snapshot (`ClaudeQuotaState` → `QuotaWindow { utilization,
resetsAtUnix }`) already carries each window's reset time, and the windows are
fixed-length by definition (`five_hour` = 5 h, `seven_day` = 7 d). The desktop
pill and TUI title bar both already recompute a *live* reset countdown from
`resetsAtUnix` between polls (`nowMs` tick on desktop, 250 ms tick in the TUI).

This change adds a **time axis** — hour/day segments plus a live "now" marker —
on top of the existing utilization fill, so the gauge conveys pace, not just
total. The required input (reset time + fixed window length) is already present,
so the marker is a rendering concern, not a data concern.

## Goals / Non-Goals

**Goals:**
- Show, per window, where "now" sits within the window (5 hour segments for the
  5-hour window, 7 day segments for the weekly window) with a live marker.
- Keep utilization (the fill) and elapsed-time (the marker) as two independent
  visual channels so the user can read pace at a glance.
- Zero backend / IPC / type changes: derive the marker in each frontend, live,
  exactly like the existing countdown.
- Degrade cleanly to today's plain bar when a window has no reset time.

**Non-Goals:**
- Per-hour / per-day **usage histograms** — the usage endpoint returns one
  aggregate utilization per window, not a time series. Buckets would require us to
  sample and persist deltas ourselves; explicitly excluded.
- Weekday/day labels under the weekly segments (needs local-TZ day-of-week math
  and certainty about fixed-vs-rolling weekly reset). Possible follow-up.
- "Pace" coloring / ▲▼ indicators that flag fill-ahead-of-time. Possible
  follow-up; this change only positions the marker, it does not editorialize.

## Decisions

### Decision: Compute the marker fraction in the frontends, live — no backend change
`elapsed = clamp(1 − (resetsAtUnix − now) / length, 0, 1)`, with
`length` = 5 h (18 000 s) for the 5-hour window and 7 d (604 800 s) for the
weekly window. This mirrors the existing live countdown, so the marker advances
smoothly between polls on the tick the frontends already run.

*Alternative considered:* add an `elapsedFraction` (or `windowStartUnix`) field to
`QuotaWindow` and compute it in `quota.rs`. Rejected — the backend only updates on
poll, so a backend-computed fraction would be stale between refreshes (the marker
would jump, not glide), and it would mean churning the Rust types, `types.ts`, and
the IPC payload for data the frontend can already derive.

### Decision: Window length is a per-window constant keyed by which window it is
The 5-hour and weekly lengths are definitional (the field names *are* the
lengths), so each frontend hardcodes the two constants rather than inferring a
length from data. Clamping the fraction to `[0, 1]` means clock skew or an
already-past reset time pins the marker to an edge instead of overflowing.

### Decision: Desktop draws gridlines + an absolutely-positioned marker rule
The `.quota-meter` track gets segment dividers (a `repeating-linear-gradient`
sized to `100% / segments`, or N inset hairline elements) and a 1px marker element
positioned at `left: <elapsed>%`. The utilization fill stays a `%`-width block
underneath. Continuous pixels make this the easy side.

### Decision: TUI widens each bar to one cell per segment and marks the active cell
Replace the shared `QUOTA_BAR_CELLS = 5` with a per-window segment count: the
5-hour bar is **5 cells** (one per hour), the weekly bar **7 cells** (one per
day). The utilization fill stays whole-cell (existing `quota_fill_cells` grammar,
generalized to take the cell count). The "now" marker is a **cell decoration** on
the current segment — primary: `Modifier::UNDERLINED` on that cell; honoring the
existing ASCII / color-depth ladder, with a glyph-swap fallback (e.g. a distinct
marker glyph in the active empty cell) where underline isn't reliable.

*Why a decoration, not a separate marker row/glyph:* the title bar is a single
line and horizontal space is scarce. Using a *decoration* channel (underline /
reverse) for time keeps it orthogonal to the *glyph-density* channel (fill =
utilization) and the *color* channel (severity), so all three read without
colliding and without extra width. The weekly bar grows from 5→7 cells (+2),
which the flush-right layout already truncates gracefully under pressure.

*Alternative considered:* keep 5 cells and overlay a marker glyph inside a cell.
Rejected — 7 days don't divide into 5 cells, and cramming fill + divider + marker
into 5 chars is illegible.

### Decision: Marker depends on reset time; segments ride with it
When `resetsAtUnix` is `None`, render today's plain unsegmented bar — no
segments, no marker. Avoids a confusing "segments without a cursor" half-state and
keeps the no-data path identical to current behavior.

## Risks / Trade-offs

- **Reinterpreting the TUI cells as time buckets could read as "N hours used."** →
  Mitigation: fill (glyph density) and marker (underline) are distinct channels;
  the marker is clearly a single highlighted cell, not a fill level. Covered by a
  render test.
- **Weekly reset may be rolling, not a fixed weekly anchor.** → The elapsed
  fraction is correct either way ("how far through the current window"); only
  *day labels* would depend on it, and labels are a non-goal here.
- **Assumes exactly 5 h / 7 d window lengths.** → True by the window names;
  clamping bounds the marker if an assumption ever breaks, so the worst case is a
  marker pinned to an edge, never a broken bar.
- **Minor TUI width growth (weekly 5→7 cells).** → Title bar already truncates the
  dim status first; the gauge sits flush-right and is the last thing dropped.
