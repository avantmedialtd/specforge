# commit-graph

## ADDED Requirements

### Requirement: Commit Reading Is Restricted to Registered Repositories

A commit-reading operation (the graph, the commit-detail file list, and the per-file diff) SHALL act only on a repository that belongs to a registered workspace, and SHALL refuse a caller-supplied repository identifier that is not the git repository of any registered workspace rather than reading it. Authorization SHALL be decided by comparing the canonical form of the supplied identifier against the canonical git directories of the registered workspaces, using the same path-canonicalization the registry uses to key its entries, so that an equivalent but differently spelled path is neither wrongly refused nor able to evade the check. This authorization SHALL be enforced at the shared application boundary so that it holds identically for every frontend and transport — the desktop command surface and the optional web command endpoint alike — and not only for whichever transport happens to route through that boundary today. This complements *Graceful Degradation Without Git*: a registered-but-unreadable repository still degrades to an empty rail, whereas an unregistered repository is refused as unauthorized.

#### Scenario: An unregistered repository is refused

- **WHEN** a commit-reading operation is invoked with a repository identifier that is not the git repository of any registered workspace
- **THEN** the operation is refused and no commit history, file list, or diff of that repository is returned
- **AND** no `git` command is run against that repository

#### Scenario: A registered repository is read normally

- **WHEN** a commit-reading operation is invoked with the repository of a registered workspace
- **THEN** the operation returns that repository's graph, file list, or diff as before

#### Scenario: The restriction holds across transports

- **WHEN** a commit-reading operation is reached through the optional web command endpoint rather than the desktop command surface
- **THEN** the same registration check applies, because it is enforced at the shared application boundary both transports use

#### Scenario: Path spelling does not defeat or trip the check

- **WHEN** a registered repository is identified by an equivalent but differently spelled path (for example with a trailing separator, a `..` segment, a symlink, or a platform verbatim prefix)
- **THEN** it is recognized as the same registered repository and read normally
