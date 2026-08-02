# Tasks: Fix the Inverted ChatGPT Quota Reading

## 1. Correct the reading

- [x] 1.1 In `crates/openspec-app/src/chatgpt_quota.rs`, change `parse_window` to treat the response's window percentage as *remaining* and store `utilization = 100 - remaining` (clamped to `0..=100`), naming the intermediate binding for what the wire value holds and documenting the Codex-app comparison that established it (`chatgpt-quota`: *Quota status-line gauge*)
- [x] 1.2 Update the module-level docs so the endpoint's reported semantics are stated once, where a future reader will look

## 2. Align the window labels

- [x] 2.1 In `crates/specforge-tui/src/ui.rs`, make `chatgpt_window_axis` return `wk` for a week-length window (within an hour tolerance) and `5h` for a five-hour one (within a ten-minute tolerance), keeping the derived `Nh`/`Nd` label for any other length; segment counts and axis length are unchanged (`chatgpt-quota`: *Quota status-line gauge*)
- [x] 2.2 Apply the identical rule to `axisFor` in `src/components/ChatGptQuotaPill.tsx` so the desktop and web strips match the TUI

## 3. Tests

- [x] 3.1 Update the existing `chatgpt_quota.rs` parser tests to the corrected meaning, and add cases pinning the inversion at both ends: response 8 → 92% displayed, response 100 → 0%, response 0 → 100%, plus clamping of an out-of-range value
- [x] 3.2 Add label tests covering `wk`, `5h`, a near-but-not-exact week, and a non-standard length falling back to the derived form — in the TUI (`ui.rs`) alongside the existing group-fitting tests

## 4. Verification

- [x] 4.1 `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `bun run build` all green
- [x] 4.2 App-level smoke against a `specforge-web` debug build with an isolated fake `HOME`/`CODEX_HOME`, confirming the strip still renders and degrades correctly with no credentials present
- [ ] 4.3 **Needs the user's own Codex login:** confirm the weekly row now reports the same consumption as the Codex desktop app, and that the row reads `wk` rather than `7d`
