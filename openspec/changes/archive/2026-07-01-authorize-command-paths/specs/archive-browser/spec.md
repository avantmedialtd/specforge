# archive-browser

## ADDED Requirements

### Requirement: Archive Reads Are Confined to Registered Workspaces

Listing a workspace's archived changes and reporting an archived change's artifact status SHALL be authorized only when the workspace is a registered (or registry-discovered) workspace, and a caller-supplied workspace that is not in the registry SHALL be refused rather than read. The workspace SHALL be matched by its canonical path against the registry's known workspace folders using the same canonicalization the registry keys on, and the check SHALL be enforced at the shared application boundary so it applies to every frontend and transport, matching the artifact-read confinement in the `spec-browser` capability. The existing sanitization of the archive directory name (rejecting path separators and `..`) SHALL remain in force.

#### Scenario: Archive listing for an unregistered workspace is refused

- **WHEN** the archive listing or archived-artifact-status operation is invoked for a workspace path that is not a registered or registry-discovered workspace
- **THEN** the operation is refused with an error
- **AND** no directory under that path is enumerated and no archived file is read

#### Scenario: Archive listing for a registered workspace succeeds

- **WHEN** the archive listing or archived-artifact-status operation is invoked for a registered workspace
- **THEN** it returns that workspace's archived changes or the archived change's artifact status as before

#### Scenario: Archive directory-name sanitization still applies

- **WHEN** an archived-artifact-status request supplies a directory name containing a path separator or a `..` segment
- **THEN** it is rejected as an invalid archive directory name, independently of the registration check
