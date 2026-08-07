# commit-graph Delta

## MODIFIED Requirements

### Requirement: Commit-Graph Rail Pane

The main window SHALL present a commit-graph rail as a third, resizable pane positioned to the far right of the tree and detail panes. The rail SHALL render the commit graph of the repository that owns the current tree selection. The rail SHALL be hideable and restorable by the user (see the *Side-Pane Visibility Toggles* requirement in the `spec-browser` capability); while visible it behaves as specified here.

When the current tree selection belongs to a git repository, the rail SHALL render that repository's graph. When the selection is a non-git (flat) workspace, or when no node is selected, the rail SHALL render an empty placeholder state and SHALL NOT error. As the tree selection moves between nodes belonging to different repositories, the rail SHALL re-target to the newly selected repository.

While the rail is hidden, the application SHALL NOT perform commit-graph reads (git subprocess work) on the rail's behalf: hiding the rail suspends graph fetching, and re-targeting the rail's repository while hidden SHALL cost nothing. Restoring the rail SHALL fetch and render the graph for the repository that owns the current tree selection at that moment.

The divider between the detail pane and the rail SHALL be draggable to resize the rail, and the chosen width SHALL persist across sessions, consistent with the existing master-detail divider.

#### Scenario: Rail shows the selected repository's graph

- **WHEN** the user selects any node belonging to a git-backed workspace
- **THEN** the rail renders the commit graph of that node's repository

#### Scenario: Rail re-targets when selection moves to another repository

- **WHEN** the rail is showing repository A's graph
- **AND** the user selects a node belonging to a different repository B
- **THEN** the rail re-renders with repository B's graph

#### Scenario: Rail is empty for non-git workspaces

- **WHEN** the user selects a node belonging to a non-git (flat) workspace, or no node is selected
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

- **WHEN** the rail is hidden while the tree selection belongs to repository A
- **AND** the user moves the selection to repository B and then restores the rail
- **THEN** the rail fetches and renders repository B's graph
