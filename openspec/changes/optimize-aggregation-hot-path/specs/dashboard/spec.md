## MODIFIED Requirements

### Requirement: Reactive Dashboard Updates

While the Dashboard is the active center-pane surface, it SHALL reflect on-disk changes within the watcher's debounce window without user action. After the watcher finishes processing a debounced batch — a change added, a change archived, content edited within a tracked change, or a repository's refs changing — the Dashboard SHALL refresh its metrics to observe the post-batch state.

A single debounced batch SHALL cause **at most one** Dashboard refresh, however many distinct cache events that batch emits. The backend deliberately emits several events per batch (for example an archival emits a change-archived event, a generic update, and the derived logical/instance diff events), and the Dashboard subscribes to more than one of them; the Dashboard SHALL coalesce all events observed within the same event-loop turn into a single refetch rather than refetching per event.

While a refresh is in flight, a further event SHALL NOT start a second concurrent refetch; it SHALL instead cause exactly one follow-up refresh after the in-flight one settles, so that overlapping batches cannot accumulate outstanding requests.

#### Scenario: Dashboard updates when a change is added

- **WHEN** the Dashboard is the active surface
- **AND** a new change directory is created on disk in a registered workspace
- **THEN** the Dashboard's active-change count reflects the new change within the debounce window

#### Scenario: Dashboard updates when a change is archived

- **WHEN** the Dashboard is the active surface
- **AND** a change is moved to `openspec/changes/archive/` on disk
- **THEN** the Dashboard's active/archived counts and lifecycle metrics reflect the archival within the debounce window
- **AND** when the archive directory is dated the viewer's local today, the today's ships feed reflects it within the debounce window

#### Scenario: Dashboard updates on commit activity

- **WHEN** the Dashboard is the active surface
- **AND** a new commit is created in a registered git-backed repository
- **THEN** the activity chart reflects the new commit within the debounce window

#### Scenario: A multi-event batch refreshes the Dashboard once

- **WHEN** the Dashboard is the active surface
- **AND** a single debounced batch emits a change-archived event, a generic update event, and a derived logical-change event
- **THEN** the Dashboard issues exactly one refresh request for that batch

#### Scenario: Overlapping batches do not stack requests

- **WHEN** a Dashboard refresh is in flight
- **AND** a further batch emits cache events before it settles
- **THEN** no second concurrent refresh request is issued
- **AND** exactly one follow-up refresh runs after the in-flight one settles
