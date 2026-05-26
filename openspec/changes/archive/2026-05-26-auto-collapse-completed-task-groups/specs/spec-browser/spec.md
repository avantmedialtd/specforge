# spec-browser Delta — Auto-Collapse Completed Task Groups

## MODIFIED Requirements

### Requirement: Default Expansion of Tree Nodes

Every collapsible node in the workspace tree SHALL be rendered with a default expansion state derived from the node's current data at render time, with no prior interaction required.

For most node types — Repo groups, flat workspaces, multi-instance logical-change parents, instance rows, the Proposal/Design/Specs artifact nodes, capability rows under Specs, and individual task rows — the default SHALL be "expanded".

For two node types the default depends on completion state:

- The **Tasks artifact node** SHALL default to "collapsed" when its change has at least one task and every task is complete; otherwise it SHALL default to "expanded".
- A **Section node** SHALL default to "collapsed" when its section has at least one task and every task in it is complete; otherwise it SHALL default to "expanded".

The user MAY override a node's default in either direction by clicking its disclosure caret. The application records overrides in two independent sets:

- A `collapsed` set of node IDs the user has closed against a default-open node.
- An `expanded` set of node IDs the user has opened against a default-closed node.

A node's rendered open/closed state SHALL be computed as follows:

- For a node whose current default is "open": open iff its ID is **not** in the `collapsed` set.
- For a node whose current default is "closed": open iff its ID **is** in the `expanded` set.

#### Scenario: First-ever launch shows the tree expanded except for completed groups

- **WHEN** the user launches the application for the first time after this change ships
- **AND** at least one workspace has been registered
- **THEN** every collapsible row whose computed default is "expanded" is rendered open
- **AND** every Tasks artifact node whose change has at least one task and all tasks complete is rendered collapsed
- **AND** every Section node whose section has at least one task and all of them complete is rendered collapsed
- **AND** every section with at least one incomplete task is rendered expanded along with its task rows

#### Scenario: New change appears with completion-aware default expansion

- **WHEN** the workspace tree is already rendered
- **AND** a new change directory is added to a registered workspace on disk, triggering a `change-added` event
- **THEN** the new change's row appears in the tree expanded
- **AND** the change's Proposal, Specs, and Design artifact rows are rendered expanded
- **AND** the Tasks artifact row is rendered collapsed iff the change's tasks are all complete, otherwise expanded
- **AND** each Section row under Tasks is rendered per the same per-node-type default rule

#### Scenario: Promoted multi-instance parent appears expanded

- **WHEN** a previously-singleton logical change gains a second instance
- **THEN** the new disclosure parent row is rendered expanded by default
- **AND** the two instance rows beneath it are visible without any user click

#### Scenario: User expand overrides an auto-collapsed Section

- **WHEN** a Section node is rendered collapsed because every task in it is complete
- **AND** the user clicks the Section row's disclosure caret
- **THEN** the Section is re-rendered expanded
- **AND** its task rows are visible

#### Scenario: User collapse overrides a default-open node

- **WHEN** an in-progress Section node is rendered expanded
- **AND** the user clicks its disclosure caret
- **THEN** the Section is re-rendered collapsed
- **AND** its row continues to display its title (no other rows beneath it change state)

### Requirement: User Collapse State Persists Across Sessions

When the user clicks the disclosure caret of any tree node, the application SHALL persist the resulting override so that, after quitting and relaunching, the node is rendered in the same state without further user action.

The persisted state SHALL consist of two independent sets of node IDs:

- `collapsedTreeNodeIds` — node IDs the user has closed against a default-open node.
- `expandedTreeNodeIds` — node IDs the user has opened against a default-closed node.

A node's rendered state on launch SHALL be computed by combining its current default (derived from the node's data) with the matching override set, as defined in the *Default Expansion of Tree Nodes* requirement.

A persisted ID in one set whose node's default has since flipped to the other polarity SHALL be ignored — it is consulted only when the node's default again matches that set's role. The application is not required to garbage-collect such inert entries.

#### Scenario: Collapsed default-open node stays collapsed after restart

