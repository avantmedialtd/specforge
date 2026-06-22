## MODIFIED Requirements

### Requirement: Registration Persistence

The list of registered workspaces SHALL be persisted to a config file managed by `openspec-core` and restored on application startup. The file SHALL remain a plain ordered array of user-registered workspaces, with no schema-version field, so that any application version reads and writes the same format. The relative order of user-registered workspaces in the file SHALL be the canonical config order: loading restores the file's array order, registering a new workspace appends it after the existing entries, unregistering a workspace removes it without reordering the others, and saving writes the entries back in that same order. Saving an unchanged set of registrations MUST NOT reorder them.

#### Scenario: Registrations survive restart

- **WHEN** the user registers a workspace
- **AND** quits and relaunches the application
- **THEN** the workspace is still present in the registered-workspaces list
- **AND** the tree pane shows the workspace's non-archived changes

#### Scenario: Registration order is preserved across restart

- **WHEN** the user registers workspaces A, then B, then C
- **AND** quits and relaunches the application
- **THEN** the persisted config order is A, B, C
- **AND** loading restores that same order

#### Scenario: Saving does not reorder existing registrations

- **WHEN** the registry is saved repeatedly without any registration or removal
- **THEN** the order of user-registered workspaces in the config file is identical each time
- **AND** it matches the order in which the workspaces were registered

#### Scenario: Unregistering preserves the order of the remaining workspaces

- **WHEN** workspaces A, B, C are registered in that order
- **AND** the user unregisters B
- **THEN** the remaining config order is A, C

#### Scenario: Promoting a discovered worktree appends it stably

- **WHEN** a repository's main worktree A is user-registered and a sibling worktree B is auto-discovered
- **AND** the user registers (promotes) B
- **THEN** B is persisted after the existing user-registered entries
- **AND** that position is stable across an application restart
- **AND** the repository group's top-level position (its earliest user-registered worktree) is unchanged

## ADDED Requirements

### Requirement: Lossless Registry Migration and Data Preservation

Upgrading to the order-preserving registry SHALL NOT lose, drop, corrupt, or silently reset any registered workspace. Because the on-disk format is an unchanged ordered array, there is no schema migration step; the registry and the application shell SHALL instead guarantee the following data-preservation invariants on the existing file.

Loading the registry SHALL NOT write to disk during normal operation — order normalisation happens in memory and is persisted only by a subsequent user-driven save (registering or unregistering a workspace). A missing config file SHALL load as an empty registry and SHALL NOT create a file. A present-but-empty file (an empty array, or a document with no `workspaces` key) SHALL load as an empty registry and SHALL leave the file untouched.

A config file that cannot be parsed SHALL NOT be erased or overwritten with an empty registry. The core loader SHALL surface the parse failure rather than returning an empty registry, and the application shell SHALL NOT downgrade that failure into an empty registry that a later save persists over the recoverable file. The shell SHALL preserve the corrupt file — by moving it aside to a distinct backup path before initialising an empty registry, or by operating without persisting over it — and SHALL surface the condition.

On load, each stored workspace path SHALL be reduced to a dedup key: the canonicalised path when it can be resolved, otherwise a lexically-normalised form of the stored path (so that two spellings of a workspace whose folder is currently missing still collapse). Duplicate entries — those whose dedup keys are equal — SHALL be collapsed to a single entry, keeping the earliest occurrence's position. Deduplication SHALL collapse only entries with equal keys and SHALL never reduce a workspace to zero entries nor merge two genuinely distinct paths.

