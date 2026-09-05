## MODIFIED Requirements

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

## REMOVED Requirements

### Requirement: Named People Roster

**Reason**: The roster existed to make the per-author leaderboard read well, and the leaderboard is removed (`dashboard`: *Per-Author Leaderboard*). Its only other consumer — the commit garden's node colouring — used it to fold a person's several git identities into one colour bucket and to produce a display label that no frontend ever rendered. Neither justifies a persisted settings field, two commands across the whole command surface, and a Settings section. SpecForge now attributes work to the canonical developer only; every other author is presented by their raw git identity, as specified by `commit-garden`'s *Author-Colored Graph Nodes*.

### Requirement: Single Identity Assignment with Canonical-Developer Precedence

**Reason**: With no roster there is at most one person, so every clause is vacuous and both scenarios are unstatable. The half of this requirement that still has force — that an identity resolving as the canonical developer wins over any other attribution — is re-anchored in `commit-garden`'s *Author-Colored Graph Nodes*, which is the surviving consumer of you-precedence. It is restated there rather than merely deleted, because this requirement was the spec tree's only normative statement of that rule.
