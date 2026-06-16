# terminal-ui Specification

## Purpose

Defines a terminal-native SpecForge frontend (`specforge-tui`) that browses the OpenSpec artifacts of every registered workspace, renders their markdown, and presents the gamified dashboard and season ladder inside a TTY — reusing the same headless state and watcher the desktop app uses, with live updates and graceful degradation over SSH and in constrained terminals.

## ADDED Requirements

### Requirement: Terminal Frontend Binary

SpecForge SHALL provide a terminal frontend, `specforge-tui`, that runs in a TTY without a GUI or WebView and operates on the same registered OpenSpec workspaces as the desktop app. The frontend SHALL be a thin presentation layer over the shared headless application service and SHALL NOT contain workspace parsing, watching, git, or dashboard/season computation of its own.

#### Scenario: Launches in a terminal against existing workspaces

- **WHEN** the user runs `specforge-tui` in a terminal on a machine that has registered workspaces
- **THEN** the frontend starts in the TTY and lists those workspaces
- **AND** it does so without launching a window or a WebView

#### Scenario: No GUI dependency

- **WHEN** `specforge-tui` runs on a headless host reached over SSH
- **THEN** it operates fully in the terminal with no display server required

### Requirement: In-Process Shared Application Service

The terminal frontend and the desktop shell SHALL consume a single headless application service (`openspec-app`) that owns settings, the dashboard assembly, first-launch backfill/seeding, watcher lifecycle, and configuration-directory resolution. The terminal frontend SHALL call the service in-process (no inter-process or serialization boundary). The dashboard assembly SHALL be reachable from automated tests independently of either frontend.

#### Scenario: Both frontends compute identical results

- **WHEN** the desktop app and the terminal frontend render the dashboard for the same workspaces and identity on the same machine
- **THEN** they present the same computed standing, progress, and ships

#### Scenario: Assembly is unit-testable

- **WHEN** the dashboard assembly is exercised by an automated test
- **THEN** it runs without instantiating a Tauri application or a terminal

### Requirement: Run Modes

The `specforge-tui` binary SHALL support three run modes from one executable: a default full interactive TUI; a `--status` snapshot mode that prints a computed summary and exits without entering an interactive loop; and a `--line` mode that prints a single ambient status line (the terminal equivalent of the desktop tray badge) and exits.

#### Scenario: Default full interactive mode

- **WHEN** the user runs `specforge-tui` with no run-mode flag
- **THEN** the interactive terminal interface is shown and accepts keyboard input until the user quits

#### Scenario: Snapshot mode is non-interactive and pipeable

- **WHEN** the user runs `specforge-tui --status`
- **THEN** a computed summary is printed to standard output and the process exits
- **AND** no alternate screen or raw-mode terminal state persists after exit

#### Scenario: Status line mode

- **WHEN** the user runs `specforge-tui --line`
- **THEN** a single line summarizing the current season standing, streak, and open-change count is printed, and the process exits

### Requirement: Master-Detail Browse and Screen Navigation

The interactive frontend SHALL present a Browse screen with a two-pane master-detail layout — a workspace/change tree on the left and an artifact-detail pane on the right — and SHALL provide modal Dashboard and Season screens. Keyboard navigation SHALL move focus between the two Browse panes, switch between screens, switch between artifact tabs in the detail pane, and scroll the focused region.

#### Scenario: Browse shows tree and detail

- **WHEN** the Browse screen is active
- **THEN** the workspace/change tree and the artifact-detail pane are both shown
- **AND** keyboard focus can be moved between them

#### Scenario: Switching screens

- **WHEN** the user invokes the Dashboard or Season screen switch
- **THEN** that screen replaces the Browse view
- **AND** returning to Browse restores the prior tree selection and detail target

#### Scenario: Selecting a change shows its artifact

- **WHEN** the user selects a change in the tree and chooses an artifact tab
- **THEN** the detail pane renders that artifact

### Requirement: Live Updates From the Watcher

The interactive frontend SHALL subscribe to the application service's filesystem-change broadcast and refresh affected views when changes occur, without the user re-issuing a command. On a change event the frontend SHALL re-read the current aggregated view from the service rather than maintaining an independent cache.

#### Scenario: A new change appears

