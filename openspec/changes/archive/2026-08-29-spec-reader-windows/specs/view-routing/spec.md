# view-routing Delta — Reader Windows for Workspace Documents

## MODIFIED Requirements

### Requirement: Addressable Viewing State

The application SHALL represent what the center pane is currently showing as an **Address**: a serializable value composed only of stable identifiers.

The Address SHALL be able to name the home surface, the settings pane, the archive browser (optionally a specific archived change), the workspace file browser, one markdown file within a browse root, and a change artifact (`proposal`, `design`, `tasks`, or a named capability spec).

The Address SHALL NOT carry resolved payloads, derived display labels, or any other value that can be re-derived from the registered workspace views. Tree nodes that render nothing — change disclosure rows and the Specs artifact node, see the *Deferred Interaction Nodes* requirement in the `spec-browser` capability — SHALL NOT be addressable, so the set of addresses matches the set of states that actually render.

An Address names *what* is shown, never *where* or *how* it is shown. Whether a document is presented in the main window's detail pane or in a reader window SHALL NOT be part of its Address, in the same way that side-pane visibility is not (see the *Side-Pane Visibility Toggles* requirement in the `spec-browser` capability, and the *Reader Presentation Is Not Part of the Address* requirement in the `reader-window` capability).

#### Scenario: An address carries identifiers only

- **WHEN** the user selects an artifact and the application forms its Address
- **THEN** the Address contains only identifiers (workspace slug, change id, artifact kind, optional capability name)
- **AND** it contains no display label, no file contents, and no preloaded view payload

#### Scenario: A non-rendering node has no address

- **WHEN** the user clicks a change disclosure row or the Specs artifact node
- **THEN** the center pane's contents are unchanged
- **AND** no new Address is formed and no history entry is created

#### Scenario: A file selected in the browser has an address

- **WHEN** the user selects a markdown file in the workspace file browser
- **THEN** an Address naming that file within its browse root is formed

#### Scenario: Presentation is not addressed

- **WHEN** the same document is shown in the detail pane and in a reader window
- **THEN** both are named by the same Address

## ADDED Requirements

### Requirement: File Addresses

An Address SHALL be able to name one markdown file by its **browse root and root-relative path**, so that any file the workspace file browser can show is linkable, restorable on load, and openable in a reader window. Its URL grammar SHALL place a reserved `file` segment between the scope prefix and the path:

- `/w/<workspace>/file/<path…>` — a file within a flat workspace
- `/r/<repo>/file/<path…>` — a file within a repository's main worktree

The path's segments SHALL each be encoded independently and joined with separators, so the address remains a readable path rather than an opaque token, and a path containing characters that require escaping round-trips unchanged.

The `file` segment SHALL be **reserved** at the position a change id otherwise occupies. This is what lets the codec continue to decide the whole grammar from a closed vocabulary with no registry data, as the *Address and URL Round-Trip Through a Pure Codec* requirement demands: without it, a path such as `openspec/specs/<capability>/spec.md` placed directly after a scope prefix is indistinguishable from a capability-spec address followed by a stray segment. A change directory named exactly `file` is consequently not addressable; this is a documented reservation, not a defect to be worked around by making the grammar data-dependent.

A repository-scoped file address SHALL name the repository's **main worktree**, consistent with the file-browser address that carries no instance segment. A file address SHALL NOT carry a worktree instance segment.

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

#### Scenario: A repository file address names the main worktree

- **WHEN** a repository-scoped file address is resolved for a repository with several active worktrees
- **THEN** it resolves against the repository's main worktree

#### Scenario: A file address into an unknown workspace reports not found

- **WHEN** a file address naming a slug that matches no registered workspace or repository is opened
- **THEN** the application reports not found
- **AND** no file is read

#### Scenario: A file address naming a missing file reports not found

- **WHEN** a file address resolves to a registered browse root but names a path that does not exist beneath it
- **THEN** the application reports not found
- **AND** it does not render an empty document
