## ADDED Requirements

### Requirement: Commit History Can Be Turned Off

The application SHALL offer a single, application-wide *Commit history* switch that turns the commit graph on or off. The switch SHALL be stored in the application settings and SHALL survive a restart. It SHALL default to on. A settings file that does not record the switch, including every file written before it existed, SHALL load with it on, so that no existing installation loses its graph.

The switch SHALL be presented in Settings in both the desktop application and the served web UI. It is not a desktop-only setting within the meaning of the *Desktop-Only Settings Are Hidden in the Web UI* requirement in the `web-ui` capability. Changing it SHALL take effect in main windows that are already open, without reopening or reloading them. The change SHALL be announced as a dedicated event rather than as a variant of the cache-event stream, so that no existing consumer of that stream in any frontend gains a case it must ignore. The event SHALL be delivered on both the desktop and the browser event transports.

While the switch is off, the commit graph SHALL NOT be rendered, and the application SHALL NOT perform commit-graph reads (git subprocess work) on its behalf, whatever the tree selection. This is the guarantee the *Commit-Graph Rail Pane* requirement gives a hidden rail. The commit graph is then not an occupant of the rail (see the *Rail Exists Only While Occupied* requirement in the `spec-browser` capability). Turning the switch on SHALL fetch and render the graph of the repository that owns the current tree selection at that moment.

A main window SHALL reflect the switch from its first frame. It SHALL NOT render the commit graph, or a rail that the graph alone would occupy, and then remove it once the stored value has been retrieved. The application settings SHALL remain the authoritative store. Because retrieving them is asynchronous, the switch SHALL additionally be mirrored in a store the frontend can read synchronously at startup, written on every change. The mirror SHALL be treated as a first-paint hint and never as the source of truth: once the authoritative value is available it SHALL be reconciled against the mirror, so a value changed by another instance of the application corrects itself rather than persisting. When no mirror is available (a first run, or a cleared store) a main window SHALL start with the switch on.

Turning the switch on from Settings SHALL show the rail on the surface where it was turned on, even if the rail had been hidden there (see the *Side-Pane Visibility Toggles* requirement in the `spec-browser` capability). Every other surface SHALL keep its own rail visibility.

The terminal frontend SHALL NOT consult the switch. Its commit graph is a screen the user opens, not a pane sharing the window with the reading surface, and it renders unconditionally (see the *Progress Surfaces in the Terminal* requirement in the `terminal-ui` capability).

#### Scenario: History is on by default

- **WHEN** SpecForge starts with a settings file that does not record the switch
- **THEN** the switch is on
- **AND** the rail renders the commit graph as it did before the switch existed

#### Scenario: Turning history off removes the graph

- **WHEN** the commit graph is rendered in the rail
- **AND** the user turns the switch off in Settings
- **THEN** the commit graph is no longer rendered in the main window

#### Scenario: No graph reads while history is off

- **WHEN** the switch is off
- **AND** the user selects nodes belonging to different repositories
- **THEN** no commit-graph read is performed for any of those selections

#### Scenario: The switch survives a restart

- **WHEN** the user turns the switch off and restarts the application
- **THEN** the switch is still off
- **AND** the commit graph is not rendered

#### Scenario: A window opened with history off never paints the graph

- **WHEN** the switch is off and no pull-request panel occupies the rail
- **AND** a main window is opened on a surface that last saw the switch off
- **THEN** no frame of that window renders the commit graph or the rail

#### Scenario: A stale mirror is corrected

- **WHEN** the switch was changed by another instance of the application since this surface last ran
- **THEN** the authoritative value takes effect once read
- **AND** the mirror is updated to match it

#### Scenario: Open windows adopt a change

- **WHEN** the switch is changed while the browser skin is connected
- **THEN** the browser skin's main window adopts the new value without a reload

#### Scenario: Turning history on shows a hidden rail

- **WHEN** the rail is hidden on a surface
- **AND** the switch is turned off and later turned on again in that surface's Settings
- **THEN** the rail is shown, rendering the commit graph of the repository that owns the current tree selection
- **AND** the rail's visibility on every other surface is unchanged

#### Scenario: The switch is offered in the browser

- **WHEN** the Settings view renders in the served web UI
- **THEN** the Commit history switch is shown
- **AND** changing it takes effect as it does in the desktop application

#### Scenario: The terminal ignores the switch

- **WHEN** the switch is off
- **THEN** the terminal frontend's History screen still renders the commit graph

## MODIFIED Requirements

### Requirement: Commit-Graph Rail Pane

While commit history is on (see the *Commit History Can Be Turned Off* requirement), the main window SHALL present the commit graph in a rail: a third, resizable pane positioned to the far right of the tree and detail panes. The rail SHALL render the commit graph of the repository that owns the current tree selection. The rail SHALL be hideable and restorable by the user (see the *Side-Pane Visibility Toggles* requirement in the `spec-browser` capability); while visible it behaves as specified here. While commit history is off, the commit graph is neither rendered nor one of the rail's occupants, and the rail exists only while a pull-request panel occupies it (see the *Rail Exists Only While Occupied* requirement in the `spec-browser` capability).

When the current tree selection belongs to a git repository, the rail SHALL render that repository's graph. When the selection is a non-git (flat) workspace, or when no node is selected, the rail SHALL render an empty placeholder state and SHALL NOT error. As the tree selection moves between nodes belonging to different repositories, the rail SHALL re-target to the newly selected repository.

While the rail is hidden, the application SHALL NOT perform commit-graph reads (git subprocess work) on the rail's behalf: hiding the rail suspends graph fetching, and re-targeting the rail's repository while hidden SHALL cost nothing. Restoring the rail SHALL fetch and render the graph for the repository that owns the current tree selection at that moment.

The divider between the detail pane and the rail SHALL be draggable to resize the rail, and the chosen width SHALL persist across sessions, consistent with the existing master-detail divider.

#### Scenario: Rail shows the selected repository's graph

- **WHEN** commit history is on
- **AND** the user selects any node belonging to a git-backed workspace
- **THEN** the rail renders the commit graph of that node's repository

#### Scenario: Rail re-targets when selection moves to another repository

- **WHEN** the rail is showing repository A's graph
- **AND** the user selects a node belonging to a different repository B
- **THEN** the rail re-renders with repository B's graph

#### Scenario: Rail is empty for non-git workspaces

- **WHEN** commit history is on
- **AND** the user selects a node belonging to a non-git (flat) workspace, or no node is selected
- **THEN** the rail renders an empty placeholder state
- **AND** the rest of the application is unaffected

#### Scenario: Rail width is resizable and persists

- **WHEN** the user drags the divider between the detail pane and the rail
- **THEN** the rail resizes to the dragged width
- **AND** the width is restored on the next launch

#### Scenario: Hidden rail performs no graph fetching

- **WHEN** the rail is hidden
- **AND** the user selects nodes belonging to different repositories
- **THEN** no commit-graph read is performed for any of those selections

#### Scenario: Restoring the rail fetches the current repository's graph

- **WHEN** commit history is on and the rail is hidden while the tree selection belongs to repository A
- **AND** the user moves the selection to repository B and then restores the rail
- **THEN** the rail fetches and renders repository B's graph
