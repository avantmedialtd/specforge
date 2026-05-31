# Let the Dashboard fill its pane width

## Why

The Dashboard is the app's home surface, but it caps its own content at `max-width: 920px` and centers it (`margin: 0 auto`). On a laptop the cap never bites — the center pane is already narrower than 920px once the sidebar and commit-graph rail take their share — so the rule is invisible there. On larger displays it bites hard: at 1920px the pane is ~1310px wide and the Dashboard leaves ~390px of dead gutter; on an ultrawide it floats a 920px column in the middle of a ~1950px pane.

The result is that the one surface meant to give a confident, at-a-glance overview wastes the space it's given precisely on the displays that have space to give. Everything inside the Dashboard is already proportional — the panel grids are `1fr 1fr`, the bars and meters are percentage-width — so the content reflows to fill on its own the moment the cap is lifted. No per-panel work is required.

## What Changes

- Remove the `max-width: 920px` and `margin: 0 auto` declarations from `.dashboard` so it fills the full width of the center pane at any window size, keeping its existing padding.
- Leave the surrounding shell untouched: the left sidebar and the commit-graph rail keep their current widths and behaviour; only the Dashboard's self-imposed cap goes away.
- Accept unbounded width as a deliberate choice: on very wide displays the two-column panels stretch and individual rows read airy. Introducing a wider breakpoint (e.g. a third column past some width) is explicitly out of scope and left as a possible future polish.

## Impact

- Affected specs: `dashboard` (new presentation requirement — the Dashboard fills the available pane width)
- Affected code: `src/App.css` (the `.dashboard` rule)
