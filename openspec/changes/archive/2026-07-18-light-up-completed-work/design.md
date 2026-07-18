# Design — Light Up Completed Work in the Workspace Tree

## Context

The workspace tree (`src/components/WorkspaceTree.tsx`) renders completion through two mechanisms today:

- **Aggregate rows** (a Section, the Tasks artifact node, and a fully-complete change/instance) show a trailing `<Check className="icon-checked" />` — a hand-rolled outline-polyline SVG (`src/components/icons.tsx`, `points="5 13 10 18 19 7"`, `stroke="currentColor"`). The class `icon-checked` has **no CSS rule anywhere**, so the glyph inherits `.row-meta`'s `--text-muted`: the completion mark is grey.
- **Leaf task rows** carry no glyph; `TaskNode` passes `struck={task.completed}`, and `.tree-row--struck .row-label` renders `text-decoration: line-through; color: var(--text-faint)` — struck and dimmed.

The only `--ok` green in the row grammar is the *in-progress* task-progress meter fill. So completion is currently the least-coloured state in the tree, inverting the universal "green = done" reading.

Two `visual-identity` invariants bound this change:

- **Outlined Chip Badges:** "the task-progress meter is the single sanctioned filled element in the row grammar … No other chip, badge, or status dot SHALL use a filled background or a glow." The glow tokens (`--glow-ok`, `--shadow-accent`, …) are reserved to an enumerated set — the selected row, the primary button, focused inputs, the focus ring, and the in-progress meter.
- **Tree Row Selection Model:** a full-row `--accent-tint` wash **is** the "selected" signal; a selected row also gets a 2px `--accent` left bar that already overrides a two-line row's workspace-colour rail (`tree-row--rail-{color}`).

The design below crosses the first invariant deliberately (and minimally) and preserves the second untouched.

## Decisions

### The completion glyph becomes the second sanctioned filled element

The done mark changes from a grey outline `✓` to a **filled `--ok` disc with a knocked-out check** — a `--surface`-coloured check punched through an `--ok` circle. This is a deliberate, bounded amendment to *Outlined Chip Badges*: the grammar now sanctions **two** filled elements, and they are symmetric — the **in-progress meter is the "not-done" fill, the completion disc is the "done" fill**, the two task-states of a change.

- **Why a disc, not just a green polyline.** A filled disc reads as an *achievement mark* rather than another hairline glyph. Recolouring the polyline to an outline `--ok` check was the quieter alternative, offered and rejected during exploration in favour of the loudest legible option.
- **Why this stays inside the spirit of the invariant.** The amendment is *bounded and principled* — exactly one new filled element, justified by the meter↔disc symmetry, inheriting none of the reserved glow. It is not a general licence for filled chips.
- **Alternative rejected — go fully loud (disc + a completion row wash).** Adding an `--ok-tint` full-row wash on completed rows was explored and rejected because it overloads the wash, which currently means *selected* and nothing else (see below).

### The disc carries no glow, and completion never washes the row

- **No glow on the disc.** The reserved-glow invariant is preserved verbatim: the completion disc has no `--glow-ok` halo. The meter's optional `--glow-ok` remains the single sanctioned `--ok` glow. This amendment adds a filled element, not a glowing one.
- **No completion wash.** A full-row background wash still means *selected*, and only *selected*. A completed-but-unselected change is signalled by its rail and disc, never by a tinted row. A welcome consequence: with no wash there is **no `--ok-tint` wash token to add** — the change reduces to recolour + one disc.

### A contrast-tuned foreground green (`--ok-strong`)

The done marks are foreground elements on the row surface, but `--ok` (`#10b981` light / `#34d399` dark) is tuned as the *fill inside the outlined progress-meter track*, where a lighter green reads fine. Used as a foreground on the light scheme's white `--surface`, `--ok` is only ~2.6:1 — below AA for the completed-task **label text** and below the 3:1 non-text floor for the disc against its background. So a single new token is added, **`--ok-strong`**: `#047857` on light (~5.3:1 on white — the same deep emerald the codebase already trusts for `--code-fg` inline-code text) and `#34d399` on dark (9.34:1).

