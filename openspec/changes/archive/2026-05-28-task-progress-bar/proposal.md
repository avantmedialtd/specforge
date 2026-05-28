# Task Counter Becomes a Progress Bar

## Why

The sidebar surfaces each change's task progress as a small monospace `completed/total` count — the outlined `.row-progress` pill on the instance row, and the `(n/n)` suffix baked into the Tasks artifact label. The number is precise, but it reads *slowly*: to gauge "how far along is this change" you have to parse two integers and divide them in your head. A fill bar communicates the same ratio pre-attentively — a single glance separates "barely started" from "nearly done" without reading any digits, and lets the eye compare progress down a column of changes at once.

This deliberately trades **magnitude** for **scannability**. A `2/4` change and a `20/40` change render as identical half-full bars; the absolute task count is no longer visible inline. That count is preserved exactly — on hover via a `title` tooltip, and for assistive technology via `role="progressbar"` attributes — so nothing is lost, only relocated out of the at-a-glance layer.

## What Changes

- The instance row's `completed/total` count (`.row-progress` in `InstanceNode`) is replaced by a fixed-width **progress bar**: an outlined track with a `--ok`-green fill whose width is `completedTasks / totalTasks`. No number is rendered inline. The exact count moves to the element's `title` ("N of M tasks") and to `role="progressbar"` / `aria-valuenow` / `aria-valuemax`.
- The Tasks artifact node label drops its `(n/n)` suffix — the label becomes plain `Tasks` — and the same bar renders in a new trailing **meta** slot on that row, mirroring the instance row's meta cluster.
- Both bars are one shared component at a single fixed width (~56px), so they read as the same object at different tree depths. The fill width animates when the watcher reports a task toggle; the transition is suppressed under `prefers-reduced-motion`.
- **At 100% the bar is not rendered.** The instance row already shows a trailing `✓` at completion, and a full bar with no number adds nothing the check doesn't say. Hiding the bar keeps "done" categorically distinct from "almost done."
- The Tasks artifact node — which has no completion glyph today because its `(n/n)` label carried that signal — gains the same trailing `✓` at 100%, so once its label loses the count it still reads as complete.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `spec-browser`: *Instance Row Chrome* now specifies a task-progress **bar** rather than a textual count, with the count exposed via tooltip + aria. *Change-Row Completion Glyph* drops the "alongside the progress count" wording — at 100% there is no count and no bar, so the trailing `✓` stands alone. *Reactive Updates from Filesystem* updates the checkbox-toggle scenario to assert the **bar fill** re-renders rather than a textual `(completed/total)` label. *Auto-Collapse of Completed Task Groups* and *Completed Section Row Shows a Completion Glyph* update their references to the Tasks node's `(n/n)` label (which no longer exists); the Tasks node gains a `✓` at 100% in its place.
- `visual-identity`: a new *Task Progress Meter* requirement codifies the bar — outlined track, `--ok` green fill, hidden at 100%, tooltip + aria contract — as a sanctioned exception to the *Outlined Chip Badges* "outlined, never filled" rule (the fill lives inside an outlined track, so the rule is bent deliberately and documented rather than violated ad hoc). *Typography System* drops "progress counters" from its list of mono *text* elements, since progress is no longer a text element in the tree row.

## Impact

- `src/components/WorkspaceTree.tsx` — `InstanceNode` (:535) swaps the `.row-progress` pill for the new bar (rendered only while `!allTasksDone`); `ArtifactNode` (:848) drops `(n/n)` from the Tasks label, renders the bar in a new `meta` slot, and renders a `✓` at 100%.
- A new progress-bar component (co-located in `WorkspaceTree.tsx` or `src/components/`) — outlined track + green fill + `title` + `role="progressbar"`, fixed width.
- `src/App.css` — the `.row-progress` rule is repurposed/replaced as the track + fill; `prefers-reduced-motion` guard on the width transition.
- `openspec/specs/spec-browser/spec.md` and `openspec/specs/visual-identity/spec.md` — deltas applied at archive time via `openspec archive`.
- No Rust changes, no IPC changes, no `src/types.ts` changes — `completedTasks` / `totalTasks` already cross the boundary.
- **Out of scope:** `FlatChangeNode` (the non-git flat-workspace change row) surfaces no task count today — there is no counter there to convert. Giving it the same bar for consistency is noted as an optional follow-up in `design.md`.
