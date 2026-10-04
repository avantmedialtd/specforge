# Hide Panel Slots for Integrations That Are Off

## Why

Settings → Layout → Side panes lists a panel-position choice for both the BitBucket and the GitHub pull-request panels, whether or not those integrations are on. An off provider's row adds a note saying it is off and a link to turn it on. In use, this reads as Settings showing components the reader never enabled. It is also noise in the one place meant to show what actually sits beside the documents.

## What Changes

- **Layout shows only what is on.** The Side panes section presents a panel-position choice only for a pull-request integration that is on. A provider that is off gets no row: no note, no link, and no loading or error placeholder while its configuration is read. With neither on, Side panes holds the Commit history switch alone.
- **The way in stays where it was.** Turning an integration on in Integrations shows its card's "Panel: … · Change in Layout" line, and the slot choice appears in Layout from then on.
- **A slot chosen before stays chosen.** The position setting is untouched. A slot persisted while a provider was on is still where its panel lands when the provider is turned on again.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `settings-view`: *The Layout Group Gathers the Side Panes' Occupants*. A panel-position choice is presented only while its provider is on, and an off provider has no row.

## Impact

- `src/components/settings/LayoutGroup.tsx`: a provider's panel row renders nothing unless its configuration has loaded and says the provider is on. The off note and its "Turn on in Integrations" button are removed, along with the now-unused `onSelectGroup` prop.
- `src/components/settings/SettingsView.tsx`: no longer passes `onSelectGroup` to `LayoutGroup`.
- `src/App.css`: removes the `.settings-off-note` rule nothing uses any more.
- `src/CLAUDE.md`: the Settings paragraph says panel slots appear in Layout only for integrations that are on.

**Deliberately unchanged.** The `bitbucket.panelPosition` and `github.panelPosition` settings and their events; the Integrations cards, including the slot line and "Change in Layout" link on an enabled card; the Commit history switch; the backend; the terminal UI.
