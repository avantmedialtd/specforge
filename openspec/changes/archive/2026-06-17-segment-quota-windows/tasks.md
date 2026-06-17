## 1. Desktop frontend (`QuotaPill.tsx` + `App.css`)

- [x] 1.1 Add an elapsed-fraction helper: `(resetsAtUnix, nowMs, lengthSecs) → clamp(1 − (resetsAt·1000 − now)/lengthMs, 0, 1)`, with per-window lengths (5 h = 18 000 s, 7 d = 604 800 s); return `null` when `resetsAtUnix` is null
- [x] 1.2 Pass each `WindowRow` its segment count (5h → 5, wk → 7) and length; compute the marker fraction live off the existing `nowMs` tick
- [x] 1.3 Render segment ticks on the `.quota-meter` track (`segments − 1` interior ticks at `((i+1)/segments)·100%`) over the existing utilization fill
- [x] 1.4 Render a 2px "now" marker element positioned at `left: <elapsed>%`; omit both ticks and marker when the fraction is `null` (plain-bar fallback)
- [x] 1.5 Add the tick + marker styles to `App.css` (relative-positioned meter; theme-adaptive `--border` ticks and `--text` marker; existing fill threshold colors and reduced-motion rule untouched)

## 2. Terminal UI (`crates/specforge-tui/src/ui.rs`)

- [x] 2.1 Generalize `quota_fill_cells` to take a per-window cell count; drop the shared `QUOTA_BAR_CELLS` and inline the cell loop (replacing `quota_bar`)
- [x] 2.2 Set per-window segment counts in `window_spans` / `quota_gauge`: 5-hour → 5 cells, weekly → 7 cells (`FIVE_HOUR_CELLS` / `SEVEN_DAY_CELLS`)
- [x] 2.3 Add an elapsed-fraction → active-segment-cell mapping (`elapsed_fraction` + `marker_cell`, pure and clamped, using the same `SystemTime` now as `countdown`)
- [x] 2.4 Render the active segment cell with the "now" marker decoration via `Modifier::UNDERLINED`, honoring the existing ASCII / color-depth glyph ladder for the fill cells
- [x] 2.5 Fall back to an unsegmented bar (no underlined cell) when `resets_at_unix` is `None`

## 3. Verification & docs

- [x] 3.1 Unit-test the elapsed-fraction math (negative → 0, past reset → 1, mid-window) and the active-cell mapping for both the 5-cell and 7-cell widths, plus the reset-time-absent fallback
- [x] 3.2 Update the TUI `render_tests` to exercise the new segmented bars (5h 5-cell, weekly 7-cell, marker at first/last cell across both widths)
- [x] 3.3 Manually verify both frontends enabled: marker advances live between polls, fill-ahead-of-marker reads as "over pace", and the no-reset-time fallback shows the plain bar (desktop gauge live-verified against the real enabled config; TUI covered by render tests)
- [x] 3.4 `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test`, and frontend typecheck + build all green
- [x] 3.5 Update the README quota section to describe the segments + "now" marker (and that it reads as pace, not a histogram)
