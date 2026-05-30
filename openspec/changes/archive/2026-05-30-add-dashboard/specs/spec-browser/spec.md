## MODIFIED Requirements

### Requirement: Master-Detail Layout

The main application window SHALL present a master-detail layout of two primary panes — a tree-navigation pane on the left and a content-rendering (detail) pane in the center — plus an optional commit-graph rail on the far right (see the *Commit-Graph Rail Pane* requirement in the `commit-graph` capability). Resizable dividers separate the panes.

The detail (center) pane SHALL render one of three targets: an OpenSpec artifact's markdown, a commit's detail view when a commit is selected in the rail, or the **Dashboard** (see the *Dashboard Home Surface* requirement in the `dashboard` capability). The Dashboard SHALL be the default target: it is rendered at startup and whenever no artifact and no commit is selected, in place of any "nothing selected" placeholder. When an artifact or a commit has been selected, the detail pane renders whichever of those targets was selected most recently; the tree drives the artifact target and the rail drives the commit target.

#### Scenario: Panes visible at startup

- **WHEN** the user opens the main window for the first time
- **THEN** the tree pane and the detail pane are visible side by side
- **AND** the commit-graph rail is visible on the far right
- **AND** the detail pane renders the Dashboard (no artifact or commit having been selected)
- **AND** the dividers between the panes can be dragged to adjust their widths

#### Scenario: Detail pane renders the Dashboard by default

- **WHEN** no artifact and no commit is selected
- **THEN** the detail pane renders the Dashboard
- **AND** no "nothing selected" placeholder is shown

#### Scenario: Detail pane renders artifact markdown by default

- **WHEN** the user selects a renderable artifact node in the tree
- **THEN** the detail pane renders that artifact's markdown

#### Scenario: Detail pane renders commit detail when a commit is selected

- **WHEN** the user selects a commit in the commit-graph rail
- **THEN** the detail pane renders that commit's detail view
- **AND** selecting an artifact node in the tree afterwards returns the detail pane to artifact markdown
