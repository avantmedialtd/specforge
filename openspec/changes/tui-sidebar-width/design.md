## Context

`browse()` in `crates/specforge-tui/src/ui.rs` splits the main area with
`Constraint::Percentage(42)` (tree) / `Percentage(58)` (detail) whenever the
width is at least `TWO_PANE_MIN_WIDTH` (90); below that it renders a single
switchable pane. The percentage is flat and uncapped, so the tree's absolute
width grows linearly with the terminal and starves the detail pane on wide
terminals.

## Goals / Non-Goals

**Goals:**
- The detail pane gets the surplus on wide terminals; the tree stays readable on
  small ones.
- One contained change to the Browse layout constraints; no behavioural change
  elsewhere.

**Non-Goals:**
- Collapsible or user-resizable sidebar, or a persisted width preference (a
  possible later change, explicitly out of scope here).
- Touching the `TWO_PANE_MIN_WIDTH` threshold or the one-pane fallback.
- Lightening pane borders / other "looks" work (separate from width).

## Decisions

### Capped-responsive tree width

Replace the percentage pair with a clamped fixed width for the tree and `Min(0)`
for the detail pane:

```
tree_width = clamp(round(area.width * 0.32), 28, 44)
constraints = [Length(tree_width), Min(0)]
```

- **Floor 28**: at the 90-column threshold, 0.32·90 ≈ 29 — enough to read change
  names and the small progress bar (border eats 2).
- **Ceiling 44**: past ~138 columns the tree stops growing and every extra column
  goes to content.
- Percentages chosen over a pure fixed width so the tree still tracks the
  terminal between the floor and ceiling.

Alternatives considered: just lowering the percentage (still unbounded on
ultrawide); a pure fixed `Length(38)` (ignores small terminals, truncates long
names sooner); a collapse/resize key (more code + a persisted toggle — deferred).
The exact constants may be nudged during implementation against the snapshot
tests.

## Risks / Trade-offs

- **Long workspace/change names truncate sooner at the ceiling** → acceptable;
  names already truncate, and the detail pane benefits more than the tree loses.
- **Constant-tuning bikeshed** → pin behaviour with snapshot tests at a few
  representative widths (90, 140, 220) and treat the constants as adjustable.
