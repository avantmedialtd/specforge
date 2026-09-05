# commit-garden Specification

## Purpose

Defines the commit garden: a section at the bottom of the Dashboard that renders, for each top-level registered entry (a repository group or a non-git flat workspace), a faithful git graph of that entry's commits for the viewer's current local calendar day — real lanes, nodes, edges, ref decorations, and subjects, exactly as the commit-graph rail would draw those same commits, with no stylized abstraction. Nodes are coloured by the author of each commit, resolved with you-precedence (the canonical developer first, else the raw git author), with the canonical developer accented; two git identities of one teammate therefore receive two colours. The garden updates live within the watcher's debounce window, re-scopes to the new day at the local midnight boundary, persists no new state, is an unconditional part of the Dashboard's progress layer, and is strictly read-only.
## Requirements
### Requirement: Per-Workspace Commit Graphs at the Dashboard Bottom

The Dashboard SHALL present a commit-garden section at the **bottom** of its content — below the analytics overview — with one plot per top-level registered entry: a repository group or a non-git (flat) workspace, mirroring the per-repository breakdown's one-entry-per-top-level-item rule, so that multiple worktrees of one repository resolve to a single plot. Plots SHALL be stacked vertically, each labelled with the same display name the Dashboard uses for that top-level entry. The section SHALL be an unconditional part of the Dashboard's progress layer and SHALL NOT be gated by any setting.

#### Scenario: One plot per top-level entry

- **WHEN** the section renders with two repository groups and one flat workspace registered
- **THEN** it shows three plots
- **AND** each plot is labelled with that entry's Dashboard display name

#### Scenario: Worktrees of one repository share a plot

- **WHEN** several registered workspaces are worktrees of the same git repository
- **THEN** the section shows a single plot for that repository rather than one per worktree

#### Scenario: Section sits at the bottom

- **WHEN** the Dashboard renders
- **THEN** the commit-garden section appears at the bottom of the Dashboard, below the analytics overview

#### Scenario: Section needs no opt-in

- **WHEN** the Dashboard renders in a fresh installation with no settings ever changed
- **THEN** the commit-garden section is present
- **AND** no setting is consulted to decide whether to compute or render it

#### Scenario: Empty registry

- **WHEN** the Dashboard renders and no workspaces are registered
- **THEN** the commit-garden section is omitted rather than rendering a blank area or an error

### Requirement: Faithful Today-Scoped Commit Graph

Each plot SHALL render a **faithful** commit graph of that entry's commits for the **current local calendar day** — the commits whose author date falls on the viewer's current local day, consistent with the commit-graph rail's day grouping. The graph SHALL place one node per commit in a lane (column), with edges where a lane continues, a branch forks, and a merge collapses — identical to what the commit-graph rail would draw for those same commits, since the same lane layout produces it. Each row SHALL show the commit subject, and ref decorations (local branch heads, remote branch heads, tags, HEAD) on the day's commits SHALL be rendered. A commit whose parent predates the current day SHALL appear as a lane root (its off-day parent is simply absent from the graph). The plot SHALL NOT impose a stylized abstraction (no plant, no trunk, no time-of-day height) — it is the real graph, scoped to today.

#### Scenario: Graph matches git's own view

- **WHEN** a plot renders a repository's today commits
- **THEN** its nodes, lanes, branch/merge topology, and refs correspond to what `git log --graph` would show for those commits

#### Scenario: A branch forks a lane and a merge collapses it

- **WHEN** the day's history contains a branch that diverged and later merged
- **THEN** the plot shows the divergence as a second lane and the merge as that lane collapsing back

#### Scenario: Decorations and subjects are shown

- **WHEN** a today commit is a branch head, a tag target, or HEAD
- **THEN** the plot renders the corresponding decoration on that commit's row alongside its subject

#### Scenario: Only the current day is shown

- **WHEN** a repository has commits from earlier days and from the current day
- **THEN** the plot shows only the current day's commits, and a commit whose parent predates the day is a lane root

### Requirement: Live, Today-Scoped Updates

The plots SHALL reflect new commits within the watcher's debounce window, driven by the existing graph-changed signal, while the Dashboard is the active surface. The plots SHALL be re-derived for the current local day on render, on graph changes, on window focus, and at the local midnight boundary, so a Dashboard left open or backgrounded across midnight re-scopes to the new day without user action. The section SHALL persist no new state to disk.

#### Scenario: A new commit appears

- **WHEN** the Dashboard is the active surface
- **AND** a new commit is created in a registered repository
- **THEN** the corresponding plot shows the new commit within the debounce window

