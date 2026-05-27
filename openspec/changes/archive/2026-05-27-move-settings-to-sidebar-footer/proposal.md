# Move Settings entrypoint to sidebar footer

## Why

The Settings entrypoint is a 16px sliders icon in a 30×30 transparent button absolutely positioned in the top-right corner of the window, floating above the right pane via `z-index: 10` (`App.tsx:125-132`, `App.css:174-195`). Two problems compound:

- **The toggle sits inside the thing it toggles.** Opening Settings swaps the right pane to `SettingsView`, but the button doesn't move — visually it ends up *inside* SettingsView, reading as part of the settings UI rather than an external switch.
- **The trigger is far from what it controls.** Settings is primarily workspace management (add/remove/rename/tint live in `SettingsView.tsx:117-145`), and workspaces are presented in the *left* sidebar. Trigger and effect are on opposite sides of the window.

Desktop convention (Slack, Linear, Figma, Discord, Notion) places the Settings entrypoint at the bottom of the left sidebar, alongside the things it configures. The current placement also reads as visually undersized because a 16px stroke-1.5 muted icon floating alone in 32px of empty titlebar has no neighbours to give it scale.

## What Changes

- Remove the floating top-right Settings button (`.settings-toggle` in `App.css`, the `<button>` in `App.tsx:125-132`).
- Add a labeled `Settings` row at the bottom of the left sidebar — icon + text, hover and active states styled to match the existing affordance vocabulary.
- Restructure `.split-pane-left` from a single scrolling container to `display: flex; flex-direction: column`, with the `WorkspaceTree` as the flexing scrollable child and the new footer as a non-scrolling sibling. This is what guarantees the footer stays pinned at the bottom of the sidebar regardless of scroll position.
- Swap the sliders glyph for a gear (vendored inline from `lucide:settings`) at 18px in the new row. The original "gear noise at 14px" concern in `icons.tsx:56-57` no longer applies at this size paired with a label.
- No behavioural change to `SettingsView` itself — only its entry mechanism moves.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `spec-browser`: adds a requirement specifying that the Settings entrypoint is rendered as a row pinned to the bottom of the sidebar pane (rather than as a floating button overlaying the master-detail surface).

## Impact

- **Frontend only.** Touches `src/App.tsx`, `src/App.css`, `src/components/icons.tsx`. No changes to the Rust workspace, IPC commands, settings file format, tray, or notifications.
- **No new dependencies.** The gear icon is vendored inline as SVG, matching the existing dependency-free icon pattern in `icons.tsx`.
- **No data migration.** The change is purely visual chrome; persisted settings, registry, and cache are untouched.
