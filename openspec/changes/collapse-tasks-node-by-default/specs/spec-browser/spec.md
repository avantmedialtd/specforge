# spec-browser

## MODIFIED Requirements

### Requirement: Default Expansion of Tree Nodes

Every collapsible node in the workspace tree SHALL be rendered with a default expansion state derived from the node's current data at render time, with no prior interaction required.

For most node types — Repo groups, flat workspaces, multi-instance logical-change parents, instance rows, the Proposal/Design/Specs artifact nodes, capability rows under Specs, and individual task rows — the default SHALL be "expanded".

The **Tasks artifact node** SHALL default to "collapsed" whenever it is collapsible (its change has at least one section), regardless of task-completion state. This keeps a change's task rows out of the tree until the user opts into them by expanding the Tasks node; the node's task-progress meter or completion ✓ remains visible in its meta slot while collapsed (see *Tasks Artifact Node Progress*).

For one further node type the default depends on completion state:

- A **Section node** SHALL default to "collapsed" when its section has at least one task and every task in it is complete; otherwise it SHALL default to "expanded".

The user MAY override a node's default in either direction by clicking its disclosure caret. The application records overrides in two independent sets:

- A `collapsed` set of node IDs the user has closed against a default-open node.
- An `expanded` set of node IDs the user has opened against a default-closed node.

A node's rendered open/closed state SHALL be computed as follows:

- For a node whose current default is "open": open iff its ID is **not** in the `collapsed` set.
- For a node whose current default is "closed": open iff its ID **is** in the `expanded` set.

Because the Tasks artifact node's default is now "closed" unconditionally, a user who opens it has the node's ID recorded in the `expanded` set, and that preference persists across restarts per the *User Collapse State Persists Across Sessions* requirement.

#### Scenario: First-ever launch shows the tree expanded except for Tasks nodes and completed Sections

- **WHEN** the user launches the application for the first time after this change ships
- **AND** at least one workspace has been registered
- **THEN** every collapsible row whose computed default is "expanded" is rendered open
- **AND** every collapsible Tasks artifact node is rendered collapsed, regardless of task-completion state
- **AND** every Section node whose section has at least one task and all of them complete is rendered collapsed
- **AND** every section with at least one incomplete task carries the "expanded" default, so its task rows are visible once its Tasks node is expanded

#### Scenario: New change appears with its Tasks node collapsed

- **WHEN** the workspace tree is already rendered
- **AND** a new change directory is added to a registered workspace on disk, triggering a `change-added` event
- **THEN** the new change's row appears in the tree expanded
- **AND** the change's Proposal, Specs, and Design artifact rows are rendered expanded
- **AND** the change's Tasks artifact row is rendered collapsed
- **AND** each Section under Tasks carries the per-node-type Section default (collapsed iff all of its tasks are complete) for when the Tasks node is expanded

#### Scenario: Promoted multi-instance parent appears expanded

- **WHEN** a previously-singleton logical change gains a second instance
- **THEN** the new disclosure parent row is rendered expanded by default
- **AND** the two instance rows beneath it are visible without any user click

#### Scenario: User expand overrides the collapsed Tasks node

- **WHEN** a Tasks artifact node is rendered collapsed by default
- **AND** the user clicks the Tasks row's disclosure caret
- **THEN** the Tasks node is re-rendered expanded
- **AND** its Section rows are visible, each per the Section default rule

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

### Requirement: Auto-Collapse of Completed Task Groups

The workspace tree SHALL auto-collapse **Section nodes** when their work is complete, so the user's attention is drawn to in-progress work. The Tasks artifact node is collapsed by default unconditionally (see *Default Expansion of Tree Nodes*) and therefore does not participate in this completion-based rule.

- A **Section node** is considered complete when its section has at least one task (`tasks.length > 0`) and every task in it is complete.

When a Section is complete, its default expansion state SHALL be "collapsed". When it is not complete (or has no tasks at all), its default expansion state SHALL be "expanded". The default is recomputed on every render from the node's current data, so transitions between in-progress and complete take effect within the watcher's debounce window with no extra user action.

The completion-based auto-collapse rule SHALL apply only to Section nodes. Change rows, Instance rows, Repo groups, flat workspaces, multi-instance logical-change parents, the Proposal/Specs/Design artifact rows, capability rows under Specs, and individual task rows SHALL continue to default to "expanded" regardless of completion state. The Tasks artifact node SHALL default to "collapsed" regardless of completion state.

A user override (a click on the disclosure caret) SHALL take precedence over the default and SHALL persist across restarts per the *User Collapse State Persists Across Sessions* requirement.

#### Scenario: Tasks artifact node defaults collapsed regardless of completion

- **WHEN** a Tasks artifact node is collapsible (its change has at least one section)
- **AND** the user has not explicitly expanded it
- **THEN** the Tasks artifact node is rendered collapsed whether the change's tasks are all complete, partially complete, or all incomplete
- **AND** its meta slot shows the task-progress meter (in progress) or the trailing `✓` (complete) per *Tasks Artifact Node Progress*

#### Scenario: Tasks node with sections but no parseable tasks still defaults collapsed

- **WHEN** a change's `tasks.md` has at least one section heading but no parseable task lines (`totalTasks === 0`)
- **THEN** the Tasks artifact node is collapsible and is rendered collapsed by default
- **AND** its meta slot shows neither a progress meter nor a `✓` glyph

#### Scenario: Section collapses when all its tasks complete

- **WHEN** a Section has at least one task and every task in it is complete
- **AND** the user has not explicitly expanded that Section since it became complete
- **THEN** the Section node is rendered collapsed

#### Scenario: Section stays expanded when partially complete

- **WHEN** a Section has at least one incomplete task
- **THEN** the Section node is rendered expanded by default

#### Scenario: Section with no tasks is unaffected by the auto-collapse rule

- **WHEN** a Section has zero tasks
- **THEN** the Section row is rendered as a leaf (no chevron), as it is today
- **AND** the auto-collapse rule does not apply

#### Scenario: Completing the last task in a change does not change its Tasks node expansion

- **WHEN** a Tasks artifact node is rendered collapsed by default
- **AND** an external edit to `tasks.md` marks the change's last incomplete task complete
- **AND** the watcher emits the update
- **THEN** the Tasks artifact node remains collapsed (it was already collapsed by default)
- **AND** its meta slot swaps the progress meter for the trailing `✓` within the watcher debounce window

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

#### Scenario: User can expand a collapsed Tasks node or auto-collapsed Section

- **WHEN** a Tasks artifact node (collapsed by default) or a Section node (collapsed because its tasks are all complete) is rendered collapsed
- **AND** the user clicks the row's disclosure caret
- **THEN** the node is re-rendered expanded
- **AND** the expansion persists across restarts
