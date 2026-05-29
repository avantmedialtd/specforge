## MODIFIED Requirements

### Requirement: Faithful Commit Graph Rendering

The rail SHALL render a faithful git commit graph built from `git log --all`: one node per commit, placed in a lane (column), with vertical edges where a lane continues and diagonal edges where a branch is created or a merge collapses. The rail SHALL render ref decorations — local branch heads, remote branch heads, tags, and the HEAD pointer — and a commit subject for each row. Author, full commit date, and abbreviated hash SHALL be available on hover.

The rail SHALL group commit rows into calendar-day sections by author date in the viewer's local time zone, inserting a labelled day-separator row above the first commit of each day (including the newest day at the top). A day-separator is presentation only: it carries no commit node, and lane edges that are alive across the day boundary SHALL pass straight through the separator band so that branch lines are never visually broken. Day grouping SHALL NOT reorder commits, alter lane assignment, or change which decorations a commit carries.

The graph SHALL carry no OpenSpec semantics: commits SHALL NOT be tinted, grouped, filtered, or otherwise annotated by their OpenSpec change, change-id trailer, or archive status. Calendar-day grouping is a neutral temporal affordance and is explicitly NOT an OpenSpec annotation; the only relationship between the rail and OpenSpec state is the repository scope inherited from the tree selection.

#### Scenario: Graph matches git's own view

- **WHEN** the rail renders a repository's graph
- **THEN** its commits, lanes, branch/merge topology, refs, and tags correspond to the output of `git log --graph --all` for that repository

#### Scenario: Decorations are shown

- **WHEN** a commit is a branch head, a tag target, or the current HEAD
- **THEN** the rail renders the corresponding ref/tag/HEAD decoration on that commit's row

#### Scenario: Hover reveals commit metadata

- **WHEN** the user hovers a commit row
- **THEN** the author, full date, and abbreviated hash are surfaced

#### Scenario: Commits are grouped under day separators

- **WHEN** two consecutive commit rows have author dates on different calendar days in the viewer's local time zone
- **THEN** a labelled day-separator row is rendered between them identifying the newer day's date
- **AND** the first commit of the newest day also has a day-separator above it

#### Scenario: Lanes pass through a day separator

- **WHEN** a lane is alive across a day boundary (a branch with commits on both days)
- **THEN** that lane's edge is drawn continuously through the separator band without a break
- **AND** the commit nodes remain aligned with their subject rows

#### Scenario: Grouping preserves faithful topology

- **WHEN** day separators are inserted
- **THEN** the commits' order, lane assignment, branch/merge edges, and decorations are identical to the ungrouped graph

#### Scenario: No OpenSpec semantics in the graph

- **WHEN** the rail renders commits that carry an `OpenSpec-Id` trailer or that archived a change
- **THEN** those commits are rendered identically to any other commit, with no change-based tint, grouping, or marker

### Requirement: Commit Selection Drives the Detail Pane

Selecting a commit in the rail SHALL render that commit's detail in the center detail pane. The tree and the rail SHALL both drive the detail pane, and the pane SHALL render whichever was selected most recently. The tree and the rail SHALL each retain their own selection highlight independently. Returning to an artifact view SHALL require only selecting an artifact in the tree — no separate "back" action.

Day-separator rows SHALL be non-interactive: they SHALL NOT be selectable, SHALL NOT carry a selection highlight, and SHALL NOT become a detail-pane render target.

#### Scenario: Clicking a commit shows its detail

- **WHEN** the user clicks a commit in the rail
- **THEN** the detail pane renders that commit's detail view

#### Scenario: Selecting a tree artifact restores the markdown

- **WHEN** the detail pane is showing a commit's detail
- **AND** the user selects an artifact node in the tree
- **THEN** the detail pane renders that artifact's markdown

#### Scenario: Tree and rail keep independent highlights

- **WHEN** the user has a tree artifact highlighted and then clicks a commit in the rail
- **THEN** the rail's commit is highlighted in the rail
- **AND** the tree's previously selected node remains highlighted in the tree

#### Scenario: Day separators are not selectable

- **WHEN** the user clicks a day-separator row in the rail
- **THEN** no commit is selected and the detail pane is unchanged
