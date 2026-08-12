## Purpose

Defines a terminal-native SpecForge frontend (`specforge-tui`) that browses the OpenSpec artifacts of every registered workspace, renders their markdown, and presents the progress dashboard and commit garden inside a TTY — reusing the same headless state and watcher the desktop app uses, with live updates and graceful degradation over SSH and in constrained terminals.

## ADDED Requirements

### Requirement: Progress Surfaces in the Terminal

The frontend SHALL render the progress surfaces — the contribution heatmap, the commit-graph rail, and the commit garden — using the data already computed by the core. The commit-graph rail SHALL be drawn from the core's precomputed graph layout. These surfaces SHALL be rendered unconditionally, without consulting any setting that would hide them.

#### Scenario: Heatmap renders contribution intensity

- **WHEN** the Dashboard screen is shown
- **THEN** a contribution heatmap is rendered as a grid of day cells whose intensity reflects activity

#### Scenario: Graph rail uses the precomputed layout

- **WHEN** a repository's commit graph is shown
- **THEN** the rail is drawn from the core's precomputed commit layout rather than re-derived in the frontend

#### Scenario: Progress surfaces need no opt-in

- **WHEN** the Dashboard or Garden screen is shown in a fresh installation with no settings ever changed
- **THEN** its content renders
- **AND** no "enable this in SpecForge" placeholder is presented in place of the content

### Requirement: Terminal Settings Screen

The interactive frontend SHALL provide a Settings screen that presents the application settings the terminal frontend can act on. The screen SHALL present a set of toggle rows — each showing its current on/off state — an Appearance control for choosing the active colour scheme, and a Workspaces section listing the user-registered workspaces with controls to add, remove, rename, recolor, and enable/disable them. The toggles SHALL include the Claude usage-quota opt-in and the ChatGPT usage-quota opt-in, and SHALL NOT include any control that hides the progress surfaces. The user SHALL be able to flip each toggle, and the change SHALL be persisted immediately to the shared application settings without a separate save action. The Appearance control SHALL let the user choose among the available colour schemes; the choice SHALL be persisted to the terminal frontend's own configuration and SHALL take effect immediately. A setting changed from this screen SHALL take effect in the running frontend without requiring a restart. The behaviour of the Workspaces section is specified by the Workspace Management from the Terminal requirement.

#### Scenario: Settings screen lists actionable toggles

- **WHEN** the Settings screen is shown
- **THEN** a row is rendered for the Claude usage-quota opt-in and for the ChatGPT usage-quota opt-in
- **AND** each row shows whether that setting is currently on or off

#### Scenario: No toggle hides the progress surfaces

- **WHEN** the Settings screen is shown
- **THEN** no row offering to disable the Dashboard, Garden, or heatmap content is rendered

#### Scenario: Settings screen offers a colour scheme control

- **WHEN** the Settings screen is shown
- **THEN** an Appearance control lists the available colour schemes and indicates the active one

#### Scenario: Settings screen lists registered workspaces

- **WHEN** the Settings screen is shown
- **THEN** a Workspaces section lists every user-registered workspace with its name and folder path
- **AND** an add-workspace control is shown

#### Scenario: Toggling a setting persists immediately

- **WHEN** the user flips a toggle on the Settings screen
- **THEN** the new value is written to the shared application settings without a separate save action
- **AND** the value is still in effect when the frontend is restarted

#### Scenario: Choosing a colour scheme persists across restart

- **WHEN** the user selects a colour scheme from the Appearance control
- **THEN** the interface is redrawn in that scheme immediately
- **AND** the same scheme is active when the frontend is restarted

#### Scenario: Toggling the quota opt-in updates the title-bar gauge

- **WHEN** the user disables the Claude usage-quota opt-in on the Settings screen
- **THEN** the title-bar quota gauge is cleared without restarting the frontend
- **AND** re-enabling it shows the gauge again once the quota poller next refreshes

## MODIFIED Requirements

### Requirement: Terminal Frontend Binary

SpecForge SHALL provide a terminal frontend, `specforge-tui`, that runs in a TTY without a GUI or WebView and operates on the same registered OpenSpec workspaces as the desktop app. The frontend SHALL be a thin presentation layer over the shared headless application service and SHALL NOT contain workspace parsing, watching, git, or dashboard computation of its own.

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
- **THEN** they present the same computed progress, leaderboard, and ships

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
- **THEN** a single line summarizing the streak and open-change count is printed, and the process exits

