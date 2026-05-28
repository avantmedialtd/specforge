# Design — Task Counter Becomes a Progress Bar

## Decisions

### Bar-only, no inline number

The bar renders **no digits**. The exact `completed/total` count survives in the `title` tooltip and in `role="progressbar"` aria attributes, not on screen.

- **Why:** the whole point is a pre-attentive ratio read. A number beside the bar pulls the eye back into reading mode and defeats the glance.
- **Accepted cost:** magnitude is gone from the inline layer — `2/4` and `20/40` look identical. We judged ratio-at-a-glance more valuable in a sidebar than absolute count, and the tooltip recovers the count on demand. This was an explicit choice over a "meter pill" (fill behind the number) and a "track + number" layout.

### The outlined track keeps the house rule intact

`visual-identity` states the rule out loud: *chips and status dots are outlined, never filled.* A progress bar is, definitionally, a fill — so it bends that rule. We bend it **deliberately and narrowly**: the track is an outlined hollow rectangle (`border`, transparent background, `--radius-sm`), and only the *progress portion* fills. The container stays outlined; the fill is its contents. This is codified as a named exception (*Task Progress Meter*) rather than a silent violation, so future list surfaces know the meter is the one sanctioned filled element.

### `--ok` green fill throughout

The fill is `--ok` green at every completion level, over a `--border` track.

- **Why:** progress reads as positive/healthy, and the green ties visually to the `✓` that takes over at 100%.
- **Trade-off considered:** a muted `--text-muted` fill (reserving green strictly for done) would sit more quietly in the deliberately-calm sidebar, but we chose the livelier green for legibility against the track.

### Hidden at 100% — the check stands alone

When `totalTasks > 0 && completedTasks === totalTasks`, the bar is **not rendered**. The trailing `✓` (already present on the instance row, newly added to the Tasks artifact row) is the sole completion signal.

- **Why:** with no number, a full green bar and a check both just mean "done" — redundant. Dropping the bar makes "done" look categorically different from "99% done," which is more useful than a full bar that's easy to misread as merely wide.
- This reuses the existing `allTasksDone(change)` predicate; the render condition becomes "bar iff `totalTasks > 0 && !allTasksDone`, check iff `allTasksDone`."

### One shared component, two call sites, fixed width

A single component (`TaskProgress`, co-located in `WorkspaceTree.tsx` or under `src/components/`) renders the track, fill, `title`, and aria. It is used in `InstanceNode`'s meta cluster and in the Tasks `ArtifactNode`'s new meta slot.

- **Fixed width (~56px)** so the two bars align as the same object across tree depths, and so the fill fraction — not the bar's own length — is what varies between rows.
- The Tasks artifact row gains a `meta` slot it does not have today; the `(n/n)` count moves *out of the label string* into that slot as a bar. This is a small cleanup: the label becomes plain `Tasks`, separating the artifact's name from its metric (the same split the instance row already uses).

### Animated fill, motion-safe

The fill width uses a short CSS `transition` so completing a task visibly nudges the bar when the watcher emits `cache-updated` and the frontend refetches. The transition is wrapped in `@media (prefers-reduced-motion: reduce)` and removed there.

### Empty / zero-task changes

When `totalTasks === 0` (a `tasks.md` exists but parses no task lines), **no bar renders** — matching today's `&& totalTasks > 0` guard on the pill. An empty track for a change with genuinely zero tasks would falsely read as "0% done."

## Open / deferred

### `FlatChangeNode` consistency (optional follow-up)

`FlatChangeNode` — the non-git flat-workspace change row — surfaces no task count today; it shows the change-id chip and a `✓` at completion. Because it never had a counter, it is out of the literal scope of "the counter should be a bar." It uses the same row grammar, so dropping the shared `TaskProgress` component into its meta cluster would be trivial and would make the two change-row types consistent. Deferred so this change converts existing counters only; revisit if uniform progress across both row types is wanted.

## What does not change

- `src/types.ts`, the Rust core, and the IPC boundary are untouched — `completedTasks` / `totalTasks` already flow to the frontend.
- The auto-collapse behaviour of the Tasks artifact node (collapsed-by-default when complete) is unchanged; only the *label* and the new `✓`/bar in its meta slot change.
- The instance row's existing trailing `✓` and its position in the meta cluster are unchanged.
