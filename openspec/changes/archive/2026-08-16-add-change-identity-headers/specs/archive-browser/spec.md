## MODIFIED Requirements

### Requirement: Read-Only Artifact Navigation

Selecting an archived change in the Archive view SHALL render that change's artifacts (proposal, design, tasks, capability specs) in read-only form, using the same markdown-rendering path as active-change artifacts, reading from the change's `openspec/changes/archive/<YYYY-MM-DD>-<id>/` directory. Selecting an archived change SHALL NOT modify the change on disk.

The reader SHALL present a control for switching between the change's artifacts. The control SHALL offer only the artifacts that exist on disk for that change — determined on demand when the change is opened — with one entry per capability spec. The proposal SHALL be shown first by default. Determining which artifacts exist is per-change and on demand, and SHALL NOT occur on the watcher's aggregation path.

The reader's header SHALL additionally display the archived change's **on-disk directory name** — the dated `<YYYY-MM-DD>-<id>` folder under `openspec/changes/archive/` — alongside the title it already shows. The `archive/` path prefix that the reader uses to address the artifact SHALL NOT appear in the displayed name: what is shown is the folder's own name, so it can be used directly as a filesystem identifier. The dated directory name is displayed in preference to the undated change id because the directory is what exists on disk.

That directory name SHALL be selectable **atomically**: a single click anywhere on it SHALL select the whole name, so the platform's own copy gesture places exactly that name on the clipboard. As in the *Change Identity Header in the Detail Pane* requirement of the `spec-browser` capability, no application-provided copy control, clipboard write, or keyboard binding SHALL be introduced for this. No branch chip SHALL be rendered: an archived change has no live worktree, so there is no branch to name.

#### Scenario: Opening an archived change renders its proposal

- **WHEN** the user selects an archived change in the Archive view
- **THEN** the application renders that change's proposal markdown read from its archive directory
- **AND** the underlying files are not modified

#### Scenario: Switching to another artifact of the open change

- **WHEN** an archived change is open and the user activates the artifact-switch control for its design, tasks, or a capability spec
- **THEN** the reader renders that artifact's markdown from the same archive directory
- **AND** only artifacts that exist on disk for that change are offered as switch targets

#### Scenario: Reader header names the archive directory

- **WHEN** an archived change is open in the reader
- **THEN** the header displays the change's dated archive directory name in full
- **AND** the displayed name carries no `archive/` prefix
- **AND** the header continues to show the change's title as it does today

#### Scenario: One click selects the whole directory name

- **WHEN** the user clicks once on the archive directory name in the reader header
- **THEN** the entire directory name is selected
- **AND** the platform's copy gesture places exactly that name on the clipboard

#### Scenario: An archived change shows no branch

- **WHEN** an archived change is open in the reader
- **THEN** no branch chip is rendered in the header