- **WHEN** a new change directory appears in a watched workspace while the interactive frontend is open
- **THEN** the tree updates to include it without user action

#### Scenario: Long-running computation does not block input

- **WHEN** a dashboard refresh that performs git scans is in progress
- **THEN** the interface continues to accept and respond to keyboard input

### Requirement: Gamified Surfaces in the Terminal

The frontend SHALL render the gamified surfaces — the contribution heatmap, the season standing and its battle-pass tier ladder, the commit-graph rail, and the commit garden — using the data already computed by the core. The commit-graph rail SHALL be drawn from the core's precomputed graph layout. The battle-pass tier ladder, which exceeds typical terminal height, SHALL be presented as a scroll region with the current tier kept in view.

#### Scenario: Heatmap renders contribution intensity

- **WHEN** the Dashboard screen is shown
- **THEN** a contribution heatmap is rendered as a grid of day cells whose intensity reflects activity

#### Scenario: Season ladder keeps the current tier visible

- **WHEN** the Season screen is shown
- **THEN** the battle-pass tier ladder is scrollable and the user's current tier is visible without scrolling

#### Scenario: Graph rail uses the precomputed layout

- **WHEN** a repository's commit graph is shown
- **THEN** the rail is drawn from the core's precomputed commit layout rather than re-derived in the frontend

### Requirement: Artifact Markdown Rendering

The detail pane SHALL render OpenSpec artifact markdown (proposal, design, tasks, and capability specs) as styled terminal text, including headings, lists, code blocks, and task checkboxes. Content the terminal cannot display (such as images) SHALL degrade to a textual representation rather than being omitted silently.

#### Scenario: Proposal renders as styled text

- **WHEN** the user views a proposal artifact in the detail pane
- **THEN** its headings, paragraphs, and lists are rendered as styled terminal text

#### Scenario: Task checkboxes render as state

- **WHEN** the user views a tasks artifact
- **THEN** complete and incomplete tasks are shown with distinct checkbox states

#### Scenario: Images degrade to text

- **WHEN** an artifact contains an image
- **THEN** the pane shows the image's alternate text instead of omitting it

### Requirement: Graceful Degradation

The frontend SHALL remain legible across terminal capabilities. It SHALL encode salient distinctions (such as activity intensity and tier rarity) in glyph as well as color, so the interface stays readable without color. It SHALL map palette colors onto a fallback ladder rather than assuming truecolor support, and SHALL adapt its layout to the terminal width, collapsing the two-pane Browse layout to a single switchable pane below a width threshold. A panic SHALL restore the terminal to a usable state.

#### Scenario: Readable without color

- **WHEN** the frontend runs in a terminal that reports no or minimal color support
- **THEN** activity intensity and tier rarity remain distinguishable by glyph

#### Scenario: Narrow terminal collapses to one pane

- **WHEN** the terminal is narrower than the two-pane threshold
- **THEN** the Browse screen shows a single pane that can be switched between tree and detail

#### Scenario: Terminal restored on panic

- **WHEN** the frontend panics
- **THEN** the terminal is returned to a usable state (cooked mode, visible cursor, normal screen) before the process exits

### Requirement: Shared Configuration With Isolated Remote State

The frontend SHALL resolve the same configuration directory the desktop app uses on the same machine, via the shared resolver, so workspaces registered in either are visible to both. When run on a different machine (for example over SSH), the frontend SHALL operate on that machine's own configuration and activity, independent of any other host.

#### Scenario: Shared registry on one machine

- **WHEN** a workspace is registered in the desktop app and `specforge-tui` is run on the same machine
- **THEN** the terminal frontend lists that workspace

#### Scenario: Isolated state across machines

- **WHEN** `specforge-tui` runs on a remote host over SSH
- **THEN** it reflects that host's own workspaces and activity and does not depend on the user's desktop machine

### Requirement: Read-Only Operation

The frontend SHALL NOT modify the contents of any registered workspace. Browsing, dashboard, and season views SHALL be presentation-only with respect to workspace files.

#### Scenario: Browsing does not alter workspace files

- **WHEN** the user navigates workspaces, reads artifacts, and views the dashboard and season screens
- **THEN** no files inside any registered workspace are created, modified, or deleted by the frontend