#### Scenario: Re-scope at midnight

- **WHEN** the Dashboard is left open or backgrounded across the local midnight boundary
- **THEN** the plots re-scope to the new day without user action, on the midnight tick or on the next render/focus

#### Scenario: No new persisted state

- **WHEN** the plots render and update over a day
- **THEN** no new file is created under the application's data directory or any workspace's `openspec/` tree for the garden

### Requirement: Dormant and Degraded States

A top-level entry with no commits on the current local day SHALL be **omitted**
from the commit-garden section rather than rendering a placeholder. A non-git
(flat) workspace, and any entry whose repository cannot be read because the
`git` binary is unavailable, SHALL likewise be omitted. When **every** registered
entry is dormant in this sense (quiet, non-git, or git-unavailable), the entire
commit-garden section SHALL be omitted, consistent with the empty-registry rule,
rather than rendering an empty area, a lonely heading, or an error. The section
SHALL NOT error when git is absent, and the rest of the Dashboard SHALL continue
to function.

#### Scenario: A quiet workspace is omitted

- **WHEN** a registered repository received no commits on the current local day
- **AND** at least one other registered entry has commits today
- **THEN** the quiet repository's plot is omitted from the section rather than
  shown as a "quiet today" placeholder
- **AND** the entries with commits today are still rendered

#### Scenario: Non-git workspace is omitted

- **WHEN** a registered workspace is not inside a git repository
- **THEN** its plot is omitted from the section

#### Scenario: Every entry quiet omits the section

- **WHEN** no registered entry has any commits on the current local day
- **THEN** the commit-garden section is omitted entirely rather than rendering a
  section of placeholders or a heading with no plots

#### Scenario: Git binary missing

- **WHEN** the `git` binary is not on PATH
- **THEN** every entry is dormant, so the commit-garden section is omitted
- **AND** the rest of the Dashboard continues to function

### Requirement: Overflow Scrolls Horizontally

When a day's concurrently-alive lanes exceed the plot's graph gutter, the graph region SHALL scroll horizontally without scrolling the commit subject out of view, consistent with the commit-graph rail's overflow behaviour, rather than widening the Dashboard.

#### Scenario: Wide day-graph scrolls without losing the subject

- **WHEN** a plot's concurrently-alive lanes exceed the gutter width
- **THEN** the graph region scrolls horizontally to reveal the additional lanes
- **AND** the commit subjects remain visible

### Requirement: Read-Only Graphs

The commit-garden section SHALL expose no operation that mutates a repository, a workspace, or any spec or task state, and SHALL offer no commit selection or commit-detail navigation from the Dashboard. Hovering a node or row MAY surface that commit's author, local time, and subject as the only metadata affordance.

#### Scenario: No mutating actions are offered

- **WHEN** the user interacts with the commit-garden section
- **THEN** no action that edits a spec, toggles a task, or mutates git or workspace state is available

#### Scenario: Nodes are not a navigation target

- **WHEN** the user clicks a node or row
- **THEN** no commit is selected and the center pane is unchanged

#### Scenario: Hover reveals commit metadata

- **WHEN** the user hovers a node or row
- **THEN** the commit's author, local time, and subject are surfaced

### Requirement: Person-Colored Graph Nodes

Each node SHALL be coloured by the **person** who authored its commit, resolved through the named-people roster: the canonical developer first (you-precedence), then the roster fold, else the raw git author. The canonical developer's nodes SHALL be visually distinguished with the application accent; every other person SHALL receive a stable, locally-derived hue keyed on their primary identity. A commit whose author is missing or empty SHALL fall back to an `Unknown` raw author. This resolution SHALL be presentational and computed at query time — it SHALL NOT modify any stored event. Colours SHALL be derived locally with no network request.

#### Scenario: Node colored by its committer

- **WHEN** commits by two different people landed on the current day
- **THEN** their nodes carry the two people's distinct colours

#### Scenario: Folded identities share one color

- **WHEN** two git identities are folded onto a single named person on the roster
- **THEN** that person's nodes all carry one colour rather than splitting by identity

#### Scenario: The developer's nodes are distinguished

- **WHEN** the canonical developer authored a commit on the current day
- **THEN** that node is coloured with the application accent

#### Scenario: An authorless commit falls back to Unknown

- **WHEN** a commit has a missing or empty author
- **THEN** its node is attributed to `Unknown` rather than dropped

#### Scenario: Coloring does not rewrite the log

- **WHEN** authors are named or merged on the roster
- **THEN** no stored activity-log event is modified
- **AND** the Dashboard's personal-frame counts are unchanged

