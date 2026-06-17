# Segment the Claude Quota Bars with a Time Marker

## Why

The Claude quota gauge (shipped in v0.8.0) shows a single number per window — how
much of the budget is spent — but nothing about *where you are in the window*. A
flat "62%" can't tell you whether you're fine or about to hit the wall: 62% used
with four hours left is relaxed; 62% used with twenty minutes left is a cliff.

The window's reset time is already in every snapshot, and the windows are
definitionally fixed lengths (5 hours, 7 days), so the gauge can already tell how
far the clock has moved through the window. Overlaying that **time axis** on the
existing utilization bar turns the gauge from "how much is left" into the more
actionable "**am I on pace?**" — read by comparing the fill (budget spent) against
a live "now" marker (time elapsed).

## What Changes

- Add a **time axis** to each window bar in both frontends. The fill still shows
  utilization; on top of it the gauge draws:
  - **Segment dividers** — the 5-hour bar split into **5 hour segments**, the
    weekly bar into **7 day segments**.
  - A live **"now" marker** at the elapsed fraction of the window, ticking forward
    between polls (computed the same way the existing reset countdown is).
- The marker's position is derived **entirely in the frontends** from the window's
  `resetsAtUnix` and its definitional length (5 h / 7 d):
  `elapsed = clamp(1 − (resetsAt − now) / length, 0, 1)`. **No new backend data,
  no new IPC fields, no extra network calls.**
- **Graceful fallback**: when a window's reset time is unknown, the gauge renders
  the current unsegmented bar (no dividers, no marker) — it never shows a time
  axis it can't anchor.
- **Desktop**: gridlines on the meter track plus an absolutely-positioned marker
  rule at the elapsed offset.
- **TUI**: each window's bar widens to one cell per segment (5 cells for hours,
  7 for days); the current segment cell carries the "now" marker via a cell
  decoration, honoring the existing ASCII / color-depth fallback ladder.

Explicitly **out of scope** (possible follow-ups, not this change): weekday/day
labels under the segments (needs local-timezone day-of-week math and certainty
about whether the weekly window is a fixed or rolling reset), and "pace" coloring
that flags when the fill outruns the marker.

Nothing is **BREAKING** — the change is additive rendering over the existing
snapshot, gated behind the same opt-in.

## Capabilities

### New Capabilities
<!-- None. This extends how an existing capability renders; it adds no new capability. -->

### Modified Capabilities
- `claude-quota`: the **Quota status-line gauge** requirement gains a time axis —
  hour/day segment dividers and a live "now" marker over the utilization fill,
  with a fallback to the unsegmented bar when a window's reset time is unknown.

## Impact

- **Frontend only.** No changes to `openspec-app` (`quota.rs`), `openspec-core`,
  the Tauri commands/events, or the `ClaudeQuotaState` / `QuotaWindow` types on
  either side of the IPC boundary. The snapshot already carries everything needed.
- **`src/components/QuotaPill.tsx` + `src/App.css`**: segment gridlines on the
  `.quota-meter` track and a marker element positioned at the elapsed fraction,
  computed live alongside the existing `nowMs` tick.
- **`crates/specforge-tui/src/ui.rs`**: per-window segment counts (5h → 5,
  wk → 7) replacing the shared `QUOTA_BAR_CELLS`, plus a current-segment marker
  decoration; new unit tests for the elapsed-fraction → marker-cell mapping
  alongside the existing `quota_fill_cells` / `quota_severity` tests.
- **Risk**: low and self-contained. The only assumption is that the windows are
  exactly 5 h and 7 d (true by their names `five_hour` / `seven_day`); the marker
  fraction is clamped to `[0, 1]` so clock skew or a stale reset time degrades to
  the window edge rather than overflowing the bar.
