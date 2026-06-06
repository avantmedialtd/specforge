# activity-log

## MODIFIED Requirements

### Requirement: Activity Event Log

The system SHALL maintain an append-only activity log of observed achievement events. Each event SHALL record its type — task completed, artifact reached, change created, or change archived — together with a timestamp and, where applicable, the owning workspace and change identifier. Each event SHALL additionally record the **author** it was observed with, as a raw `(name, email)` identity in which either component MAY be absent. The author SHALL be stored verbatim rather than pre-resolved to a particular developer, so that attribution to the canonical developer is determined at query time against the current identity configuration; an event recorded before authorship was captured (an author-less event) SHALL be treated as the local developer's. The log SHALL be append-only: recorded events SHALL NOT be retroactively removed or rewritten. The activity log SHALL be the source of truth for the Dashboard's today, streak, heatmap, and milestone views.

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

### Requirement: Achievement Detection from Watcher Re-Parses

When the watcher processes a debounced batch and re-parses a workspace, the system SHALL compare the new parse against the previously cached state and record an achievement event for each net-positive transition: an increase in a change's completed-task count (recorded with the increase as its magnitude), an artifact reaching a new status, a change newly appearing, and a change moving to archived. A decrease — a task being unchecked or a task line being removed — SHALL NOT produce a negative or spurious event.

A change that has left the active set SHALL be classified as **moved to archived** (as opposed to deleted) when a directory corresponding to its logical id `<id>` exists under `openspec/changes/archive/` — recognised under either the bare name `<id>` or the conventional dated form `<YYYY-MM-DD>-<id>` produced by the archive tooling. The dated form SHALL be matched by stripping a leading `YYYY-MM-DD-` prefix and comparing the remainder to `<id>` exactly, so an unrelated archive entry does not match. A change that has left the active set with no such archive directory SHALL be classified as deleted and SHALL NOT record an archival achievement. This classification governs both the recorded archival achievement and the archive-transition cache event the watcher emits, and so SHALL hold for flat (non-git) workspaces that rely on live capture, not only for git-backed workspaces.

A live achievement recorded from a re-parse SHALL be attributed to the watched repository's **local git identity** (`git config user.name` / `user.email`, read repository-local with the usual global fallback). A workspace with no resolvable git identity SHALL record an author-less event. The known consequence — a change that arrived from another developer via a pull during the watch window being attributed to the local identity — is accepted for live events, because the corresponding historical event is attributed correctly by the git backfill.

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

#### Scenario: Archival detection recognises the dated archive directory

- **WHEN** a re-parse shows active change `<id>` is gone and a directory named `<YYYY-MM-DD>-<id>` now exists under `openspec/changes/archive/`
- **THEN** the change is classified as archived
- **AND** a change-archived achievement is recorded

#### Scenario: Removal with no archive directory is a deletion

- **WHEN** a re-parse shows active change `<id>` is gone and no directory named `<id>` or `<YYYY-MM-DD>-<id>` exists under `openspec/changes/archive/`
- **THEN** the change is classified as deleted
- **AND** no archival achievement is recorded

#### Scenario: A live achievement is attributed to the local git identity

- **WHEN** a live achievement is recorded from a re-parse of a git-backed workspace
- **THEN** the recorded event's author is the watched repository's local git identity

#### Scenario: A flat workspace records an author-less live event

- **WHEN** a live achievement is recorded from a workspace with no resolvable git identity
- **THEN** the recorded event carries no author
- **AND** it resolves as the local developer's activity

### Requirement: Git Backfill of Historical Achievements

On first observation of a git-backed workspace, or when its activity log holds no prior history, the system SHALL backfill achievement events from git history over a bounded window: change creation and archival dates from the earliest commit adding files under `openspec/changes/<id>/` and `openspec/changes/archive/<id>/`, commit activity from the commit log, and task completions by diffing `tasks.md` checkbox state across commits. Backfilled achievements SHALL be attributed to the **author of the commit** they are reconstructed from (`%an` / `%ae`) rather than discarding it, so that history shared with other developers is attributed to whoever performed the work. Backfill SHALL be bounded and SHALL NOT read a repository's entire history. A non-git (flat) workspace SHALL contribute no backfilled history.

#### Scenario: Backfill populates history for a git-backed workspace

- **WHEN** a git-backed workspace is observed and its activity log has no prior history
- **THEN** achievement events are reconstructed from git history within the bounded window

#### Scenario: Backfilled achievements carry their commit author

- **WHEN** an achievement is reconstructed from a commit during backfill
- **THEN** the event's author is that commit's author identity

#### Scenario: Backfill is bounded

- **WHEN** a repository contains history older than the backfill window
- **THEN** events outside the window are not reconstructed
- **AND** the backfill does not read the repository's entire history

#### Scenario: Non-git workspaces contribute no backfill

- **WHEN** a registered workspace is not inside a git repository
- **THEN** no events are backfilled for it
- **AND** it relies on live capture going forward
