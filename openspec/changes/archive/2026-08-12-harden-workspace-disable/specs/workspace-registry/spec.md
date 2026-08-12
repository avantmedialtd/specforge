# workspace-registry Specification Delta

## MODIFIED Requirements

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
