# commit-garden Specification

## Purpose

Defines the commit garden: a section at the bottom of the Dashboard that renders, for each top-level registered entry (a repository group or a non-git flat workspace), a faithful git graph of that entry's commits for the viewer's current local calendar day — real lanes, nodes, edges, ref decorations, and subjects, exactly as the commit-graph rail would draw those same commits, with no stylized abstraction. Nodes are coloured by the author of each commit, resolved with you-precedence (the canonical developer first, else the raw git author), with the canonical developer accented; two git identities of one teammate therefore receive two colours. The garden updates live within the watcher's debounce window, re-scopes to the new day at the local midnight boundary, persists no new state, is an unconditional part of the Dashboard's progress layer, and is strictly read-only.
## Requirements
### Requirement: Per-Workspace Commit Graphs at the Dashboard Bottom

The Dashboard SHALL present a commit-garden section at the **bottom** of its content — the final section, below the contribution heatmap — with one plot per top-level registered entry that has commits on the viewer's current local calendar day: a repository group or a non-git (flat) workspace, mirroring the one-entry-per-top-level-item rule the `dashboard` capability's *Cross-Workspace Summary Metrics* requirement applies to its retained data, so that multiple worktrees of one repository resolve to a single plot. Entries without commits today are omitted per the *Dormant and Degraded States* requirement. Plots SHALL be stacked vertically, each labelled with the same display name the Dashboard uses for that top-level entry, and ordered per the *Deterministic Plot Order* requirement. The section SHALL be an unconditional part of the Dashboard's progress layer and SHALL NOT be gated by any setting.

#### Scenario: One plot per top-level entry with commits today

- **WHEN** the section renders with two repository groups that received commits today, a third that did not, and one flat workspace registered
- **THEN** it shows two plots
- **AND** each plot is labelled with that entry's Dashboard display name

#### Scenario: Worktrees of one repository share a plot

- **WHEN** several registered workspaces are worktrees of the same git repository
- **THEN** the section shows a single plot for that repository rather than one per worktree

#### Scenario: Section sits at the bottom

- **WHEN** the Dashboard renders
- **THEN** the commit-garden section appears at the bottom of the Dashboard, below the contribution heatmap
- **AND** no analytics band is rendered above it

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

### Requirement: Author-Colored Graph Nodes

Each node SHALL be coloured by the **author** of its commit, resolved with you-precedence: an author that resolves as the canonical developer, per the `developer-identity` capability's query-time "is this me?" test, SHALL be treated as the developer, and every other author SHALL be keyed on their own normalised git author key. The canonical developer's nodes SHALL be visually distinguished with the application accent; every other author SHALL receive a stable, locally-derived hue keyed on that normalised author key.

It follows that two git identities of one teammate SHALL receive two colours, exactly as two unrelated authors would, and SHALL count as two in the distinct-author count the *Plot Caption* requirement specifies. This is the accepted consequence of resolving authors without a named-people roster: only the canonical developer's own identities fold together, and they fold through the developer's alias list rather than through any roster.

A commit whose author is missing or empty SHALL fall back to an `Unknown` raw author. This resolution SHALL be presentational and computed at query time — it SHALL NOT modify any stored event. Colours SHALL be derived locally with no network request.

#### Scenario: Node colored by its committer

- **WHEN** commits by two different authors landed on the current day
- **THEN** their nodes carry the two authors' distinct colours

#### Scenario: The developer's nodes are distinguished

- **WHEN** the canonical developer authored a commit on the current day
- **THEN** that node is coloured with the application accent

#### Scenario: The developer's aliases share the accent

- **WHEN** the developer authored today's commits under two identities, both recorded as aliases of the canonical developer
- **THEN** every one of those nodes carries the application accent

#### Scenario: One author's two identities receive two colours

- **WHEN** an author other than the canonical developer committed today under two different git identities
- **THEN** those nodes carry two distinct colours
- **AND** the section counts them as two distinct authors

#### Scenario: An authorless commit falls back to Unknown

- **WHEN** a commit has a missing or empty author
- **THEN** its node is attributed to `Unknown` rather than dropped

#### Scenario: Coloring does not rewrite the log

- **WHEN** the garden colours its nodes
- **THEN** no stored activity-log event is modified
### Requirement: Deterministic Plot Order

