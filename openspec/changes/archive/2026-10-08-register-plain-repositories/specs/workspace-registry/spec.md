## MODIFIED Requirements

### Requirement: Manual Workspace Registration

The application SHALL allow the user to register a workspace by selecting a folder on disk that either contains an `openspec/` subdirectory or lies inside a git working tree. A folder that contains an `openspec/` subdirectory SHALL be registered at the selected path. A folder that lacks an `openspec/` subdirectory but lies inside a git working tree SHALL be registered at that working tree's root, as git itself reports it (`git rev-parse --show-toplevel`). That root is the deepest linked worktree containing the folder, or a submodule's or `--separate-git-dir` repository's own work tree. It is never a git directory. A folder that neither contains an `openspec/` subdirectory nor lies inside a git working tree MUST be rejected with a user-visible message that names both accepted forms. When the `git` binary cannot be invoked, only a folder containing an `openspec/` subdirectory SHALL be accepted.

A working-tree root that is the user's home directory or a filesystem root MUST NOT be registered without an `openspec/` subdirectory. Such a root is too broad to watch, query and list. The rejection message SHALL say so and SHALL name what to pick instead. This guards a mistaken pick, such as a downloads folder inside a home directory kept under git for dotfiles.

#### Scenario: Valid folder is registered

- **WHEN** the user opens the settings view and selects a folder containing an `openspec/` subdirectory
- **THEN** the folder is added to the registered-workspaces list
- **AND** the folder appears in the tree pane as a top-level workspace node

#### Scenario: Invalid folder is rejected

- **WHEN** the user selects a folder that does not contain an `openspec/` subdirectory and does not lie inside a git working tree
- **THEN** the folder is not added to the registered-workspaces list
- **AND** a message indicates the folder is neither an OpenSpec workspace nor a git repository

#### Scenario: A git repository without OpenSpec is registered

- **WHEN** the user selects the root of a git working tree that has no `openspec/` subdirectory
- **THEN** the folder is added to the registered-workspaces list
- **AND** it appears in the tree pane as a top-level row reporting no OpenSpec (see *OpenSpec Presence Is Derived on Every Aggregation*)

#### Scenario: A subfolder of a git working tree registers the working tree's root

- **WHEN** the user selects `acme-api/docs/`, a folder without an `openspec/` subdirectory inside the working tree rooted at `acme-api/`
- **THEN** `acme-api/` is added to the registered-workspaces list
- **AND** `acme-api/docs/` is not

#### Scenario: A subfolder containing OpenSpec is registered as selected

- **WHEN** the user selects `monorepo/packages/specs/`, which contains an `openspec/` subdirectory, inside the working tree rooted at `monorepo/`
- **THEN** `monorepo/packages/specs/` is added to the registered-workspaces list

#### Scenario: Without git only an OpenSpec folder is accepted

- **WHEN** the `git` binary cannot be invoked and the user selects a folder without an `openspec/` subdirectory
- **THEN** the folder is not added to the registered-workspaces list
- **AND** the message names both accepted forms

#### Scenario: A separate-git-dir work tree registers itself, not its git store

- **WHEN** the user selects a subfolder of the work tree of a repository created with `--separate-git-dir`, and the work tree has no `openspec/` subdirectory
- **THEN** the work tree's root is added to the registered-workspaces list
- **AND** the git store is not

#### Scenario: A home directory under git is too broad to register without OpenSpec

- **WHEN** the user's home directory is a git working tree and the user selects `~/Downloads/`, which has no `openspec/` subdirectory
- **THEN** nothing is added to the registered-workspaces list
- **AND** a message explains that the repository is too broad to track and names what to pick instead

### Requirement: Filesystem Watching of Registered Workspaces

For each registered workspace, the application SHALL watch the workspace's `openspec/changes/` directory for additions, removals, and modifications, using a debounced event stream to coalesce bursts of filesystem events.

A tracked workspace that has no `openspec/` subdirectory when its watcher is established SHALL instead have the **direct entries** of its own folder watched, never its folder recursively, so that a build or dependency install inside it delivers nothing to the debounced stream. When an `openspec/` subdirectory appears in that folder, the application SHALL begin watching it exactly as for a workspace that had one from the start. Within the same debounce window it SHALL re-parse the workspace's changes and refresh the aggregated view, so that its changes and its OpenSpec presence appear without a restart or re-registration.

#### Scenario: Watcher established on registration

- **WHEN** a workspace containing an `openspec/` subdirectory is added to the registered list
- **THEN** a filesystem watcher is established on that workspace's `openspec/changes/` directory

#### Scenario: Watcher disposed on removal

- **WHEN** a workspace is removed from the registered list
- **THEN** the filesystem watcher for that workspace is disposed

#### Scenario: Burst of edits is coalesced

- **WHEN** multiple files inside one registered workspace are modified within the debounce window
- **THEN** the cache and UI receive a single coalesced update event, not one event per file

#### Scenario: A workspace without OpenSpec does not watch its tree recursively

- **WHEN** a tracked workspace has no `openspec/` subdirectory and thousands of files are written beneath a nested build directory inside it
- **THEN** no re-parse and no view refresh is triggered by those writes

#### Scenario: An OpenSpec folder created after registration is picked up

- **WHEN** a registered git repository without an `openspec/` subdirectory gains one, for example because the user ran `openspec init` and then created a change in it
- **THEN** within the debounce window the repository's top-level row reports OpenSpec present
- **AND** the new change appears in the tree pane without a restart or re-registration
- **AND** later edits under its `openspec/changes/` directory are observed like any other workspace's

