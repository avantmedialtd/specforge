## Context

Leaf-task rows in `WorkspaceTree.tsx` (`TaskNode`) signal completion only via a leading glyph swap: `CheckSquare` (green, `var(--ok)`, via the `.row-icon .icon-checked` rule) when done, `Square` (faint) when pending. The label text itself is identical in both states. `TaskNode` is the *only* caller that passes an `icon` to the shared `Row` primitive — capability-spec leaf rows already render glyph-free. The aggregate "all done" `✓` glyph (`Check`) appears separately in the `row-meta` slot of Section, flat-Change, and Instance rows and is out of scope here.

This is a purely presentational frontend change: no Rust, IPC, type-boundary, or persisted-state impact. A design doc is warranted only because there are a few small-but-real API/cleanup decisions to pin before coding.

## Goals / Non-Goals

**Goals:**
- Completed leaf tasks render with `line-through` + dimmed (`var(--text-faint)`) label text.
- Leaf-task rows carry no leading completion glyph in either state.
- Completion becomes a non-colour cue (text decoration), readable at the word level.
- Remove the code/CSS that the task glyph leaves dead.

**Non-Goals:**
- Changing the aggregate `✓` glyphs on Section / flat-Change / Instance rows.
- Changing the `(completed/total)` task-progress labels.
- Making task checkboxes interactive (the Read-Only Viewer contract is unchanged).
- Any backend, IPC, watcher, or settings change.

## Decisions

**1. Mark completion with a generic `struck` prop on `Row`, not a task-specific one.**
`Row` gains an optional `struck?: boolean` that toggles a `tree-row--struck` modifier class; CSS applies `line-through` + dim to that row's `.row-label`. `TaskNode` passes `struck={task.completed}`.
- *Why over alternatives:* keeping the decoration in CSS on the row (rather than passing a pre-styled label node from `TaskNode`) keeps `Row` the single styling authority and lets the selection/hover states compose via CSS specificity. A presentation-level name (`struck`) over a domain name (`completed`) leaves the door open if Sections later adopt the same treatment — the node decides *when* to strike, `Row` only knows *how*.

**2. Remove the `icon` column from task leaves entirely.**
`TaskNode` stops passing `icon`. Task leaves then render `chevron-spacer → label`, identical to capability-spec leaves — this *removes* the one exception, it does not introduce a new layout. Because `TaskNode` was the sole `icon=` consumer, the `icon` prop on `Row` and its `{icon && <span className="row-icon">…}` branch become dead and are removed along with the `.row-icon`, `.row-icon .icon-unchecked`, and `.row-icon .icon-checked` CSS rules.

**3. Pair strikethrough with dimming.**
Done labels use both `text-decoration: line-through` and `color: var(--text-faint)`. Strikethrough alone on full-contrast text reads like an edit/redaction mark; dimming communicates "done, receded." This matches the approved preview.

**4. Scope guard — leaf tasks only.**
The aggregate `Check` glyphs and progress counts are untouched. This is a deliberate idiom split (Sections show `✓` when fully done; tasks show strikethrough). Acceptable because the Section `✓` is an *aggregate* completion badge, not a per-item checkbox, so the two cues don't directly contradict on the same element.

**5. Keep the `CheckSquare` / `Square` icon definitions in `icons.tsx`.**
The imports in `WorkspaceTree.tsx` MUST be removed (else `noUnusedLocals` fails the build), but the exported icon components are left in place. `self_write.rs` exists precisely to support future interactive editing (checkbox toggling); a clickable checkbox would want these primitives back. They are cheap, dependency-free, and unused exports are not flagged, so retaining them avoids a needless delete-then-readd churn.

## Risks / Trade-offs

- **Selected + completed legibility** → A done task that is also the selected row gets the selection-highlight background *and* faint struck text, which may be hard to read. Mitigation: verify in `bun tauri dev`; if needed, add `.tree-row.selected.tree-row--struck .row-label` to keep the strikethrough but lift the colour off `--text-faint` on selection.
- **Loss of the pending affordance** → Removing the empty `Square` drops the only "this is a checkbox-style task" hint; pending tasks look like plain rows. Mitigation: depth indentation under the Section already frames them as tasks, consistent with spec leaves. Accepted.
- **Truncation interaction** → `.row-label` uses ellipsis overflow; `line-through` must still render cleanly on truncated text. Low risk (browsers handle this), but part of visual verification.

## Migration Plan

No data or state migration. Ship as a frontend-only change; rollback is a straight revert. Persisted tree expand/collapse state is unaffected (node IDs and default-open logic are unchanged).

## Open Questions

- Whether the selected + completed state needs the contrast tweak from Risks, or reads acceptably as-is — to be resolved during visual verification, not before coding.
