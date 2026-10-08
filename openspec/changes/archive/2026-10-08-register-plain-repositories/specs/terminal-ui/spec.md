## MODIFIED Requirements

### Requirement: Workspace Management from the Terminal

The interactive frontend SHALL allow the user to manage the registered-workspace set from the Settings screen: add a workspace by path (an OpenSpec workspace or a git repository, by the shared rule in the *Manual Workspace Registration* requirement of the `workspace-registry` capability), remove a user-registered workspace, set a workspace's display name and palette colour, and disable or re-enable a workspace. These operations SHALL go through the shared application service so the registry, the filesystem watcher, and the presentation store stay consistent, and their effects SHALL appear in the running frontend without a restart. The Settings workspace list SHALL present the user-registered workspaces only; auto-discovered worktrees SHALL NOT appear as manageable rows.

Every operation available on the focused workspace row SHALL be advertised in the frontend's key hints, so no control — the disable toggle included — is reachable only by prior knowledge. Because disabling removes a workspace's top-level row from the Browse tree, the Settings list SHALL remain its home: a disabled workspace SHALL stay listed there, SHALL be visibly marked as disabled, and SHALL be re-enabled from that same row, so the terminal frontend is a complete surface for the operation and does not require the desktop shell or the web UI to undo it.

Disabling SHALL be applied immediately without a confirmation step — it is reversible from the row that performed it — and SHALL be keyed the same way the shared service keys presentation overrides, so sibling worktrees of one repository share a single disabled state.

#### Scenario: Add a workspace by path

- **WHEN** the user invokes the add-workspace control and enters the path of a folder that contains an `openspec/` subdirectory, or of a folder inside a git working tree
- **THEN** the folder is registered, a filesystem watcher is established for it, and it appears in the Settings workspace list and the Browse tree without a restart

#### Scenario: Invalid path is rejected with a message

- **WHEN** the user enters a path that does not exist, is not a directory, or neither contains an `openspec/` subdirectory nor lies inside a git working tree
- **THEN** the workspace is not added
- **AND** a message indicates why the folder is neither an OpenSpec workspace nor a git repository
- **AND** the add prompt remains open for correction

#### Scenario: The add prompt names both accepted forms

- **WHEN** the add-workspace prompt is open
- **THEN** its label asks for the path of an OpenSpec workspace or a git repository

#### Scenario: Remove a workspace with cascade awareness

- **WHEN** the user removes a user-registered workspace and confirms
- **THEN** the workspace and any worktrees discovered through it are unregistered, their watchers are disposed, and they disappear from the Settings list and the Browse tree without a restart

#### Scenario: Rename a workspace

- **WHEN** the user sets a display name for a workspace
- **THEN** the name is persisted to the presentation store and shown in the Settings list and the Browse tree
- **AND** clearing the name reverts the workspace to its default basename

#### Scenario: Set a workspace colour

- **WHEN** the user selects a palette colour for a workspace
- **THEN** the colour token is persisted to the presentation store and the workspace's row is tinted accordingly in the Browse tree
- **AND** selecting "none" clears the colour back to the default untinted row

#### Scenario: Disable a workspace from the Settings screen

- **WHEN** the user invokes the disable control on a workspace row
- **THEN** the disabled state is persisted to the presentation store without a confirmation step
- **AND** that workspace's top-level row leaves the Browse tree without a restart
- **AND** the row stays in the Settings list, marked as disabled

#### Scenario: Re-enable a workspace from the same row

- **WHEN** the user invokes the same control on a workspace row that is already disabled
- **THEN** the workspace is re-enabled and its top-level row returns to the Browse tree without a restart
- **AND** the disabled marker is cleared from its Settings row

#### Scenario: A disable that cannot be persisted is reported, not swallowed

- **WHEN** the user invokes the disable control and the presentation store cannot persist the change
- **THEN** the failure is reported in the terminal's status line
- **AND** the row continues to show the stored state rather than the attempted one

#### Scenario: The disable control is advertised

- **WHEN** the cursor is on a workspace row in the Settings screen
- **THEN** the key hints name the control that disables and re-enables it, alongside the add, remove, rename and colour controls

#### Scenario: Disabling does not stop the workspace being tracked

- **WHEN** a workspace is disabled
- **THEN** its filesystem watcher keeps running and its changes keep reaching the Dashboard
- **AND** only its presence in the Browse tree is withdrawn

## ADDED Requirements

### Requirement: Rows Without OpenSpec in the Browse Tree

The Browse tree SHALL list a top-level row that reports no OpenSpec (see the *OpenSpec Presence Is Derived on Every Aggregation* requirement in the `workspace-registry` capability), rendered in the scheme's dimmed style, with the note `no OpenSpec` where its change count would be. The frontend SHALL NOT hide such a row on its own: which rows appear in the tree is decided by the shared service alone (see the *Disabled Rows Excluded From the Tree Pane* requirement in the `workspace-registry` capability). Selecting the row SHALL show, in the detail pane, a single explanatory line stating that the repository has no OpenSpec folder, and that its files and pull requests are browsable in the desktop and web frontends. When the row later reports OpenSpec present, it SHALL render as an ordinary row without a restart.

#### Scenario: A repository without OpenSpec is listed dimmed

- **WHEN** a registered git repository without OpenSpec is enabled
- **THEN** its top-level row appears in the Browse tree in the dimmed style
- **AND** the row shows `no OpenSpec` instead of a change count

#### Scenario: Selecting the row explains itself

- **WHEN** the user selects a top-level row that reports no OpenSpec
- **THEN** the detail pane shows the explanatory line
- **AND** no artifact tabs are offered

#### Scenario: The row becomes ordinary when OpenSpec appears

- **WHEN** a dimmed row's repository gains an `openspec/` subdirectory with an active change
- **THEN** within the watcher's debounce window the row renders undimmed with its change count and the change beneath it

