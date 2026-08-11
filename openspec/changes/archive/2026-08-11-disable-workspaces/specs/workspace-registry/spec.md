# workspace-registry Specification Delta

## ADDED Requirements

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
commit state — SHALL take defined default values rather than stale ones. The
row's name SHALL be resolved without a subprocess, using the same fallback the
application already applies when git is unavailable.

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

### Requirement: Disabled Rows Excluded From the Tree Pane

The aggregated view returned to a frontend SHALL exclude every disabled
top-level row, so the tree pane shows only enabled workspaces. The exclusion
SHALL be applied once, in the shared command that serves the aggregated view, so
that the desktop, web, and terminal frontends all observe it without
frontend-specific filtering.

#### Scenario: A disabled workspace disappears from the tree

- **WHEN** the user disables a registered workspace
- **THEN** the tree pane no longer shows a top-level row for it
- **AND** none of its changes are reachable through the tree

#### Scenario: Every frontend observes the same exclusion

- **WHEN** a workspace is disabled
- **AND** the aggregated view is requested by the desktop, web, or terminal frontend
- **THEN** none of the three responses contains the disabled workspace's row

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
- **AND** it contributes to the streak, contribution heatmap, and season score exactly as it would for an enabled workspace

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

## MODIFIED Requirements

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
be removed.

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