### Requirement: Worktree Auto-Discovery

When a workspace inside a git repository is registered, the application SHALL automatically discover every other worktree of the same repository via `git worktree list --porcelain` and register each discovered worktree as a tracked workspace with origin `Discovered`. Discovered workspaces SHALL NOT be filtered by path — worktrees under `.claude/worktrees/` or any other location are included.

Nor SHALL discovered workspaces be filtered by whether they contain an `openspec/` subdirectory, for any repository that has at least one user-registered workspace located at one of its worktree roots. A worktree without an `openspec/` folder is tracked like any other. It contributes no changes, but it contributes its branch to pull-request links, its markdown files to the repository's file browser, and its working-tree status to the repository's dirty rollup. A repository whose user-registered workspaces all lie **below** their worktree roots SHALL discover only the worktrees whose root contains an `openspec/` subdirectory.

A root that `git worktree list --porcelain` names but that holds no `.git` entry SHALL never be tracked. Such a root is a bare repository's own directory, or the git store that a submodule or `--separate-git-dir` repository lists as its main worktree. None of these is a working tree.

The same rules apply to runtime reconciliation (see *Dynamic Worktree Tracking via Meta-Watcher*).

#### Scenario: Sibling worktrees are auto-discovered at registration time

- **WHEN** the user registers a workspace that is a worktree of a repository with two other existing worktrees
- **THEN** the application registers the two other worktrees as tracked workspaces with origin `Discovered`
- **AND** each discovered worktree contributes its changes to the repository's aggregated view

#### Scenario: Harness worktrees under `.claude/worktrees/` are auto-discovered

- **WHEN** the repository has a worktree whose path is under `.claude/worktrees/`
- **THEN** the application discovers and tracks that worktree
- **AND** does not filter or hide it from the aggregated view

#### Scenario: A worktree without an OpenSpec folder is discovered

- **WHEN** a repository registered at its main worktree's root has a second worktree on a branch that predates the repository's `openspec/` folder
- **THEN** the second worktree is tracked with origin `Discovered`
- **AND** a pull request whose head branch is checked out in it links to it (see the *Matching a Pull Request to a Worktree* requirement in the `pull-request-worktree-links` capability)

#### Scenario: Every worktree of a repository without OpenSpec is discovered

- **WHEN** the user registers the root of a git repository without OpenSpec that has two other worktrees
- **THEN** both other worktrees are tracked with origin `Discovered`

#### Scenario: A repository registered below its root keeps the OpenSpec filter

- **WHEN** a repository's only user-registered workspace is `monorepo/packages/specs/`, and the repository has a second worktree whose root has no `openspec/` subdirectory
- **THEN** the second worktree is not tracked
- **AND** the repository's own worktree root is not tracked beside the registered subfolder

#### Scenario: A git store is never tracked as a worktree

- **WHEN** the user registers the root of a linked worktree of a repository whose main entry in `git worktree list` is a bare directory or a `--separate-git-dir` store
- **THEN** that directory or store is not tracked, at registration or at any later reconciliation

## ADDED Requirements

### Requirement: OpenSpec Presence Is Derived on Every Aggregation

Every top-level row of the aggregated view SHALL report whether it has OpenSpec, as a boolean `hasOpenSpec` on both the repository variant and the flat-workspace variant:

- A repository has OpenSpec when **any** of its tracked worktrees contains an `openspec/` subdirectory.
- A flat workspace has OpenSpec when its folder contains one.

$$\text{hasOpenSpec}(\text{repo}) = \bigvee_{w \,\in\, \text{tracked worktrees}} \big[\, w/\texttt{openspec} \text{ is a directory} \,\big]$$

The value SHALL be derived from the filesystem each time the view is aggregated, without spawning a git process, so a disabled row aggregated cold reports it as accurately as a warm one. It SHALL NOT be written to the registry's config file or to the presentation store, and no configuration written by an earlier version SHALL need migrating for it. A row reporting no OpenSpec SHALL contribute zero active changes, zero archived changes, nothing to the tray badge and no change notification, and SHALL otherwise remain an ordinary top-level row.

#### Scenario: A repository without OpenSpec reports no OpenSpec

- **WHEN** a registered git repository has no `openspec/` subdirectory in any tracked worktree
- **THEN** its row in the aggregated view carries `hasOpenSpec: false`
- **AND** it contributes zero active changes and nothing to the tray badge

#### Scenario: OpenSpec on a feature branch only counts

- **WHEN** a repository's main worktree has no `openspec/` subdirectory but one of its tracked worktrees does
- **THEN** the repository's row carries `hasOpenSpec: true`

#### Scenario: Presence follows the disk without re-registration

- **WHEN** the user runs `openspec init` in a registered repository that previously had no `openspec/` subdirectory
- **THEN** the next aggregation reports `hasOpenSpec: true` for its row
- **AND** the registry's config file is unchanged

#### Scenario: Presence is never persisted

- **WHEN** the registry's config file is saved after registering a repository without OpenSpec
- **THEN** the file is the same plain ordered array of `{ uri, name }` entries as for any other registration
- **AND** it contains no field recording OpenSpec presence

#### Scenario: A disabled row reports presence without git

- **WHEN** a disabled repository is aggregated cold
- **THEN** its `hasOpenSpec` value matches the filesystem
- **AND** no git subprocess is spawned to compute it

#### Scenario: The flag crosses the boundary in camelCase

- **WHEN** a repository row and a flat-workspace row are serialized for a frontend
- **THEN** each carries the key `hasOpenSpec`
- **AND** neither carries `has_open_spec` or any other snake_case key
