## MODIFIED Requirements

### Requirement: Local Identity Resolution from Git Config

The system SHALL resolve the local developer identity from `git config user.name` and `user.email`, read per registered git-backed workspace (repository-local configuration with the usual global fallback). Workspaces that are worktrees of the same git repository SHALL share one read, because they share that repository's configuration. A per-worktree identity configured through `extensions.worktreeConfig` is therefore not distinguished from its repository's. The resolved identity is a `(name, email)` pair in which either component MAY be absent. When `git` is unavailable, or a workspace is not inside a git repository, or neither value is configured, the system SHALL resolve no identity for that workspace's events, so a live event recorded there is author-less (see the `activity-log` capability), and SHALL NOT error. The system SHALL be able to enumerate the distinct candidate identities observed across all registered workspaces, for seeding the identity configuration and for surfacing suggestions to the user. Enumeration reads every folder through git, including a folder outside any repository, where git applies only the global and system configuration. Such a folder therefore contributes at most the global identity as a candidate, which is how an installation with only non-git workspaces is seeded.

#### Scenario: Identity resolved from a git-backed workspace

- **WHEN** a registered workspace is inside a git repository with `user.name` and `user.email` configured
- **THEN** the local identity for that workspace resolves to that name and email

#### Scenario: No identity outside git

- **WHEN** a registered workspace is not inside a git repository, or `git` is not available
- **THEN** no local identity is resolved for that workspace's events
- **AND** no error is raised

#### Scenario: Distinct candidates enumerated across workspaces

- **WHEN** two registered git-backed workspaces in different repositories are configured with different git identities
- **THEN** enumerating candidate identities returns both distinct identities

## ADDED Requirements

### Requirement: Bounded Candidate Identity Detection

Enumerating candidate identities SHALL read git configuration at most once per distinct git repository among the registered and discovered workspaces. It SHALL read each workspace that is not inside a git repository on its own. The number of git invocations is bounded by repositories, not by worktrees:

$$spawns \le 2 \cdot (repositories + flat\ workspaces)$$

The enumeration SHALL keep its first-seen order and its dedupe by normalised author key. The candidates it returns SHALL be those the per-workspace sweep returned, except for the per-worktree configuration that *Local Identity Resolution from Git Config* now shares per repository.

Enumeration SHALL NOT run on the desktop's main (UI) thread, and SHALL NOT run inline on an async runtime worker on any transport. It runs on a blocking pool. Each enumeration requested for the identity settings SHALL read git afresh, not from a memo. An edit to the global git configuration is then reflected the next time those settings are opened.

Surfaces that show only the saved identity configuration, such as the Dashboard's profile band, SHALL read that configuration without enumerating candidates, so showing them spawns no git process.

First-run seeding SHALL take the first identity found in registration order and SHALL stop reading once it has one. The first-run seed is the identity written when no alias is configured. Seeding is the one read that MAY run during application start-up, before the first paint, so that the profile shows the seeded identity from the start. It is bounded by stopping early instead of by moving off the start-up path.

#### Scenario: Worktrees of one repository are read once

- **WHEN** a repository is registered with five tracked worktrees and candidate identities are enumerated
- **THEN** that repository's git identity is read once, not five times

#### Scenario: A flat workspace is still read

- **WHEN** a registered workspace is not inside a git repository and candidate identities are enumerated
- **THEN** that workspace is read on its own
- **AND** outside a repository git applies only the global and system configuration, so it contributes at most the global identity
- **AND** no error is raised

#### Scenario: Opening the Dashboard spawns no git process

- **WHEN** the Dashboard mounts and renders the developer's profile band
- **THEN** it reads the saved identity configuration only
- **AND** no `git config` invocation is made

#### Scenario: Identity settings read the global configuration afresh

- **WHEN** the user changes their global `user.email` while SpecForge is running
- **AND** then opens the identity settings
- **THEN** the candidate list reflects the new email

#### Scenario: Enumeration does not block the window

- **WHEN** the identity settings request the identity payload on the desktop
- **THEN** the git invocations run off the main thread
- **AND** the payload returned is the same saved configuration and candidate list as before, for the same configuration on disk

#### Scenario: First-run seeding stops at the first identity

- **WHEN** no alias is configured, and the first registered repository in registration order has a git identity
- **THEN** the seed is that identity
- **AND** no other repository's git configuration is read for seeding
