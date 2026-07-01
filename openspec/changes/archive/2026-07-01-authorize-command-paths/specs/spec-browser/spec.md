# spec-browser

## ADDED Requirements

### Requirement: Artifact Reads Are Confined to Registered Workspaces

Reading an OpenSpec artifact's markdown SHALL be authorized only when the workspace it is read from is a registered (or registry-discovered) workspace, and a caller-supplied workspace that is not in the registry SHALL be refused rather than read, even when the requested path resolves to a real `openspec/changes/…` file on disk. This authorization SHALL be applied in addition to the existing path-traversal guard (which keeps the resolved file within the workspace's `openspec/changes/` subtree): the traversal guard bounds *where within a workspace* a read may reach, and this requirement bounds *which workspaces* may be read at all. The workspace SHALL be matched by its canonical path against the registry's known workspace folders using the same canonicalization the registry keys on, and the check SHALL be enforced at the shared application boundary so it holds for every frontend and transport that can read artifacts.

#### Scenario: An artifact read against an unregistered workspace is refused

- **WHEN** an artifact-read is requested for a workspace path that is not a registered or registry-discovered workspace
- **THEN** the read is refused with an error
- **AND** no file under that path is read, even if an `openspec/changes/.../<artifact>.md` file exists there

#### Scenario: An artifact read against a registered workspace succeeds

- **WHEN** an artifact-read is requested for a change in a registered workspace
- **THEN** the artifact's markdown is returned as before, subject to the existing path-traversal guard

#### Scenario: The confinement holds across transports

- **WHEN** an artifact-read is reached through the optional web command endpoint rather than the desktop command surface
- **THEN** the same registered-workspace requirement applies, because it is enforced at the shared application boundary
