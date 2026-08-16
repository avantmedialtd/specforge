## ADDED Requirements

### Requirement: Change Identity Header in the Detail Pane

While the detail pane's target is an OpenSpec artifact, the pane SHALL render a **change-identity header** above the artifact's markdown, naming the change the artifact belongs to. The header applies to the artifact target only: the commit detail view, the Dashboard, the workspace file browser, the Archive view, and the Settings view each carry their own header and SHALL be unaffected.

**Content.** The header SHALL display the change's **directory name** — the `openspec/changes/<name>` folder name, which is the identifier a user hands to external tooling — rendered verbatim and in full, with no truncation, ellipsis, or transformation. It SHALL NOT substitute the change's `proposal.md` title, which the tree already shows (see *Two-Line Sole-Change-Row Layout*) and which is not the change's filesystem identity. Following the name, the header SHALL show the owning worktree's branch as an outlined chip (per *visual-identity → Outlined Chip Badges*). When the artifact belongs to a flat (non-git) workspace, or the worktree's branch is otherwise not known, no chip SHALL be rendered and the header SHALL show the name alone.

**Selection and copy.** The change name SHALL be selectable **atomically**: a single click anywhere on it SHALL select the entire name, so that the platform's own copy gesture places exactly that name on the clipboard. No application-provided copy control, clipboard write, or keyboard binding SHALL be introduced for this — the copy path is the host platform's, so it behaves identically in the desktop window, in a browser served over loopback, and in a browser served over a non-loopback bind where the asynchronous clipboard API is unavailable (see the `web-ui` capability). The branch chip SHALL NOT be part of the atomically-selectable region, so selecting the name never carries the branch with it.

**Persistence while reading.** The header SHALL remain visible while the artifact's content scrolls, so the change's identity is answerable at any scroll position rather than only at the top of the document.

**Anchoring.** Because the header occupies the top of the pane's scroll port, scroll anchors (see *Section and Task Scroll Anchors*) SHALL account for its height: a section or task scrolled to SHALL come to rest fully visible below the header, never underneath it.

**Placement.** The header SHALL be horizontally aligned with the artifact's prose column — sharing its width bound and horizontal origin — so it reads as heading the document rather than floating in the pane. Its background SHALL span the pane's width so scrolled content does not show through it.

#### Scenario: Detail pane names the change it is rendering

- **WHEN** the user selects any artifact of a change in the tree
- **THEN** the detail pane displays that change's directory name above the rendered markdown
- **AND** the name is shown in full, with no truncation or ellipsis
- **AND** the name shown is the change's directory name, not its proposal title

#### Scenario: Branch appears as a chip beside the name

- **WHEN** the detail pane renders an artifact of a change in a git worktree on a named branch
- **THEN** the header shows that branch name as an outlined chip following the change name

#### Scenario: A flat-workspace artifact shows no branch chip

- **WHEN** the detail pane renders an artifact of a change in a flat (non-git) workspace
- **THEN** the header shows the change's directory name alone
- **AND** no branch chip is rendered

#### Scenario: One click selects the whole name

- **WHEN** the user clicks once on the change name in the header
- **THEN** the entire change name is selected
- **AND** the platform's copy gesture places exactly that name on the clipboard

#### Scenario: Selecting the name excludes the branch

- **WHEN** the change name is selected by a single click
- **THEN** the selection contains the change name only
- **AND** the branch chip's text is not part of the selection

#### Scenario: Copy works where the asynchronous clipboard API is unavailable

- **WHEN** the web UI is reached over a non-loopback bind on a plain-HTTP origin, where `navigator.clipboard` is not exposed
- **THEN** clicking the change name still selects it in full
- **AND** the platform's copy gesture still copies it, because no application clipboard write is involved

#### Scenario: Identity survives scrolling a long artifact

- **WHEN** the user scrolls an artifact long enough that its first line leaves the viewport
- **THEN** the change-identity header remains visible

#### Scenario: An anchored section is not obscured by the header

- **WHEN** the user selects a section or task row that scrolls the artifact to that anchor
- **THEN** the anchored section or task comes to rest fully visible
- **AND** it is not positioned underneath the change-identity header

#### Scenario: Non-artifact targets are unaffected

- **WHEN** the detail pane renders the Dashboard, a commit's detail view, the workspace file browser, the Archive view, or the Settings view
- **THEN** no change-identity header is rendered over it
- **AND** each of those views keeps the header it renders today
