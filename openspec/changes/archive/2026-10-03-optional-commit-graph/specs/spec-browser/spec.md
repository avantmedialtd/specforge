## ADDED Requirements

### Requirement: Rail Exists Only While Occupied

The far-right pane of the main window, the rail (called the commit-graph rail where it hosts the commit graph), SHALL have two kinds of occupant:

- the commit graph, while commit history is on (see the *Commit History Can Be Turned Off* requirement in the `commit-graph` capability);
- each pull-request panel that is positioned in a rail slot (`right-top` or `right-bottom`) and whose feature is enabled (see the *Side Panes Host the Pull-Request Panel* requirement).

The main window SHALL render the rail only while it has at least one occupant.

While the rail has no occupant it SHALL be absent rather than hidden. The main window SHALL render no rail, no divider between the rail and the detail pane, and no restore affordance for the rail, and the detail pane SHALL extend to the window's right edge. While the rail is absent, its keyboard toggle and, on macOS, its View menu item SHALL change nothing; in particular they SHALL NOT change the rail's remembered visibility. The rail's visibility (see the *Side-Pane Visibility Toggles* requirement) applies only while the rail exists. When the rail gains an occupant, it SHALL return in the visibility it last had on that surface, at its remembered width clamped to the window's current constraints.

Whether a pull-request panel occupies the rail SHALL be decided from its provider's current state, not from the panel having been displayed. A rail that is absent or hidden SHALL gain the panel as an occupant when the panel's feature is enabled, and SHALL lose it when the feature is disabled, without the panel being displayed in between.

An occupant with nothing to show still occupies the rail. The rail SHALL NOT be removed while it shows the commit graph's empty placeholder (no node selected, the Dashboard, a non-git workspace, a repository without commits) or a pull-request panel with no rows, so that the layout does not shift as the user navigates.

#### Scenario: No occupant leaves no rail

- **WHEN** commit history is off
- **AND** no pull-request panel with its feature enabled is positioned in a rail slot
- **THEN** the main window renders no rail, no rail divider and no rail restore affordance
- **AND** the detail pane extends to the window's right edge

#### Scenario: A pull-request panel keeps the rail

- **WHEN** commit history is off
- **AND** a pull-request panel with its feature enabled is positioned at `right-top`
- **THEN** the rail is rendered holding that panel and no commit graph

#### Scenario: A disabled panel parked in a rail slot does not keep the rail

- **WHEN** commit history is off
- **AND** the only pull-request panel positioned in a rail slot has its feature disabled
- **THEN** the main window renders no rail

#### Scenario: The rail toggle does nothing while the rail is absent

- **WHEN** the rail is absent and was shown when it last existed on this surface
- **AND** the user presses Cmd/Ctrl+Alt+B
- **THEN** no rail, rail divider or rail restore affordance appears
- **AND** when the rail later gains an occupant, it is shown rather than hidden

#### Scenario: Enabling a panel brings the rail back

- **WHEN** commit history is off and the rail is absent
- **AND** the user enables a pull-request feature whose panel is positioned in a rail slot
- **THEN** the rail appears holding that panel, without a reload, in the visibility it last had on this surface

#### Scenario: Disabling the last panel retires a hidden rail's restore affordance

- **WHEN** commit history is off, the rail is hidden, and its only occupant is a pull-request panel
- **AND** the user disables that panel's feature
- **THEN** the rail's restore affordance is removed
- **AND** the rail is absent

#### Scenario: An empty placeholder keeps the rail

- **WHEN** commit history is on
- **AND** the detail pane shows the Dashboard with no repository selected
- **THEN** the rail is rendered showing the commit graph's empty placeholder
- **AND** selecting a change in a git repository leaves the rail's presence and width unchanged

## MODIFIED Requirements

### Requirement: Side-Pane Visibility Toggles

The tree-navigation pane (sidebar) and the commit-graph rail SHALL each be independently hideable and restorable, in both the desktop application and the served web UI; the rail is hideable while it exists (see the *Rail Exists Only While Occupied* requirement). Any combination of hidden/shown SHALL be reachable. With both side panes hidden, or with the sidebar hidden and the rail absent, the detail pane SHALL occupy the full window width. The detail pane itself SHALL NOT be hideable.

Each visibility SHALL be togglable by keyboard: Cmd+B (macOS) / Ctrl+B (Windows, Linux) for the sidebar, and Cmd+Alt+B (macOS) / Ctrl+Alt+B (Windows, Linux) for the rail, with the same bindings active in the served web UI. While the rail is absent, its binding SHALL change nothing.

