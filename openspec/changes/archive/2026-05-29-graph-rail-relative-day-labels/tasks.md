## 1. Relative day labelling

- [x] 1.1 In `src/components/GraphRail.tsx`, add a module-level `Intl.RelativeTimeFormat(undefined, { numeric: "auto" })` instance and a local-midnight `startOfDay` helper for DST-safe day math.
- [x] 1.2 Rewrite `dayLabel(iso)` to compute the rounded calendar-day delta between the commit day and the current day (both anchored to local midnight) and branch on it: keep the existing `Number.isNaN` fallback to the raw ISO string.
- [x] 1.3 Map `diff === 0` → `Today` and `diff === -1` → `Yesterday` via the `RelativeTimeFormat` instance (locale-aware).
- [x] 1.4 Map `diff` in `-2..-6` → weekday name via `toLocaleDateString(undefined, { weekday: "long" })`.
- [x] 1.5 Leave the existing `{ weekday: "short", month: "short", day: "numeric" }` absolute format as the fallback for all other deltas (7+ days back and any future-dated commit).
- [x] 1.6 Confirm `dayKey` is untouched so grouping boundaries are unchanged.

## 2. Verify

- [x] 2.1 `bun run build` passes (tsc strict, including `noUnusedLocals`/`noUnusedParameters`).
- [x] 2.2 Confirmed the rail's label output for this repo's real commit dates — `Today` / `Yesterday` / `Wednesday` then absolute dates — via the standalone logic check; the change is live in the already-running `bun tauri dev` window via HMR (CSS uppercasing applies).
- [x] 2.3 `formatTimestamp` (hover tooltip) and `computeGeometry` (lane edges through separator bands) are untouched by this edit — only `dayLabel`'s returned string changed — so both behave as before.
