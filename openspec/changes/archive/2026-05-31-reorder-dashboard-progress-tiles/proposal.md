# Reorder Today's Progress tiles to lead with shipped

## Why

The Today's Progress band is the Dashboard's topmost hero — the first thing the user sees on the app's home surface. Today it leads with `tasks done` and buries `shipped` in the second slot, with `started` last:

```
NOW:   ✔ tasks done   🏆 shipped     ⎇ commits     ✚ started
```

Shipping a whole change is the rarest, highest-value event the band tracks, and it is the product's north star — it deserves to lead. Ordering the four counts by *coarsest unit of work first* puts the two change-level events at the front as a natural pair (a whole change shipped, a whole change started — the death and birth of a change) and the within-change increments behind them:

```
WANT:  🏆 shipped      ✚ started      ⎇ commits     ✔ tasks done
       └──── whole-change events ────┘ └──── increments ────┘
```

This also resolves the only ambiguity in the ordering: `started` belongs second because it is the same grain as `shipped`, not because of where it falls in a change's chronology.

The tradeoff is deliberate and accepted: the two tiles now leading (`shipped`, `started`) are the ones most often `0` on a given day, so the band may open with two zeros, where the old order led with the liveliest non-zero number (`tasks done`). Leading with the north star even when it is zero is the intended message, and the existing encouraging zero-state nudge already softens an all-zero day.

This is a pure presentation reorder: no metric is added, removed, or recomputed, and no count's value, animation, average comparison, or zero-state behaviour changes — only the left-to-right order of the tiles (and the matching order of the heatmap day drill-down strip) does.

## What Changes

- Reorder the four `<HaulTile>`s in the `TodayHaul` component to render **shipped → started → commits → tasks** (currently tasks → shipped → commits → started).
- Reorder the `parts.push(...)` calls in `HeatmapDetail` so the heatmap day drill-down strip lists its present kinds in the same order (`🏆 shipped · ✚ started · ⎇ commits · ✔ tasks`), keeping the hero and the drill-down coherent.
- Update the `dashboard` spec: relist the Today's Progress kinds in the new order, state that the band presents them as a fixed coarsest-first sequence, pin that order with a new scenario, and relist the heatmap drill-down breakdown kinds to match.

## Impact

- Affected specs: `dashboard` (Today's Progress Hero — kind list reordered and the display order now an asserted contract; Streak and Contribution Heatmap — day drill-down breakdown order)
- Affected code: `src/components/DashboardView.tsx` (`TodayHaul` tile order; `HeatmapDetail` parts order). No change to data shapes, the `TodayProgress`/`HeatmapCell` types, the Rust core, or any metric computation.
