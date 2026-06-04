# activity-log

## MODIFIED Requirements

### Requirement: Achievement Detection from Watcher Re-Parses

When the watcher processes a debounced batch and re-parses a workspace, the system SHALL compare the new parse against the previously cached state and record an achievement event for each net-positive transition: an increase in a change's completed-task count (recorded with the increase as its magnitude), an artifact reaching a new status, a change newly appearing, and a change moving to archived. A decrease — a task being unchecked or a task line being removed — SHALL NOT produce a negative or spurious event.

A change that has left the active set SHALL be classified as **moved to archived** (as opposed to deleted) when a directory corresponding to its logical id `<id>` exists under `openspec/changes/archive/` — recognised under either the bare name `<id>` or the conventional dated form `<YYYY-MM-DD>-<id>` produced by the archive tooling. The dated form SHALL be matched by stripping a leading `YYYY-MM-DD-` prefix and comparing the remainder to `<id>` exactly, so an unrelated archive entry does not match. A change that has left the active set with no such archive directory SHALL be classified as deleted and SHALL NOT record an archival achievement. This classification governs both the recorded archival achievement and the archive-transition cache event the watcher emits, and so SHALL hold for flat (non-git) workspaces that rely on live capture, not only for git-backed workspaces.

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
