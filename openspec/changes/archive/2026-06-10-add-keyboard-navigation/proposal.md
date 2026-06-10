# Make the spec browser keyboard-operable

## Why

The workspace tree — the app's primary navigation surface — is built from `<div onClick>` rows with no `tabIndex`, role, or key handling, so SpecForge cannot be navigated without a mouse. The design system already ships the focus treatment (`.tree-row:focus-visible`, App.css:517) and the visual-identity spec contains a keyboard-focus scenario (spec.md:153-157) that can never fire today: the visual recipe, selection plumbing, and expansion persistence all exist — only the keyboard wiring is missing. Smaller gaps repeat across the shell: split-pane dividers are mouse-drag-only, Settings/Archive cannot be dismissed with Escape, and several focusable controls lack visible focus styles.

## What Changes

- Workspace tree becomes a WAI-ARIA tree with roving tabindex: one Tab stop; Arrow keys move a current row through visible rows; Right/Left expand/collapse and jump to parent; Home/End jump to the extremes; Enter/Space activates (grouping rows toggle expansion; artifact/section/task rows open in the center pane); first-letter typeahead jumps to the next matching visible row.
- Debounced follow-focus: resting keyboard focus on an artifact/section/task row for ~150ms opens it in the center pane, so holding an arrow key does not fire a read per keystroke; grouping rows never drive the center pane (unchanged from click behavior).
- Tree rows expose ARIA semantics: `role="treeitem"`, `aria-expanded`, `aria-selected`, `aria-level`; children wrappers get `role="group"`. Dim missing-artifact rows stay focusable but inert (`aria-disabled`).
- Keyboard focus survives tree refreshes: when a cache event removes the focused row, focus falls back to the nearest surviving ancestor derived from the hierarchical node ID.
- Split-pane dividers become focusable separators resizable with Arrow keys, exposing `aria-valuenow`/`aria-valuemin`/`aria-valuemax`.
- Escape closes the Settings and Archive panes.
- Focus-visible sweep: visible focus styles for the focusable controls that lack them (archive rows/tabs/back/select, secondary buttons, graph-rail load-more, finishes rows, season-recap close), and `.archive-search` moves from `:focus` to `:focus-visible`.
- Palette swatches in Settings get ≥24px hit areas and `role="radio"` + `aria-checked` semantics under their existing `radiogroup` container (visuals unchanged).
- Tree row components are memoized so per-keystroke focus movement does not re-render the whole tree.

Out of scope: app-level accelerators (Cmd-1 etc.) and a Cmd-K quick-open palette — the latter is a planned follow-up change that builds on this one.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `spec-browser`: two ADDED requirements — keyboard navigation of the workspace tree (roving tabindex, key map, debounced follow-focus, typeahead, ARIA semantics, focus resilience across refreshes), and keyboard operability of the surrounding shell (divider resize, Escape dismissal of Settings/Archive). No existing requirements change; the visual treatment defers to the visual-identity spec's existing keyboard-focus scenario.

## Impact

- `src/components/WorkspaceTree.tsx` — the bulk: roving tabindex, key handling, ARIA attributes, typeahead, focus fallback, `React.memo` on row/node components.
- `src/components/SplitPane.tsx` — focusable dividers with Arrow-key resize and ARIA value attributes.
- `src/App.tsx` — Escape handling for Settings/Archive; no selection-contract changes (`handleSelect` is reused as-is).
- `src/components/SettingsView.tsx` — palette swatch semantics.
- `src/App.css` — focus-visible additions; swatch hit-area; the dead `.tree-row:focus-visible` rule becomes live.
- No Rust changes; no IPC changes; read-only contract untouched.
