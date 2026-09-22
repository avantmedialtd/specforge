## ADDED Requirements

### Requirement: Side Panes Host the Pull-Request Panel

When the BitBucket pull-request feature is enabled (see the *Opt-in Pull-Request Tracking* requirement in the `bitbucket-pull-requests` capability), the main window SHALL render the pull-request panel in exactly one of four slots: above the workspace tree or between the tree and the sidebar footer entrypoints in the tree-navigation pane, or above or below the commit graph in the commit-graph rail. The slot SHALL be the persisted position setting (see *Panel Position Is a Persisted Setting* in the same capability). No fifth pane, divider, or visibility toggle SHALL be introduced for it.

A panel placed in a side pane is part of that pane: hiding the pane through the *Side-Pane Visibility Toggles* SHALL hide the panel with it, and restoring the pane SHALL restore the panel. The panel SHALL NOT be independently hideable through those toggles.

The panel's body SHALL be bounded in height and scroll internally, so that with the panel in either sidebar slot the Settings and Archive entrypoints and any usage-quota strips beneath them remain fully visible and operable at every viewport height at which they are reachable without the panel (see the *Master-Detail Layout* requirement), and so that with the panel in either rail slot the commit graph still absorbs the remaining height. When the feature is disabled the layout SHALL be identical to the layout without the panel.

#### Scenario: The default slot sits above the footer entrypoints

- **WHEN** the feature is enabled with the default position
- **THEN** the panel renders in the tree-navigation pane between the workspace tree and the Archive entrypoint
- **AND** the tree above it still scrolls to absorb reduced height

#### Scenario: A rail slot leaves the graph scrolling

- **WHEN** the position is `right-top` or `right-bottom`
- **THEN** the panel renders in the commit-graph rail at that edge
- **AND** the commit graph keeps its own scrolling within the remaining rail height

#### Scenario: The panel hides with its pane

- **WHEN** the panel is in a sidebar slot and the user hides the sidebar with Cmd/Ctrl+B or its collapse chevron
- **THEN** the panel is hidden with the sidebar
- **AND** restoring the sidebar brings the panel back in the same slot

#### Scenario: Footer entrypoints stay reachable with a long list

- **WHEN** the panel is in a sidebar slot, expanded, holding more rows than fit, at a viewport height short enough that the tree must scroll
- **THEN** the Settings entrypoint, the Archive entrypoint and any usage-quota strips are fully visible and can be activated
- **AND** the panel's body scrolls internally to reach its remaining rows

#### Scenario: A disabled feature changes nothing

- **WHEN** the feature is disabled
- **THEN** the tree-navigation pane and the commit-graph rail render exactly as they did before this capability existed
