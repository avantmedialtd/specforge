## 1. Swap the Settings icon

- [x] 1.1 In `src/components/icons.tsx`, replace the body of the `Settings` export with the inline SVG path from `lucide:settings` (https://icon-sets.iconify.design/lucide/settings/). Keep the same `<Svg>` wrapper (24×24 viewBox, currentColor, strokeWidth 1.5) and the same export name so import sites remain unchanged. Remove the old sliders SVG body and update the doc comment on the export to describe the new glyph.

## 2. Restructure the sidebar layout

- [x] 2.1 In `src/App.css`, change `.split-pane-left` from `overflow: auto` to `display: flex; flex-direction: column`. Keep the existing `flex-shrink: 0`, `border-right`, `background`, and the macOS `padding-top` rule unchanged.
- [x] 2.2 Add a new `.sidebar-tree` rule (or equivalent) that wraps the `WorkspaceTree`: `flex: 1; min-height: 0; overflow: auto`. The `min-height: 0` is required so the wrapper can shrink below its content height and leave room for the footer.
- [x] 2.3 In `src/App.tsx`, wrap the `WorkspaceTree` in the new `.sidebar-tree` `<div>` when passing it as the `left` slot of `SplitPane`.

## 3. Add the sidebar-footer Settings row

- [x] 3.1 In `src/App.css`, add a `.sidebar-footer-button` rule for the new row: full sidebar width, ~36-40px tall, transparent background, `--text-muted` colour, icon + label flexbox with left-aligned content, padding consistent with existing sidebar rows. Add hover and `.active` states using `--surface-2` background and `--text` colour (mirroring the current `.settings-toggle:hover` / `.active` rules being removed). Add a 1px `--border` top border separating the footer from the tree above.
- [x] 3.2 In `src/App.tsx`, render a `<button class="sidebar-footer-button">` as a sibling of the `.sidebar-tree` wrapper inside the `SplitPane` left slot. Wire `onClick` to `setShowSettings((s) => !s)`, set `aria-label="Toggle settings"` and `title="Settings"`, and apply the `active` class when `showSettings` is true. Render `<SettingsIcon width={18} height={18} />` followed by a `<span>Settings</span>` label.

## 4. Remove the obsolete floating button

- [x] 4.1 In `src/App.tsx`, delete the `<button class="settings-toggle">…</button>` element (currently at `App.tsx:125-132`) that sits as a sibling of `SplitPane`.
- [x] 4.2 In `src/App.css`, remove the `.settings-toggle` and `.settings-toggle:hover, .settings-toggle.active` rules (currently `App.css:174-195`).

## 5. Verify in the running app

- [x] 5.1 Run `bun run build` and resolve any TypeScript errors.
- [x] 5.2 Start `bun tauri dev`, observe the sidebar footer renders with the gear glyph and "Settings" label, and verify: (a) no floating top-right button is present; (b) clicking the footer row opens `SettingsView` and applies the active treatment to the row; (c) a second click closes `SettingsView` and returns the row to idle; (d) selecting a renderable tree node while Settings is open closes Settings; (e) scrolling a tall workspace tree keeps the footer pinned at the bottom of the sidebar.
- [x] 5.3 Verify the macOS hidden-inset titlebar still renders cleanly with the traffic-light safe-area pad preserved (no regression from the `.split-pane-left` change).