- **Where it applies.** The completion disc fill, the completed-change rail, and the completed-task label all use `--ok-strong`; the disc's knocked-out check stays `--surface`, which now clears ~5.3:1 against the deeper disc in light and remains high-contrast in dark. `--ok` keeps its single existing job — the in-progress meter fill — so "in-progress green" and "done green" are the same family but the done variant is deep enough to carry as foreground.
- **Why not reuse `--code-fg`.** It is the identical light value, but it means "inline code" — coupling completion state to the code colour would make one token answer to two unrelated concepts. A dedicated `--ok-strong` keeps the token's meaning honest.
- **Why one token, not per-theme literals.** Literal colours outside the token layer are prohibited by *Design Token Layer*; `--ok-strong` carries the light/dark split the same way every other state colour does.

### Completed change → `--ok` rail, subordinate to selection

A completed two-line change row (the flattened singleton `InstanceNode` and `FlatChangeNode`) swaps its `tree-row--rail-{workspaceColor}` for an `--ok` rail.

- **Composition with selection.** Selection must still win. The stylesheet already overrides the workspace-colour rail with the `--accent` bar on `.selected`; the new `tree-row--complete` rail rule sits at the same or lower specificity, so a selected completed change shows the accent bar unchanged. No change to the *Tree Row Selection Model* requirement is needed — selection wins by its own existing rule.
- **The trade.** A completed change loses its workspace-identity colour on the rail. That is acceptable: a done change's salient fact is that it is done, not which workspace it belongs to — and the workspace swatch on the change-name line still ties it home.
- **Multi-instance child rows** are single-line and have no rail; their completion is carried by the disc in the meta slot, unchanged in placement.

### Completed leaf tasks: green text, line-through retained

`.tree-row--struck .row-label` changes `color` from `--text-faint` to `--ok`; `text-decoration: line-through` stays.

- **Why keep the line-through.** It is the colour-independent "done" signal for a row type that has no glyph, so completion never depends on colour alone. Green becomes reinforcement — mirroring how the disc's *check shape*, not its green, carries the semantic at the aggregate rows.
- **Why atoms get text, not the disc.** A near-complete change would otherwise sprinkle many filled discs down its task list, competing with the milestone disc and re-crowding the grammar we just amended once. Keeping atoms as green struck text and reserving the disc for milestones (section / change) produces a clean weight hierarchy — **light-green atoms, heavy-green milestones** — while still honouring "green at every level."

### Accessibility: shape carries meaning, colour reinforces

At every level the *shape* is the colour-independent signal — the check inside the disc for milestones, the line-through for leaf tasks — and green is redundant reinforcement, so the design is safe for colour-vision deficiency. The foreground green is `--ok-strong` (see above), which clears AA for the completed-task text in both schemes (~5.3:1 light, 9.34:1 dark). The disc's knocked-out check uses `--surface` so it punches through to the surface plane, clearing ~5.3:1 against the deeper `--ok-strong` disc in light; the disc is sized (~15px) and carries an interior check so it never reads as one of the 4px `--ok`/`--warn` status dots.

## Open / deferred

- **The moment of completion (motion).** A one-shot animation when work flips to done — the meter racing to 100%, the disc popping in, a restrained forge-spark reusing the Dashboard's existing `confetti-fly` / `tierup-in` keyframes — was explored and deferred. It composes cleanly on top of this static baseline.
- **Workspace-level roll-up.** A per-workspace completion ring, and turning the workspace swatch `--ok` when every change is done, was explored and deferred to its own change; it targets the always-visible top rows this change does not touch.
- **Completion beyond tasks.** Treating a change with proposal + design + specs but no `tasks.md` as "complete" is out of scope; completion stays task-derived.
