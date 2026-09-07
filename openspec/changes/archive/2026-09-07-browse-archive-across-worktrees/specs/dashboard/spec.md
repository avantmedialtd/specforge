## MODIFIED Requirements

### Requirement: Ship Selection Opens the Archive Browser

Selecting an entry in the today's ships feed SHALL open the Archive browser with that archived change pre-selected, rather than navigating to the active-change read path — an archived change no longer resides under `openspec/changes/<id>/`. This navigation SHALL be read-only, consistent with the Dashboard's read-only operation.

A feed entry SHALL be resolved to its owning top-level row by the repository it belongs to, not by the worktree path the change was archived from. A change is routinely archived from inside a feature worktree that hosts no active change afterwards, and such a worktree is neither the repository's main worktree nor any active change instance's path; resolving by worktree path alone would fail to open a perfectly reachable repository's ship.

Pre-selection SHALL hold regardless of how the archiving worktree entered the registry. A worktree that SpecForge auto-discovered — rather than one the user registered directly — is the ordinary case for a project that archives from inside feature worktrees, and it is the case in which the archived change exists in **no other** worktree of the repository. The application SHALL NOT fall back to the repository's main worktree when the named change is absent there, and SHALL NOT silently discard the pre-selection because the archiving worktree is missing from the user-registered listing. Because the Archive browser lists a repository's archived changes across all of its tracked worktrees (see the *Union Archive Listing Across a Repository's Worktrees* requirement in the `archive-browser` capability), the named change is present in that listing whichever worktree holds it.

Because the feed is deliberately unfiltered (see the *Dashboard Includes Disabled Workspaces* requirement), it SHALL also list ships whose top-level row is not present in the tree pane — a disabled row, or one that is no longer registered. Such an entry SHALL be visibly marked as such, and selecting it SHALL navigate to the settings view, where a disabled row is re-enabled and an unregistered one re-added. No feed entry SHALL be rendered as a control that does nothing when selected.

Selecting an entry SHALL NOT itself change any workspace's disabled state: parking is an explicit settings decision, and a navigation gesture never reverses it.

#### Scenario: Selecting a ship opens it in the Archive browser

- **WHEN** the user selects an entry in the today's ships feed
- **THEN** the Archive browser opens with that change pre-selected

#### Scenario: Selecting a ship archived from a worktree with no active change

- **WHEN** a change was archived inside a worktree that now hosts no active change
- **AND** its repository is present in the tree pane
- **THEN** selecting its feed entry opens the Archive browser for that repository with the change pre-selected

#### Scenario: Selecting a ship archived in an auto-discovered worktree

- **WHEN** a change was archived inside a worktree that SpecForge auto-discovered rather than one the user registered directly
- **AND** the change is absent from the repository's main worktree because its branch has not merged
- **THEN** selecting its feed entry opens the Archive browser with that change pre-selected
- **AND** the pre-selection is not discarded because the archiving worktree is absent from the user-registered workspace listing
- **AND** the view does not fall back to the repository's main worktree, whose archive does not contain the change

#### Scenario: Selecting a ship whose top-level row is disabled

- **WHEN** the user selects a feed entry whose owning repository is disabled
- **THEN** the settings view opens, where the workspace's toggle can be switched back on
- **AND** the workspace's disabled state is unchanged by the selection

#### Scenario: A ship whose top-level row is not in the tree is marked in the feed

- **WHEN** the today's ships feed renders an entry whose owning top-level row is disabled or no longer registered
- **THEN** the entry is marked as such alongside its workspace label
- **AND** the entry is still listed

#### Scenario: Ship selection performs no mutation

- **WHEN** the user selects an entry in the today's ships feed
- **THEN** the only effect is navigation — into the Archive browser, or into the settings view for a row that is not in the tree
- **AND** no spec, task, change, git state, or workspace disabled state is modified
