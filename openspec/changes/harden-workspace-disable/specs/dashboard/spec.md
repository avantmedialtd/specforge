# dashboard Specification Delta

## MODIFIED Requirements

### Requirement: Ship Selection Opens the Archive Browser

Selecting an entry in the today's ships feed SHALL open the Archive browser with that archived change pre-selected, rather than navigating to the active-change read path — an archived change no longer resides under `openspec/changes/<id>/`. This navigation SHALL be read-only, consistent with the Dashboard's read-only operation.

A feed entry SHALL be resolved to its owning top-level row by the repository it belongs to, not by the worktree path the change was archived from. A change is routinely archived from inside a feature worktree that hosts no active change afterwards, and such a worktree is neither the repository's main worktree nor any active change instance's path; resolving by worktree path alone would fail to open a perfectly reachable repository's ship.

Because the feed is deliberately unfiltered (see the *Dashboard Unaffected by Workspace Disable* requirement), it SHALL also list ships whose top-level row is not present in the tree pane — a disabled row, or one that is no longer registered. Such an entry SHALL be visibly marked as such, and selecting it SHALL navigate to the settings view, where a disabled row is re-enabled and an unregistered one re-added. No feed entry SHALL be rendered as a control that does nothing when selected.

Selecting an entry SHALL NOT itself change any workspace's disabled state: parking is an explicit settings decision, and a navigation gesture never reverses it.

#### Scenario: Selecting a ship opens it in the Archive browser

- **WHEN** the user selects an entry in the today's ships feed
- **THEN** the Archive browser opens with that change pre-selected

#### Scenario: Selecting a ship archived from a worktree with no active change

- **WHEN** a change was archived inside a worktree that now hosts no active change
- **AND** its repository is present in the tree pane
- **THEN** selecting its feed entry opens the Archive browser for that repository with the change pre-selected

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

### Requirement: Dashboard Unaffected by Workspace Disable

Disabling a top-level row (see the *Workspace Disable State* requirement in the
`workspace-registry` capability) SHALL have no effect on any Dashboard surface.
A disabled workspace SHALL continue to contribute to the cross-workspace summary
metrics, the per-repository breakdown, the git-mined activity chart, the change
lifecycle metrics, today's ships feed, the today's-progress hero, the streak and
contribution heatmap, the season standing, and the permanent career tier.

This asymmetry is deliberate. Disabling is an attention control, not an
existence control: it silences the tree pane, the tray badge, and desktop
notifications, while the Dashboard remains the unfiltered record of what the
user has registered and accomplished. It follows that the Dashboard's
active-change total will exceed the number of changes reachable through the tree
pane whenever any workspace is disabled, and the Dashboard SHALL note that its
totals include disabled workspaces so the discrepancy is legible rather than
surprising. That note SHALL count disabled **top-level rows** — the rows the
tree actually drops — and not registered folders: the disabled flag is stored
per row, so a repository the user registered at several worktrees has several
registered folders carrying it while the tree loses exactly one row.

Because the Dashboard reads only cache-derived fields from the aggregated view —
active and archived logical changes, task rollups, and capability-spec counts —
and never the git-derived working-tree fields, a disabled row's omitted git state
SHALL NOT degrade any Dashboard figure.

#### Scenario: Summary metrics include disabled workspaces

- **WHEN** two workspaces are registered, one enabled with five active changes and one disabled with four
- **THEN** the Dashboard's active-change summary reports nine
- **AND** the tree pane shows only the enabled workspace's five

#### Scenario: Per-repository breakdown keeps a row for a disabled workspace

- **WHEN** a registered repository is disabled
- **THEN** the Dashboard's breakdown still shows an entry for it
- **AND** that entry shows its active-change and archived-change counts
- **AND** it is labelled with the same display name it had before being disabled

#### Scenario: Activity chart and lifecycle metrics include disabled repositories

- **WHEN** a disabled repository received commits within the chart's window
- **THEN** those commits are reflected in the activity chart's daily buckets
- **AND** the repository's changes contribute to the lifecycle throughput metrics

#### Scenario: Ships from a disabled workspace still appear

- **WHEN** a change in a disabled workspace is archived today
- **THEN** it appears in today's ships feed
- **AND** the entry is marked as belonging to a disabled workspace
- **AND** selecting it leads to the settings view where the workspace can be re-enabled, rather than doing nothing (see the *Ship Selection Opens the Archive Browser* requirement)

#### Scenario: The disabled-workspace note counts rows, not registered folders

- **WHEN** one repository is registered at two worktrees and is disabled
- **THEN** the Dashboard's note reports one disabled workspace
- **AND** the tree pane has dropped exactly one top-level row

#### Scenario: Streak, heatmap, and season standing are unaffected

- **WHEN** a workspace is disabled for a period during which the user completes tasks and archives changes in it
- **THEN** those days count toward the streak and the contribution heatmap
- **AND** the achievements contribute to the season score, season objectives, and permanent career tier
- **AND** no streak day is lost as a result of the workspace having been disabled

#### Scenario: Dashboard renders when every workspace is disabled

- **WHEN** every registered workspace is disabled
- **THEN** the Dashboard renders without error
- **AND** its summary metrics, breakdown, and activity chart still reflect all registered workspaces
- **AND** the tray badge is hidden and the tree pane is empty
