## MODIFIED Requirements

### Requirement: Per-Author Leaderboard for Shared Repositories

The Dashboard SHALL present a per-author **leaderboard** ranking authors by their shipped changes, completed tasks, and commits over the Dashboard's bounded window, derived from the authored achievements and commit authorship. The leaderboard SHALL resolve each observed author through the named-people roster: identities folded onto one person SHALL be **combined into a single row**, summing their shipped changes, completed tasks, and commits, and labelled with that person's custom display name; an observed author not on the roster SHALL keep its raw git label. This roster resolution SHALL be presentational and computed at query time — it SHALL NOT modify any stored event and SHALL NOT affect season scoring, season naming, objectives, or any deterministic generation. The leaderboard SHALL render only for history that, **after roster resolution**, holds **more than one distinct author**; for a repository (or an aggregate) whose recorded history resolves to a single author, the leaderboard SHALL be omitted rather than shown as a list of one. The local developer's row SHALL include the developer's live activity in addition to their commit-authored history. The leaderboard SHALL be read-only and computed locally; selecting it SHALL NOT mutate any workspace or git state.

#### Scenario: Leaderboard appears for a multi-author repository

- **WHEN** a registered repository's recorded history holds more than one distinct author
- **THEN** the Dashboard shows a leaderboard ranking those authors by shipped changes, completed tasks, and commits over the window

#### Scenario: Leaderboard is omitted for a solo repository

- **WHEN** all recorded history resolves to a single author
- **THEN** no leaderboard is shown

#### Scenario: The developer's row includes live activity

- **WHEN** the leaderboard renders and the developer has recorded live achievements
- **THEN** the developer's row reflects both their commit-authored history and their live activity

#### Scenario: Folded identities form one summed, named row

- **WHEN** two of an author's git identities are folded onto a single named person on the roster
- **THEN** the leaderboard shows one row for that person, labelled with their custom display name
- **AND** that row sums the shipped changes, completed tasks, and commits of both identities rather than splitting them across two rows

#### Scenario: A custom name labels an author's row

- **WHEN** an observed author is given a custom display name on the roster
- **THEN** the leaderboard labels that author's row with the custom name rather than the raw git name or email

#### Scenario: Merging the only other author omits the leaderboard

- **WHEN** the sole author other than the developer is folded onto the developer
- **THEN** the history resolves to a single author and no leaderboard is shown

#### Scenario: Roster resolution does not affect season standing

- **WHEN** authors are named or merged on the roster
- **THEN** the developer's season score, band, tier, objectives, and equipped treatment are unchanged

#### Scenario: Leaderboard does not mutate state

- **WHEN** the user interacts with the leaderboard
- **THEN** no spec file, workspace, or git state is modified
