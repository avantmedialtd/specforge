## ADDED Requirements

### Requirement: Activity Event Log

The system SHALL maintain an append-only activity log of observed achievement events. Each event SHALL record its type — task completed, artifact reached, change created, or change archived — together with a timestamp and, where applicable, the owning workspace and change identifier. The log SHALL be append-only: recorded events SHALL NOT be retroactively removed or rewritten. The activity log SHALL be the source of truth for the Dashboard's today, streak, heatmap, and milestone views.

#### Scenario: An achievement is recorded

- **WHEN** an achievement is observed
- **THEN** an event is appended recording its type, timestamp, and (where applicable) its workspace and change identifier

#### Scenario: The log is append-only

- **WHEN** workspace state later changes in a way that would lower a previously recorded total
- **THEN** prior events are not removed or altered

### Requirement: Achievement Detection from Watcher Re-Parses

When the watcher processes a debounced batch and re-parses a workspace, the system SHALL compare the new parse against the previously cached state and record an achievement event for each net-positive transition: an increase in a change's completed-task count (recorded with the increase as its magnitude), an artifact reaching a new status, a change newly appearing, and a change moving to archived. A decrease — a task being unchecked or a task line being removed — SHALL NOT produce a negative or spurious event.

#### Scenario: A completed-task increase records a task-completed achievement

- **WHEN** a re-parse shows a change's completed-task count increased by N
- **THEN** a task-completed achievement is recorded with magnitude N

#### Scenario: Unchecking a task records nothing

- **WHEN** a re-parse shows a change's completed-task count decreased
- **THEN** no achievement event is recorded for that change's task count

#### Scenario: A new change records a created achievement

- **WHEN** a re-parse shows a change that was not present before
- **THEN** a change-created achievement is recorded

#### Scenario: An archival records a shipped achievement

- **WHEN** a re-parse shows a change moved to the archive
- **THEN** a change-archived achievement is recorded

### Requirement: Git Backfill of Historical Achievements

On first observation of a git-backed workspace, or when its activity log holds no prior history, the system SHALL backfill achievement events from git history over a bounded window: change creation and archival dates from the earliest commit adding files under `openspec/changes/<id>/` and `openspec/changes/archive/<id>/`, commit activity from the commit log, and task completions by diffing `tasks.md` checkbox state across commits. Backfill SHALL be bounded and SHALL NOT read a repository's entire history. A non-git (flat) workspace SHALL contribute no backfilled history.

#### Scenario: Backfill populates history for a git-backed workspace

- **WHEN** a git-backed workspace is observed and its activity log has no prior history
- **THEN** achievement events are reconstructed from git history within the bounded window

#### Scenario: Backfill is bounded

- **WHEN** a repository contains history older than the backfill window
- **THEN** events outside the window are not reconstructed
- **AND** the backfill does not read the repository's entire history

#### Scenario: Non-git workspaces contribute no backfill

- **WHEN** a registered workspace is not inside a git repository
- **THEN** no events are backfilled for it
- **AND** it relies on live capture going forward

### Requirement: App-Data Persistence and Workspace Read-Only Guarantee

The activity log SHALL be persisted in the application's data directory, alongside the application's other settings, and SHALL NOT be written inside any registered workspace's `openspec/` tree or any workspace file. Recording achievements and backfilling history SHALL NOT mutate workspace state and SHALL NOT run any git operation that changes history or working-tree state.

#### Scenario: The log persists across restarts

- **WHEN** the application is restarted
- **THEN** previously recorded achievements remain available

#### Scenario: The log is never written into a workspace

- **WHEN** an achievement is recorded or history is backfilled
- **THEN** no file under any workspace's `openspec/` tree is created or modified
- **AND** no git operation that changes history or working-tree state is run

### Requirement: Bounded, Time-Bucketed Queries

The activity log SHALL support querying events bucketed by local calendar day over a bounded window for the Dashboard's heatmap and today views, consistent with the commit-graph rail's day grouping. Cumulative totals sufficient to evaluate milestone thresholds SHALL be derivable from the log.

#### Scenario: Queries return per-day buckets in local time

- **WHEN** the Dashboard requests activity for its window
- **THEN** events are bucketed by the viewer's local calendar day

#### Scenario: The query window is bounded

- **WHEN** the log contains events older than the requested window
- **THEN** the query returns only events within the bounded window
