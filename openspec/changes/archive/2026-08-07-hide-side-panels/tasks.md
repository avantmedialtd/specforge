# Hide Side Panels — Tasks

## 1. Menu and Event Plumbing (Rust shell)

- [x] 1.1 Add pane-toggle event name constants (`toggle-sidebar`, `toggle-commit-rail`) in `crates/openspec-app/src/events.rs`, re-export them from `crates/specforge/src/events.rs` alongside the existing cache-event names, and mirror both constants in `src/types.ts` (`application-menu`: *View Submenu Pane Toggles*).
- [x] 1.2 Add a View submenu to the macOS menu in `crates/specforge/src/menu.rs` with "Toggle Sidebar" (Cmd+B accelerator) and "Toggle Commit Rail" (Cmd+Alt+B accelerator); the menu-event handler shows the main window if it is hidden, then emits the corresponding toggle event to the webview (`application-menu`: *View Submenu Pane Toggles*).

## 2. SplitPane Visibility (frontend)

- [x] 2.1 Add controlled `leftHidden` / `farHidden` props to `src/components/SplitPane.tsx`: a hidden pane and its divider unmount entirely while the pane's width state stays in `SplitPane` so restore returns the remembered width through the existing clamps (design D2; `spec-browser`: *Side-Pane Visibility Toggles* — independent toggles, width recovery).
- [x] 2.2 Add `onToggleLeft` / `onToggleFar` callbacks and render the chevron affordances in `SplitPane.tsx`: a collapse chevron at the top of each visible side pane, and while a pane is hidden a floating restore chevron overlaying the center pane's matching top corner — z-ordered above content, excluded from the macOS titlebar drag region, offset clear of the traffic lights (design D5; `spec-browser`: *Side-Pane Visibility Toggles* — restore affordances).

## 3. App State, Input Routing, and CSS (frontend)

- [x] 3.1 Add sidebar/rail visibility state in `src/App.tsx`, initialised from and persisted to `localStorage` keys `specforge.sidebarHidden` / `specforge.railHidden` following the `RAIL_WIDTH_KEY` pattern, wired to the new `SplitPane` props (design D1; `spec-browser`: *Side-Pane Visibility Toggles* — persistence scenario).
- [x] 3.2 Register the input paths per surface in `src/App.tsx` (design D4): a `keydown` handler for Cmd/Ctrl+B and Cmd/Ctrl+Alt+B active only when NOT (Tauri ∧ macOS), and a Tauri event listener for the two toggle events from task 1.1 active only under Tauri — exactly one handler per surface (`application-menu`: *View Submenu Pane Toggles* — single-fire scenario; `spec-browser`: keyboard toggles).
- [x] 3.3 Gate rail data work on visibility in `src/App.tsx`: pass `null` to `useCommitGraph` while the rail is hidden and keep `applyGraphRepoId` tracking selection so restore fetches the current repository's graph (design D3; `commit-graph`: *Commit-Graph Rail Pane* — hidden-rail no-fetch and restore scenarios).
- [x] 3.4 Style the chevrons and hidden-pane layout in `src/App.css`, including a sidebar-hidden modifier on the split-pane root that gives the center pane the `body[data-platform="mac"]` traffic-light top clearance the sidebar normally provides (design D6; `spec-browser`: *Side-Pane Visibility Toggles* — macOS window-controls scenario).

## 4. Verification

- [x] 4.1 Run `cargo test` (workspace) — green, including the specforge crate's menu changes compiling on all platforms (View submenu is `#[cfg(target_os = "macos")]`-scoped like the rest of the custom menu).
- [x] 4.2 Run `bun run build` — strict typecheck and bundle pass with the new props, constants, and handlers.
- [x] 4.3 Manual smoke via `bun run wt:dev` walking the spec scenarios: toggle each pane by chevron, shortcut, and View menu (independence, exactly one toggle per Cmd+B press); hide both panes and confirm full-width content with both restore chevrons visible and clear of the traffic lights; restore panes and confirm remembered widths; relaunch and confirm visibility persisted; with the rail hidden, move the tree selection across repositories and confirm no commit-graph fetches occur (dev-mode `invokeLogged` output), then restore the rail and confirm it fetches the currently selected repository's graph.
- [x] 4.4 Load the served web UI (`specforge-web` debug build serving `dist/`) and confirm the keyboard toggles and chevrons work in a browser tab, where no native menu exists.
