# commit-graph

## ADDED Requirements

### Requirement: Commit References Are Injection-Safe Arguments

Any commit reference supplied to a commit-reading operation (the commit-detail file list and the per-file diff) SHALL be treated as untrusted data rather than as a command-line argument to the underlying git invocation, such that no reference value can cause git to write, delete, or otherwise mutate any file or the working tree, nor invoke an external program — it can only cause git to read the named commit. To achieve this the application SHALL both (a) reject a reference that is not a plausible git object id (a hexadecimal string of 4 to 64 characters) before it is used, and (b) pass the reference to git in a position that git cannot interpret as an option (after an end-of-options marker). Guarantee (b) SHALL hold at the point where the git command is constructed, so that it protects every frontend and transport that can reach these operations — the desktop command surface and the optional web command endpoint alike — independent of any per-frontend validation. This strengthens the *Read-Only Operation* requirement: that one ensures the UI offers no mutating action; this one ensures the argument-passing path cannot be coerced into a mutating action either.

#### Scenario: A reference shaped like an option cannot write a file

- **WHEN** a commit-reading operation is invoked with a reference value that resembles a git option that would write to a path (for example, a value requesting diff output be written to a file)
- **THEN** no file is created, truncated, or modified as a result
- **AND** the operation returns an error or an empty result rather than executing the option

#### Scenario: A malformed reference is rejected

- **WHEN** a commit-reading operation is invoked with a reference that is not a hexadecimal object id (for example, an empty string, a branch name, or a leading-dash string)
- **THEN** the operation is refused with an error indicating an invalid reference
- **AND** git is not asked to act on that value

#### Scenario: A legitimate commit hash still resolves

- **WHEN** a commit-reading operation is invoked with a valid commit hash from the graph
- **THEN** the operation returns that commit's file list or diff as before

#### Scenario: The guarantee holds across transports

- **WHEN** a commit-reading operation is reached through the optional web command endpoint rather than the desktop command surface
- **THEN** the same reference-safety guarantees apply, because they are enforced where the git command is constructed rather than in a single frontend
