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