- **WHEN** the user collapses a default-open node (e.g., the Proposal artifact row of some change, or an in-progress Section)
- **AND** the user quits and relaunches the application
- **THEN** the same node is rendered in its collapsed state in the restored tree
- **AND** sibling nodes the user did not collapse are rendered per their own defaults

#### Scenario: Re-expanded default-open node stays expanded after restart

- **WHEN** the user has previously collapsed and persisted a default-open node
- **AND** the user re-expands that node in the current session
- **AND** the user quits and relaunches the application
- **THEN** the node is rendered in its expanded state in the restored tree

#### Scenario: Expanded default-closed node stays expanded after restart

- **WHEN** a Section node is collapsed by default because every task in it is complete
- **AND** the user clicks its disclosure caret to expand it
- **AND** the user quits and relaunches the application
- **THEN** the same Section is rendered in its expanded state in the restored tree
- **AND** other completed Sections the user did not expand remain collapsed

#### Scenario: Re-collapsed default-closed node stays collapsed after restart

- **WHEN** the user has previously expanded and persisted a default-closed Section
- **AND** the user re-collapses that Section in the current session
- **AND** the user quits and relaunches the application
- **THEN** the Section is rendered in its collapsed state in the restored tree

#### Scenario: Settings file with no expanded-IDs field loads cleanly

- **WHEN** the user launches a version of the application that supports the expanded-overrides set for the first time
- **AND** the existing settings file on disk was written by a previous version that has no `expandedTreeNodeIds` field
- **THEN** the application loads the settings file successfully
- **AND** the existing `collapsedTreeNodeIds` field is honoured as before
- **AND** the tree is rendered with an empty `expanded` override set, so every default-closed node renders collapsed until the user expands it

#### Scenario: Persistence write is bounded by user toggles

- **WHEN** the user toggles the same node open and closed several times in rapid succession
- **THEN** the persisted state eventually reflects the final toggled position for both sets
- **AND** the application does not write a settings file for every intermediate state (writes to each set are coalesced)

#### Scenario: Stale expand-override survives default flip without surfacing

