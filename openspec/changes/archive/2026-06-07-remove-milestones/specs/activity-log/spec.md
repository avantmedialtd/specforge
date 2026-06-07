## MODIFIED Requirements

### Requirement: Activity Event Log

The system SHALL maintain an append-only activity log of observed achievement events. Each event SHALL record its type — task completed, artifact reached, change created, or change archived — together with a timestamp and, where applicable, the owning workspace and change identifier. Each event SHALL additionally record the **author** it was observed with, as a raw `(name, email)` identity in which either component MAY be absent. The author SHALL be stored verbatim rather than pre-resolved to a particular developer, so that attribution to the canonical developer is determined at query time against the current identity configuration; an event recorded before authorship was captured (an author-less event) SHALL be treated as the local developer's. The log SHALL be append-only: recorded events SHALL NOT be retroactively removed or rewritten. The activity log SHALL be the source of truth for the Dashboard's today, streak, and heatmap views.

#### Scenario: An achievement is recorded

- **WHEN** an achievement is observed
- **THEN** an event is appended recording its type, timestamp, and (where applicable) its workspace and change identifier

#### Scenario: An achievement records its observed author

- **WHEN** an achievement is observed with a known author identity
- **THEN** the appended event records that author's raw `(name, email)` identity

#### Scenario: Author is stored raw, not pre-resolved

- **WHEN** an achievement's author is recorded
- **THEN** the event stores the observed identity verbatim
- **AND** whether that author is the canonical developer is decided at query time against the current identity configuration, not frozen into the event

#### Scenario: Author-less events are treated as the local developer

- **WHEN** an event was recorded before authorship was captured and carries no author
- **THEN** it resolves as the local developer's activity

#### Scenario: The log is append-only

- **WHEN** workspace state later changes in a way that would lower a previously recorded total
- **THEN** prior events are not removed or altered

### Requirement: Bounded, Time-Bucketed Queries

The activity log SHALL support querying events bucketed by local calendar day over a bounded window for the Dashboard's heatmap and today views, consistent with the commit-graph rail's day grouping. The log SHALL additionally support querying events over a **calendar-month (season) window** in the viewer's local time zone, sufficient — together with the existing commit mining — to derive a season's weighted score and the progress of its objectives, **without recording any new event kind**. Cumulative totals sufficient to evaluate the permanent career tier SHALL be derivable from the log.

#### Scenario: Queries return per-day buckets in local time

- **WHEN** the Dashboard requests activity for its window
- **THEN** events are bucketed by the viewer's local calendar day

#### Scenario: The query window is bounded

- **WHEN** the log contains events older than the requested window
- **THEN** the query returns only events within the bounded window

#### Scenario: Season-window queries are supported

- **WHEN** the Dashboard requests a season's activity
- **THEN** the log returns the events within that calendar-month window in the viewer's local time zone
- **AND** those events are sufficient, together with the commit mining, to derive the season's weighted score and objective progress

#### Scenario: Seasons introduce no new event kind

- **WHEN** season standings are derived
- **THEN** they are computed from the existing event kinds and the existing commit mining
- **AND** no new event kind is introduced into the log
