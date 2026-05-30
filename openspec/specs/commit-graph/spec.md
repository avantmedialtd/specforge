# commit-graph Specification

## Purpose
TBD - created by archiving change add-commit-graph-rail. Update Purpose after archive.
## Requirements
### Requirement: Commit-Graph Rail Pane

The main window SHALL present a commit-graph rail as a third, resizable pane positioned to the far right of the tree and detail panes. The rail SHALL be always visible (not a mode the user toggles into) and SHALL render the commit graph of the repository that owns the current tree selection.

When the current tree selection belongs to a git repository, the rail SHALL render that repository's graph. When the selection is a non-git (flat) workspace, or when no node is selected, the rail SHALL render an empty placeholder state and SHALL NOT error. As the tree selection moves between nodes belonging to different repositories, the rail SHALL re-target to the newly selected repository.

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

### Requirement: Faithful Commit Graph Rendering

The rail SHALL render a faithful git commit graph built from `git log --all`: one node per commit, placed in a lane (column), with vertical edges where a lane continues and diagonal edges where a branch is created or a merge collapses. The rail SHALL render ref decorations — local branch heads, remote branch heads, tags, and the HEAD pointer — and a commit subject for each row. Author, full commit date, and abbreviated hash SHALL be available on hover.

The rail SHALL group commit rows into calendar-day sections by author date in the viewer's local time zone, inserting a labelled day-separator row above the first commit of each day (including the newest day at the top). A day-separator is presentation only: it carries no commit node, and lane edges that are alive across the day boundary SHALL pass straight through the separator band so that branch lines are never visually broken. Day grouping SHALL NOT reorder commits, alter lane assignment, or change which decorations a commit carries.

The day-separator label SHALL be relative to the viewer's current calendar day in their local time zone. The current day SHALL be labelled `Today` and the immediately preceding day `Yesterday`. Days two through six before the current day SHALL be labelled by weekday name alone (e.g. `Wednesday`); within this window a bare weekday name is unambiguous because the current day and the prior six days span all seven weekday names exactly once. Days seven or more before the current day SHALL be labelled by absolute date in the existing compact form (e.g. `Mon, May 25`). Relative wording SHALL be locale-aware, consistent with the rail's other locale-respecting date formatting. The label text SHALL NOT change grouping, ordering, lane assignment, or which day a commit falls under.

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
- **THEN** a labelled day-separator row is rendered between them identifying the newer day
- **AND** the first commit of the newest day also has a day-separator above it

#### Scenario: Current and prior days read Today and Yesterday

- **WHEN** a day-separator marks the viewer's current calendar day in their local time zone
- **THEN** the separator is labelled `Today`
- **AND** a separator for the immediately preceding calendar day is labelled `Yesterday`

#### Scenario: Recent days within the week read as weekday names

- **WHEN** a day-separator marks a calendar day two to six days before the viewer's current day
- **THEN** the separator is labelled with that day's weekday name (e.g. `Wednesday`) and no absolute date

#### Scenario: Older days keep an absolute date

- **WHEN** a day-separator marks a calendar day seven or more days before the viewer's current day
- **THEN** the separator is labelled with the existing compact absolute date (e.g. `Mon, May 25`)

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

### Requirement: Lane Layout and Overflow

Commit lanes SHALL be assigned by a deterministic layout that reclaims a lane as soon as its branch merges, so that the visible lane count at any vertical position reflects only the branches concurrently alive there rather than the total branch count. When the concurrently-alive lane count exceeds the rail's current width, the graph region SHALL scroll horizontally without scrolling the commit subject out of view, and the rail SHALL remain resizable so the user can widen it to view more lanes at once.

#### Scenario: Compaction keeps the common case narrow

- **WHEN** a repository has many branches that were created and merged over time
- **THEN** rows away from active merge points render with only the lanes alive at that point, not one lane per branch in the repository

#### Scenario: Overflow scrolls horizontally without losing the subject

