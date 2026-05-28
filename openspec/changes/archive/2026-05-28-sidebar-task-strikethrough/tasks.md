## 1. Row primitive + TaskNode (`src/components/WorkspaceTree.tsx`)

- [x] 1.1 Add an optional `struck?: boolean` prop to `RowProps` and have `Row` append a `tree-row--struck` modifier class to its root `<div>` when `struck` is true (compose it with the existing `selected` / `top-level` / `dim` class logic)
- [x] 1.2 In `TaskNode`, stop passing `icon=`; pass `struck={task.completed}` to `Row` instead
- [x] 1.3 Remove the now-dead `icon` prop from `RowProps` and the `{icon && <span className="row-icon">{icon}</span>}` branch in `Row` (TaskNode was the only `icon=` consumer)
- [x] 1.4 Remove the now-unused `CheckSquare` and `Square` imports from the `./icons` import block (required — `noUnusedLocals` would otherwise fail the build); leave `Check` imported (still used by aggregate rows) and leave the `CheckSquare` / `Square` definitions in `icons.tsx` intact

## 2. Styling (`src/App.css`)

- [x] 2.1 Add a rule for completed task labels: `.tree-row--struck .row-label { text-decoration: line-through; color: var(--text-faint); }`
- [x] 2.2 Remove the now-dead `.row-icon`, `.row-icon .icon-unchecked`, and `.row-icon .icon-checked` rules (only the removed task glyph used them)

## 3. Verify

- [x] 3.1 Run `bun run build` — `tsc --noEmit` must pass (confirms no unused-import/param regressions from the glyph removal) and the bundle builds
- [x] 3.2 Verified against a faithful fixture that links the real `App.css` and reproduces `Row`'s exact markup (rendered light + dark): completed tasks render struck-through + dimmed with no leading glyph, pending tasks render plain with no glyph, and the aggregate `✓` on the fully-done Section / flat-Change rows plus the `(completed/total)` count are unchanged. (Verified via fixture rather than the native Tauri WebView, which the available screenshot tooling can't capture; `bun run build` already confirmed the component/TS wiring.)
- [x] 3.3 Confirmed a completed + selected task row stays legible. No fallback rule needed: the real selection treatment is an accent `border-left` only (`.tree-row.selected`), not a background fill, so the faint struck label never sits on a highlight fill.