A pre-existing file whose array order is arbitrary (written by an earlier version's unordered save) SHALL have that order preserved verbatim on load — it becomes stable immediately and canonical on the next user-driven save. No implicit re-sorting SHALL be applied.

Discovered (non-user-registered) worktrees SHALL NOT be written to the config file and SHALL NOT change the persisted order of user-registered entries.

Saving the registry SHALL be atomic — written to a uniquely-named temporary file in the target's directory, flushed, and renamed over the target — so that a crash or a concurrent writer can never leave a truncated or partially-written registry. Save failures SHALL be surfaced, not swallowed.

#### Scenario: Loading never writes to disk

- **WHEN** the registry is loaded from an existing config file
- **THEN** the file on disk is left byte-for-byte unchanged by the load
- **AND** no normalised order is persisted until the next register or unregister

#### Scenario: Missing or empty config file does not create or alter a file

- **WHEN** the registry is loaded and the config file is missing
- **THEN** the registry is empty
- **AND** no config file is created
- **WHEN** the registry is loaded and the config file contains an empty workspace array
- **THEN** the registry is empty
- **AND** the file is left unchanged

#### Scenario: Corrupt config file is never erased

- **WHEN** the core registry is loaded and the config file contains invalid JSON
- **THEN** loading fails with a surfaced error
- **AND** the config file is left unchanged
- **WHEN** the application shell starts with that corrupt config file
- **THEN** the shell does not start with an empty registry that a later save writes over the corrupt file
- **AND** the corrupt file is preserved (moved aside to a backup or left untouched)
- **AND** no registered workspaces are silently replaced with an empty registry

#### Scenario: Duplicate resolvable paths are collapsed, never dropped

- **WHEN** the config file contains two entries that canonicalise to the same existing path
- **THEN** the registry loads exactly one entry for that path
- **AND** the entry keeps the position of its earliest occurrence

#### Scenario: Duplicate spellings of a missing folder are collapsed on the fallback key

- **WHEN** the config file contains two differently-spelled entries for the same workspace whose folder no longer exists on disk
- **THEN** the entries are reduced to a single entry via the lexical fallback key
- **AND** the workspace is not dropped to zero entries
- **AND** two genuinely distinct missing paths remain two separate entries

#### Scenario: A pre-existing arbitrary order is preserved, not re-sorted

- **WHEN** an older version's config file has its workspaces in an arbitrary order
- **AND** the new version loads it
- **THEN** the workspaces load in that same arbitrary order
- **AND** no alphabetical or other re-sorting is applied
- **AND** that order becomes canonical once the user next registers or unregisters a workspace

#### Scenario: Auto-discovered worktrees never enter the saved file

- **WHEN** a user-registered file with workspaces A, B, C in a git repository that has additional worktrees is loaded
- **AND** the sibling worktrees are auto-discovered
- **AND** the registry is saved
- **THEN** the saved file contains exactly A, B, C in that order
- **AND** no discovered worktree paths appear in the file

#### Scenario: Saving is atomic

- **WHEN** the registry is saved
- **THEN** a reader concurrent with the save never observes a truncated or partially-written file
- **AND** a save that fails partway leaves the previous config file intact

### Requirement: Deterministic Config-Ordered Top-Level View

The aggregated view returned for the tree pane SHALL list its top-level rows — repository groups and flat (non-git) workspaces — in a deterministic order derived from the config order of user-registered workspaces. Recomputing the view from an unchanged registry MUST yield an identical order of top-level rows, regardless of which cache event triggered the recomputation and regardless of hash-map seeding.

A repository group SHALL be positioned at the config position of its earliest user-registered worktree. A flat workspace SHALL be positioned at its own config position. Repository groups and flat workspaces SHALL be interleaved so that the overall top-level order matches the config order. A newly registered workspace SHALL appear after the previously registered ones.

Auto-discovered worktrees carry no config position and SHALL NOT affect any repository group's top-level position; they contribute their instances only *within* their repository group.

#### Scenario: Top-level order is stable across recomputation

- **WHEN** the aggregated view is computed repeatedly from an unchanged registry
- **THEN** the order of top-level rows is identical on every computation
- **AND** the order does not depend on which cache event triggered the recomputation

#### Scenario: Top-level order matches config order with interleaving

- **WHEN** the user has registered, in order, a flat workspace, then a git repository, then another flat workspace
- **THEN** the tree pane lists those three top-level rows in that same order (flat, repo, flat)

#### Scenario: A repository group is positioned by its earliest user-registered worktree

- **WHEN** a repository has two user-registered worktrees registered at config positions 1 and 3, with a flat workspace at position 2
- **THEN** the repository group appears at position 1 (its earliest user-registered worktree)
- **AND** the flat workspace appears after it

#### Scenario: Discovering a worktree does not change top-level order

- **WHEN** a new worktree of an already-displayed repository is auto-discovered at runtime
- **THEN** the repository group keeps its existing top-level position
- **AND** no other top-level row changes position

#### Scenario: A newly registered workspace appears last

- **WHEN** the user registers an additional workspace
- **THEN** its top-level row appears after all previously registered top-level rows
