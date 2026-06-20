## MODIFIED Requirements

### Requirement: Master-Detail Browse and Screen Navigation

The interactive frontend SHALL present a Browse screen with a two-pane master-detail layout — a workspace/change tree on the left and an artifact-detail pane on the right — and SHALL provide modal Dashboard, Season, and Settings screens. In two-pane mode the tree pane's width SHALL be bounded so that, as the terminal widens, the surplus width goes to the detail pane rather than the tree growing without limit; the tree SHALL still be allotted enough width to read change names on smaller terminals. Keyboard navigation SHALL move focus between the two Browse panes, switch between screens, switch between artifact tabs in the detail pane, and scroll the focused region.

#### Scenario: Browse shows tree and detail

- **WHEN** the Browse screen is active
- **THEN** the workspace/change tree and the artifact-detail pane are both shown
- **AND** keyboard focus can be moved between them

#### Scenario: Detail pane receives surplus width on wide terminals

- **WHEN** the Browse screen is shown in two-pane mode on a wide terminal
- **THEN** the tree pane is held to a bounded width
- **AND** the additional width beyond that bound is given to the detail pane

#### Scenario: Switching screens

- **WHEN** the user invokes the Dashboard, Season, or Settings screen switch
- **THEN** that screen replaces the Browse view
- **AND** returning to Browse restores the prior tree selection and detail target

#### Scenario: Selecting a change shows its artifact

- **WHEN** the user selects a change in the tree and chooses an artifact tab
- **THEN** the detail pane renders that artifact
