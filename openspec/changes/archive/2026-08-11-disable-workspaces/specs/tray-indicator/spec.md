# tray-indicator Specification Delta

## MODIFIED Requirements

### Requirement: Active-Change Badge

The tray icon SHALL display a badge whose value equals the count of non-archived
*logical changes* across all tracked workspaces that are not disabled. A
non-archived logical change is one whose `(repository_id, change_name)` tuple has
at least one instance that is not under `openspec/changes/archive/`. For non-git
workspaces (which have no repository identifier), each non-archived change
directory directly under `openspec/changes/` contributes 1 to the count, as
before. The badge MUST be hidden when the count is zero.

A logical change touched by multiple worktrees SHALL contribute 1 to the badge,
not N — the badge counts distinct in-flight changes, not file copies.

A top-level row that the user has disabled (see the *Workspace Disable State*
requirement in the `workspace-registry` capability) SHALL contribute nothing to
the badge, regardless of how many non-archived logical changes it holds. The
badge is an attention surface; it is not a complete census of tracked work, and
the Dashboard remains the unfiltered record.

#### Scenario: Multi-worktree change contributes 1 to the badge

- **WHEN** a repository has a logical change present in three worktrees, with at least one instance non-archived
- **THEN** the badge value includes 1 for that logical change, not 3

#### Scenario: Badge reflects mixed git and non-git workspaces

- **WHEN** a tracked git repository has two non-archived logical changes and a tracked non-git workspace has one non-archived change
- **THEN** the badge displays "3"

#### Scenario: Badge hidden when no active logical changes

- **WHEN** every tracked workspace has zero non-archived logical changes
- **THEN** the tray badge is not displayed

#### Scenario: Badge decrements only when the last active instance is archived

- **WHEN** one instance of a multi-instance logical change is archived
- **AND** at least one other instance of the same logical change is still active
- **THEN** the badge value does not change

#### Scenario: Badge decrements when the final active instance is archived

- **WHEN** the last non-archived instance of a logical change is archived
- **THEN** the badge value decreases by one within the watcher debounce window

#### Scenario: Badge increments on a brand-new logical change

- **WHEN** a change directory with a new name (not present in any other worktree of the repository) is created in a tracked worktree
- **THEN** the badge value increases by one within the watcher debounce window

#### Scenario: Badge does not increment when a new instance joins an existing logical change

- **WHEN** a new worktree appears that contains a change whose `(repository_id, change_name)` tuple already had at least one active instance
- **THEN** the badge value does not change

#### Scenario: Disabling a workspace decrements the badge by its active count

- **WHEN** a tracked repository with four non-archived logical changes is disabled
- **THEN** the badge value decreases by four
- **WHEN** the repository is re-enabled
- **THEN** the badge value increases by four

#### Scenario: Changes appearing in a disabled workspace never reach the badge

- **WHEN** a workspace is disabled
- **AND** a new logical change appears in one of its worktrees
- **THEN** the badge value does not change

#### Scenario: Badge hidden when every workspace with active changes is disabled

- **WHEN** every tracked workspace holding a non-archived logical change is disabled
- **THEN** the badge is not displayed

### Requirement: Desktop Notification on New Change

The application SHALL display a desktop notification when a logical change first
appears in a repository — that is, when a `(repository_id, change_name)` tuple
has its first instance added in any tracked worktree. The notification SHALL NOT
fire when an additional instance of an already-tracked logical change appears
(for example, a Claude harness worktree opens and contains a copy of an existing
change).

The application SHALL NOT dispatch a notification for a logical change belonging
to a disabled top-level row (see the *Workspace Disable State* requirement in the
`workspace-registry` capability). Suppression applies to the notification only —
the change is still parsed, still cached, and still recorded in the activity log.

#### Scenario: First instance of a new logical change emits notification

- **WHEN** a change directory with a name not present in any other worktree of its repository is created in a tracked worktree
- **THEN** a desktop notification is dispatched identifying the repository and the change name

#### Scenario: Additional instance of an existing logical change is silent

- **WHEN** a worktree appears (or is created with `git worktree add`) and contains a change whose name already exists in another tracked worktree of the same repository
- **THEN** no desktop notification is dispatched for the appearance of that instance

#### Scenario: New change in a disabled workspace is silent

- **WHEN** a workspace is disabled
- **AND** a change directory with a name not present in any other worktree of its repository is created in one of its worktrees
- **THEN** no desktop notification is dispatched
- **AND** the change is still recorded in the activity log

#### Scenario: Re-enabling does not replay suppressed notifications

- **WHEN** a workspace was disabled while several new logical changes appeared in it
- **AND** the user re-enables the workspace
- **THEN** no desktop notification is dispatched for those changes
- **AND** they appear in the tree pane

### Requirement: Desktop Notification on Archive Transition

The application SHALL display a desktop notification when a logical change
transitions from active to archived — that is, when the last non-archived
instance of a `(repository_id, change_name)` tuple is moved into
`openspec/changes/archive/`. Per-instance archive moves that leave at least one
other instance still active SHALL NOT trigger a notification.

The application SHALL NOT dispatch a notification for an archive transition
belonging to a disabled top-level row. As with new-change notifications,
suppression applies to the notification only — the transition is still recorded
in the activity log and still reaches the Dashboard's shipped haul.

#### Scenario: Final-instance archive emits notification

- **WHEN** the last non-archived instance of a logical change is moved into the archive directory of its worktree
- **THEN** a desktop notification is dispatched indicating the logical change has been archived

#### Scenario: Non-final-instance archive is silent

- **WHEN** one instance of a multi-instance logical change is moved into the archive directory of its worktree
- **AND** at least one other instance of the same logical change is still active
- **THEN** no desktop notification is dispatched

#### Scenario: Archive transition in a disabled workspace is silent but still recorded

- **WHEN** a workspace is disabled
- **AND** the last non-archived instance of one of its logical changes is moved into the archive directory
- **THEN** no desktop notification is dispatched
- **AND** the archival is recorded in the activity log
- **AND** the change appears in the Dashboard's shipped haul for that day
