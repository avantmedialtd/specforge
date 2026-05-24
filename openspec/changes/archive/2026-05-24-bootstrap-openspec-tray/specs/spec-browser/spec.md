# spec-browser

## ADDED Requirements

### Requirement: Master-Detail Layout

The main application window SHALL present a two-pane master-detail layout: a tree-navigation pane on the left and a content-rendering pane on the right. A resizable divider separates the two panes.

#### Scenario: Both panes visible at startup

- **WHEN** the user opens the main window for the first time
- **THEN** both the tree pane and the detail pane are visible side by side
- **AND** the divider between them can be dragged to adjust pane widths

### Requirement: Workspace Tree Hierarchy

The tree pane SHALL display registered workspaces as top-level groups, each containing the workspace's non-archived changes. Each change exposes four artifact nodes in fixed order — Proposal, Specs, Design, Tasks — mirroring the structure of the artifex VSCode extension. The Specs node, when present, contains one child per capability spec file. The Tasks node, when present, contains one child per section in `tasks.md`, and each section contains one child per task line.

#### Scenario: Multiple workspaces shown as groups

- **WHEN** two workspaces are registered, each containing at least one non-archived change
- **THEN** the tree shows two top-level workspace nodes
- **AND** each workspace node contains its respective changes as children

#### Scenario: Artifact nodes appear in fixed order

- **WHEN** a change node is expanded
- **THEN** the four artifact nodes appear in the order: Proposal, Specs, Design, Tasks

#### Scenario: Tasks decomposed into sections and individual tasks

- **WHEN** a change's Tasks node is expanded
- **AND** `tasks.md` contains numbered sections each holding tasks
- **THEN** each section appears as a child of the Tasks node
- **AND** each task appears as a child of its containing section, with its completion state indicated by an icon

#### Scenario: Archived changes are not shown

- **WHEN** a change directory exists under `openspec/changes/archive/` of a registered workspace
- **THEN** that change is not shown in the tree

### Requirement: Markdown Rendering of Leaf Artifacts

Clicking a leaf artifact node (Proposal, Design, Tasks, or an individual capability spec under the Specs node) SHALL render that artifact's markdown file in the detail pane. Rendering MUST support GitHub-Flavored Markdown including syntax-highlighted fenced code blocks.

#### Scenario: Click proposal renders proposal.md

- **WHEN** the user clicks a change's Proposal node
- **THEN** the detail pane shows the rendered content of `proposal.md` for that change

#### Scenario: Click design renders design.md

- **WHEN** the user clicks a change's Design node
- **THEN** the detail pane shows the rendered content of `design.md` for that change

#### Scenario: Click tasks renders tasks.md

- **WHEN** the user clicks a change's Tasks node
- **THEN** the detail pane shows the rendered content of `tasks.md` for that change

#### Scenario: Click individual capability spec renders that spec.md

- **WHEN** the user clicks a child node under the Specs artifact node
- **THEN** the detail pane shows the rendered content of `specs/<capability>/spec.md` for that change

### Requirement: Section and Task Scroll Anchors

Clicking a section or individual-task node SHALL render `tasks.md` in the detail pane (if not already rendered) and scroll the detail pane to the corresponding heading or line.

#### Scenario: Click section scrolls to heading

- **WHEN** the user clicks a section node under a Tasks artifact
- **THEN** the detail pane shows the rendered `tasks.md` for that change
- **AND** the pane is scrolled so the section's heading is visible at the top of the pane

#### Scenario: Click task scrolls to task line

- **WHEN** the user clicks an individual task node under a section
- **THEN** the detail pane shows the rendered `tasks.md` for that change
- **AND** the pane is scrolled so the task's line is visible

### Requirement: Deferred Interaction Nodes

In v1, clicking a workspace node, a change node, or the Specs artifact node SHALL produce no observable effect in the detail pane. These node types are reserved for later UX work.

#### Scenario: Click workspace is a no-op

- **WHEN** the user clicks a top-level workspace node
- **THEN** the detail pane's current contents are unchanged

#### Scenario: Click change is a no-op

- **WHEN** the user clicks a change node
- **THEN** the detail pane's current contents are unchanged

#### Scenario: Click Specs artifact node is a no-op

- **WHEN** the user clicks the Specs artifact node of a change
- **THEN** the detail pane's current contents are unchanged

### Requirement: Reactive Updates from Filesystem

The tree pane and the detail pane SHALL reflect on-disk changes within the watcher's debounce window without requiring user action.

#### Scenario: Tree updates when new change appears

- **WHEN** a new change directory is created on disk in a registered workspace
- **THEN** the new change appears as a child of that workspace in the tree

#### Scenario: Detail pane updates when shown file is edited

- **WHEN** the detail pane is currently rendering an artifact's markdown
- **AND** that markdown file is modified on disk
- **THEN** the detail pane re-renders with the updated content

#### Scenario: Tree updates when change is archived on disk

- **WHEN** a change directory is moved from `openspec/changes/<id>/` to `openspec/changes/archive/<id>/`
- **THEN** the change is removed from the tree

### Requirement: Window State Persistence

The main window's position, size, and maximised state SHALL persist across application restarts.

#### Scenario: Window size restored after relaunch

- **WHEN** the user resizes the main window to a non-default dimension
- **AND** quits and relaunches the application
- **THEN** the main window opens at the previously set size and position

### Requirement: Read-Only Viewer

In v1, the application SHALL NOT modify any spec file as a result of user interaction with the rendered content.

#### Scenario: Task checkboxes are not interactive

- **WHEN** the detail pane is rendering a `tasks.md` containing markdown task checkboxes
- **THEN** clicking a rendered checkbox does not modify the underlying file
