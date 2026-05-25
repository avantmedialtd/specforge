# workspace-registry

## ADDED Requirements

### Requirement: Git Repository Detection

When a workspace is registered, the application SHALL detect whether the workspace lives inside a git repository by invoking `git rev-parse --git-common-dir` against the workspace path. The canonicalised result of that command identifies the repository for the purpose of grouping worktrees. Workspaces that are not inside a git repository SHALL continue to be treated as standalone (flat) workspaces and are not subject to worktree aggregation.

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
