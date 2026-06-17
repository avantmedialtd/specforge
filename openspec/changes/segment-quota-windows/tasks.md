## 1. Desktop frontend (`QuotaPill.tsx` + `App.css`)

- [ ] 1.1 Add an elapsed-fraction helper: `(resetsAtUnix, nowMs, lengthSecs) → clamp(1 − (resetsAt·1000 − now)/lengthMs, 0, 1)`, with per-window lengths (5 h = 18 000 s, 7 d = 604 800 s); return `null` when `resetsAtUnix` is null
- [ ] 1.2 Pass each `WindowRow` its segment count (5h → 5, wk → 7) and length; compute the marker fraction live off the existing `nowMs` tick
- [ ] 1.3 Render segment dividers on the `.quota-meter` track (gridlines sized to `100% / segments`) over the existing utilization fill
- [ ] 1.4 Render a 1px "now" marker element positioned at `left: <elapsed>%`; omit both dividers and marker when the fraction is `null` (plain-bar fallback)
- [ ] 1.5 Add the divider + marker styles to `App.css` (reuse the meter idiom; respect `prefers-reduced-motion`, keep the threshold fill colors)

## 2. Terminal UI (`crates/specforge-tui/src/ui.rs`)

- [ ] 2.1 Generalize `quota_fill_cells` / `quota_bar` to take a per-window cell count instead of the shared `QUOTA_BAR_CELLS`
- [ ] 2.2 Set per-window segment counts in `window_spans` / `quota_gauge`: 5-hour → 5 cells, weekly → 7 cells
- [ ] 2.3 Add an elapsed-fraction → active-segment-cell mapping (reuse/extend the existing `countdown` time math; clamp to bounds)
- [ ] 2.4 Render the active segment cell with the "now" marker decoration (primary `Modifier::UNDERLINED`; honor the ASCII / color-depth ladder with a glyph-swap fallback)
- [ ] 2.5 Fall back to today's unsegmented bar when `resets_at_unix` is `None`

## 3. Verification & docs

- [ ] 3.1 Unit-test the elapsed-fraction math (negative → 0, past reset → 1, mid-window) and the active-cell mapping for both the 5-cell and 7-cell widths, plus the reset-time-absent fallback
- [ ] 3.2 Update the TUI `render_tests` expectations for the new segmented bars (5h 5-cell, weekly 7-cell, with/without marker)
- [ ] 3.3 Manually verify both frontends enabled: marker advances live between polls, fill-ahead-of-marker reads as "over pace", and the no-reset-time fallback shows the plain bar
- [ ] 3.4 `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test`, and frontend typecheck + build all green
- [ ] 3.5 Update the README quota section to describe the segments + "now" marker (and that it reads as pace, not a histogram)
