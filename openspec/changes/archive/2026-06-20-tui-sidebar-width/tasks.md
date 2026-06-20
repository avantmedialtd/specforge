## 1. Layout change

- [x] 1.1 In `ui.rs` `browse()`, compute `tree_width = clamp(round(area.width * 0.32), 28, 44)`
- [x] 1.2 Replace the `Percentage(42)/Percentage(58)` constraints with `[Length(tree_width), Min(0)]`, leaving the `TWO_PANE_MIN_WIDTH` branch and one-pane fallback untouched

## 2. Tests

- [x] 2.1 Update/add `render_tests.rs` snapshots for the Browse layout at representative widths (e.g. 90, 140, 220), asserting the tree is bounded and the detail pane takes the surplus
- [x] 2.2 Run `cargo test -p specforge-tui` and confirm the suite passes
