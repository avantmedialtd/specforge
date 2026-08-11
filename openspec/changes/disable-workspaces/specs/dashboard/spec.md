# dashboard Specification Delta

## ADDED Requirements

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
surprising.

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
- **AND** selecting it opens the archive browser as it would for an enabled workspace

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
