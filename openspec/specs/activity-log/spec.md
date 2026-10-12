# activity-log Specification

## Purpose

Defines the append-only achievement log behind SpecForge's progress views: task completions, artifact statuses, change creations, and archivals observed live when the watcher re-parses a registered workspace, plus a bounded backfill reconstructed from git history, each event stamped with the raw `(name, email)` author it was seen with rather than a pre-resolved developer. The log persists in the application's data directory as `activity.jsonl`, one event per line and written only by the instance holding the persistence lease — never inside a workspace's `openspec/` tree, and never via a git operation that mutates history or the working tree — and answers bounded queries bucketed by the viewer's local calendar day. It owns recording, storage, and querying only: the Dashboard surfaces that read it (today, streak, heatmap) are specified by `dashboard`, and the rules that fold a raw author onto a canonical developer by `developer-identity`.
## Requirements
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

### Requirement: App-Data Persistence and Workspace Read-Only Guarantee

The activity log SHALL be persisted in the application's data directory, alongside the application's other settings, and SHALL NOT be written inside any registered workspace's `openspec/` tree or any workspace file. Recording achievements and backfilling history SHALL NOT mutate workspace state and SHALL NOT run any git operation that changes history or working-tree state.

Persistence is performed by the one instance that holds the persistence lock (*Single Persisting Instance*). An achievement that the persisting instance also observes SHALL remain available across restarts. An achievement observed only by an instance that does not hold the lock, for example in a workspace registered only in that instance's session, SHALL remain available for that instance's session and MAY be absent after a restart.

#### Scenario: The log persists across restarts

- **WHEN** the application is restarted
- **THEN** previously recorded achievements remain available

#### Scenario: The log is never written into a workspace

- **WHEN** an achievement is recorded or history is backfilled
- **THEN** no file under any workspace's `openspec/` tree is created or modified
- **AND** no git operation that changes history or working-tree state is run

#### Scenario: An achievement seen only by a non-persisting instance lasts its session

- **WHEN** an instance that does not hold the persistence lock records an achievement that the persisting instance does not observe
- **THEN** that instance reports the achievement for the rest of its session
- **AND** after a restart the achievement may be absent from the log

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

### Requirement: Incremental, Ordered, Crash-Safe Log Persistence

Persisting newly recorded events SHALL cost work proportional to those events, not to the size of the log. Recording a batch SHALL append exactly that batch's events to the persisted log in a single write, and SHALL NOT re-serialise or rewrite any previously persisted event. A watcher batch that yields several achievements, including several archivals, SHALL be persisted as one batch.

$$bytes\_written(batch) = \sum_{e \in batch} |line(e)|$$

The persisted order of events SHALL equal the order in which they were recorded in the running instance, whichever thread records them (the watcher, Dashboard lifecycle reconciliation, or the launch backfill), so a later recording SHALL NOT be overwritten on disk by an earlier one.

No failure SHALL lose an event that was already persisted:

- An interrupted append SHALL corrupt at most the event being appended. On the next load an incomplete (unterminated) trailing entry that follows at least one complete entry SHALL be discarded, and every complete entry before it SHALL be kept.
- A persisted log that contains any other unparseable content, including a complete but unparseable final entry, SHALL be preserved byte-for-byte under a sibling name before anything else is written, and every entry that does parse SHALL still be loaded.
- A persisted log that cannot be parsed at all SHALL be moved aside rather than overwritten or truncated, even when that leaves the running log empty and so triggers the git backfill.
- A log persisted in the previous whole-file JSON array format SHALL be migrated once, completely and atomically, to the append format, and the previous file SHALL be kept rather than deleted. Keeping it SHALL NOT overwrite a previously kept copy. Every previously recorded event SHALL survive the migration unchanged.

#### Scenario: Recording a batch appends only that batch

- **WHEN** a log already holding thousands of persisted events records a batch of two achievements
- **THEN** exactly two entries are appended to the persisted log
- **AND** the bytes of every previously persisted entry are unchanged

#### Scenario: Several archivals in one batch persist once

- **WHEN** a single watcher batch shows three changes moved to the archive
- **THEN** three change-archived achievements are recorded
- **AND** they are persisted with one append

#### Scenario: Concurrent recorders persist in record order

- **WHEN** the watcher and a Dashboard lifecycle reconciliation record events at the same time in one instance
- **THEN** the persisted log lists the events in the order the instance recorded them
- **AND** reloading the log yields every event from both recorders