The commit-garden section SHALL order its plots by today's commit count descending, then by display label ascending, then by a stable per-entry key ascending — a repository's identity, or a flat workspace's URI.

All three keys are required. The commit count leads with the entry that moved most today. The label stops two equally busy entries trading places between refreshes. The per-entry key is what makes the ordering **total**: display labels carry no uniqueness guarantee — two worktrees of unrelated repositories can present the same basename, and two entries can be given the same display-name override — so without a third key two entries sharing a label would fall back to whatever order the registry emitted.

The ordering SHALL NOT depend on the entry's active-change count, its archived-change count, or its position in the registry, so that a change to any of those does not reorder the section.

The ordering SHALL be a pure function of the presented entries, so that the same set of entries yields the same order regardless of the order in which they were registered or returned.

This is an ordering of repositories, not of authors, and is therefore outside the prohibition in the `dashboard` capability's *Personal Progress Frame* requirement.

#### Scenario: The busiest entry leads

- **WHEN** one registered entry received four commits today and another received one
- **THEN** the entry with four commits is presented above the entry with one

#### Scenario: Equal commit counts are broken by label

- **WHEN** two registered entries received the same number of commits today
- **THEN** they are presented in ascending display-label order
- **AND** their relative order is unchanged by a subsequent refresh that does not change either count

#### Scenario: Registration order does not decide the order

- **WHEN** an entry registered later received more commits today than one registered earlier
- **THEN** the later-registered entry is presented first

#### Scenario: Active changes do not reorder the section

- **WHEN** two entries received the same number of commits today and hold different numbers of active changes
- **THEN** their order is decided by their display labels rather than by their active-change counts

#### Scenario: Entries sharing a display label are still ordered

- **WHEN** two registered entries present the same display label and received the same number of commits today
- **THEN** they are presented in ascending order of their stable per-entry keys
- **AND** their relative order does not depend on which was registered first

### Requirement: Plot Caption

Each plot SHALL carry a caption naming the entry and summarising its day: the entry's Dashboard display label, its count of commits today, its count of distinct authors today, and its count of active changes.

Both counts SHALL be suppressed at the floor at which they stop carrying information: the distinct-author count only when the day's commits carry more than one author, and the active-change count only when the entry holds at least one active change. The two segments SHALL follow the same rule, so the caption never states a count that cannot vary.

The active-change count SHALL be that entry's **registry-wide** count of active (non-archived) changes, the same figure the `dashboard` capability's *Cross-Workspace Summary Metrics* requirement retains per top-level item. It is a live state count, not a today-scoped one, and it is therefore not comparable with the today's-progress hero's in-flight count, which is scoped to the canonical developer.

The caption SHALL keep the entry's label legible when the available width cannot hold the whole caption: the label SHALL NOT be the element that collapses, since the commit garden is the only Dashboard surface that names these entries.

Every frontend that renders the commit garden SHALL present the same caption, per the `terminal-ui` capability's *In-Process Shared Application Service* requirement.

#### Scenario: Caption names the entry and its day

- **WHEN** a plot renders for an entry with commits today and at least one active change
- **THEN** its caption presents that entry's display label, its count of commits today, and its count of active changes

#### Scenario: Author count appears only when authors differ

- **WHEN** every one of an entry's commits today carries the same author
- **THEN** the caption does not present a distinct-author count

#### Scenario: Active count appears only when work is in flight

- **WHEN** an entry has commits today and holds no active changes
- **THEN** the caption does not present an active-change count
- **AND** it still presents the entry's label and its count of commits today

#### Scenario: Two identities count as two authors

- **WHEN** an author other than the canonical developer committed today under two different git identities
- **THEN** the caption counts them as two distinct authors

#### Scenario: The active count is registry-wide

- **WHEN** an entry holds active changes that the canonical developer did not create
- **THEN** the caption's active-change count includes them

#### Scenario: The terminal frontend presents the same caption

- **WHEN** the terminal frontend renders its commit-garden screen
- **THEN** each plot's caption presents the same label, commit count, author count and active-change count the desktop presents, under the same suppression rules

#### Scenario: A narrow pane does not collapse the entry's name

- **WHEN** the available width cannot hold a plot's whole caption
- **THEN** the entry's label remains legible rather than being the element truncated away

