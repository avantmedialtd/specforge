## MODIFIED Requirements

### Requirement: Web-Flavoured Workspace Registration

In the web UI the user SHALL be able to register a workspace by supplying its path without a native OS folder dialog, and the supplied path SHALL flow into the same registration operation the desktop frontend uses. The web UI SHALL apply no acceptance rule of its own. A path is accepted or rejected only by the shared operation (see the *Manual Workspace Registration* requirement in the `workspace-registry` capability), and the web UI SHALL show the shared operation's rejection message unaltered.

#### Scenario: Registering a workspace by path in the browser

- **WHEN** the user supplies a workspace path in the web UI and confirms registration
- **THEN** the path is passed to the shared workspace-registration operation
- **AND** registration succeeds or fails by the same rules as the desktop frontend (a folder containing an `openspec/` subdirectory, or a folder inside a git working tree, is accepted)

#### Scenario: Registering a git repository without OpenSpec in the browser

- **WHEN** the user supplies the path of a git repository that has no `openspec/` subdirectory
- **THEN** the repository is registered
- **AND** its top-level row appears in the web UI's tree with the "no OpenSpec" marker (see the *Workspace Tree Hierarchy* requirement in the `spec-browser` capability)
