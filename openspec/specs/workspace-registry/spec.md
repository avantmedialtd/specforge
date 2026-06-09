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

The list of registered workspaces SHALL be persisted to a config file managed by `openspec-core` and restored on application startup.

#### Scenario: Registrations survive restart

- **WHEN** the user registers a workspace
- **AND** quits and relaunches the application
- **THEN** the workspace is still present in the registered-workspaces list
- **AND** the tree pane shows the workspace's non-archived changes

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

The main window SHALL include a settings view, reachable from a discoverable affordance in the main window chrome, that surfaces: the registered-workspaces list with add and remove controls, a per-workspace inline display-name field, a per-workspace palette swatch picker that accepts one of the curated palette tokens or "none", a launch-on-login toggle, and a notifications-enabled toggle.

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

The application SHALL persist a per-top-level-row presentation entry (display name and palette colour) in a presentation store separate from the workspace registry. Presentation entries SHALL be keyed by the identity of the top-level row they decorate: a canonical workspace path for a flat (non-git) workspace, and a canonical git common directory for a repository group. Presentation entries SHALL survive application restarts. When the user unregisters the last user-registered workspace associated with a given key (a flat workspace's path, or any user-registered workspace whose repository identifier matches a repo-keyed entry), the presentation entry for that key SHALL be removed.

The presented display name MUST be normalised so that an empty string is stored as absent. The presented colour MUST be one of a fixed curated palette of tokens (`indigo`, `blue`, `teal`, `green`, `amber`, `orange`, `rose`, `purple`) or absent. No other colour values SHALL be accepted by the store.

#### Scenario: Presentation entry persists across restart

- **WHEN** the user sets a display name and colour for a registered workspace
- **AND** quits and relaunches the application
- **THEN** the presentation entry is restored from disk
- **AND** the workspace continues to render with the chosen display name and colour

#### Scenario: Presentation entry cleaned up when underlying workspace is unregistered

- **WHEN** the user unregisters a flat workspace that has a presentation entry
- **THEN** the presentation entry keyed by that workspace's path is removed from the store
- **AND** re-registering the same path afterwards starts with the default display name and no colour

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

### Requirement: Presentation Fields on Listed Workspaces

The data returned by the workspace listing command and the aggregated repo-view command SHALL include the configured display name (or absent if none) and colour (or absent if none) for each top-level row. When no presentation entry exists for a row, both fields SHALL be absent and consumers SHALL render the row exactly as they did before the presentation store was introduced.

#### Scenario: Workspace list includes display name and colour

- **WHEN** the frontend requests the list of registered workspaces
- **THEN** each workspace entry includes its configured display name (or null) and colour token (or null)
- **AND** workspaces with no presentation entry return null for both fields

#### Scenario: Repo view includes display name and colour for the group

- **WHEN** the frontend requests the aggregated repo-and-flat view
- **THEN** each repo group entry includes its configured display name (or null) and colour token (or null)
- **AND** flat workspace entries in the same view also include their per-workspace display name and colour

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

