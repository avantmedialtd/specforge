## MODIFIED Requirements

### Requirement: Read-Only Artifact Navigation

Selecting an archived change in the Archive view SHALL render that change's artifacts (proposal, design, tasks, capability specs) in read-only form, using the same markdown-rendering path as active-change artifacts, reading from the change's `openspec/changes/archive/<YYYY-MM-DD>-<id>/` directory **within the worktree whose copy is currently selected** (see *Copy Selection Within an Opened Archived Change*). Selecting an archived change SHALL NOT modify the change on disk.

The reader SHALL render through the **shared change header** — the header the detail pane renders above a live change's artifact (see *Change Identity Header in the Detail Pane*, *Artifact Tab Strip in the Change Header* and *Instance Switcher in the Change Header* in the `spec-browser` capability) — in its **read-only form**, rather than through chrome of its own. The reader SHALL NOT stack a second header, copy row or artifact-switch control above or below the shared one. What the read-only form changes is fixed here: the header's leading row SHALL carry a control that returns to the archive listing, together with the selected copy's archive date and the change's title; no branch chip is rendered; the instance switcher offers the change's **copies**; and the tab strip is built from the on-demand artifact status of the selected copy.

The tab strip SHALL offer only the artifacts that exist on disk for that change — determined on demand when the change is opened — with one entry per capability spec. The proposal SHALL be shown first by default. Determining which artifacts exist is per-change and on demand, and SHALL NOT occur on the watcher's aggregation path. When the selected copy changes, the offered artifacts SHALL be re-determined against the newly selected copy, since two copies of one archived change need not contain the same artifacts.

The reader SHALL display the archived change's **on-disk directory name** — the dated `<YYYY-MM-DD>-<id>` folder under `openspec/changes/archive/` — as the header's identity, alongside the title shown in the leading row. The `archive/` path prefix that the reader uses to address the artifact SHALL NOT appear in the displayed name: what is shown is the folder's own name, so it can be used directly as a filesystem identifier. The dated directory name is displayed in preference to the undated change id because the directory is what exists on disk. Where copies carry different directory names, the name displayed SHALL be that of the copy currently selected, so it always names a directory that exists in the worktree being read.

That directory name SHALL **copy itself when clicked**, exactly as specified by the *Change Identity Header in the Detail Pane* requirement in the `spec-browser` capability, which the reader shares: a single primary click SHALL place exactly that name on the clipboard and select it as confirmation, the outcome SHALL be indicated and announced, a refused write SHALL leave the value selected, and the value SHALL be keyboard-activatable as a tab stop without introducing any global chord. No branch chip SHALL be rendered: an archived change has no live worktree, and the worktree its artifact is read from routinely hosts other, active changes whose branch was never the archived change's. This prohibition binds the copy switcher too — naming the copies is permitted, naming their branches is not. Copies SHALL be labelled as *Copy Selection Within an Opened Archived Change* specifies, so adopting the shared switcher changes what the control looks like, never what it says.

Because the reader now renders the shared header flush with the top of the pane, the reader's identity **takes the same titlebar-strip clearance** the live header takes (see *Clearance of the native titlebar strip* in *Change Identity Header in the Detail Pane*). This supersedes the previous contract, under which the identity sat below archive chrome of its own and needed no clearance.

#### Scenario: Opening an archived change renders its proposal

- **WHEN** the user selects an archived change in the Archive view
- **THEN** the application renders that change's proposal markdown read from its archive directory
- **AND** the underlying files are not modified

#### Scenario: The archive reader renders the shared header

- **WHEN** an archived change is open in the reader
- **THEN** the pane shows exactly one header above the document: the shared change header in its read-only form
- **AND** its leading row offers a control returning to the archive listing, the selected copy's archive date, and the change's title
- **AND** no separate archive header, copy row, or artifact-switch control is rendered outside that header

#### Scenario: Switching to another artifact of the open change

- **WHEN** an archived change is open and the user activates the tab for its design, tasks, or a capability spec
- **THEN** the reader renders that artifact's markdown from the same archive directory
- **AND** only artifacts that exist on disk for that change are offered as tabs

#### Scenario: Switching copy re-determines the offered artifacts

- **WHEN** an archived change is open and the user selects a different worktree's copy in the header's switcher
- **AND** that copy contains a different set of artifacts on disk
- **THEN** the tab strip offers the artifacts that exist in the newly selected copy

#### Scenario: Reader names the archive directory

- **WHEN** an archived change is open in the reader
- **THEN** the header's identity is the change's dated archive directory name in full
- **AND** the displayed name carries no `archive/` prefix
- **AND** the reader continues to show the change's title, in the header's leading row

#### Scenario: Reader names the selected copy's directory

- **WHEN** an archived change is open whose copies carry different dated directory names
- **AND** the user selects a particular copy
- **THEN** the displayed directory name is the one that exists in the selected worktree

#### Scenario: One click copies the whole directory name

- **WHEN** the user clicks once on the archive directory name
- **THEN** exactly that directory name is placed on the clipboard
- **AND** it is also selected, as confirmation of what was copied
- **AND** the outcome is indicated and announced

#### Scenario: An archived change shows no branch

- **WHEN** an archived change is open in the reader
- **AND** the worktree its artifacts are read from is on a named branch
- **THEN** no branch chip is rendered
- **AND** that worktree's branch is not shown anywhere in the reader, including in the copy switcher

#### Scenario: The archive reader takes titlebar clearance

- **WHEN** the application runs in the native window on macOS and an archived change is open
- **THEN** the directory name lies clear of the titlebar drag region, exactly as a live change's name does in the detail pane
- **AND** a click on it copies the name rather than starting a window drag
