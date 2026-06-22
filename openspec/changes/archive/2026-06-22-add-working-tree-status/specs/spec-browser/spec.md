# spec-browser

## ADDED Requirements

### Requirement: Working-Tree Status Indicators

The tree pane SHALL surface git working-tree status for git-backed repositories
through two indicator families, leaving non-git (flat) workspaces unchanged.

On each repository node, when the repository's dirty rollup is set, the tree
SHALL render a whole-repo **dirty** indicator; when the repository additionally
has uncommitted specs, the tree SHALL render a **distinct** specs-uncommitted
indicator alongside it, so that an uncommitted source file is visually
distinguishable from an uncommitted spec. Both indicators SHALL be absent when
the repository is clean.

On each change-instance row, the tree SHALL render a commit-state chip when the
instance's spec commit state is `Modified` or `Untracked`, positioned alongside
the existing divergence chip. A `Committed` instance SHALL render no such chip.

#### Scenario: Repo with an uncommitted spec shows both rollup indicators

- **WHEN** a repository node renders and the repository has a worktree with an
  untracked or modified change directory
- **THEN** the node shows the whole-repo dirty indicator
- **AND** the node shows the distinct specs-uncommitted indicator

#### Scenario: Repo dirty only from non-spec files shows one indicator

- **WHEN** a repository is dirty solely from files outside `openspec/`
- **THEN** the node shows the whole-repo dirty indicator
- **AND** the node does not show the specs-uncommitted indicator

#### Scenario: Clean repo shows no indicators

- **WHEN** a repository and all its worktrees are clean
- **THEN** the repository node shows neither indicator

#### Scenario: Untracked instance shows a commit-state chip

- **WHEN** a change-instance row renders for a worktree whose copy of the change
  is untracked
- **THEN** the row shows an "untracked" commit-state chip beside the divergence
  chip

#### Scenario: Committed instance shows no commit-state chip

- **WHEN** a change-instance row renders for a worktree whose copy of the change
  is fully committed
- **THEN** the row shows no commit-state chip

#### Scenario: Flat workspace is unaffected

- **WHEN** a non-git (flat) workspace renders in the tree
- **THEN** no working-tree status indicators are shown for it
