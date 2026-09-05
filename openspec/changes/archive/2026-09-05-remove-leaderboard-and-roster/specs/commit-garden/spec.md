## ADDED Requirements

### Requirement: Author-Colored Graph Nodes

Each node SHALL be coloured by the **author** of its commit, resolved with you-precedence: an author that resolves as the canonical developer, per the `developer-identity` capability's query-time "is this me?" test, SHALL be treated as the developer, and every other author SHALL be keyed on their own normalised git author key. The canonical developer's nodes SHALL be visually distinguished with the application accent; every other author SHALL receive a stable, locally-derived hue keyed on that normalised author key.

It follows that two git identities of one teammate SHALL receive two colours, exactly as two unrelated authors would, and SHALL count as two in any distinct-author caption the section presents. This is the accepted consequence of resolving authors without a named-people roster: only the canonical developer's own identities fold together, and they fold through the developer's alias list rather than through any roster.

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

## REMOVED Requirements

### Requirement: Person-Colored Graph Nodes

**Reason**: Renamed to *Author-Colored Graph Nodes* (added above), which resolves each commit with you-precedence and then the raw normalised git author key, rather than through the named-people roster that is removed by this change (`developer-identity`: *Named People Roster*). Renamed rather than modified in place because the *Folded identities share one color* scenario must disappear, and `openspec archive` rejects a MODIFIED block that drops a scenario present in the current spec. The requirement name also carried roster vocabulary — after this change the garden colours by author, not by person. The *Coloring does not rewrite the log* scenario is re-pointed away from roster editing, and its second assertion (that the Dashboard's personal-frame counts are unchanged) is dropped rather than re-pointed at aliasing, because adding an alias *does* change those counts — see the *Claiming an alias folds activity into the developer's counts* scenario in the `dashboard` capability's *Personal Progress Frame* requirement.
