## ADDED Requirements

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
