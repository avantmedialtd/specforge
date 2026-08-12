## ADDED Requirements

### Requirement: Bounded, Per-Day Queries

The activity log SHALL support querying events bucketed by local calendar day over a bounded window for the Dashboard's heatmap and today views, consistent with the commit-graph rail's day grouping. The log SHALL NOT be required to support any wider or differently-shaped window query, and no derived view SHALL cause a new event kind to be recorded.

#### Scenario: Queries return per-day buckets in local time

- **WHEN** the Dashboard requests activity for its window
- **THEN** events are bucketed by the viewer's local calendar day

#### Scenario: The query window is bounded

- **WHEN** the log contains events older than the requested window
- **THEN** the query returns only events within the bounded window

#### Scenario: Derived views introduce no new event kind

- **WHEN** the Dashboard's progress layer is derived from the log
- **THEN** it is computed from the existing event kinds and the existing commit mining
- **AND** no new event kind is introduced into the log

## REMOVED Requirements

### Requirement: Bounded, Time-Bucketed Queries

**Reason**: Replaced by *Bounded, Per-Day Queries*, which drops the calendar-month (season) window query and the career-totals derivability clause. Renamed rather than modified in place because its two season scenarios must disappear, and `openspec archive` rejects a MODIFIED block that drops a scenario present in the current spec.
