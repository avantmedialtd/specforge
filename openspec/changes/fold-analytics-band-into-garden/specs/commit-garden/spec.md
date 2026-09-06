## ADDED Requirements

### Requirement: Deterministic Plot Order

The commit-garden section SHALL order its plots by today's commit count descending, then by display label ascending. Both keys are required: the commit count leads with the entry that moved most today, and the label is what stops two entries with equal commit counts trading places between refreshes.

The ordering SHALL NOT depend on the entry's active-change count, its archived-change count, or its position in the registry, so that a change to any of those does not reorder the section.

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

### Requirement: Plot Caption

Each plot SHALL carry a caption naming the entry and summarising its day: the entry's Dashboard display label, its count of commits today, its count of distinct authors today, and its count of active changes.

The distinct-author count SHALL be presented only when the day's commits carry more than one author, so a solo day's caption does not state a count that cannot vary.

The active-change count SHALL be that entry's **registry-wide** count of active (non-archived) changes, the same figure the `dashboard` capability's *Cross-Workspace Summary Metrics* requirement retains per top-level item. It is a live state count, not a today-scoped one, and it is therefore not comparable with the today's-progress hero's in-flight count, which is scoped to the canonical developer.

#### Scenario: Caption names the entry and its day

- **WHEN** a plot renders for an entry with commits today
- **THEN** its caption presents that entry's display label, its count of commits today, and its count of active changes

#### Scenario: Author count appears only when authors differ

- **WHEN** every one of an entry's commits today carries the same author
- **THEN** the caption does not present a distinct-author count

#### Scenario: Two identities count as two authors

- **WHEN** an author other than the canonical developer committed today under two different git identities
- **THEN** the caption counts them as two distinct authors

#### Scenario: The active count is registry-wide

- **WHEN** an entry holds active changes that the canonical developer did not create
- **THEN** the caption's active-change count includes them

## MODIFIED Requirements

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
