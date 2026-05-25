# spec-browser Delta — Expand Sidebar By Default

## ADDED Requirements

### Requirement: Default Expansion of Tree Nodes

Every collapsible node in the workspace tree SHALL be rendered expanded by default the first time the user encounters it, with no prior interaction required. This applies at every depth — Repo groups, flat workspaces, multi-instance logical-change parents, instance rows, artifact nodes (Specs, Tasks), capability rows under Specs, sections under Tasks — down to and including individual task rows.

The expansion state SHALL be modelled as the set of node IDs the user has explicitly collapsed. A node SHALL be rendered as open if and only if its ID is **not** in that set. Toggling a node adds its ID to the set if absent or removes it if present.

#### Scenario: First-ever launch shows the tree fully expanded

- **WHEN** the user launches the application for the first time after this change ships
- **AND** at least one workspace has been registered
- **THEN** every collapsible row in the workspace tree is rendered in its expanded state
- **AND** the artifact rows (Proposal, Specs, Design, Tasks) of every change are visible without further interaction
- **AND** the Tasks artifact's section rows and each section's task rows are visible without further interaction

#### Scenario: New change appears expanded after watcher event

- **WHEN** the workspace tree is already rendered
- **AND** a new change directory is added to a registered workspace on disk, triggering a `change-added` event
- **THEN** the new change's row appears in the tree
- **AND** the change's artifact rows, section rows, and task rows are all rendered expanded without any user click

#### Scenario: Promoted multi-instance parent appears expanded

- **WHEN** a previously-singleton logical change gains a second instance
- **THEN** the new disclosure parent row is rendered expanded by default
- **AND** the two instance rows beneath it are visible without any user click

### Requirement: User Collapse State Persists Across Sessions

When the user collapses a tree node, the application SHALL persist that collapse so that, after quitting and relaunching, the same node is rendered collapsed without further user action. When the user re-expands a previously-collapsed node, the application SHALL persist that re-expansion so that, after relaunching, the node is again rendered expanded.

The persisted state SHALL be the set of node IDs that the user has collapsed. Nodes whose IDs are not in the set SHALL be rendered expanded, including nodes whose IDs were never observed in any previous session (e.g., changes or sections created after the persisted set was last written).

#### Scenario: Collapsed node stays collapsed after restart

- **WHEN** the user collapses an artifact row (e.g., the Tasks node of some change)
- **AND** the user quits and relaunches the application
- **THEN** the same artifact row is rendered in its collapsed state in the restored tree
- **AND** sibling nodes the user did not collapse remain expanded

#### Scenario: Re-expanded node stays expanded after restart

- **WHEN** the user has previously collapsed and persisted a node
- **AND** the user re-expands that node in the current session
- **AND** the user quits and relaunches the application
- **THEN** the node is rendered in its expanded state in the restored tree

#### Scenario: Settings file with no persisted collapses loads cleanly

- **WHEN** the user launches a version of the application that supports persisted collapses for the first time
- **AND** the existing settings file on disk was written by a previous version with no field for collapsed node IDs
- **THEN** the application loads the settings file successfully
- **AND** the tree is rendered with every node expanded (empty collapsed set)

#### Scenario: Persistence write is bounded by user toggles

- **WHEN** the user toggles the same node open and closed several times in rapid succession
- **THEN** the persisted state eventually reflects the final toggled position
- **AND** the application does not write a settings file for every intermediate state (writes are coalesced)

### Requirement: Removal of First-Sight Auto-Expansion Effect

The application SHALL NOT maintain a separate "first time we see this node, mark it expanded" code path. The default-expanded behaviour is a consequence of the inverted state model (collapses tracked, opens implicit), not of any seeding logic that runs on view changes.

#### Scenario: No second-mount re-seeding

- **WHEN** the watcher emits a `cache-updated` event that causes the tree's `views` prop to re-render
- **THEN** the application does not run any effect that adds node IDs to an "expanded" set in response

#### Scenario: User-collapse survives a tree re-render

- **WHEN** the user has collapsed a node
- **AND** the watcher subsequently emits a `cache-updated` event for that workspace
- **THEN** the node remains collapsed after the re-render
- **AND** the application does not re-open the node as a side effect of the view change