#### Scenario: A torn final entry does not empty the log

- **WHEN** the application stopped partway through appending an event
- **AND** the application is restarted
- **THEN** every event persisted before the interrupted one is available
- **AND** the git backfill does not run because of the interruption

#### Scenario: A damaged log is preserved, not overwritten

- **WHEN** the persisted log contains an unparseable entry in its middle
- **AND** the application is restarted and records a new achievement
- **THEN** the damaged file's original bytes remain available under a sibling name
- **AND** every parseable entry, as well as the new achievement, is in the log

#### Scenario: An unreadable log is moved aside before backfill

- **WHEN** the persisted log cannot be parsed at all, including when it holds a single unterminated fragment
- **AND** the application is restarted
- **THEN** the unreadable file is moved aside under a sibling name rather than overwritten or truncated
- **AND** the git backfill may repopulate history into a new log

#### Scenario: A legacy log is migrated without loss

- **WHEN** the application starts with only a log in the previous JSON array format
- **THEN** every event in it is available after startup, with identical fields and order
- **AND** later achievements are appended in the new format
- **AND** the previous file is kept under a sibling name

#### Scenario: A second legacy file never overwrites the kept copy

- **WHEN** a previous-format log has already been migrated and kept
- **AND** a previous-format log appears again beside the append-format log, for example written by an older build
- **AND** the application is restarted
- **THEN** the append-format log is loaded
- **AND** the new previous-format file is kept under a further sibling name, and the copy kept earlier is unchanged

### Requirement: Single Persisting Instance

When several SpecForge instances share the application's data directory, for example the desktop app beside a standalone web server, the terminal UI or a worktree dev slot, at most one of them SHALL persist activity events at any moment. An instance SHALL persist only while it holds an exclusive advisory lock on the persisted log. An instance that does not hold the lock SHALL still record events in memory for its own session's views, and SHALL NOT write them to the persisted log. An instance that acquires the lock after another releases it SHALL persist only the events it records from then on. A process that exits SHALL release the lock without any clean-shutdown step.

#### Scenario: Two instances observing one achievement persist it once

- **WHEN** two instances share the data directory and watch the same workspace
- **AND** a task is checked off in that workspace
- **THEN** the persisted log gains exactly one task-completed achievement for it

#### Scenario: A non-persisting instance still shows its own session's activity

- **WHEN** an instance that does not hold the lock records an achievement
- **THEN** that instance's Dashboard reflects the achievement
- **AND** the persisted log is not written by that instance

#### Scenario: Persistence passes to a surviving instance

- **WHEN** the instance holding the lock exits
- **AND** another instance then records an achievement
- **THEN** that instance acquires the lock and appends the achievement to the persisted log

### Requirement: Fixed-Process Task-History Backfill

Reconstructing task completions for the git backfill SHALL run a fixed number of git processes per repository, independent of how many commits or `tasks.md` revisions fall in the backfill window: one history walk that yields, for each commit, the post-commit object id of each active `openspec/changes/<id>/tasks.md` the commit changed, and one batched object read that receives only object ids, never paths.

$$git\_processes(repo) \le 2$$

The reconstructed completions SHALL be identical to those the per-commit derivation produced for the same history and window. Specifically:

- **The window and commit set are unchanged**: commits across all refs within the bounded window that touched `openspec/changes`, processed oldest first.
- **Merges are judged the same way**: a merge commit counts the same changed paths a dense combined diff reports.
- **A deleted file reads as zero completed tasks.** It resets the baseline and emits nothing.
- **Only positive deltas against the last completed count seen for that path are emitted**, attributed to the commit's author.

#### Scenario: Backfill cost does not grow with commit count

- **WHEN** a repository has hundreds of commits touching `openspec/changes` within the backfill window
- **THEN** reconstructing its task completions runs at most two git processes

#### Scenario: Batched reconstruction matches the per-commit result

- **WHEN** a fixture history contains task check-offs, an uncheck, a deleted and re-created `tasks.md`, a merge commit and an archive move
- **THEN** the reconstructed completions (timestamp, change, magnitude and author) equal those the per-commit derivation produces for the same history

#### Scenario: A deleted tasks file emits nothing and resets the baseline

- **WHEN** a commit deletes a change's `tasks.md` and a later commit re-creates it with two completed tasks
- **THEN** no completion is emitted for the deletion
- **AND** a completion of magnitude two is emitted for the re-creation

