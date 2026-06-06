# activity-log

## MODIFIED Requirements

### Requirement: Bounded, Time-Bucketed Queries

The activity log SHALL support querying events bucketed by local calendar day over a bounded window for the Dashboard's heatmap and today views, consistent with the commit-graph rail's day grouping. The log SHALL additionally support querying events over a **calendar-month (season) window** in the viewer's local time zone, sufficient — together with the existing commit mining — to derive a season's weighted score and the progress of its objectives, **without recording any new event kind**. Cumulative totals sufficient to evaluate milestone thresholds and the permanent career tier SHALL be derivable from the log.

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
