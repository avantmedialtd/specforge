## MODIFIED Requirements

### Requirement: Change Lifecycle Metrics

The Dashboard SHALL present change-lifecycle metrics derived from git history: the throughput (the number of changes archived within the recent window) and the average time-to-archive (the mean elapsed time between a change's creation and its archival). A change's creation date SHALL be the date of the earliest commit that added a file under its `openspec/changes/<id>/` directory; its archive date SHALL be the date of the earliest commit that added a file under `openspec/changes/archive/<id>/`. Only changes for which both dates are recoverable from git SHALL contribute to the average time-to-archive.

Lifecycle data is derived from append-only history — once a change's creation and archival commits exist, their instants do not change — so the application SHALL derive a repository's lifecycle data **at most once per change to that repository's history**, rather than once per Dashboard fetch. A Dashboard fetch whose repositories' histories have not moved since the previous fetch SHALL issue no lifecycle-mining git invocation.

When a repository's history moves, the application SHALL re-derive that repository's lifecycle data and SHALL NOT re-derive any other repository's. The Dashboard SHALL reflect the moved history within the watcher's debounce window, so the freshness the *Reactive Dashboard Updates* requirement guarantees is unchanged.

Concurrent derivations for the same repository SHALL be collapsed into a single mining invocation, so that overlapping fetches cannot each issue their own.

The metrics SHALL be identical to those a per-fetch derivation would produce for the same history.

#### Scenario: Throughput counts recent archives

- **WHEN** three changes were archived within the window and others were archived earlier
- **THEN** the throughput metric reports the three changes archived within the window

#### Scenario: Average time-to-archive uses recoverable lifecycles

- **WHEN** a set of changes has both a creation commit and an archive commit recoverable from git
- **THEN** the average time-to-archive is the mean of each such change's archive-date-minus-creation-date
- **AND** changes whose creation or archive date cannot be recovered from git are excluded from the average

#### Scenario: No recoverable lifecycles

- **WHEN** no change has both a recoverable creation and archive date
- **THEN** the average time-to-archive renders as unavailable rather than as an error or a zero average

#### Scenario: An unchanged repository is not re-mined

- **WHEN** the Dashboard is fetched
- **AND** it is fetched again with no intervening change to any registered repository's history
- **THEN** the second fetch issues no lifecycle-mining git invocation
- **AND** its lifecycle metrics are identical to the first fetch's

#### Scenario: A commit re-mines only its own repository

- **WHEN** a commit is created in repository A
- **AND** repositories B and C are also registered
- **THEN** the next Dashboard fetch re-derives lifecycle data for A only
- **AND** issues no lifecycle-mining git invocation for B or C
- **AND** the resulting metrics reflect A's new history

#### Scenario: Concurrent fetches mine once

- **WHEN** two Dashboard fetches are issued concurrently for a repository whose lifecycle data is not yet derived
- **THEN** exactly one lifecycle-mining invocation is issued for that repository
- **AND** both fetches observe the same lifecycle data

### Requirement: Graceful Degradation Without Git

When the `git` binary is unavailable, or a registered workspace is not inside a git repository, the Dashboard SHALL still render its non-git sections (summary metrics, per-repository breakdown, today's ships feed) and SHALL render the git-derived sections (activity chart, lifecycle metrics) using only the data recoverable from the available git-backed repositories. The today's ships feed's membership is determined from the dated archive directory and SHALL render without git; only its per-entry relative archive time, which is git-derived, SHALL be omitted when git is unavailable. The Dashboard SHALL NOT error when git is absent.

A **failed** lifecycle derivation SHALL NOT be retained as though it were a result. Because a repository with no changes and a repository whose mining failed both yield no lifecycle data, the application SHALL distinguish the two and retain only successful derivations; a failed derivation SHALL be retried on a subsequent fetch rather than serving an empty lifecycle for the remainder of the session. A repository that genuinely has no changes is a successful derivation and MAY be retained.

#### Scenario: Git binary missing

- **WHEN** the `git` binary is not on PATH
- **THEN** the Dashboard renders its summary metrics, per-repository breakdown, and today's ships feed
- **AND** the today's ships entries render without their relative archive times
- **AND** the activity chart and lifecycle metrics render an empty or unavailable state rather than erroring

#### Scenario: Mixed git and non-git workspaces

- **WHEN** some registered workspaces are git-backed and others are flat
- **THEN** the summary metrics and breakdown include every workspace
- **AND** the activity chart and lifecycle metrics include only the git-backed repositories

#### Scenario: A transient mining failure is retried

- **WHEN** a repository's lifecycle derivation fails
- **AND** the Dashboard is fetched again
- **THEN** the derivation is retried for that repository
- **AND** a subsequent successful derivation is reflected in the lifecycle metrics

#### Scenario: A repository with no changes is retained

- **WHEN** a repository's lifecycle derivation succeeds and finds no changes
- **AND** the Dashboard is fetched again with that repository's history unmoved
- **THEN** no further lifecycle-mining invocation is issued for that repository
