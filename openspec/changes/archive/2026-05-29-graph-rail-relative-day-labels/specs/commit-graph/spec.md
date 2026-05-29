## MODIFIED Requirements

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
