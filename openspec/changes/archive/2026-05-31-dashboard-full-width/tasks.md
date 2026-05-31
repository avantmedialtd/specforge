# Tasks

## 1. Remove the Dashboard width cap

- [x] 1.1 In `src/App.css`, delete the `max-width: 920px` and `margin: 0 auto` declarations from the `.dashboard` rule, keeping its `padding`.

## 2. Verify

- [x] 2.1 Run `bun run build` (tsc + bundle) to confirm no type or build regressions.
- [x] 2.2 Launch `bun tauri dev`, open the Dashboard, and confirm it fills the center pane edge-to-edge (between the sidebar and the commit-graph rail) on a wide window with no dead gutters and no horizontal scrollbar.
- [x] 2.3 Resize the window narrow and confirm the existing `@media (max-width: 720px)` single-column collapse still applies and nothing overflows.
