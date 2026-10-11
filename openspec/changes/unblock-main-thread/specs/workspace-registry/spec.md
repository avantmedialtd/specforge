## ADDED Requirements

### Requirement: Registry Lock Excludes Git Subprocesses

The workspace registry SHALL NOT be held locked while a git subprocess runs on behalf of worktree reconciliation or workspace registration. This covers `git worktree list`, `git rev-parse` for the common directory or the working-tree root, and the per-worktree path canonicalization and working-tree checks that filter git's output. All of that work SHALL be gathered with no registry lock held. The lock is permitted only to read the registry state those facts are combined with and to apply the resulting additions, removals or promotions.

Every decision that depends on registry state SHALL be taken under the lock against the registry's state at that moment, never against a copy taken before the git work began. This includes whether a path is already user-registered, whether a discovered entry is promoted, whether a repository is registered at one of its worktree roots, and which worktrees are new. A registration or reconcile that races another registry mutation SHALL therefore produce the outcome it would have produced had the two run one after the other.

When the registry has changed between gathering and applying on three consecutive attempts, the operation MAY gather once more with the registry lock held, so that it always completes. This bounded fallback is the only case in which a reconcile or registration holds the registry across git.

The git work of reconciliation and registration SHALL NOT run on an async runtime worker thread. It SHALL execute on a blocking pool, so that the runtime stays available to service events and commands while those subprocesses are outstanding. This mirrors the *Non-Blocking Aggregated Recompute* requirement in the `working-tree-status` capability.

The observable outcome of reconciliation and registration is unchanged. The same on-disk worktrees and the same registry state yield the same added, removed and promoted entries, in the same order, with the same persisted file. See the *Dynamic Worktree Tracking via Meta-Watcher*, *Worktree Auto-Discovery* and *Registration Persistence* requirements.

#### Scenario: A worktree reconcile does not hold the registry across git

- **WHEN** a `.git/worktrees/` event triggers a reconcile for a repository and its `git worktree list` is still outstanding
- **AND** a command that reads the registry is issued
- **THEN** the command acquires the registry without waiting for `git worktree list` to complete

#### Scenario: A registration does not hold the registry across its git discovery

- **WHEN** a workspace registration is resolving its repository and listing that repository's worktrees
- **AND** a command that reads the registry is issued
- **THEN** the command acquires the registry without waiting for those git invocations to complete

#### Scenario: A registration re-validates against the current registry

- **WHEN** a folder's registration has gathered its git facts
- **AND** the same folder becomes user-registered by another request before the registration applies
- **THEN** the registration is refused as already registered
- **AND** the registry holds exactly one user-registered entry for that folder

#### Scenario: A discovered worktree registered concurrently is promoted, not duplicated

- **WHEN** a folder's registration has gathered its git facts
- **AND** a reconcile adds that folder as a discovered worktree before the registration applies
- **THEN** the registration promotes the discovered entry to user-registered
- **AND** the registry holds one entry for that folder

#### Scenario: Repeated races fall back to a locked gather and still complete

- **WHEN** the registry changes between gathering and applying on three consecutive attempts of one reconcile
- **THEN** the reconcile gathers once more with the registry lock held and applies the result
- **AND** the registry ends in the state a serial run of the competing mutations and the reconcile would have produced

#### Scenario: Reconcile and registration git work runs off the async runtime

- **WHEN** a reconcile or a registration runs its git subprocesses
- **THEN** those subprocesses are awaited on a blocking pool, not on an async runtime worker thread

#### Scenario: The split reconcile produces the same discovered set

- **WHEN** a reconcile runs over a repository whose `git worktree list` reports a given set of worktrees, prunable ones included
- **THEN** the worktrees added and the paths removed are the same as before this requirement, in the same deterministic order
- **AND** a prunable worktree is still classified as removed

### Requirement: Workspace Commands Run Off the UI Thread

On the desktop, every command that reads the workspace registry, reads the aggregated workspace view, or authorizes a caller-supplied workspace or browse root SHALL run off the main (UI) thread. This covers listing workspaces, reading the workspace views, the archived-artifact status check and opening an artifact link. A command's subprocess or filesystem work SHALL NOT block window painting or input handling.

Authorizing a browse root for the file browser and for document watches SHALL run on a blocking pool, never inline on an async runtime worker. This includes the fallback that resolves a root's git repository when the root is not itself a tracked folder. The same holds for the web transport's dispatch of these commands.

Authorization SHALL canonicalize the caller's path once and compare it against the registry's stored entries as stored, because the registry stores its entries canonical. It SHALL re-resolve stored entries only when that comparison misses. The fallback keeps an entry recorded under a non-canonical spelling (a folder that was missing when it was saved) authorizable exactly as before.

Every command's result SHALL be unchanged. That covers the listed and filtered rows, the presentation fields joined onto them, and the acceptance or refusal of each path. See the *Disabled Rows Excluded From the Tree Pane* and *Presentation Fields on Listed Workspaces* requirements, and the *Browsing Is Confined to Registered Workspaces* requirement in the `workspace-file-browser` capability.

#### Scenario: Reading the workspace view does not block the window

- **WHEN** the desktop frontend requests the workspace views or the workspace list
- **THEN** the view is cloned, filtered, joined and serialized off the main thread
- **AND** the returned rows are identical to those the synchronous command returned for the same state

#### Scenario: A browse-root fallback resolves its repository off the async runtime

- **WHEN** the file browser lists, reads, watches or unwatches a document under a root that is not itself a tracked folder but lies inside a registered repository
- **THEN** the git invocation that resolves the root's repository runs on a blocking pool
- **AND** the root is accepted exactly as before

#### Scenario: Opening an artifact link does not block the window

- **WHEN** the user activates an artifact link whose root needs the repository fallback to authorize
- **THEN** the authorization and its git invocation run off the main thread
- **AND** the link opens, or is refused, exactly as before

#### Scenario: A registered path is authorized without re-resolving every entry

- **WHEN** a command authorizes a path that matches a stored registry entry
- **THEN** the caller's path is canonicalized once
- **AND** the match is found by comparing the stored entries as stored, with no further path resolution

#### Scenario: An entry stored under a non-canonical spelling is still authorized

- **WHEN** a registry entry was saved under a spelling that is not its canonical form
- **AND** a command authorizes a path that canonicalizes to that entry's canonical form
- **THEN** the as-stored comparison misses, the fallback re-resolves the stored entries, and the path is accepted as before

#### Scenario: An unregistered root is still refused

- **WHEN** a command names a root that is neither a tracked folder nor inside a registered repository
- **THEN** it is refused with the same error as before, and nothing is read or watched
