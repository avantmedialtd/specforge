## MODIFIED Requirements

### Requirement: Read-Only Artifact Navigation

Selecting an archived change in the Archive view SHALL render that change's artifacts (proposal, design, tasks, capability specs) in read-only form, using the same markdown-rendering path as active-change artifacts, reading from the change's `openspec/changes/archive/<YYYY-MM-DD>-<id>/` directory. Selecting an archived change SHALL NOT modify the change on disk.

The reader SHALL present a control for switching between the change's artifacts. The control SHALL offer only the artifacts that exist on disk for that change — determined on demand when the change is opened — with one entry per capability spec. The proposal SHALL be shown first by default. Determining which artifacts exist is per-change and on demand, and SHALL NOT occur on the watcher's aggregation path.

The reader SHALL additionally display the archived change's **on-disk directory name** — the dated `<YYYY-MM-DD>-<id>` folder under `openspec/changes/archive/` — alongside the title it already shows. The `archive/` path prefix that the reader uses to address the artifact SHALL NOT appear in the displayed name: what is shown is the folder's own name, so it can be used directly as a filesystem identifier. The dated directory name is displayed in preference to the undated change id because the directory is what exists on disk.

That directory name SHALL **copy itself when clicked**, exactly as specified by the *Change Identity Header in the Detail Pane* requirement in the `spec-browser` capability, which the reader shares: a single primary click SHALL place exactly that name on the clipboard and select it as confirmation, the outcome SHALL be indicated and announced, a refused write SHALL leave the value selected, and the value SHALL be keyboard-activatable as a tab stop without introducing any global chord. (This supersedes the previous contract, which specified selection only and forbade an application clipboard write.) No branch chip SHALL be rendered: an archived change has no live worktree, and the worktree its artifact is read from routinely hosts other, active changes whose branch was never the archived change's.

The reader's identity is not flush with the top of the window — it sits below the archive header and the artifact-switch control — so it takes no titlebar-strip clearance.

#### Scenario: Opening an archived change renders its proposal

- **WHEN** the user selects an archived change in the Archive view
- **THEN** the application renders that change's proposal markdown read from its archive directory
- **AND** the underlying files are not modified

#### Scenario: Switching to another artifact of the open change

- **WHEN** an archived change is open and the user activates the artifact-switch control for its design, tasks, or a capability spec
- **THEN** the reader renders that artifact's markdown from the same archive directory
- **AND** only artifacts that exist on disk for that change are offered as switch targets

#### Scenario: Reader names the archive directory

- **WHEN** an archived change is open in the reader
- **THEN** the reader displays the change's dated archive directory name in full
- **AND** the displayed name carries no `archive/` prefix
- **AND** the reader continues to show the change's title as it does today

#### Scenario: One click copies the whole directory name

- **WHEN** the user clicks once on the archive directory name
- **THEN** exactly that directory name is placed on the clipboard
- **AND** it is also selected, as confirmation of what was copied
- **AND** the outcome is indicated and announced

#### Scenario: An archived change shows no branch

- **WHEN** an archived change is open in the reader
- **AND** the worktree its artifacts are read from is on a named branch
- **THEN** no branch chip is rendered
- **AND** that worktree's branch is not shown anywhere in the reader

#### Scenario: The archive reader needs no titlebar clearance

- **WHEN** the application runs in the native window on macOS and an archived change is open
- **THEN** the directory name is already clear of the titlebar drag region, below the archive header
- **AND** it is clickable without any additional offset
