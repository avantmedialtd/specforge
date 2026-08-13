# workspace-registry Specification

## Purpose

Defines how the application discovers, persists, and observes the set of OpenSpec workspaces the user has registered, including the filesystem watching and in-memory cache that feed the tree pane and tray badge, plus the user-facing settings surface for managing the registry.
## Requirements
### Requirement: Manual Workspace Registration

The application SHALL allow the user to register a workspace by selecting a folder on disk that contains an `openspec/` subdirectory. A folder lacking an `openspec/` subdirectory MUST be rejected with a user-visible message.

#### Scenario: Valid folder is registered

- **WHEN** the user opens the settings view and selects a folder containing an `openspec/` subdirectory
- **THEN** the folder is added to the registered-workspaces list
- **AND** the folder appears in the tree pane as a top-level workspace node

#### Scenario: Invalid folder is rejected

- **WHEN** the user selects a folder that does not contain an `openspec/` subdirectory
- **THEN** the folder is not added to the registered-workspaces list
- **AND** a message indicates the folder is not a valid OpenSpec workspace

### Requirement: Workspace Removal

The application SHALL allow the user to remove a workspace from the registered-workspaces list. Removal MUST dispose any filesystem watcher associated with the removed workspace and update the badge accordingly.

#### Scenario: Workspace removed via settings

- **WHEN** the user removes a workspace from the registered list in the settings view
- **THEN** the workspace is no longer shown in the tree pane
- **AND** the badge count decreases by the number of non-archived changes that workspace contributed

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

### Requirement: Filesystem Watching of Registered Workspaces

For each registered workspace, the application SHALL watch the workspace's `openspec/changes/` directory for additions, removals, and modifications, using a debounced event stream to coalesce bursts of filesystem events.

#### Scenario: Watcher established on registration

- **WHEN** a workspace is added to the registered list
- **THEN** a filesystem watcher is established on that workspace's `openspec/changes/` directory

#### Scenario: Watcher disposed on removal

- **WHEN** a workspace is removed from the registered list
- **THEN** the filesystem watcher for that workspace is disposed

#### Scenario: Burst of edits is coalesced

- **WHEN** multiple files inside one registered workspace are modified within the debounce window
- **THEN** the cache and UI receive a single coalesced update event, not one event per file

### Requirement: In-Memory Cache of Parsed State

The application SHALL maintain an in-memory cache of parsed OpenSpec state (changes, artifacts, sections, tasks) for each registered workspace. The cache MUST be kept consistent with on-disk state by the watcher and MUST be the source of truth for the tree pane and badge — neither queries the filesystem directly.

#### Scenario: Cache reflects on-disk change within debounce window

- **WHEN** a file under a registered workspace's `openspec/changes/` directory is modified
- **THEN** the in-memory cache for that workspace is updated within the watcher debounce window
- **AND** subsequent reads from the tree pane and badge use the updated cache

### Requirement: Settings View

The main window SHALL include a settings view, reachable from a discoverable
affordance in the main window chrome, that surfaces: the registered-workspaces
list with add and remove controls, a per-workspace inline display-name field, a
per-workspace palette swatch picker that accepts one of the curated palette
tokens or "none", a per-workspace enabled/disabled toggle, a launch-on-login
toggle, and a notifications-enabled toggle.

The enabled/disabled toggle SHALL be the only surface from which a workspace is
disabled or re-enabled; no tree-pane or window-chrome affordance advertises or
alters the disabled state.

A per-workspace control whose change cannot be persisted SHALL report the failure
on the control the user operated, and SHALL continue to show the stored state
rather than the attempted one. Reporting the failure only to a developer console
does not satisfy this: the desktop and terminal frontends have no console the user
can see, so a rejected write would otherwise be indistinguishable from a control
that silently does nothing.

Where a toggle governs a repository group with more than one user-registered
worktree, the settings view SHALL make that shared scope legible on each affected
row, before the toggle is used. The rows are per registered folder while the flag
is per repository, so a user who operates one row moves the others; that
many-to-one relationship SHALL be visible rather than inferred from the result.

#### Scenario: Settings view shows registered workspaces

- **WHEN** the user opens the settings view
- **THEN** every currently registered workspace is listed with its folder path and a remove control
- **AND** an add-workspace control is visible

#### Scenario: Launch-on-login toggle is persisted and applied

