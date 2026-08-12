## Purpose

Defines the commit garden: a section at the bottom of the Dashboard that renders, for each top-level registered entry (a repository group or a non-git flat workspace), a faithful git graph of that entry's commits for the viewer's current local calendar day — real lanes, nodes, edges, ref decorations, and subjects, exactly as the commit-graph rail would draw those same commits, with no stylized abstraction. Nodes are coloured by the person who authored each commit, resolved through the named-people roster (you-precedence, then the roster fold, else the raw git author), with the canonical developer accented. The garden updates live within the watcher's debounce window, re-scopes to the new day at the local midnight boundary, persists no new state, is an unconditional part of the Dashboard's progress layer, and is strictly read-only.

## ADDED Requirements

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

## MODIFIED Requirements

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

## REMOVED Requirements

### Requirement: Person-Colored Nodes

**Reason**: Replaced by *Person-Colored Graph Nodes*, which drops the clause and scenario asserting that roster resolution does not affect season scoring. Renamed rather than modified in place because that scenario must disappear, and `openspec archive` rejects a MODIFIED block that drops a scenario present in the current spec.