### Requirement: Master-Detail Browse and Screen Navigation

The interactive frontend SHALL present a Browse screen with a two-pane master-detail layout — a workspace/change tree on the left and an artifact-detail pane on the right — and SHALL provide modal Dashboard, Garden, History, and Settings screens. The screens SHALL be reachable by a **contiguous** run of number keys beginning at Browse, leaving no unbound gap in the sequence. In two-pane mode the tree pane's width SHALL be bounded so that, as the terminal widens, the surplus width goes to the detail pane rather than the tree growing without limit; the tree SHALL still be allotted enough width to read change names on smaller terminals. Keyboard navigation SHALL move focus between the two Browse panes, switch between screens, switch between artifact tabs in the detail pane, and scroll the focused region.

#### Scenario: Browse shows tree and detail

- **WHEN** the Browse screen is active
- **THEN** the workspace/change tree and the artifact-detail pane are both shown
- **AND** keyboard focus can be moved between them

#### Scenario: Detail pane receives surplus width on wide terminals

- **WHEN** the Browse screen is shown in two-pane mode on a wide terminal
- **THEN** the tree pane is held to a bounded width
- **AND** the additional width beyond that bound is given to the detail pane

#### Scenario: Switching screens

- **WHEN** the user invokes the Dashboard, Garden, History, or Settings screen switch
- **THEN** that screen replaces the Browse view
- **AND** returning to Browse restores the prior tree selection and detail target

#### Scenario: Screen keys are contiguous

- **WHEN** the user presses each number key in the screen-switch range
- **THEN** every key in that range activates a screen
- **AND** no key within the range is unbound
- **AND** the displayed key legend matches the bindings

#### Scenario: Selecting a change shows its artifact

- **WHEN** the user selects a change in the tree and chooses an artifact tab
- **THEN** the detail pane renders that artifact

### Requirement: Read-Only Operation

The frontend SHALL NOT modify the contents of any registered workspace. Browsing, dashboard, and settings views SHALL be presentation-only with respect to workspace files. The frontend MAY persist application configuration to the shared configuration directory — including application settings, the registered-workspace list, and per-workspace presentation overrides (display name, palette colour, and disabled state) — which lives outside any registered workspace; doing so SHALL NOT constitute modifying a workspace. Registering, unregistering, or disabling a workspace changes only the application's record of which folders to observe and how to present them; it SHALL NOT create, modify, or delete any file inside the affected folder.

#### Scenario: Browsing does not alter workspace files

- **WHEN** the user navigates workspaces, reads artifacts, and views the dashboard and garden screens
- **THEN** no files inside any registered workspace are created, modified, or deleted by the frontend

#### Scenario: Settings writes target app config, not workspaces

- **WHEN** the user toggles a setting from the Settings screen
- **THEN** only the application settings file in the shared configuration directory is written
- **AND** no files inside any registered workspace are created, modified, or deleted

#### Scenario: Registering a workspace does not write inside the workspace

- **WHEN** the user adds or removes a workspace from the Settings screen
- **THEN** only the application's configuration directory (the registry and presentation stores) is written
- **AND** no files inside the added or removed folder are created, modified, or deleted

#### Scenario: Disabling a workspace does not write inside the workspace

- **WHEN** the user disables or re-enables a workspace from the Settings screen
- **THEN** only the presentation store in the application's configuration directory is written
- **AND** no files inside that workspace are created, modified, or deleted

## REMOVED Requirements

### Requirement: Gamified Surfaces in the Terminal

**Reason**: Renamed to *Progress Surfaces in the Terminal* (added above), which drops the season standing and the battle-pass tier ladder along with the season system, and drops the "gamified" framing now that no setting gates these surfaces.

### Requirement: Settings Screen

**Reason**: Renamed to *Terminal Settings Screen* (added above), which drops the gamification toggle from the required rows and its live-update scenario. Renamed rather than modified in place because that scenario must disappear, and `openspec archive` rejects a MODIFIED block that drops a scenario present in the current spec.
