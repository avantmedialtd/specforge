## Context

The Settings entrypoint lives at `App.tsx:125-132` as a `<button class="settings-toggle">` positioned `absolute; top: var(--space-1); right: var(--space-2); z-index: 10` inside `.app-shell`. The button overlays the *right* pane while toggling between the right pane's two views (`DetailPane` and `SettingsView`), which creates the visual ambiguity the proposal addresses.

The sidebar (`.split-pane-left` in `App.css:208-219`) is currently a single overflow container — one scrolling region that holds the `WorkspaceTree`. To pin a footer below the tree, the scrolling region needs to move *into* the tree (or onto a wrapping element around it), leaving the sidebar root free to lay out a tree+footer stack without scrolling them as a single unit.

The `SplitPane` component (`src/components/SplitPane.tsx`) accepts `left` and `right` as `ReactNode`. The sidebar's left slot is currently a single `<WorkspaceTree>` element. Adding a footer means either (a) wrapping tree + footer in App.tsx and passing the wrapped node to `left`, or (b) restructuring `SplitPane` to take a discrete footer slot. The composition approach is cleaner — `SplitPane` stays generic.

The application supports a hidden-inset macOS titlebar with a 32px safe-area pad applied to the sidebar (`App.css:217-219`). Removing the floating top-right button leaves the titlebar visually clean; the new sidebar footer is below the safe-area pad and therefore unaffected by it.

## Goals / Non-Goals

**Goals:**
- Replace the floating Settings button with a labeled row at the bottom of the left sidebar that stays visible while the tree scrolls.
- Restructure the sidebar's CSS so that the scroll region belongs to the tree, not the sidebar container, with no `position: sticky` involved.
- Vendor a gear icon inline (consistent with the existing dependency-free pattern in `src/components/icons.tsx`) and use it at 18px in the new row.
- Preserve every existing toggle behaviour: a second click closes Settings, selecting any renderable tree node closes Settings, and `SettingsView` itself is unchanged.

**Non-Goals:**
- Changing the contents or layout *inside* `SettingsView`.
- Moving the "+ Add workspace" action out of `SettingsView`.
- Adding a second action (e.g. a quick "+ Add workspace" shortcut) to the sidebar footer — single Settings affordance only.
- Any Rust, IPC, settings-file, tray, or notification change.
- Touching the `SplitPane` component's API.

## Decisions

### Decision: Pin the footer with flex column, not `position: sticky`

The sidebar root (`.split-pane-left`) becomes `display: flex; flex-direction: column`. The tree wrapper becomes `flex: 1; min-height: 0; overflow: auto`. The footer is a plain block sibling with no flex grow.

**Alternatives considered:**
- **`position: sticky; bottom: 0` on the footer inside the existing overflow container.** Works in simple cases but interacts unpredictably with `overflow: auto` parents — the sticky element can be clipped, lose its sticky context, or fight with scroll padding. The flex restructure has no such ambiguity.
- **Render the footer as an absolutely-positioned overlay above the sidebar.** Re-creates the original problem (toggle floats above the thing it lives in) and would mask the bottom-most tree rows when the sidebar is short enough to not scroll.
- **Keep the sidebar as one overflow container and accept the footer scrolling with the tree.** Defeats the purpose of the change — the user would have to scroll to reach Settings.

The `min-height: 0` on the tree wrapper is necessary because flex children otherwise refuse to shrink below their content size, which would push the footer out of the viewport when the tree is tall.

### Decision: Compose the footer in App.tsx, not inside `WorkspaceTree`

`App.tsx` already owns the `showSettings` state and the toggle handler. The footer is presentational chrome wired to that state. Putting the footer button inside `WorkspaceTree` would force `WorkspaceTree` to take new `onToggleSettings` / `settingsOpen` props that have nothing to do with the tree.

The left slot of `SplitPane` becomes a `<div class="sidebar">` containing two children: the `WorkspaceTree` (now in a flexible scroll wrapper) and the footer button. `SplitPane` stays unchanged.

**Alternatives considered:**
- **Add a discrete `footer` prop to `SplitPane`.** Over-generalises the splitter for a one-off need.
- **Move state into `WorkspaceTree`.** Couples the tree to settings concerns.

### Decision: Swap the `Settings` icon from sliders to a gear, vendored inline

The current `Settings` export in `src/components/icons.tsx:58-69` is a three-row sliders mark, chosen at 14px specifically because a gear felt noisy at that size. The new row renders at 18px paired with a "Settings" text label — both the size and the label make a gear the conventional and immediately-recognisable mark.

The gear is vendored as inline SVG taken from `lucide:settings` (https://icon-sets.iconify.design/lucide/settings/), following the existing `icons.tsx` pattern: a `<Svg>` wrapper with 24×24 viewBox, currentColor, strokeWidth 1.5. No npm dependency is added.

The existing sliders `Settings` export has no other callers (only `App.tsx` imports it, for the button being removed). It SHALL be replaced by the new gear under the same export name, so the import site needs only to drop its width/height props or update them.

**Alternatives considered:**
- **Keep the sliders mark, just bigger.** Safe but misses the chance to land on a more universally-scanned glyph now that the noise concern is gone.
- **Use a different sliders mark (e.g. `lucide:sliders-horizontal`).** Fresh but no clear win over the conventional gear at this size.
- **Add `iconify` as a runtime dependency.** Excessive for a single icon; conflicts with the explicit philosophy comment in `icons.tsx:1-3`.

### Decision: Row dimensions and styling

The row is rendered as a `<button>` element so the affordance is keyboard-accessible by default. Height ~36-40px, full sidebar width, left-aligned icon (18px) and label, padding consistent with existing sidebar row padding tokens.

- **Idle**: transparent background, `--text-muted` colour.
- **Hover**: `--surface-2` background, `--text` colour.
- **Active (Settings open)**: same as hover.

A 1px top border in `--border` separates the footer from the tree above it, matching the existing border vocabulary already used between the sidebar and the right pane.

The exact spacing tokens (which existing `--space-*` variables to use) are an implementation detail to be confirmed against the existing sidebar row rhythm during the work.

## Risks / Trade-offs

- **Risk: Existing CSS selectors targeting `.split-pane-left > *` could break.** → Mitigation: a quick grep of `App.css` for `.split-pane-left ` confirms scope before refactoring (only the padding-top safe-area rule is currently coupled to the container, and it stays on the container).
- **Risk: The macOS safe-area pad (32px `padding-top`) now applies to a flex column instead of a single overflow container.** → Mitigation: flex respects padding the same way; the pad simply offsets the first flex child (the tree wrapper).
- **Risk: A second `Settings` icon name in `icons.tsx` could confuse future readers about which is in use.** → Mitigation: replace the export in place (same name, new glyph) so the import site doesn't need updating beyond the size prop, and remove the sliders SVG body entirely.
- **Trade-off: Sidebar height is now consumed by a footer row even on very short windows.** → Acceptable — the row is small, and a window short enough for this to matter is unusable for the tree anyway.

## Migration Plan

The change is self-contained and ships in one commit. No data migration, no feature flag, no rollback complexity beyond reverting the diff. The settings file format, IPC surface, and Rust crates are untouched.

## Open Questions

None blocking. The sidebar row's exact `--space-*` tokens and border colour token can be picked during implementation to match the existing sidebar rhythm.
