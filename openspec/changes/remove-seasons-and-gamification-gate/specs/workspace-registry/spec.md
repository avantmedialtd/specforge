## MODIFIED Requirements

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