Each visible side pane SHALL display a collapse affordance (a chevron control) at its top. While a side pane is hidden, a restore affordance SHALL be displayed in the corresponding top corner of the detail pane (top-left for the sidebar, top-right for the rail), so that restoring a pane never requires a keyboard shortcut, a menu, or an application restart. An absent rail is not hidden, since there is nothing to restore, and SHALL display no restore affordance.

On a device that reports no hover capability, these collapse and restore affordances SHALL be rendered visibly at rest rather than being revealed by pointer hover, so that pane visibility stays operable where neither hover nor a hardware keyboard is available (see the *Essential Controls Are Discoverable Without Hover* requirement in the `touch-input` capability).

Each pane's visibility SHALL persist across sessions in frontend view state, consistent with how the rail width persists (see the *Commit-Graph Rail Pane* requirement in the `commit-graph` capability); visibility SHALL NOT be stored in application settings. Whether commit history is on is a separate application setting, not part of the rail's visibility (see the *Commit History Can Be Turned Off* requirement in the `commit-graph` capability). A hidden pane's width SHALL be preserved: restoring the pane SHALL bring back the width it had when hidden, clamped to the window's current constraints. A hidden pane's divider SHALL NOT be rendered.

Pane visibility is ambient view state: it SHALL NOT be part of the Address, the URL, or navigation history (see the `view-routing` capability), and navigating — including Back/Forward — SHALL NOT change pane visibility.

On macOS in the desktop application, while the sidebar is hidden the detail pane SHALL reserve the top clearance for the window controls (traffic lights) and the titlebar drag strip that the sidebar normally provides, so that detail-pane content is not obscured.

#### Scenario: Sidebar toggles independently

- **WHEN** the user presses Cmd/Ctrl+B or activates the sidebar's collapse chevron
- **THEN** the sidebar and its divider are hidden and the detail pane widens to absorb the space
- **AND** the commit-graph rail's visibility is unchanged
- **AND** a restore affordance appears in the detail pane's top-left corner

#### Scenario: Rail toggles independently

- **WHEN** the rail exists and the user presses Cmd/Ctrl+Alt+B or activates the rail's collapse chevron
- **THEN** the rail and its divider are hidden and the detail pane widens to absorb the space
- **AND** the sidebar's visibility is unchanged
- **AND** a restore affordance appears in the detail pane's top-right corner

#### Scenario: Both panes hidden yields full-width content

- **WHEN** the sidebar and the rail are both hidden
- **THEN** the detail pane occupies the full window width
- **AND** restore affordances for both panes remain visible in the detail pane's top corners
- **AND** both keyboard toggles remain active

#### Scenario: Pane affordances are visible at rest without hover

- **WHEN** the served web UI is loaded on a device that reports no hover capability
- **THEN** the visible side panes' collapse chevrons are visible at rest
- **AND** activating one hides its pane and reveals a restore affordance that is likewise visible at rest
- **AND** the pane can be restored without a keyboard

#### Scenario: Restoring a pane recovers its previous width

- **WHEN** the user hides a side pane and later restores it
- **THEN** the pane returns at the width it had when hidden, clamped to fit the current window

#### Scenario: Visibility persists across sessions

- **WHEN** the user hides the rail and quits the application
- **AND** relaunches it
- **THEN** the rail is still hidden and the sidebar is still visible

#### Scenario: Navigation does not change visibility

- **WHEN** a side pane is hidden
- **AND** the user navigates to any address, including via Back/Forward
- **THEN** the pane remains hidden and the address is unaffected by pane visibility

#### Scenario: Hidden sidebar keeps macOS window controls clear

- **WHEN** the sidebar is hidden in the desktop application on macOS
- **THEN** the detail pane's content starts below the traffic-light / titlebar drag area rather than underneath it

#### Scenario: Hidden sidebar with no rail yields full-width content

- **WHEN** the sidebar is hidden and the rail is absent
- **THEN** the detail pane occupies the full window width
- **AND** only the sidebar's restore affordance is displayed
- **AND** pressing Cmd/Ctrl+Alt+B changes nothing

### Requirement: Side Panes Host the Pull-Request Panel

When the BitBucket pull-request feature or the GitHub pull-request feature is enabled (see the *Opt-in Pull-Request Tracking* requirement in the `bitbucket-pull-requests` capability and the *Opt-in GitHub Pull-Request Tracking* requirement in the `github-pull-requests` capability), the main window SHALL render that provider's pull-request panel in exactly one of four slots:

- above the workspace tree in the tree-navigation pane;
- between the tree and the sidebar footer entrypoints in the tree-navigation pane;
- at the top of the commit-graph rail (above the commit graph while commit history is on);
- at the bottom of the commit-graph rail (below the commit graph while commit history is on).

