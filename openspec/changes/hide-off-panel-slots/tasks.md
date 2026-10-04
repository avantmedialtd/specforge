## 1. Frontend

- [x] 1.1 In `src/components/settings/LayoutGroup.tsx`, make `PanelPositionRow` render nothing unless its configuration has loaded and says the provider is on: no row while loading, none on a load failure, none while off. Remove the off note and its "Turn on in Integrations" button, then drop the now-unused `onSelectGroup` prop from `LayoutGroup`, `BitbucketPanelRow` and `GithubPanelRow`, and stop passing it in `src/components/settings/SettingsView.tsx` (`settings-view`: *The Layout Group Gathers the Side Panes' Occupants*).
- [x] 1.2 In `src/App.css`, delete the `.settings-off-note` rule. Grep `src/` to confirm nothing references it any more.
- [x] 1.3 In `src/CLAUDE.md`, change the Settings paragraph to say panel slots appear in Layout only for integrations that are on.

## 2. Verification

- [x] 2.1 Confirm `bun run build` (strict `tsc`, then bundle) and `bun test` are green.
- [x] 2.2 Smoke-test in the browser loop on isolated state: a debug `specforge-serve` serving the rebuilt `dist/`, driven by Chrome MCP. Walk the scenarios:
  - with both integrations off, Layout's Side panes holds only the Commit history switch, with no panel row, note or placeholder;
  - with GitHub turned on, the GitHub panel row appears and no BitBucket row does;
  - a slot changed from another client is reflected in the open row;
  - a slot set while on survives turning GitHub off and on again;
  - the enabled GitHub card's "Change in Layout" link still lands on the row.
