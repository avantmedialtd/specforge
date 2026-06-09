## MODIFIED Requirements

### Requirement: Cross-Workspace Summary Metrics

The Dashboard SHALL present, aggregated across every registered workspace, the total number of active (non-archived) changes — rendered as a compact summary line alongside the total archived count, not as a metric card. The Dashboard SHALL NOT present standalone Overview cards for the task rollup, for the count of active changes that touch a capability spec, or for the registered repository/worktree counts.

#### Scenario: Active-change summary reflects all workspaces

- **WHEN** the Dashboard renders with multiple registered workspaces
- **THEN** the active-change count equals the total number of non-archived changes across all of them

#### Scenario: No Overview summary cards

- **WHEN** the Dashboard renders its analytics
- **THEN** no card for the task rollup, the changes-touching-specs count, or the repository/worktree counts is shown

#### Scenario: Empty registry

- **WHEN** no workspaces are registered
- **THEN** the Dashboard renders without error
- **AND** the active-change summary shows a zero count
