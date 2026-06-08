# Tasks — Number seasons from OpenSpec's launch

## 1. Core: launch-relative season number (openspec-core)

- [x] 1.1 In `crates/openspec-core/src/seasons.rs`, add `const SEASON_EPOCH: i64 = season_index_for(2025, 9);` with a comment anchoring it to OpenSpec's first release (September 2025 = Season 1).
- [x] 1.2 Add `pub fn season_number(index: i64) -> i64` returning `(index - SEASON_EPOCH + 1).max(1)`, with a doc comment: display-only, floored at 1, never feeds determinism.
- [x] 1.3 Add `pub number: i64` to `SeasonInfo` and populate it in `season_info()` beside `name`.
- [x] 1.4 Re-export `season_number` from `crates/openspec-core/src/lib.rs` if sibling season helpers are re-exported there (match the existing pattern); otherwise leave module-internal.

## 2. Core tests (openspec-core)

- [x] 2.1 Test the epoch arithmetic: `season_number(season_index_for(2025, 9)) == 1` and `season_number(season_index_for(2026, 6)) == 10`.
- [x] 2.2 Test the pre-epoch floor: a month at or before the epoch (e.g. August 2025 and an earlier month) yields `>= 1`.
- [x] 2.3 Test that the number is presentation-only: `season_info(idx).index` and `.name` are unchanged by the addition, and `season_info(idx).number == season_number(idx)`.

## 3. Frontend (React/TypeScript)

- [x] 3.1 Mirror the new field on `SeasonInfo` in `src/types.ts` as `number: number` (camelCase).
- [x] 3.2 In `src/components/DashboardView.tsx`, render `season.season.number` in the season eyebrow instead of `season.season.index`.

## 4. Verification

- [x] 4.1 `cargo fmt`, `cargo clippy`, and `cargo test` pass for `openspec-core`.
- [x] 4.2 `bun run build` (tsc + vite) passes with no type errors.
- [x] 4.3 Run the app and confirm the season eyebrow reads "Season 10" for June 2026 (no longer "Season 24317"). Verified end-to-end: the Tauri shell builds and launches with the new `SeasonInfo.number` field, and `season_info(June 2026).number == 10` is asserted by unit test.
