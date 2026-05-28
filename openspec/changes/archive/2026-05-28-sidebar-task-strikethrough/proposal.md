# Strike through completed tasks in the sidebar

## Why

A completed leaf task in the sidebar tree is signalled only by a leading glyph swap — a green checked box (`CheckSquare`) versus a faint empty box (`Square`). The two differ only by a checkmark drawn *inside* a 14px box, so done-vs-pending is hard to scan and leans on green colour to carry the meaning. Striking through the task text reads completion at the word level and is a non-colour cue (better for colour-blind users), matching the familiar todo-list idiom.

## What Changes

- Completed leaf-task rows render their label with strikethrough **and** dimmed text (`var(--text-faint)`) instead of a leading checked-box glyph.
- The per-task leading glyph is removed entirely — both the completed (`CheckSquare`) and pending (`Square`) variants. Pending tasks render as normal-weight, full-contrast text with no glyph. This makes task leaves glyph-free, consistent with the existing capability-spec leaf rows.
- Scope is **leaf tasks only**. The aggregate "all done" `✓` glyphs at the Section, flat-Change, and per-Instance rows are unchanged, and the `(completed/total)` task-progress counts are unchanged.
- Remove the now-dead pieces: the `CheckSquare` / `Square` imports in `WorkspaceTree.tsx` (which would otherwise fail `noUnusedLocals`) and the `.row-icon` / `.icon-unchecked` / `.icon-checked` CSS rules, which only the task glyph consumed.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `spec-browser`: adds a requirement governing how a completed leaf-task row renders (strikethrough + dimmed text, no leading glyph) and that pending task rows render plain. The structural "Workspace Tree Hierarchy" requirement is unchanged; the Section completion-glyph behaviour is explicitly retained.

## Impact

- `src/components/WorkspaceTree.tsx` — `TaskNode` stops passing an `icon`; the shared `Row` primitive gains a way to mark a row as struck (completed). Remove the unused glyph imports.
- `src/App.css` — add the strikethrough + dim treatment for completed task labels; remove the dead `.row-icon` family of rules.
- `src/components/icons.tsx` — `CheckSquare` / `Square` exports become unused (optional removal; `Check` is still used by aggregate rows).
- `openspec/specs/spec-browser/spec.md` — gains the new leaf-task rendering requirement.
- No Rust, IPC, or type-boundary changes. Purely frontend presentation.
