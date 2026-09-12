## MODIFIED Requirements

### Requirement: Navigation Reveal Is Transient

When an address resolves to an artifact of a change, the application SHALL reveal that change in the workspace tree: the change's top-level row (repository group or flat workspace) SHALL be shown open and the change's row SHALL be shown selected. Which artifact and which instance the address names is shown by the change header in the detail pane (see *Artifact Tab Strip in the Change Header* and *Instance Switcher in the Change Header* in the `spec-browser` capability), not by the tree, which stops at the change row.

The reveal SHALL be a pure function of the resolved address, never independently tracked state. A top-level row opened by a reveal SHALL return to the disclosure state it had once the user navigates to an address that does not reveal a change beneath it. The tree persists no disclosure state across sessions (see *Workspace Tree Hierarchy* in the `spec-browser` capability), so a reveal has nothing it could write and SHALL perform no settings write.

#### Scenario: An artifact address reveals its change

- **WHEN** an address names an artifact of a change whose top-level row is currently closed
- **THEN** that row is shown open so the change's row is visible
- **AND** the change's row is shown as selected
- **AND** the addressed artifact is the active tab in the change header

#### Scenario: Following a link performs no settings write

- **WHEN** the user follows an address that opens a top-level row they had closed
- **THEN** no settings write is performed as a result of the reveal

#### Scenario: A revealed row reverts after navigating away

- **WHEN** a top-level row was opened by a reveal and the user then navigates to an address that reveals nothing beneath it
- **THEN** that row renders in the disclosure state it had before the reveal
