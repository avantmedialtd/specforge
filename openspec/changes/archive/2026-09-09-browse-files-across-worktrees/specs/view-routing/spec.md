## MODIFIED Requirements

### Requirement: File Addresses

An Address SHALL be able to name one markdown file by its **browse root and root-relative path**, so that any file the workspace file browser can show is linkable, restorable on load, and openable in a reader window. Its URL grammar SHALL place a reserved `file` segment between the scope prefix and the path:

- `/w/<workspace>/file/<path…>` — a file within a flat workspace
- `/r/<repo>/file/<path…>` — a file within a repository, resolved against its tracked worktrees

The path's segments SHALL each be encoded independently and joined with separators, so the address remains a readable path rather than an opaque token, and a path containing characters that require escaping round-trips unchanged.

The `file` segment SHALL be **reserved** at the position a change id otherwise occupies. This is what lets the codec continue to decide the whole grammar from a closed vocabulary with no registry data, as the *Address and URL Round-Trip Through a Pure Codec* requirement demands: without it, a path such as `openspec/specs/<capability>/spec.md` placed directly after a scope prefix is indistinguishable from a capability-spec address followed by a stray segment. A change directory named exactly `file` is consequently not addressable; this is a documented reservation, not a defect to be worked around by making the grammar data-dependent.

A repository-scoped file address SHALL name the **repository**, not one of its worktrees, and SHALL resolve against the repository's pooled listing (see *Union Markdown Listing Across a Repository's Worktrees* in the `workspace-file-browser` capability). Which worktree's copy is rendered is a per-preview choice and is deliberately **not** addressed: resolution SHALL open a default copy — the main worktree's when it holds the path, otherwise the first copy that does.

A file address SHALL NOT carry a worktree instance segment. This prohibition is unchanged and load-bearing: a worktree token is registry data, and admitting one into the grammar would defeat the closed-vocabulary property the reserved `file` segment exists to preserve. Keeping the copy choice out of the Address is what lets that property survive a browse root that now spans several worktrees.

(This supersedes the previous contract, under which a repository-scoped file address named the repository's main worktree. A repository with one tracked worktree resolves exactly as it did before.)

A file address SHALL carry no host filesystem path, only a registry slug and a path relative to the browse root that slug resolves to, per the *Workspace Identity Is a Registry Slug* requirement.

Resolution SHALL follow the *Cold-Load Address Resolution* requirement: a file address into an unknown slug SHALL report not found, one into a disabled workspace SHALL say so, and one naming a file that does not exist beneath a resolvable root SHALL report not found rather than rendering an empty document.

#### Scenario: A file address round-trips

- **WHEN** an Address naming a file within a browse root is encoded to a URL path and decoded again
- **THEN** the decoded Address is equal to the original, including the full relative path

#### Scenario: A nested path keeps its structure

- **WHEN** a file address names a path several directories deep, such as a capability specification beneath the workspace's `openspec/specs/` directory
- **THEN** the encoded path contains that relative path after the reserved `file` segment
- **AND** decoding recovers exactly the same relative path

#### Scenario: A path segment needing escapes survives

- **WHEN** a file address names a path whose segments contain characters that require percent-encoding
- **THEN** the encoded path escapes them per segment
- **AND** decoding recovers the original path unchanged

#### Scenario: The reserved segment disambiguates from an artifact address

- **WHEN** a path whose relative portion begins with `openspec/specs/` is decoded as a file address
- **THEN** it decodes to a file address naming that whole relative path
- **AND** it does not decode to a capability-spec artifact address

#### Scenario: The codec decodes a file address with no registry data

- **WHEN** a file address is decoded with no registered workspaces available
- **THEN** decoding succeeds and yields the scope slug and the relative path
- **AND** no command is dispatched to the backend

#### Scenario: A file address carries no host path

- **WHEN** an Address is formed for a file in a registered workspace
- **THEN** the Address contains that workspace's registry slug and a root-relative path
- **AND** it contains no absolute filesystem path

#### Scenario: A file address carries no worktree segment

- **WHEN** an Address is formed for a file whose copies live in several of a repository's worktrees
- **THEN** the Address names the repository and the root-relative path only
- **AND** it contains no segment identifying a worktree

#### Scenario: A repository file address resolves to a default copy

- **WHEN** a repository-scoped file address is resolved for a repository with several tracked worktrees that all hold the path
- **THEN** it resolves against the repository's main worktree

#### Scenario: A repository file address resolves to the only worktree holding the file

- **WHEN** a repository-scoped file address names a path held by exactly one tracked worktree, which is not the main worktree
- **THEN** it resolves against that worktree
- **AND** it does not report not found

#### Scenario: A file address into an unknown workspace reports not found

- **WHEN** a file address naming a slug that matches no registered workspace or repository is opened
- **THEN** the application reports not found
- **AND** no file is read

#### Scenario: A file address naming a missing file reports not found

- **WHEN** a file address resolves to a registered browse root but names a path that does not exist beneath it
- **THEN** the application reports not found
- **AND** it does not render an empty document
