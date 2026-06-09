## MODIFIED Requirements

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
