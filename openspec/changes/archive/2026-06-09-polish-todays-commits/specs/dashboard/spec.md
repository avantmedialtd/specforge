## MODIFIED Requirements

### Requirement: Today's Ships Quiet State

The today's ships feed SHALL, when no change has been archived on the viewer's local today, present a quiet-day note rather than hiding the feed or showing stale prior-day entries.

#### Scenario: Nothing shipped yet today

- **WHEN** no change is archived to a directory dated the viewer's local today
- **THEN** the today's ships feed shows a quiet-day note
- **AND** it does not show changes archived on earlier days