- **WHEN** the user has expanded a completed Section (its ID is in the `expanded` set)
- **AND** a new incomplete task is added to that Section, flipping its default to "open"
- **THEN** the Section is rendered open (because the default-open path consults the `collapsed` set, which does not contain the ID)
- **AND** when the Section later returns to fully-complete, the persisted ID in the `expanded` set causes the Section to render expanded again (matching the user's earlier preference)

### Requirement: Tree Expansion Has No First-Sight Auto-Expansion Effect

The application SHALL NOT maintain a separate "first time we see this node, mark it expanded (or collapsed)" code path. A node's default state is derived from its current data on every render — not from any one-shot seeding effect that runs on view changes — and the user's override (if any) is applied on top of that default.

#### Scenario: No second-mount re-seeding

- **WHEN** the watcher emits a `cache-updated` event that causes the tree's `views` prop to re-render
- **THEN** the application does not run any effect that mutates the `collapsed` or `expanded` override sets in response to the new view

#### Scenario: User override survives a tree re-render

- **WHEN** the user has expanded an auto-collapsed Section or collapsed a default-open Section
- **AND** the watcher subsequently emits a `cache-updated` event for that workspace
- **THEN** the user's override is preserved after the re-render
- **AND** the application does not flip the node's state as a side effect of the view change

## ADDED Requirements

### Requirement: Auto-Collapse of Completed Task Groups

The workspace tree SHALL auto-collapse two node types when their work is complete, so the user's attention is drawn to in-progress work:

- The **Tasks artifact node** of a change is considered complete when the change has at least one task (`totalTasks > 0`) and every task is complete (`completedTasks === totalTasks`).
- A **Section node** is considered complete when its section has at least one task and every task in it is complete.

When a node is complete, its default expansion state SHALL be "collapsed". When it is not complete (or has no tasks at all), its default expansion state SHALL be "expanded". The default is recomputed on every render from the node's current data, so transitions between in-progress and complete take effect within the watcher's debounce window with no extra user action.

The auto-collapse rule SHALL apply only to Tasks artifact nodes and Section nodes. Change rows, Instance rows, Repo groups, flat workspaces, multi-instance logical-change parents, the Proposal/Specs/Design artifact rows, capability rows under Specs, and individual task rows SHALL continue to default to "expanded" regardless of completion state.

A user override (a click on the disclosure caret) SHALL take precedence over the default and SHALL persist across restarts per the *User Collapse State Persists Across Sessions* requirement.

#### Scenario: Tasks artifact collapses when all tasks complete

- **WHEN** a change has at least one task and every task is complete
- **AND** the user has not explicitly expanded the Tasks artifact node since it became complete
- **THEN** the Tasks artifact node is rendered collapsed
- **AND** its `(n/n)` label still indicates the completion count

#### Scenario: Section collapses when all its tasks complete

- **WHEN** a Section has at least one task and every task in it is complete
- **AND** the user has not explicitly expanded that Section since it became complete
- **THEN** the Section node is rendered collapsed

#### Scenario: Tasks artifact stays expanded when partially complete

- **WHEN** a change has at least one task and at least one of them is incomplete
- **THEN** the Tasks artifact node is rendered expanded by default

#### Scenario: Section stays expanded when partially complete

- **WHEN** a Section has at least one incomplete task
- **THEN** the Section node is rendered expanded by default

#### Scenario: Section with no tasks is unaffected by the auto-collapse rule

- **WHEN** a Section has zero tasks
- **THEN** the Section row is rendered as a leaf (no chevron), as it is today
- **AND** the auto-collapse rule does not apply

#### Scenario: Tasks artifact with no tasks is unaffected by the auto-collapse rule

- **WHEN** a change's `tasks.md` exists but contains no parseable tasks (`totalTasks === 0`)
- **THEN** the Tasks artifact node renders per its existing behaviour (collapsible iff it has sections, defaulting to expanded)
- **AND** the auto-collapse rule does not apply

#### Scenario: Completing the last task in a Section auto-collapses it

- **WHEN** a Section node is rendered expanded with at least one incomplete task
- **AND** an external edit to `tasks.md` marks the last incomplete task complete
- **AND** the watcher emits the update
- **THEN** the Section node is re-rendered collapsed within the watcher debounce window
- **AND** the user does not need to take any action

#### Scenario: Adding an incomplete task to a complete Section re-expands it

- **WHEN** a Section node is rendered collapsed because every task in it is complete
- **AND** an external edit to `tasks.md` adds a new incomplete task to that section
- **AND** the watcher emits the update
- **THEN** the Section node is re-rendered expanded within the watcher debounce window

#### Scenario: User can expand an auto-collapsed group

- **WHEN** a Tasks artifact node or Section node is rendered collapsed because of the auto-collapse rule
- **AND** the user clicks the row's disclosure caret
- **THEN** the node is re-rendered expanded
- **AND** the expansion persists across restarts

### Requirement: Completed Section Row Shows a Completion Glyph

Every Section row whose section has at least one task and whose every task is complete SHALL display a ✓ glyph in the row's meta column, regardless of the row's current expansion state.

This glyph distinguishes a Section that is collapsed because all its tasks are done from a Section the user has manually collapsed while work is still in progress. It mirrors the existing completion indicators elsewhere in the tree: the green ✓ icon on the Change row when its tasks are all complete, and the `(n/n)` label on the Tasks artifact node.

The Tasks artifact node SHALL NOT receive an additional glyph; its existing `Tasks (n/n)` label already conveys completion textually, and an icon on its left edge already indicates the artifact's presence.

#### Scenario: Completed Section shows the glyph while collapsed

- **WHEN** a Section is rendered collapsed because every task in it is complete
- **THEN** the Section row displays a ✓ glyph in its meta column

#### Scenario: Completed Section shows the glyph while expanded

- **WHEN** a Section is rendered expanded (either by default because it has incomplete tasks, or because the user explicitly expanded an auto-collapsed Section)
- **AND** every task in the Section is in fact complete
- **THEN** the Section row displays the ✓ glyph in its meta column

#### Scenario: In-progress Section shows no glyph

- **WHEN** a Section has at least one incomplete task
- **THEN** the Section row does not display the ✓ glyph

#### Scenario: Empty Section shows no glyph

- **WHEN** a Section has zero tasks
- **THEN** the Section row does not display the ✓ glyph
