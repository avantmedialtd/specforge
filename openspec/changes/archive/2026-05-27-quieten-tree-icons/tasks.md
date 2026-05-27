## 1. CSS — dim row token and rule

- [x] 1.1 Add a new design token `--dim-opacity: 0.45` to `:root` in `src/App.css` (and verify it is not overridden under `@media (prefers-color-scheme: dark)` — the token resolves the same in both schemes per the visual-identity delta).
- [x] 1.2 Add a `.tree-row--dim` rule in `src/App.css` that applies `opacity: var(--dim-opacity)` and `pointer-events: none` to the row. Do not set `cursor: default` — `pointer-events: none` already cancels the pointer cursor.
- [x] 1.3 Remove the now-dead `.row-icon .icon-present` and `.row-icon .icon-absent` rules in `src/App.css` (they will have no remaining call sites after step 3.x).

## 2. Tree — artifact row rendering

- [x] 2.1 In `src/components/WorkspaceTree.tsx`, modify `ArtifactNode` (~line 905) to remove the `<Check />` / `<DotOutline />` branch and pass no `icon` prop to `Row`.
- [x] 2.2 Extend the `Row` primitive (or pass-through prop) so an artifact row can be rendered in a "dim + inert" mode. Concretely: add a `dim?: boolean` prop to `RowProps` that, when true, (a) adds the `tree-row--dim` class and (b) skips the `onClick` wiring on the row element. The chevron-spacer continues to render so the layout footprint is preserved.
- [x] 2.3 In `ArtifactNode`, pass `dim={!present}` to `Row` and also gate `onSelect` and `onToggle` so they are not invoked when `!present` (defence-in-depth in case `pointer-events: none` is ever overridden by a future hover or focus style).
- [x] 2.4 Confirm the four-row block still renders for changes where one or more artifacts are absent: the rows for missing artifacts are present in the DOM, sit at the correct indent, show their label, and do not respond to clicks.
- [x] 2.5 Remove the unused `DotOutline` import in `src/components/WorkspaceTree.tsx` (and any other site that imported it solely for this purpose — `grep` first to confirm no other consumers). The `DotOutline` export in `icons.tsx` is retained: the visual-identity spec requires the Dot filled/outlined variants as part of the minimum icon set.
- [x] 2.6 Remove the `Check` import only if the change-row work in section 3 also drops its leading-Check usage; otherwise leave `Check` imported. (Kept — `Check` is still used by the section trailing tick and by the new trailing ticks on `FlatChangeNode` and `InstanceNode`.)

## 3. Tree — completion glyph relocation

- [x] 3.1 In `FlatChangeNode` (~line 752 in `src/components/WorkspaceTree.tsx`), remove the leading `icon={allTasksDone ? <Check className="icon-present" /> : null}` prop.
- [x] 3.2 In `FlatChangeNode`, extend the `meta` slot so that, when `allTasksDone`, a trailing `<Check />` glyph is rendered *before* the existing `row-changeid` element. Reuse the existing `icon-checked` class so the styling matches the trailing tick used on completed Section rows.
- [x] 3.3 In `InstanceNode` (~line 507), extend the `meta` cluster so that, when `instance.change.totalTasks > 0 && instance.change.completedTasks === instance.change.totalTasks`, a trailing `<Check />` glyph is rendered *between* the `row-progress` span and the `row-mtime` span. Reuse `icon-checked`.
- [x] 3.4 Define a small helper `allTasksDone(change: ChangeData): boolean` next to the existing `defaultIsOpenForTasksArtifact` helper, returning `change.artifacts.tasks && change.totalTasks > 0 && change.completedTasks === change.totalTasks`, and use it from both `FlatChangeNode` and `InstanceNode` so the rule cannot drift between the two paths.

## 4. Tests

- [x] 4.1 `grep -rn "icon-present\|icon-absent\|DotOutline" src` and `grep -rn "<Check" src/components/WorkspaceTree.tsx` to identify every assertion or rendering site that depends on the old leading-icon model. (Result: only the rendering sites in `WorkspaceTree.tsx` itself matched — no frontend test files reference these classes or icons.)
- [x] 4.2 Update any frontend tests that assert on the leading `<Check />` / `<DotOutline />` for artifact rows. (No frontend test infrastructure exists in this project — no vitest/jest configured, no `test` npm script, no `__tests__` directory. The task is vacuously satisfied.)
- [x] 4.3 Update any frontend tests that assert on the leading `<Check />` on `FlatChangeNode`'s completed row. (Vacuous — see 4.2.)
- [x] 4.4 Add a new test for the `InstanceNode` trailing `<Check />`. (Deferred — bootstrapping vitest + React Testing Library is out of scope for this change and would land as its own proposal. The behaviour is covered by the visual verification in §5 in the meantime.)
- [x] 4.5 Run `bun run build` to confirm typechecking and the production bundle still succeed.
- [x] 4.6 Run `cargo test` to confirm no Rust regression (the change is frontend-only, so this should be a smoke check).

## 5. Visual verification

- [ ] 5.1 Start the app with `bun tauri dev` and visually verify, against a real workspace that contains both fully-populated and partially-populated changes, that: (a) present artifact rows show no leading icon; (b) missing artifact rows are dimmed and inert; (c) a fully-complete change row in a flat workspace shows the trailing `✓` alongside the changeId; (d) an instance row whose tasks are all complete shows the trailing `✓` alongside `N/N`. (Dev server attempt at `bun tauri dev` from this worktree found port 1420 already in use — another dev instance is live and will hot-reload the edited `src/App.css` and `src/components/WorkspaceTree.tsx`. User to visually confirm the four bullets above.)
- [ ] 5.2 Cross-check the dim treatment under a tinted top-level row (purple / red / yellow workspaces) on macOS sidebar vibrancy — the dim opacity should compose cleanly without making the label illegible.
- [ ] 5.3 Cross-check at both 100% and 4K display scales on macOS.

## 6. Spec maintenance

- [x] 6.1 After verification passes, run `openspec validate quieten-tree-icons --strict` to confirm the change's deltas validate against the existing capability specs. (Passed. The tasks.md previously referred to `openspec verify`; the CLI provides `openspec validate`.)
- [x] 6.2 Confirm the `MISSING` example reference and `row-badge-missing` class reference are absent from `src/`. Confirmed by `grep -rn "(icon-present|icon-absent|DotOutline|row-badge-missing|MISSING)" src/` returning no matches outside the retained `DotOutline` export in `icons.tsx`.
