# dashboard

## ADDED Requirements

### Requirement: Activity Scope Selection (Me / Everyone)

The Dashboard SHALL provide an activity **scope** control with two scopes, *Me* and *Everyone*, defaulting to *Me*, with both scopes always reachable (the team view SHALL NOT be removed). The scope SHALL qualify the "across all registered workspaces" aggregation of the gamified, activity-log-derived views — specifically the *Today's Progress Hero*, *Streak and Contribution Heatmap*, and *Milestones and Badges* requirements — by selecting which achievements are counted:

- under **Me**, only achievements whose recorded author resolves to the canonical developer (per the `developer-identity` capability's query-time resolution, with author-less legacy events counted as the developer's) SHALL be counted;
- under **Everyone**, all achievements SHALL be counted, reproducing the pre-scope behaviour.

The in-flight (active-change) count of the Today's Progress band SHALL likewise be scope-aware: under **Me** it SHALL count active changes attributed to the developer (by the change's creation author), and under **Everyone** it SHALL count all active changes. Switching scope SHALL recompute these views without re-running the git history mining, which is shared across scopes.

#### Scenario: Default scope is Me

- **WHEN** the Dashboard first renders
- **THEN** the activity scope is *Me*
- **AND** an *Everyone* scope remains selectable

#### Scenario: Me scope counts only the developer's achievements

- **WHEN** the scope is *Me*
- **AND** the activity log holds achievements by the developer and by other authors
- **THEN** the today, streak, heatmap, and milestone views count only the achievements resolving to the developer

#### Scenario: Everyone scope counts all achievements

- **WHEN** the scope is switched to *Everyone*
- **THEN** the same views count every recorded achievement regardless of author

#### Scenario: Adding an alias moves activity into the Me scope

- **WHEN** activity recorded under an identity not yet claimed is excluded from the *Me* scope
- **AND** that identity is added as an alias of the developer
- **THEN** the *Me* scope subsequently counts that activity, without the log being rewritten

#### Scenario: Switching scope does not re-mine git

- **WHEN** the user switches between *Me* and *Everyone*
- **THEN** the scoped views recompute from the in-memory activity log
- **AND** the git history mining is not re-run for the switch

### Requirement: Developer Profile Surface

The Dashboard SHALL present a developer **profile** surface identifying the canonical developer by the display name from the identity configuration and by an **avatar**. The avatar SHALL be generated locally as a deterministic identicon derived from the developer's normalised identity key, tinted from the application's existing token palette, and SHALL NOT be fetched over the network or transmit identity data off the machine. The profile surface SHALL present the developer's *Me*-scoped streak and earned milestones as a personal highlight reel, retaining the encouraging zero state when the developer has no recorded activity.

#### Scenario: Profile shows the developer's name and a local avatar

- **WHEN** the profile surface renders with an identity configured
- **THEN** it shows the canonical display name
- **AND** it shows a locally-generated identicon avatar derived from the developer's identity, with no network request

#### Scenario: Profile reflects the developer's own activity

- **WHEN** the profile surface renders
- **THEN** the streak and earned milestones it shows are computed over the *Me*-scoped achievements

#### Scenario: Empty profile is encouraging

- **WHEN** the developer has no recorded *Me*-scoped activity
- **THEN** the profile renders an encouraging zero state rather than an error or a discouraging empty board

### Requirement: Per-Author Leaderboard for Shared Repositories

The Dashboard SHALL present a per-author **leaderboard** ranking authors by their shipped changes, completed tasks, and commits over the Dashboard's bounded window, derived from the authored achievements and commit authorship. The leaderboard SHALL render only for history that holds **more than one distinct author**; for a repository (or an aggregate) whose recorded history has a single author, the leaderboard SHALL be omitted rather than shown as a list of one. The local developer's row SHALL include the developer's live activity in addition to their commit-authored history. The leaderboard SHALL be read-only and computed locally; selecting it SHALL NOT mutate any workspace or git state.

#### Scenario: Leaderboard appears for a multi-author repository

- **WHEN** a registered repository's recorded history holds more than one distinct author
- **THEN** the Dashboard shows a leaderboard ranking those authors by shipped changes, completed tasks, and commits over the window

#### Scenario: Leaderboard is omitted for a solo repository

- **WHEN** all recorded history resolves to a single author
- **THEN** no leaderboard is shown

#### Scenario: The developer's row includes live activity

- **WHEN** the leaderboard renders and the developer has recorded live achievements
- **THEN** the developer's row reflects both their commit-authored history and their live activity

#### Scenario: Leaderboard does not mutate state

- **WHEN** the user interacts with the leaderboard
- **THEN** no spec file, workspace, or git state is modified
