# Terminal UI Sidebar Width Cap

## Why

The Browse screen splits its width with a flat `Constraint::Percentage(42)` for
the workspace/change tree and `Percentage(58)` for the artifact-detail pane. 42%
is nearly half the screen handed to a narrow tree (a few names plus a small
progress bar), and because it is an uncapped percentage it only gets worse as the
terminal widens: on a 200-column terminal the tree eats ~84 columns while the
proposal/design/tasks markdown — the actual content — is squeezed into the rest.
The sidebar is also neither collapsible nor resizable; `Tab` only moves focus.

## What Changes

- Replace the flat 42/58 percentage split with a **capped, responsive** tree
  width: the sidebar scales down on small terminals and stops growing past a
  sensible maximum on wide ones, so the detail pane always gets the surplus.
- Keep the existing two-pane/one-pane behaviour: above the current width
  threshold the layout is two panes; below it, the single-switchable-pane
  fallback is unchanged.

## Capabilities

### New Capabilities
<!-- none — this extends the existing terminal-ui capability -->

### Modified Capabilities
- `terminal-ui`: the **Master-Detail Browse and Screen Navigation** requirement
  is clarified so the Browse split gives the detail pane the surplus width on
  wide terminals rather than a fixed near-half to the tree.

## Impact

- `crates/specforge-tui/src/ui.rs` — the `browse()` layout constraints change
  from `Percentage(42)/Percentage(58)` to a clamped `Length` for the tree plus
  `Min(0)` for the detail pane. Self-contained; no change to the 90-column
  two-pane threshold, focus model, or any other screen.
- Snapshot tests in `render_tests.rs` covering the Browse layout are updated for
  the new widths.
