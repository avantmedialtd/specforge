# workspace-registry Delta — Aggregate Views on Workspace Registration

## ADDED Requirements

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
