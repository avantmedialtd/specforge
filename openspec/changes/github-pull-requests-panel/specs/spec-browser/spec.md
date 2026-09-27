## MODIFIED Requirements

### Requirement: Side Panes Host the Pull-Request Panel

When the BitBucket pull-request feature or the GitHub pull-request feature is enabled (see the *Opt-in Pull-Request Tracking* requirement in the `bitbucket-pull-requests` capability and the *Opt-in GitHub Pull-Request Tracking* requirement in the `github-pull-requests` capability), the main window SHALL render that provider's pull-request panel in exactly one of four slots: above the workspace tree or between the tree and the sidebar footer entrypoints in the tree-navigation pane, or above or below the commit graph in the commit-graph rail. Each panel's slot SHALL be its own provider's persisted position setting (see *Panel Position Is a Persisted Setting* in the `bitbucket-pull-requests` capability and *GitHub Panel Position Is a Persisted Setting* in the `github-pull-requests` capability). Each panel's header SHALL name its provider. When both panels are set to the same slot, both SHALL render there, the BitBucket panel above the GitHub panel. No fifth pane, divider, or visibility toggle SHALL be introduced for either.

A panel placed in a side pane is part of that pane: hiding the pane through the *Side-Pane Visibility Toggles* SHALL hide the panel with it, and restoring the pane SHALL restore the panel. The panels SHALL NOT be independently hideable through those toggles.

Each panel's body SHALL be bounded in height and scroll internally, and SHALL keep that bound whether or not it shares its slot, so that with any panels in the sidebar slots the Settings and Archive entrypoints and any usage-quota strips beneath them remain fully visible and operable at every viewport height at which they are reachable without the panels (see the *Master-Detail Layout* requirement).

While at least one panel is rendered in the tree-navigation pane, the workspace tree SHALL reserve a height of one fifth of the viewport height, and the panels SHALL yield height before the tree does: the tree SHALL NOT fall more than one pixel below its reserve while any panel in the same pane still has height to give, and SHALL give up its reserve only as far as needed to keep the footer entrypoints and quota strips fully visible. While at least one panel is rendered in the commit-graph rail, the commit graph SHALL hold the same reserve on the same terms and SHALL absorb the remaining rail height. A pane in which no panel is rendered — because both features are disabled, or because every enabled panel is positioned in the other pane — SHALL lay out exactly as it does without the panels, with no reserve applied.

Yielding height is distinct from collapsing: at the shortest heights, where the panels have yielded everything, a panel's header is clipped along with its body so the footer promise holds, whereas a panel the user has collapsed SHALL keep its one-line header.

$$\text{reserve} = \tfrac{1}{5} \cdot \text{viewport height}$$

#### Scenario: The default slot sits above the footer entrypoints

- **WHEN** either feature is enabled with its default position
- **THEN** its panel renders in the tree-navigation pane between the workspace tree and the Archive entrypoint
- **AND** the tree above it still scrolls to absorb reduced height

#### Scenario: A rail slot leaves the graph scrolling

- **WHEN** a panel's position is `right-top` or `right-bottom`
- **THEN** the panel renders in the commit-graph rail at that edge
- **AND** the commit graph keeps its own scrolling within the remaining rail height

#### Scenario: Headers name their providers

- **WHEN** both panels are rendered
- **THEN** one panel's header names BitBucket and the other's names GitHub

#### Scenario: Two panels share a slot in order

- **WHEN** both features are enabled and both positions are `right-bottom`
- **THEN** both panels render below the commit graph, the BitBucket panel above the GitHub panel
- **AND** the commit graph keeps its reserve and its own scrolling

#### Scenario: Stacked panels leave the tree its reserve

- **WHEN** both panels are in `left-bottom`, expanded, each holding twenty rows, in a window 800 pixels tall
- **THEN** the workspace tree keeps its reserve of one fifth of the window height, to within one pixel
- **AND** the Settings entrypoint, the Archive entrypoint and any usage-quota strips are fully visible
- **AND** each panel's body scrolls internally to reach its remaining rows

#### Scenario: At the shortest heights the footer still wins

- **WHEN** the window is so short that the footer entrypoints fit only if the tree gives up its reserve
- **THEN** the panels have yielded all their height, the tree falls below its reserve, and the Settings entrypoint, the Archive entrypoint and any usage-quota strips remain fully visible

#### Scenario: The panel hides with its pane

- **WHEN** a panel is in a sidebar slot and the user hides the sidebar with Cmd/Ctrl+B or its collapse chevron
- **THEN** the panel is hidden with the sidebar
- **AND** restoring the sidebar brings the panel back in the same slot

#### Scenario: Footer entrypoints stay reachable with a long list

- **WHEN** a panel is in a sidebar slot, expanded, holding more rows than fit, at a viewport height short enough that the tree must scroll
- **THEN** the Settings entrypoint, the Archive entrypoint and any usage-quota strips are fully visible and can be activated
- **AND** the panel's body scrolls internally to reach its remaining rows

#### Scenario: Disabled features change nothing

- **WHEN** both features are disabled, with both positions at their default `left-bottom`
- **THEN** the tree-navigation pane and the commit-graph rail render exactly as they did before this capability existed
- **AND** no height reserve is applied to the workspace tree or the commit graph

#### Scenario: A pane without a panel takes no reserve

- **WHEN** the only enabled panel is positioned in the commit-graph rail
- **THEN** no height reserve is applied to the workspace tree
