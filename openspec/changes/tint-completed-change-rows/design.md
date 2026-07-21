## Context

The workspace tree already styles completion (change `2026-07-18-light-up-completed-work`): a completed two-line change row carries an `--ok-strong` left rail and a filled `--ok-strong` completion disc with a knocked-out `--surface` check. The row grammar renders these through:

- `WorkspaceTree.tsx` — the `Row` primitive receives a `complete` flag and computes `railClass = detail != null && complete ? " tree-row--complete" : (… " tree-row--rail-{color}")`. The flag is set from `allTasksDone(change)` at the flattened singleton `InstanceNode` and at `FlatChangeNode` — the only two-line change rows. So **`tree-row--complete` already lands on exactly the rows this change targets.**
- `App.css` — `.tree-row--complete { border-left-color: var(--ok-strong); }` (the rail), sitting below the `.tree-row.selected` rail override which wins by specificity.

The prior change deliberately stopped short of a wash: its design records "*Alternative rejected — go fully loud (disc + a completion row wash)*", reasoning that a full-row wash meant *selected* and nothing else. This change revisits that call: with the tree in daily use, a completed row's body reads as indistinguishable from an in-progress one, so completion needs a highlight the eye lands on — a soft green wash — provided selection's claim on the *accent* wash is preserved.

Two `visual-identity` requirements bound this:

- **Completed-State Styling** — currently states "A completed change SHALL NOT receive a full-row background wash; the full-row wash remains the exclusive signal of selection." This clause is reversed (for the completion wash specifically), while selection keeps priority.
- **Tree Row Selection Model** — currently states hover on an unselected row renders `--surface-2` "uniformly on every row." The completed-row hover-deepen needs a carve-out here.

## Goals / Non-Goals

**Goals:**

- A completed two-line change row renders a soft `--ok`-family background wash, additive to the existing rail + disc.
- The wash deepens on hover (mirroring the selected-row hover) instead of clearing to the neutral hover.
- Selection continues to fully override the completion treatment (bar + accent wash win), with zero regression to the current selection behaviour.
- Text and the completion disc remain legible on the washed row in both schemes.
- Ship as a **pure-CSS + token** change with no Rust and (ideally) no TSX change.

**Non-Goals:**

- No wash on Sections, the Tasks artifact node, multi-instance child rows, or leaf tasks — the change row only.
- No motion / completion animation.
- No workspace-level roll-up (top always-visible rows untouched).
- No change to how completion is *derived* — it stays `completedTasks === totalTasks`.

## Decisions

### Pure-CSS: reuse the existing `tree-row--complete` hook

The `complete` flag already flows to `Row` and already emits `tree-row--complete` on the two exact row types in scope. The wash therefore attaches to that existing class — a `background` added to the current `.tree-row--complete` rule, plus a `.tree-row--complete:hover` rule. **No new prop threading and no TSX change are anticipated;** implementation confirms this before declaring done. (If a future refactor split the rail class from a wash class, the `complete` flag is still the single source — but there is no reason to split today.)

### Token derivation — `--ok-tint` / `--ok-tint-strong` mirror the accent-tint pair

Two tokens are added, following the established `--accent-tint` / `--accent-tint-strong` pattern (a base wash + a hover-deepened wash), over the `--ok` hue:

| Token | Light | Dark | Mirrors |
|---|---|---|---|
| `--ok-tint` | `rgba(16, 185, 129, 0.10)` | `rgba(52, 211, 153, 0.14)` | `--accent-tint` alpha |
| `--ok-tint-strong` | `rgba(16, 185, 129, 0.16)` | `rgba(52, 211, 153, 0.22)` | `--accent-tint-strong` alpha |

- **Why derive from `--ok`, not `--ok-strong`.** Washes in this system derive from the *bright* state colour (`--accent-tint` derives from `--accent`, not a darker variant); `--ok` (`#10b981` light / `#34d399` dark) is that bright green. `--ok-strong` is a *foreground* token (disc fill, rail, label) and stays foreground. Keeping the wash on `--ok` and the marks on `--ok-strong` mirrors the accent family's fill/foreground split.
- **Why the alphas are matched to accent-tint.** The `--accent-tint` wash is already accepted as a background that `--text` clears ≥4.5:1 on (per *Tree Row Selection Model*). Using identical alphas over a green hue keeps composited lightness in the same band, so text contrast on the completion wash is no worse than on the selection wash — a contrast argument by parity rather than a fresh per-token audit (still verified in both schemes).
- **Rejected — a single wash token with no hover variant.** Without `--ok-tint-strong`, hovering a completed row would either clear to grey (jarring — the green vanishes on hover) or stay flat (no hover feedback). The pair mirrors `selected` / `selected:hover` and gives consistent affordance.