- **WHEN** the concurrently-alive lanes at some row exceed the rail's width
- **THEN** the graph region for that row can be scrolled horizontally to reveal the additional lanes
- **AND** the commit subject remains visible

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

### Requirement: Commit Detail View

The commit-detail view rendered in the center pane SHALL show the commit's metadata (abbreviated and full hash, author, date, and full message), the list of files the commit changed with per-file added/removed line counts, and the textual diff of the change. The metadata SHALL include the commit's git trailers — the `Key: value` lines of the message's last paragraph as recognized by git's own trailer parser — rendered as a list of key/value pairs in git's emitted order, with every value shown when a key appears more than once. Trailers SHALL be presented as neutral commit metadata: the `OpenSpec-Id` trailer SHALL receive no styling, link, or marker that distinguishes it from any other trailer, and a commit whose message carries no trailers SHALL render no trailer section. A breadcrumb SHALL indicate the commit context and that selecting an artifact returns to the artifact view.

#### Scenario: Detail view lists changed files and diff

- **WHEN** the commit-detail view renders for a commit
- **THEN** it shows the commit's metadata, the changed-files list with added/removed counts, and the diff

#### Scenario: Commit trailers are listed

- **WHEN** the commit-detail view renders for a commit whose message carries git trailers (e.g. `OpenSpec-Id` and `Co-Authored-By`)
- **THEN** each trailer is shown as a key/value pair in git's emitted order

#### Scenario: Repeated trailer keys are all shown

- **WHEN** a commit carries the same trailer key more than once (e.g. two `Co-Authored-By` lines)
- **THEN** every occurrence is listed and not collapsed to a single entry

#### Scenario: Body prose is not shown as a trailer

- **WHEN** a commit's message has a multi-paragraph body and only its last paragraph contains trailers
- **THEN** only the recognized trailers are listed and the body prose is not mistaken for a trailer

#### Scenario: OpenSpec-Id is rendered as a neutral trailer

- **WHEN** a commit carries an `OpenSpec-Id` trailer
- **THEN** it is displayed identically to any other trailer, with no link, tint, or marker distinguishing it

#### Scenario: A commit with no trailers shows no trailer section

- **WHEN** the commit-detail view renders for a commit whose message carries no trailers
- **THEN** no trailer list or empty trailer affordance is shown

#### Scenario: Breadcrumb indicates how to return

- **WHEN** the commit-detail view is shown
- **THEN** a breadcrumb identifies the commit and indicates that selecting an artifact returns to the artifact view

### Requirement: Live Graph Updates

The rail SHALL reflect changes to the repository's refs within the watcher's debounce window without user action. New commits, branch creation/deletion, branch-head movement, tag changes, and HEAD movement SHALL cause the rail to refresh.

#### Scenario: New commit appears

- **WHEN** a new commit is created in the repository on disk
- **THEN** the rail renders the new commit within the debounce window

#### Scenario: Branch movement is reflected

- **WHEN** a branch head moves, is created, or is deleted on disk
- **THEN** the rail's decorations and topology update within the debounce window

### Requirement: Read-Only Operation

The rail and commit-detail view SHALL expose no operation that mutates the repository. Checkout, branch create/delete, merge, rebase, cherry-pick, reset, and any other history- or working-tree-mutating git operation SHALL NOT be reachable from the rail.

#### Scenario: No mutating actions are offered

- **WHEN** the user interacts with the rail or the commit-detail view
- **THEN** no action that checks out, moves, rewrites, or deletes commits, branches, or working-tree state is available

### Requirement: Graceful Degradation Without Git

When the `git` binary is unavailable, or the selected workspace is not inside a git repository, the rail SHALL render its empty placeholder state and the rest of the application SHALL continue to function, consistent with the degrade-to-empty behaviour of the existing git integration.

#### Scenario: Git binary missing

- **WHEN** the `git` binary is not on PATH
- **THEN** the rail renders empty
- **AND** the tree and detail panes continue to function

