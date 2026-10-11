## ADDED Requirements

### Requirement: Main Worktree Resolved Without a Subprocess

The aggregated recompute SHALL resolve each repository row's main worktree with no
git subprocess, for warm and cold rows alike. It SHALL NOT invoke
`git worktree list` to determine the main worktree. Worktree discovery and
tracking (see *Worktree Auto-Discovery* and *Dynamic Worktree Tracking via
Meta-Watcher*) is separate from aggregation, and this requirement does not change
it.

The resolved main worktree SHALL equal the path git itself reports as the main
worktree (the first entry of `git worktree list --porcelain`) for every
repository layout: a plain repository whose common directory is
`<worktree>/.git`, a `--separate-git-dir` store, a submodule whose common
directory lies under `<superproject>/.git/modules/<name>`, and a bare repository.
For a WSL repository the resolved path SHALL be in the same Windows UNC form the
registry stores for that repository's worktrees (see the *Linux and UNC Path
Translation* requirement in the `wsl-workspaces` capability). Because warm and
cold rows share the derivation, a row's main worktree, the name derived from it,
and which instance is tagged as the main worktree SHALL NOT change when the row is
disabled or re-enabled (see *Cold Aggregation of Disabled Rows*).

#### Scenario: A recompute spawns no worktree listing

- **WHEN** a warm repository row is recomputed, by a file-change batch, a git
  event, or the window-focus refresh
- **THEN** no `git worktree list` invocation is issued as part of that recompute
- **AND** the row's main worktree, name and main-worktree instance tag are
  unchanged from what they were before the recompute

#### Scenario: The derived main worktree matches git for every layout

- **WHEN** a repository is created in each of the plain, separate-git-dir,
  submodule and bare layouts
- **THEN** for each, the main worktree the aggregation resolves equals the first
  entry of that repository's `git worktree list --porcelain`

#### Scenario: Disabling a row does not change its main worktree

- **WHEN** a warm repository row is disabled and aggregated cold
- **THEN** its main worktree and derived name are identical to the warm row's