### CSS ordering + specificity — the load-bearing detail

Backgrounds are decided by specificity, then source order. The applicable selectors and their specificity:

| Selector | Specificity | Background |
|---|---|---|
| `.tree-row:hover` | (0,2,0) | `--surface-2` |
| `.tree-row.selected` | (0,2,0) | `--accent-tint` |
| `.tree-row.selected:hover` | (0,3,0) | `--accent-tint-strong` |
| `.tree-row--complete` (new wash) | (0,1,0) | `--ok-tint` |
| `.tree-row--complete:hover` (new) | (0,2,0) | `--ok-tint-strong` |

Case analysis (must all hold):

- **Completed, unselected, idle** → only `.tree-row--complete` (0,1,0) applies → `--ok-tint`. ✓
- **Completed, unselected, hover** → `.tree-row:hover` (0,2,0) vs `.tree-row--complete:hover` (0,2,0): a tie broken by **source order**, so `.tree-row--complete:hover` MUST be placed *after* `.tree-row:hover` → `--ok-tint-strong` wins. This is the one placement the implementation must get right.
- **Completed, selected, idle** → `.tree-row.selected` (0,2,0) beats `.tree-row--complete` (0,1,0) by specificity, regardless of order → `--accent-tint`. ✓ (Same mechanism that already makes the selection rail beat the completion rail.)
- **Completed, selected, hover** → `.tree-row.selected:hover` (0,3,0) beats `.tree-row--complete:hover` (0,2,0) by specificity → `--accent-tint-strong`. ✓
- **Not completed** → no `--complete` rule applies; existing behaviour is untouched. ✓

Because `.tree-row.selected` / `.tree-row.selected:hover` outrank the completion rules by specificity, selection wins **without** depending on source order — so the new rules can live beside the existing `.tree-row--complete` rail rule. Only the completed-hover-vs-plain-hover tie is order-sensitive, and both are placed after `.tree-row:hover`. The rail already relies on this exact specificity story, so this extends a proven pattern rather than inventing one.

### The wash is additive; the disc is unaffected internally

The completion disc is a self-contained `--ok-strong` circle with a `--surface` check punched through it. Its internal contrast (check vs disc) is independent of whatever sits *behind* the disc, so a washed row does not touch the disc's legibility. Externally, a deep-green disc on a very pale-green wash still separates (the wash is ≤0.16/0.22 alpha — near-white / near-surface). Rail (a 2px edge), wash (the body), and disc (a ~15px glyph) operate at three different scales, so "three greens" reads as one coherent done-state, not as noise — verified visually in both schemes.

## Risks / Trade-offs

- **Source-order fragility on the completed-hover tie** → The `.tree-row--complete:hover` rule must sit after `.tree-row:hover`; a later reorder could silently revert completed-row hover to grey. Mitigation: place the new rules immediately adjacent to the existing `.tree-row--complete` rail rule (already after the hover/selected block) with a comment recording the specificity contract; confirm the four hover/selection cases in the running app.
- **Text contrast on the green wash** → `--text` and the muted change-id detail sit on `--ok-tint`. Mitigation: alphas matched to `--accent-tint`, which `--text` already clears ≥4.5:1 on; still explicitly verified in light and dark.
- **Over-greening a completed row** → rail + wash + disc are all green. Mitigation: the wash is deliberately low-alpha; the three elements live at different scales. If it reads heavy in practice, the wash alpha is the single tuning knob. Accepted for now.
- **Reopening a deliberately-closed door** → the prior change explicitly rejected a completion wash to protect "wash means selected." Mitigation: this change narrows that invariant precisely — the *accent* wash still means selected and still wins; the green wash is a distinct, subordinate state — rather than abandoning it, and the spec delta records the narrowing on both requirements.

## Migration Plan

Pure additive presentation change: two new tokens, two new/extended CSS rules. No data, no persisted state, no IPC surface touched. Rollback is reverting the CSS + token additions; nothing else references the new tokens.
