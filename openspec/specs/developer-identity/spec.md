# developer-identity Specification

## Purpose
TBD - created by archiving change add-developer-profile. Update Purpose after archive.
## Requirements
### Requirement: Local Identity Resolution from Git Config

The system SHALL resolve the local developer identity from `git config user.name` and `user.email`, read per registered git-backed workspace (repository-local configuration with the usual global fallback). The resolved identity is a `(name, email)` pair in which either component MAY be absent. When `git` is unavailable, or a workspace is not inside a git repository, or neither value is configured, the system SHALL resolve no identity for that workspace and SHALL NOT error. The system SHALL be able to enumerate the distinct candidate identities observed across all registered git-backed workspaces, for seeding the identity configuration and for surfacing suggestions to the user.

#### Scenario: Identity resolved from a git-backed workspace

- **WHEN** a registered workspace is inside a git repository with `user.name` and `user.email` configured
- **THEN** the local identity for that workspace resolves to that name and email

#### Scenario: No identity outside git

- **WHEN** a registered workspace is not inside a git repository, or `git` is not available
- **THEN** no local identity is resolved for that workspace
- **AND** no error is raised

#### Scenario: Distinct candidates enumerated across workspaces

- **WHEN** two registered git-backed workspaces are configured with different git identities
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

### Requirement: Named People Roster

The system SHALL maintain, alongside the canonical developer configuration, a roster of named people, each holding a custom display name and a set of git identities (`(name, email)` pairs) that all fold onto that person. A git identity recorded under a person SHALL resolve to that person, and SHALL be labelled with that person's display name, wherever per-author activity is presented. Resolution SHALL be evaluated against the **current** roster at query time, so naming a person or folding an identity onto them SHALL retroactively relabel and re-group previously recorded activity by those identities **without rewriting any stored event**. The roster SHALL be editable: people MAY be created and removed, renamed, and have identities added or removed. The roster SHALL be persisted alongside the application's other settings in the application's data directory, SHALL NOT be written inside any workspace's `openspec/` tree, and SHALL be computed and stored locally without transmitting identity data off the machine.

#### Scenario: Folded identities resolve to one named person

- **WHEN** two distinct git identities are folded onto a single roster person
- **THEN** both identities resolve to that person
- **AND** activity by either is labelled with that person's custom display name

#### Scenario: Naming is retroactive and does not rewrite events

- **WHEN** activity was previously recorded under an identity that is later folded onto a named person
- **THEN** that past activity resolves to the named person without the stored events being modified or removed

#### Scenario: Roster persists across restarts and stays local

- **WHEN** the application is restarted
- **THEN** the previously saved people and their identities remain available
- **AND** no file under any workspace's `openspec/` tree was written and no identity data was transmitted off the machine

### Requirement: Manual Identity Entry

The system SHALL allow a git identity to be recorded by explicit entry of a name and/or an email, not only by selecting an automatically detected candidate. Manual entry SHALL be available both for the canonical developer — adding further self-aliases — and for any person on the roster. An entry that yields no usable identity (neither a non-empty name nor a non-empty email) SHALL be rejected. The number of identities recorded for the canonical developer SHALL NOT be bounded above, subject to retaining at least one.

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

### Requirement: Single Identity Assignment with Canonical-Developer Precedence

Each git identity, by its normalised key, SHALL belong to at most one person across the whole roster, including the canonical developer. When an identity would otherwise belong to more than one person, the canonical developer SHALL take precedence: an identity that resolves as the canonical developer SHALL NOT also be attributed to a roster person. Recording an identity under one person SHALL remove it from any other person that previously held it.

#### Scenario: The canonical developer takes precedence

- **WHEN** the same identity is present both on a roster person and among the canonical developer's identities
- **THEN** it resolves as the canonical developer and is not attributed to the roster person

#### Scenario: Reassignment is exclusive

- **WHEN** an identity already held by one roster person is recorded under another person
- **THEN** it is removed from the first person and held only by the second