Each panel's slot SHALL be its own provider's persisted position setting (see *Panel Position Is a Persisted Setting* in the `bitbucket-pull-requests` capability and *GitHub Panel Position Is a Persisted Setting* in the `github-pull-requests` capability). Each panel's header SHALL name its provider. When both panels are set to the same slot, both SHALL render there, the BitBucket panel above the GitHub panel. No fifth pane, divider, or visibility toggle SHALL be introduced for either.

A panel placed in a side pane is part of that pane: hiding the pane through the *Side-Pane Visibility Toggles* SHALL hide the panel with it, and restoring the pane SHALL restore the panel. The panels SHALL NOT be independently hideable through those toggles. A panel whose feature is enabled and which is positioned in a rail slot is an occupant of the rail, and keeps the rail present while commit history is off (see the *Rail Exists Only While Occupied* requirement).

Each panel's body SHALL be bounded in height and scroll internally, and SHALL keep that bound whether or not it shares its slot. This keeps the Settings and Archive entrypoints, and any usage-quota strips beneath them, fully visible and operable with any panels in the sidebar slots, at every viewport height at which they are reachable without the panels (see the *Master-Detail Layout* requirement). The one exception is a rail without the commit graph, described below.

While at least one panel is rendered in the tree-navigation pane, the workspace tree SHALL reserve a height of one fifth of the viewport height, and the panels SHALL yield height before the tree does. The tree SHALL NOT fall more than one pixel below its reserve while any panel in the same pane still has height to give, and SHALL give up its reserve only as far as needed to keep the footer entrypoints and quota strips fully visible. While at least one panel is rendered in the commit-graph rail alongside the commit graph, the commit graph SHALL hold the same reserve on the same terms and SHALL absorb the remaining rail height. A pane in which no panel is rendered, because both features are disabled or because every enabled panel is positioned in the other pane, SHALL lay out exactly as it does without the panels, with no reserve applied.

In a rail without the commit graph (commit history off, at least one panel rendered there), the panels SHALL stack in slot order, the `right-top` slot's panels above the `right-bottom` slot's, and no reserve SHALL apply. A panel's body there SHALL NOT be held to the bound above. The panels together SHALL be able to use the rail's full height, sharing it when their rows need more height than the rail has, each scrolling internally. A collapsed panel SHALL keep its one-line header.

Yielding height is distinct from collapsing: at the shortest heights, where the panels have yielded everything, a panel's header is clipped along with its body so the footer promise holds, whereas a panel the user has collapsed SHALL keep its one-line header.

$$\text{reserve} = \tfrac{1}{5} \cdot \text{viewport height}$$

#### Scenario: The default slot sits above the footer entrypoints

- **WHEN** either feature is enabled with its default position
- **THEN** its panel renders in the tree-navigation pane between the workspace tree and the Archive entrypoint
- **AND** the tree above it still scrolls to absorb reduced height

#### Scenario: A rail slot leaves the graph scrolling

- **WHEN** commit history is on
- **AND** a panel's position is `right-top` or `right-bottom`
- **THEN** the panel renders in the commit-graph rail at that edge
- **AND** the commit graph keeps its own scrolling within the remaining rail height

#### Scenario: Headers name their providers

- **WHEN** both panels are rendered
- **THEN** one panel's header names BitBucket and the other's names GitHub

#### Scenario: Two panels share a slot in order

- **WHEN** commit history is on
- **AND** both features are enabled and both positions are `right-bottom`
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

#### Scenario: A disabled feature changes nothing

- **WHEN** both features are disabled, with both positions at their default `left-bottom`
- **THEN** the tree-navigation pane and the commit-graph rail render exactly as they did before this capability existed
- **AND** no height reserve is applied to the workspace tree or the commit graph

#### Scenario: A pane without a panel takes no reserve

- **WHEN** the only enabled panel is positioned in the commit-graph rail
- **THEN** no height reserve is applied to the workspace tree

#### Scenario: A rail without the graph gives its panel the full height

- **WHEN** commit history is off
- **AND** the only panel in the rail is expanded at `right-top`, holding more rows than fit, in a window 800 pixels tall
- **THEN** the panel's body is taller than the bound it keeps in the tree-navigation pane, filling the rail's height
- **AND** the body scrolls internally to reach its remaining rows
- **AND** no height reserve is applied

#### Scenario: Two panels share a rail without the graph in slot order

- **WHEN** commit history is off
- **AND** the GitHub panel is at `right-top` and the BitBucket panel at `right-bottom`, both expanded, each holding more rows than fit
- **THEN** the GitHub panel renders above the BitBucket panel
- **AND** together they fill the rail's height, each body scrolling internally
