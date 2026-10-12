# developer-identity Specification

## Purpose

Defines who SpecForge attributes work to: the canonical developer ("me"), folding one or more git `(name, email)` identities — detected from `git config` across the registered workspaces, or entered by hand — onto a single display name. Resolution is a pure, query-time fold over the *current* configuration (the normalised author key and the "is this me?" test), so adding an alias retroactively reattributes past activity without rewriting a single stored record; the identity config persists alongside the application's other settings and is read and edited through the `get_identity`, `set_display_name`, and `set_identity_aliases` commands, never written inside a workspace's `openspec/` tree and never transmitted off the machine. SpecForge attributes work to no *other* named person: an author who is not the canonical developer is presented by their raw git identity, with no naming or merging affordance in any frontend. It owns only the mapping, not the data or its display — the `activity-log` capability owns the raw author recorded verbatim on each event, and the `dashboard` and `commit-garden` capabilities own the profile and node colouring that render the resolved identity.
## Requirements
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

### Requirement: Identity Aliases Fold Onto One Canonical Developer

The system SHALL maintain an identity configuration holding a canonical display name and a set of alias identities, each a `(name, email)` pair, that all resolve to the same canonical developer ("me"). A developer who acts under several git identities — multiple email addresses (for example work, personal, and `noreply` addresses) or name variants — SHALL be able to fold them onto the one canonical developer by recording them as aliases. The configuration SHALL be editable: aliases MAY be added and removed, and the canonical display name MAY be set, without altering any previously recorded activity.

#### Scenario: Multiple identities fold onto one developer

- **WHEN** two git identities differing only by email are both recorded as aliases of the canonical developer
- **THEN** both identities resolve to the same canonical developer

#### Scenario: Display name is independent of git identity

- **WHEN** the user sets a canonical display name
- **THEN** that display name labels the canonical developer regardless of the name component of any underlying git identity

#### Scenario: Editing aliases does not rewrite activity

- **WHEN** an alias is added or removed
- **THEN** previously recorded activity is not modified or removed

### Requirement: Author Key and Query-Time "Me" Resolution

The system SHALL derive a normalised author key from an observed identity: the lowercased, trimmed email when an email is present, otherwise the lowercased, trimmed name. The system SHALL provide a pure resolution — *is this observed author the canonical developer?* — that compares the observed author's normalised key against the normalised keys of the canonical identity and every alias, where an email-bearing author matches only an identity with the same email and a name-only author matches only a name-only identity. Because this resolution is evaluated against the **current** configuration rather than baked into stored events, adding an alias SHALL retroactively cause previously recorded activity by that identity to resolve as the canonical developer.

#### Scenario: Author key prefers email

- **WHEN** an observed identity has both a name and an email
- **THEN** its normalised author key is derived from the lowercased, trimmed email

#### Scenario: An observed author is resolved as the canonical developer

- **WHEN** an observed author's normalised key matches the canonical identity or one of its aliases
- **THEN** the author resolves as the canonical developer

#### Scenario: Adding an alias retroactively reclaims past activity

- **WHEN** activity was previously recorded under an identity that was not yet an alias
- **AND** that identity is subsequently added as an alias
- **THEN** the previously recorded activity resolves as the canonical developer without the records being rewritten

### Requirement: Identity Configuration Persistence and Workspace Read-Only Guarantee

The identity configuration SHALL be persisted in the application's data directory, alongside the application's other settings, and SHALL NOT be written inside any registered workspace's `openspec/` tree or any workspace file. Resolving identities and reading git configuration SHALL NOT mutate workspace state and SHALL NOT run any git operation that changes history or working-tree state. Identity resolution and any avatar derived from an identity SHALL be computed locally and SHALL NOT transmit identity data off the machine.

#### Scenario: Configuration persists across restarts

- **WHEN** the application is restarted
- **THEN** the previously saved canonical display name and aliases remain available

#### Scenario: Configuration is never written into a workspace

- **WHEN** the identity configuration is saved
- **THEN** no file under any workspace's `openspec/` tree is created or modified
- **AND** no git operation that changes history or working-tree state is run

#### Scenario: Identity data stays local

- **WHEN** an identity is resolved or an avatar is derived from it
- **THEN** the computation completes without transmitting identity data off the machine

### Requirement: Manual Identity Entry

The system SHALL allow a git identity to be recorded by explicit entry of a name and/or an email, not only by selecting an automatically detected candidate. Manual entry SHALL be available for the canonical developer, adding further self-aliases. An entry that yields no usable identity (neither a non-empty name nor a non-empty email) SHALL be rejected. The number of identities recorded for the canonical developer SHALL NOT be bounded above, subject to retaining at least one.

#### Scenario: A self-alias is added by hand

- **WHEN** the developer enters an additional name and/or email that the application did not auto-detect from any workspace
- **THEN** it is recorded as one of the developer's identities
- **AND** activity under it thereafter resolves as the canonical developer

#### Scenario: Multiple self-aliases all resolve as the developer

- **WHEN** the developer records several identities for themselves
- **THEN** every one of them resolves as the canonical developer

#### Scenario: An empty entry is rejected

- **WHEN** an entry has neither a non-empty name nor a non-empty email
- **THEN** it is not recorded

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

