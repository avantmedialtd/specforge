## ADDED Requirements

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

## MODIFIED Requirements

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