- **WHEN** the user enables the launch-on-login toggle
- **THEN** the application registers itself for launch at the next system login via the operating system's autostart mechanism
- **AND** the toggle state is persisted across application restarts

#### Scenario: Notifications-enabled toggle suppresses notifications

- **WHEN** the user disables the notifications-enabled toggle
- **THEN** subsequent new-change and archive-transition events do not dispatch desktop notifications
- **AND** the toggle state is persisted across application restarts

#### Scenario: Inline display-name field renames the workspace

- **WHEN** the user edits the inline display-name field for a listed workspace and commits the change
- **THEN** the new display name is persisted to the presentation store under the workspace's row-identity key
- **AND** subsequent renderings of that workspace in the Settings list and the tree pane show the new name
- **AND** clearing the field reverts the workspace to its default name (the folder basename or, for a repository group, the main worktree's basename)

#### Scenario: Palette swatch picker sets the workspace colour

- **WHEN** the user selects a palette swatch for a listed workspace
- **THEN** the chosen colour token is persisted to the presentation store under the workspace's row-identity key
- **AND** the tree pane re-renders the workspace's parent row with the corresponding tint
- **WHEN** the user selects the "none" swatch
- **THEN** the persisted colour is cleared
- **AND** the workspace's parent row reverts to the default untinted background

#### Scenario: Disable toggle removes the workspace from the tree

- **WHEN** the user switches a listed workspace's toggle to disabled
- **THEN** the workspace remains listed in the settings view, marked as disabled
- **AND** the tree pane no longer shows a top-level row for it
- **WHEN** the user switches the toggle back to enabled
- **THEN** the workspace reappears in the tree pane in its original position

#### Scenario: Disabling from one row of a repository updates its siblings

- **WHEN** a repository has two user-registered worktrees listed in the settings view
- **AND** the user disables the repository from one of those rows
- **THEN** both rows show the disabled state

#### Scenario: A rejected toggle reports the failure on the row

- **WHEN** the user switches a listed workspace's toggle and the presentation store cannot persist the change
- **THEN** the failure is reported on that workspace's row
- **AND** the toggle continues to show the stored state rather than the attempted one

#### Scenario: A shared toggle declares its scope before use

- **WHEN** a repository has more than one user-registered worktree listed in the settings view
- **THEN** each of those rows states that its toggle, display name, and colour are shared with the repository's other listed folders
- **AND** a workspace listed only once carries no such statement

### Requirement: Missing Workspace Handling

When a registered workspace's folder no longer exists on disk, the application SHALL mark that workspace as missing in the settings view without crashing the watcher or removing the workspace from the registered list.

#### Scenario: Missing folder shown as such

- **WHEN** a registered workspace's folder is deleted while the application is running
- **THEN** the workspace appears in the settings view with a "missing" indicator
- **AND** the tree pane shows the workspace with no children (or hides it pending user action)
- **AND** no further filesystem watching is attempted on the missing path until the user re-registers it or removes it

### Requirement: Git Repository Detection

When a workspace is registered, the application SHALL detect whether the workspace lives inside a git repository by invoking `git rev-parse --git-common-dir` against the workspace path. The canonicalised result of that command identifies the repository for the purpose of grouping worktrees. Path canonicalisation MUST normalise verbatim extended-length and UNC forms (for example `\\?\UNC\wsl.localhost\Ubuntu\…`) to a single simplified representation, so that the same repository reached through differently-shaped but equivalent paths always yields one identifier and is never split into two. Workspaces that are not inside a git repository SHALL continue to be treated as standalone (flat) workspaces and are not subject to worktree aggregation.

#### Scenario: Workspace inside a git repository is recognised as such

- **WHEN** the user registers a workspace that lies inside a git repository
- **THEN** the application records the canonicalised git common directory as the workspace's repository identifier
- **AND** the workspace is associated with every other worktree that shares the same repository identifier

#### Scenario: Workspace outside a git repository remains flat

- **WHEN** the user registers a workspace whose path is not inside any git repository
- **THEN** the workspace has no repository identifier
- **AND** the workspace is rendered as a standalone top-level entry with no worktree aggregation

#### Scenario: `git` is missing on PATH

- **WHEN** the application attempts to detect a workspace's repository and the `git` binary cannot be invoked
- **THEN** the workspace has no repository identifier
- **AND** the workspace continues to function as a flat workspace without aborting registration

#### Scenario: Equivalent path forms yield a single repository identifier

- **WHEN** a repository is reached through two equivalent but differently-shaped paths — for example a simplified UNC path and its verbatim `\\?\UNC\…` extended-length form
- **THEN** canonicalisation normalises both to the same representation
- **AND** the repository is assigned exactly one identifier, so its worktrees aggregate together and the badge counts it once

### Requirement: Default-Branch Resolution

For each detected repository, the application SHALL resolve a default branch using a cascade: (1) the symbolic ref `refs/remotes/origin/HEAD`, (2) the `init.defaultBranch` config value, (3) the branch currently checked out in the repository's main worktree at first detection. If none of these resolve, the repository SHALL have no default branch and no instance is tagged as the default.

#### Scenario: Default branch resolved from remote HEAD

- **WHEN** the repository has a remote `origin` with a configured HEAD
- **THEN** the application records the stripped remote-HEAD ref name (e.g. `main`) as the repository's default branch

#### Scenario: Default branch resolved from init.defaultBranch when no remote HEAD

- **WHEN** the repository has no remote HEAD configured
- **AND** `init.defaultBranch` is set in the repository's config
- **THEN** the application records that value as the repository's default branch

#### Scenario: Default branch resolved from main worktree when neither is set

- **WHEN** the repository has neither a remote HEAD nor an `init.defaultBranch` value
- **THEN** the application records the branch currently checked out in the main worktree as the repository's default branch

#### Scenario: No default branch when all detection methods fail

- **WHEN** none of the detection methods succeed
- **THEN** the repository has no default branch
- **AND** no instance of any logical change in that repository is tagged as the default-branch instance

### Requirement: Worktree Auto-Discovery

When a workspace inside a git repository is registered, the application SHALL automatically discover every other worktree of the same repository via `git worktree list --porcelain` and register each discovered worktree as a tracked workspace with origin `Discovered`. Discovered workspaces SHALL NOT be filtered by path — worktrees under `.claude/worktrees/` or any other location are included.

#### Scenario: Sibling worktrees are auto-discovered at registration time

- **WHEN** the user registers a workspace that is a worktree of a repository with two other existing worktrees
- **THEN** the application registers the two other worktrees as tracked workspaces with origin `Discovered`
- **AND** each discovered worktree contributes its changes to the repository's aggregated view

#### Scenario: Harness worktrees under `.claude/worktrees/` are auto-discovered

- **WHEN** the repository has a worktree whose path is under `.claude/worktrees/`
- **THEN** the application discovers and tracks that worktree
- **AND** does not filter or hide it from the aggregated view

### Requirement: Origin Distinction Between User-Registered and Discovered

Each tracked workspace SHALL be tagged with its origin — `UserRegistered` or `Discovered` (with a reference to the repository identifier that triggered the discovery). Only `UserRegistered` entries SHALL be persisted to the config file. `Discovered` entries SHALL be recomputed at every application startup and as worktrees are added or removed at runtime.

#### Scenario: Only user-registered workspaces persist across restart

- **WHEN** the application is quit and relaunched
- **THEN** every `UserRegistered` workspace is restored from the config file
- **AND** every `Discovered` workspace is re-derived by scanning the worktrees of each user-registered repository, not loaded from the config file

#### Scenario: Older config files without an origin field default to user-registered

- **WHEN** the application reads a config file written by an older version that omits the `origin` field
- **THEN** each entry is loaded as if it had origin `UserRegistered`
- **AND** worktrees of detected repositories are then auto-discovered as normal

### Requirement: Dynamic Worktree Tracking via Meta-Watcher

For each repository that has at least one tracked workspace, the application SHALL install a non-recursive filesystem watcher on the repository's `.git/worktrees/` directory and reconcile the discovered-worktree set whenever that directory changes. New worktrees that appear at runtime SHALL be added without user action; removed worktrees (including those whose path was deleted before `git worktree prune` ran) SHALL be dropped.

#### Scenario: New worktree appears at runtime

- **WHEN** a new git worktree of a tracked repository is created while the application is running
- **THEN** the application detects the change within the debounce window
- **AND** the new worktree is registered as `Discovered`
- **AND** the worktree's `openspec/changes/` directory gains a filesystem watcher
- **AND** the tree pane shows the new worktree's instances under the existing logical changes (or as new logical changes, as appropriate)

#### Scenario: Worktree removed via `git worktree remove`

- **WHEN** an existing tracked worktree is removed via `git worktree remove`
- **THEN** the application detects the change within the debounce window
- **AND** the worktree's watcher is torn down
- **AND** the worktree's instances disappear from the tree pane

#### Scenario: Worktree directory deleted without `git worktree prune`

- **WHEN** an existing tracked worktree's directory is deleted on disk but `.git/worktrees/<name>/` still exists
- **THEN** the application's reconciler classifies the worktree as removed (because `git worktree list --porcelain` reports it as prunable)
- **AND** the worktree is dropped from the tracked set

### Requirement: Cascade Removal of Discovered Workspaces

When the user unregisters a `UserRegistered` workspace that is the last user-registered entry for its repository, the application SHALL also drop every `Discovered` workspace tagged with that repository identifier and tear down the associated meta-watcher and default-branch watcher.

#### Scenario: Removing the last user-registered workspace of a repo cascades

- **WHEN** the user unregisters a workspace whose repository has no other user-registered entry
- **THEN** every `Discovered` workspace for that repository is also removed from the tracked set
- **AND** the meta-watcher on the repository's `.git/worktrees/` directory is disposed
- **AND** the default-branch watcher for the repository is disposed

#### Scenario: Removing a non-last user-registered workspace of a repo preserves discovery

- **WHEN** the user unregisters a workspace whose repository still has at least one other user-registered entry
- **THEN** the discovered workspaces and the meta-watcher are kept in place

### Requirement: Workspace Presentation Persistence

The application SHALL persist a per-top-level-row presentation entry (display
name, palette colour, and disabled state) in a presentation store separate from
the workspace registry. Presentation entries SHALL be keyed by the identity of
the top-level row they decorate: a canonical workspace path for a flat (non-git)
workspace, and a canonical git common directory for a repository group.
Presentation entries SHALL survive application restarts. When the user
unregisters the last user-registered workspace associated with a given key (a
flat workspace's path, or any user-registered workspace whose repository
identifier matches a repo-keyed entry), the presentation entry for that key SHALL
be removed, in full — display name, colour, **and** disabled state together.

Unregistration SHALL resolve the row whose presentation entry is to be cleaned up
using the same path canonicalisation the workspace registry keys its own entries
with. Any path spelling the registry accepts for unregistration MUST therefore
also clean up that row's presentation entry: a registration and its presentation
entry SHALL NOT be separable by the spelling of the path passed to the removal.

The cleanup SHALL be scoped to user-registered removals. Unregistering a path the
registry does not hold, or a discovered worktree the user never registered
directly, SHALL NOT remove any presentation entry — a repository group's shared
entry survives for as long as the repository has any user-registered workspace.

The presented display name MUST be normalised so that an empty string is stored
as absent. The presented colour MUST be one of a fixed curated palette of tokens
(`indigo`, `blue`, `teal`, `green`, `amber`, `orange`, `rose`, `purple`) or
absent. No other colour values SHALL be accepted by the store. The disabled state
MUST default to enabled when absent.

An entry SHALL be considered empty — and therefore eligible for pruning on save —
only when it carries no display name, no colour, **and** is not disabled. Setting
the disabled state MUST NOT overwrite an entry's display name or colour, and
setting a display name or colour MUST NOT overwrite its disabled state.

#### Scenario: Presentation entry persists across restart

- **WHEN** the user sets a display name and colour for a registered workspace
- **AND** quits and relaunches the application
- **THEN** the presentation entry is restored from disk
- **AND** the workspace continues to render with the chosen display name and colour

#### Scenario: Presentation entry cleaned up when underlying workspace is unregistered

- **WHEN** the user unregisters a flat workspace that has a presentation entry
- **THEN** the presentation entry keyed by that workspace's path is removed from the store
- **AND** re-registering the same path afterwards starts with the default display name, no colour, and enabled

#### Scenario: Disabled-only presentation entry is cleaned up too

- **WHEN** a registered flat workspace is disabled and carries no display name and no colour, so its presentation entry holds only the disabled state
- **AND** the user unregisters that workspace
- **THEN** the presentation entry keyed by that workspace's path is removed from the store
- **AND** re-registering the same path afterwards yields an enabled row that appears in the tree pane, rather than a silently re-parked one

#### Scenario: Cleanup follows the registry's canonical identity, not the caller's spelling

- **WHEN** a workspace is unregistered using a path spelling that differs from the stored registry key but canonicalises to it (for example a Windows verbatim `\\?\` or `\\?\UNC\` form of the same directory)
- **THEN** the registration is removed
- **AND** that row's presentation entry is removed with it, leaving nothing behind to re-apply on re-registration

#### Scenario: Unregistering an unknown path leaves the store untouched

- **WHEN** unregistration is invoked for a path the registry does not hold
- **THEN** no registration is removed
- **AND** no presentation entry is removed

#### Scenario: Repo-keyed presentation cleaned up when the last user-registered workspace for the repo is unregistered

- **WHEN** the user has a repository with two user-registered workspaces and a presentation entry keyed by that repository
- **AND** the user unregisters one of the two workspaces
- **THEN** the presentation entry is preserved (the repository still has a user-registered workspace)
- **WHEN** the user unregisters the remaining user-registered workspace for that repository
- **THEN** the presentation entry keyed by the repository's common directory is removed from the store

#### Scenario: Empty display-name input falls back to default

- **WHEN** the user submits an empty display name from the Settings view
- **THEN** the persisted display name is stored as absent (not an empty string)
- **AND** the workspace renders with its default name (the folder basename for flat workspaces or the main worktree's basename for repository groups)

#### Scenario: Invalid colour value is rejected

- **WHEN** an attempt is made to set a colour value that is not one of the curated palette tokens and is not absent
- **THEN** the presentation store rejects the update with an error
- **AND** the existing entry, if any, is left unchanged

#### Scenario: A disabled-only entry is not treated as empty

- **WHEN** an entry has no display name and no colour but is disabled
- **THEN** the store does not prune it on save
- **AND** the entry is present after a reload

#### Scenario: Clearing name and colour on a disabled entry keeps it disabled

- **WHEN** a disabled workspace has its display name and colour cleared
- **THEN** the entry is retained
- **AND** the workspace is still disabled

### Requirement: Presentation Fields on Listed Workspaces

The data returned by the workspace listing command and the aggregated repo-view
command SHALL include the configured display name (or absent if none), the
colour (or absent if none), and the disabled state for each top-level row. When
no presentation entry exists for a row, the display name and colour SHALL be
absent, the row SHALL be reported as enabled, and consumers SHALL render the row
exactly as they did before the presentation store was introduced.

#### Scenario: Workspace list includes display name and colour

- **WHEN** the frontend requests the list of registered workspaces
- **THEN** each workspace entry includes its configured display name (or null), colour token (or null), and disabled state
- **AND** workspaces with no presentation entry return null for name and colour, and report as enabled

#### Scenario: Repo view includes display name and colour for the group

- **WHEN** the frontend requests the aggregated repo-and-flat view
- **THEN** each repo group entry includes its configured display name (or null) and colour token (or null)
- **AND** flat workspace entries in the same view also include their per-workspace display name and colour

#### Scenario: Listing reports disabled workspaces so Settings can render the toggle

- **WHEN** a workspace is disabled and the frontend requests the list of registered workspaces
- **THEN** the workspace is present in the response, marked disabled
- **AND** it is not omitted from the listing the way it is omitted from the tree pane's aggregated view

### Requirement: Aggregated View Freshness on Registration Change

When the user registers or unregisters a workspace via the corresponding IPC commands, the aggregated repo-and-flat view returned by the next `get_workspace_views` call SHALL reflect the post-registration set of tracked workspaces. The freshness guarantee is anchored to the IPC command's return — by the time `register_workspace` or `unregister_workspace` resolves, a subsequent `get_workspace_views` request MUST already include the just-added workspace and exclude the just-removed workspace (and any discovered worktrees that cascaded with it). The frontend MUST NOT need to wait for an intervening filesystem event or an application restart for the tree pane to reflect the change.

#### Scenario: View reflects a newly-registered workspace immediately

- **WHEN** the user registers a workspace via the Settings view
- **AND** the `register_workspace` command has returned successfully
- **AND** the frontend then requests the aggregated repo-and-flat view
- **THEN** the response includes a top-level entry for the newly-registered workspace
- **AND** the inclusion does not depend on any intervening filesystem event for that workspace

#### Scenario: View reflects an unregistered workspace immediately

- **WHEN** the user removes a workspace from the Settings view
- **AND** the `unregister_workspace` command has returned successfully
- **AND** the frontend then requests the aggregated repo-and-flat view
- **THEN** the response no longer contains a top-level entry for the removed workspace
- **AND** the removal does not depend on any intervening filesystem event

#### Scenario: Cascade removal updates the view in one shot

- **WHEN** the user unregisters the last user-registered workspace of a repository that also has one or more discovered worktrees
- **AND** the `unregister_workspace` command has returned successfully
- **AND** the frontend then requests the aggregated repo-and-flat view
- **THEN** the response no longer contains the repository's group entry
- **AND** the response no longer contains entries for the discovered worktrees that cascaded with the removal
- **AND** the freshness applies to the single command return — the frontend does not need to call `get_workspace_views` repeatedly or wait for any subsequent event

#### Scenario: Auto-discovered sibling worktrees appear in the view immediately

- **WHEN** the user registers a workspace inside a git repository that has additional worktrees
- **AND** the `register_workspace` command has returned successfully
- **AND** the frontend then requests the aggregated repo-and-flat view
- **THEN** the response includes the repository's group entry containing every auto-discovered worktree
- **AND** the inclusion does not depend on any intervening filesystem event

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

### Requirement: Bounded Repository Watcher Footprint

The filesystem-watching infrastructure SHALL maintain at most one native
filesystem watcher per registered repository for all repository-level git
signals — worktree-set changes, default-branch changes, refs/commit-graph
changes, and working-tree index changes — rather than a separate watcher per
signal. Installing a repository's watcher SHALL be idempotent: a repository that
is already watched MUST NOT gain an additional watcher when it is re-registered or
re-discovered. When the last tracked workspace belonging to a repository is
removed, that repository's watcher SHALL be disposed.

#### Scenario: One watcher per repository

- **WHEN** a repository with any number of tracked worktrees is being watched
- **THEN** the application holds exactly one repository-level filesystem watcher
  for that repository's git signals

#### Scenario: Watcher count stays bounded over a long session

- **WHEN** repositories are registered, re-discovered, and have their worktrees
  added and removed repeatedly over a long-running session
- **THEN** the number of repository-level watchers equals the number of distinct
  registered repositories
- **AND** it does not accumulate beyond that count

#### Scenario: Removing a repository disposes its watcher

- **WHEN** the last tracked workspace belonging to a repository is removed from
  the registry
- **THEN** that repository's filesystem watcher is disposed

### Requirement: Coalesced Repository Refresh

A burst of git filesystem events for a single repository SHALL be coalesced into
at most one working-tree status recompute and at most one commit-graph refresh per
debounce window, rather than one recompute per raw filesystem event. Such bursts
include the many index and ref writes produced by a rebase, fetch, or checkout.

The coalescing bound SHALL hold across the *contents* of a batch as well as its
raw events: when a single debounced batch adds or removes multiple tracked
worktrees, the application SHALL perform at most one aggregated recompute for that
batch, rather than one per added or removed worktree. Watcher installation and
teardown for the affected worktrees happen first; the single recompute and its
derived events follow, and observe the post-batch set of worktrees.

The `WorkspaceRemoved` and `Updated` events announcing individual worktree
additions and removals SHALL still be emitted per affected worktree — only the
aggregated recompute is coalesced — and the derived logical/instance diff events
SHALL be emitted after them, computed once against the post-batch state.

#### Scenario: A multi-write git operation triggers one refresh

- **WHEN** a git operation writes many index and ref entries for a repository
  within a single debounce window
- **THEN** the repository's working-tree status is recomputed at most once for
  that window
- **AND** its commit-graph is refreshed at most once for that window

#### Scenario: Adding several worktrees at once triggers one recompute

- **WHEN** a single debounced batch adds three worktrees to a tracked repository
- **THEN** the aggregated view is recomputed exactly once for that batch
- **AND** the recomputed view includes all three new worktrees

#### Scenario: Removing several worktrees at once triggers one recompute

- **WHEN** a single debounced batch removes two worktrees from a tracked repository
- **THEN** the aggregated view is recomputed exactly once for that batch
- **AND** a `WorkspaceRemoved` event is emitted for each removed worktree

### Requirement: Workspace Disable State

Each top-level row — a repository group or a flat (non-git) workspace — SHALL
carry a disabled flag, persisted in the presentation store under the same
row-identity key used for display name and colour. A row with no stored flag
SHALL be treated as enabled, so a presentation file written before this
capability existed loads with every workspace enabled.

Because the flag is keyed by top-level row, every user-registered workspace that
shares a repository identifier shares one disabled state.

The flag SHALL live entirely in the presentation store. The workspace registry
file SHALL NOT gain a disabled field, a schema-version field, or any other new
key, so the cross-version read/write compatibility guaranteed by the
*Registration Persistence* and *Lossless Registry Migration and Data
Preservation* requirements is preserved.

#### Scenario: Disabled state persists across restart

- **WHEN** the user disables a registered workspace
- **AND** quits and relaunches the application
- **THEN** the workspace is still disabled
- **AND** it is still present in the registered-workspaces list in its original config position

#### Scenario: An entry carrying only a disabled flag is not pruned

- **WHEN** a workspace is disabled and has neither a display-name override nor a colour override
- **THEN** the presentation store persists an entry for that row
- **AND** the entry is not discarded as empty when the store is saved
- **AND** the disabled state is restored on the next load

#### Scenario: Setting the disabled flag preserves display name and colour

- **WHEN** a workspace has a configured display name and palette colour
- **AND** the user disables it
- **THEN** the display name and colour are unchanged
- **WHEN** the user re-enables it
- **THEN** the display name and colour are still unchanged

#### Scenario: Sibling worktrees of one repository share a single disabled state

- **WHEN** a repository has two user-registered worktrees
- **AND** the user disables the repository from either of their Settings rows
- **THEN** both rows report the workspace as disabled
- **AND** the repository group is disabled as a single top-level row

#### Scenario: Presentation file predating this capability loads as enabled

- **WHEN** the presentation store loads a file whose entries carry no disabled field
- **THEN** every row is treated as enabled
- **AND** any stored display names and colours are preserved

#### Scenario: Disabled state is cleaned up on unregister

- **WHEN** the user unregisters the last user-registered workspace for a row that is disabled
- **THEN** the presentation entry for that key — including the disabled flag — is removed from the store
- **AND** re-registering the same path afterwards starts enabled

### Requirement: Cold Aggregation of Disabled Rows

A disabled top-level row SHALL remain present in the aggregated snapshot, but
SHALL be aggregated without performing any git invocation on its behalf. The
application SHALL NOT invoke `git worktree list`, `git branch`, or `git status`
for a disabled row during aggregation.

A cold row's cache-derived content — its active logical changes, its archived
logical changes, and each change's task counts and capability-spec artifacts —
SHALL be as accurate as a warm row's. Its git-derived fields — the resolved
default branch, the repository dirty rollup, the dirty-worktree list, the
uncommitted-specs flag, and each instance's branch, default-branch tag, and spec
commit state — SHALL take defined default values rather than stale ones.

A cold row's *identity* SHALL NOT be a casualty of skipping git. The repository
identifier, its main worktree, the name derived from that worktree's basename,
and which instance is tagged as the main worktree SHALL be the values the row
carries while enabled, resolved without a subprocess. Disabling a row SHALL NOT
change how it is identified or labelled on any surface where it remains visible,
including the Dashboard's per-repository breakdown and today's ships. This
requirement is stated as an outcome rather than as a mechanism: a fallback that
happens to agree with git only for the ordinary `<worktree>/.git` layout does not
satisfy it, because a repository whose git common directory lies elsewhere — a
submodule's `<superproject>/.git/modules/<name>`, a `--separate-git-dir` store, or
a bare repository — would then visibly rename itself the moment it is disabled.

Disabling a flat (non-git) workspace performs no git work either way and SHALL
be behaviourally identical apart from the row's exclusion from the surfaces named
in the *Disabled Rows Excluded From the Tree Pane* requirement.

#### Scenario: No git subprocess is spawned for a disabled repository

- **WHEN** the aggregated view is recomputed with one enabled and one disabled repository registered
- **THEN** git is invoked for the enabled repository's worktrees
- **AND** git is not invoked for the disabled repository's worktrees

#### Scenario: A cold row reports accurate change counts

- **WHEN** a disabled repository has three non-archived changes and two archived changes in its cache
- **THEN** its row in the aggregated snapshot reports three active logical changes and two archived logical changes
- **AND** each active logical change carries its completed-task and total-task counts

#### Scenario: A cold row's git-derived fields are defaulted, not stale

- **WHEN** a repository with uncommitted work is disabled
- **AND** the aggregated view is recomputed
- **THEN** the row's dirty rollup, dirty-worktree list, uncommitted-specs flag, and default branch hold their default values
- **AND** no instance reports a branch or a non-default spec commit state

#### Scenario: A cold row keeps its display name and top-level position

- **WHEN** a repository at config position 2 of three is disabled
- **THEN** its row remains at position 2 in the aggregated snapshot
- **AND** it is labelled with its configured display name, or its main worktree's basename when none is configured

#### Scenario: A disabled repository keeps the identity it had while enabled

- **WHEN** a repository whose git common directory is not `<worktree>/.git` — a submodule, a `--separate-git-dir` store, or a bare repository — is disabled
- **THEN** its main worktree, the name derived from it, and its main-worktree instance tagging are the same values the row reported while enabled
- **AND** no git subprocess was invoked to determine them

### Requirement: Disabled Rows Excluded From the Tree Pane

The aggregated view returned to a frontend SHALL exclude every disabled
top-level row, so the tree pane shows only enabled workspaces. The exclusion
SHALL be applied once, in the shared command that serves the aggregated view, so
that the desktop, web, and terminal frontends all observe it without
frontend-specific filtering.

That exclusion — together with the presentation join (display name and tint)
applied to the rows that survive it — SHALL have exactly one implementation, and
it SHALL live in the shared application-service accessor rather than in any
frontend shell. Each frontend's aggregated-view entry point SHALL delegate to
that accessor; no frontend shell SHALL carry its own copy of the filter or the
join. The single implementation MUST be reachable from the workspace's automated
test and mutation-testing gates, so a regression in either half is caught without
launching a frontend.

#### Scenario: A disabled workspace disappears from the tree

- **WHEN** the user disables a registered workspace
- **THEN** the tree pane no longer shows a top-level row for it
- **AND** none of its changes are reachable through the tree

#### Scenario: Every frontend observes the same exclusion

- **WHEN** a workspace is disabled
- **AND** the aggregated view is requested by the desktop, web, or terminal frontend
- **THEN** none of the three responses contains the disabled workspace's row

#### Scenario: The exclusion and the presentation join have a single implementation

- **WHEN** the disabled-row exclusion or the presentation join is changed in the shared aggregated-view accessor
- **THEN** the desktop, web, and terminal frontends all observe the change with no per-frontend edit
- **AND** no frontend shell contains a second copy of the filter or the join to fall out of step with it

#### Scenario: Enabled workspaces keep their relative order

- **WHEN** workspaces A, B, C are registered in that order and B is disabled
- **THEN** the tree pane lists A then C
- **AND** re-enabling B restores it to its original position between them

### Requirement: Disabled Workspaces Continue To Be Watched

Disabling a workspace SHALL NOT dispose its filesystem watcher, SHALL NOT remove
its entries from the in-memory cache, and SHALL NOT stop achievement recording
for it. A disabled workspace's parsed state SHALL continue to track on-disk
state within the watcher debounce window, exactly as an enabled workspace's does.

#### Scenario: A disabled workspace's cache stays current

- **WHEN** a workspace is disabled
- **AND** a file under its `openspec/changes/` directory is modified
- **THEN** the in-memory cache for that workspace is updated within the watcher debounce window

#### Scenario: Achievements continue to be recorded while disabled

- **WHEN** a workspace is disabled
- **AND** a task is completed in one of its changes
- **THEN** the achievement is recorded in the activity log
- **AND** it contributes to the streak and the contribution heatmap exactly as it would for an enabled workspace

#### Scenario: Watcher count is unchanged by disabling

- **WHEN** a repository with tracked worktrees is disabled
- **THEN** the number of installed filesystem watchers and repository-level watchers is unchanged

### Requirement: Re-enable Freshness

When the user toggles a top-level row's disabled flag via the corresponding IPC
command, the aggregated view returned by the next request SHALL already reflect
the new state. Re-enabling a row SHALL recompute that row warm — performing the
git work skipped while it was cold — before the command returns, so the frontend
does not need to wait for an intervening filesystem event or an application
restart.

#### Scenario: Re-enabling restores a warm row in one shot

- **WHEN** the user re-enables a disabled repository that has uncommitted work
- **AND** the toggle command has returned successfully
- **AND** the frontend then requests the aggregated view
- **THEN** the repository's row is present
- **AND** its dirty rollup and per-instance branches reflect the current git state
- **AND** the freshness applies to the single command return, with no intervening filesystem event required

#### Scenario: Disabling takes effect on the next view request

- **WHEN** the user disables a workspace
- **AND** the toggle command has returned successfully
- **AND** the frontend then requests the aggregated view
- **THEN** the response no longer contains that workspace's row

