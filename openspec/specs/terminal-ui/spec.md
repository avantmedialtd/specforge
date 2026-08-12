# terminal-ui Specification

## Purpose

Defines a terminal-native SpecForge frontend (`specforge-tui`) that browses the OpenSpec artifacts of every registered workspace, renders their markdown, and presents the progress dashboard and commit garden inside a TTY — reusing the same headless state and watcher the desktop app uses, with live updates and graceful degradation over SSH and in constrained terminals.
## Requirements
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

The interactive frontend SHALL present a Browse screen with a two-pane master-detail layout — a workspace/change tree on the left and an artifact-detail pane on the right — and SHALL provide modal Dashboard, Season, and Settings screens. In two-pane mode the tree pane's width SHALL be bounded so that, as the terminal widens, the surplus width goes to the detail pane rather than the tree growing without limit; the tree SHALL still be allotted enough width to read change names on smaller terminals. Keyboard navigation SHALL move focus between the two Browse panes, switch between screens, switch between artifact tabs in the detail pane, and scroll the focused region.

#### Scenario: Browse shows tree and detail

- **WHEN** the Browse screen is active
- **THEN** the workspace/change tree and the artifact-detail pane are both shown
- **AND** keyboard focus can be moved between them

#### Scenario: Detail pane receives surplus width on wide terminals

- **WHEN** the Browse screen is shown in two-pane mode on a wide terminal
- **THEN** the tree pane is held to a bounded width
- **AND** the additional width beyond that bound is given to the detail pane

#### Scenario: Switching screens

- **WHEN** the user invokes the Dashboard, Season, or Settings screen switch
- **THEN** that screen replaces the Browse view
- **AND** returning to Browse restores the prior tree selection and detail target

#### Scenario: Selecting a change shows its artifact

- **WHEN** the user selects a change in the tree and chooses an artifact tab
- **THEN** the detail pane renders that artifact

### Requirement: Live Updates From the Watcher

The interactive frontend SHALL subscribe to the application service's filesystem-change broadcast and refresh affected views when changes occur, without the user re-issuing a command. On a change event the frontend SHALL re-read the current aggregated view from the service rather than maintaining an independent cache.

The refresh SHALL include the body of the artifact currently shown in the detail pane, re-read from the service, even when the change event leaves the user's selection unchanged. This gives the terminal frontend the same detail-pane freshness the desktop shell provides — see the *Reactive Updates from Filesystem* requirement in the `spec-browser` capability.

A re-read the user did not initiate SHALL preserve the detail pane's scroll offset, clamped so that at least one line of the new body remains visible, and SHALL leave the displayed body in place when the read fails, rather than replacing it with the failure message. A load the user initiated — selecting a different change, or switching artifact tab — SHALL continue to reset the offset to the top and to render a read failure in place of the body. A re-read whose reply arrives after the user has moved to a different selection or tab SHALL be discarded rather than displayed; when such a re-read supersedes a still-outstanding user-initiated load, it SHALL adopt that load's presentation so the reader still arrives at the top of the artifact they chose.

#### Scenario: A new change appears

- **WHEN** a new change directory appears in a watched workspace while the interactive frontend is open
- **THEN** the tree updates to include it without user action

#### Scenario: The open artifact's body refreshes without a selection change

- **WHEN** the detail pane is showing an artifact and the user's selection has not moved
- **AND** that artifact's file is modified on disk
- **THEN** the detail pane shows the updated content without user action

#### Scenario: A shrunken body clamps the preserved offset

- **WHEN** the detail pane is showing an artifact and the user has scrolled deep into it
- **AND** an on-disk edit shortens that artifact to fewer lines than the current offset
- **THEN** the detail pane shows the new body with its offset reduced so content is still visible
- **AND** the pane is not left blank

#### Scenario: An artifact that appears becomes reachable without moving the cursor

- **WHEN** the detail pane is showing a change whose only artifact is its proposal
- **AND** an on-disk write adds `design.md` or `tasks.md` to that change
- **THEN** the artifact tab strip offers the new artifacts without the user moving the tree cursor off the change and back
- **AND** the tab the user was reading remains the active tab

#### Scenario: Scroll offset survives a watcher-driven re-read

- **WHEN** the detail pane is showing an artifact and the user has scrolled down within it
- **AND** a filesystem change triggers a re-read of that artifact
- **THEN** the detail pane's scroll offset is unchanged

#### Scenario: A failed re-read leaves the reader's content in place

- **WHEN** the detail pane is showing an artifact
- **AND** a re-read the user did not initiate fails, because the artifact was archived or removed or is mid-write
- **THEN** the detail pane continues to show the body it already had, at its existing scroll offset
- **AND** the failure message does not replace it

#### Scenario: A re-read that supersedes the user's own load still starts at the top

- **WHEN** the user selects a change and a filesystem change triggers a re-read before that selection's body has arrived
- **THEN** the detail pane shows the selected artifact with its scroll offset at the top

#### Scenario: Selecting a different change still starts at the top

- **WHEN** the user selects a different change, or switches to a different artifact tab
- **THEN** the detail pane loads that artifact and its scroll offset returns to the top

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

### Requirement: Terminal Dashboard Notes Disabled Workspaces

The terminal frontend's Dashboard screen SHALL note, whenever at least one
top-level row is disabled, that its totals include disabled workspaces. This
satisfies, for the terminal client, the *Dashboard Unaffected by Workspace
Disable* requirement in the `dashboard` capability: the Dashboard remains the
unfiltered record while the Browse tree hides exactly those rows, so without the
note a terminal user reads the difference between the two as a counting bug. The
note SHALL be omitted entirely when no row is disabled, and SHALL be placed with
the totals it qualifies rather than out of view.

The number the note reports SHALL be the count of *top-level rows* the Browse
tree drops — not the count of registered entries carrying the disabled flag. The
flag is keyed per top-level row, so a repository registered at several worktrees
is one row and SHALL be counted once, however many of its worktrees are
registered. Each non-git (flat) workspace is its own top-level row and SHALL be
counted individually.

Disabling a workspace SHALL NOT change any figure the Dashboard reports; the
note explains the totals, it does not adjust them.

#### Scenario: No note when nothing is disabled

- **WHEN** the Dashboard screen is shown and no top-level row is disabled
- **THEN** no disabled-workspace note is rendered

#### Scenario: Note appears when a workspace is parked

- **WHEN** one top-level row is disabled and the Dashboard screen is shown
- **THEN** the Dashboard notes that its totals include one disabled workspace
- **AND** parking a second row raises the number the note reports to two

#### Scenario: A repository registered at two worktrees counts once

- **WHEN** one repository is user-registered at two of its worktrees, so the Settings list shows two entries for it
- **AND** that repository is disabled
- **THEN** the Browse tree loses one top-level row
- **AND** the Dashboard's note reports one disabled workspace, not two

#### Scenario: The note explains the totals rather than changing them

- **WHEN** a workspace holding active changes is disabled
- **THEN** the Dashboard's summary totals are unchanged
- **AND** the Browse tree no longer reaches that workspace's changes
- **AND** the note accounts for the difference

### Requirement: Artifact Markdown Rendering

The detail pane SHALL render OpenSpec artifact markdown (proposal, design, tasks, and capability specs) as styled terminal text, including headings, lists, code blocks, and task checkboxes. Content the terminal cannot display (such as images) SHALL degrade to a textual representation rather than being omitted silently.

Links SHALL be presented with their destination discoverable — as the link text with its target shown textually, or as a terminal hyperlink (OSC 8) whose target the hosting terminal emulator may offer to open. When the hosting terminal is not known to support OSC 8 hyperlinks, the textual presentation SHALL be used, so the destination remains discoverable rather than being swallowed with the escape sequence — the same capability-fallback shape as the *Graceful Degradation* requirement. The terminal frontend itself SHALL NOT spawn any opener process in response to link content; any opening is the terminal emulator's own click-through behaviour.

#### Scenario: Proposal renders as styled text

- **WHEN** the user views a proposal artifact in the detail pane
- **THEN** its headings, paragraphs, and lists are rendered as styled terminal text

#### Scenario: Task checkboxes render as state

- **WHEN** the user views a tasks artifact
- **THEN** complete and incomplete tasks are shown with distinct checkbox states

#### Scenario: Images degrade to text

- **WHEN** an artifact contains an image
- **THEN** the pane shows the image's alternate text instead of omitting it

#### Scenario: A link's destination is discoverable

- **WHEN** an artifact contains a link — external or to a workspace file such as an HTML mockup
- **THEN** the pane presents the link with its destination visible textually or as a terminal hyperlink
- **AND** on a terminal not known to support OSC 8 hyperlinks the destination is shown textually
- **AND** the terminal frontend spawns no opener process

### Requirement: Graceful Degradation

The frontend SHALL remain legible across terminal capabilities. It SHALL encode salient distinctions (such as activity intensity and tier rarity) in glyph as well as color, so the interface stays readable without color. It SHALL map palette colors onto a fallback ladder rather than assuming truecolor support, and SHALL adapt its layout to the terminal width, collapsing the two-pane Browse layout to a single switchable pane below a width threshold. The colours of the active scheme SHALL be subject to the same capability fallback ladder, and an environment that disables colour (such as `NO_COLOR` or a terminal that reports no colour support) SHALL override the selected scheme and render without colour. A panic SHALL restore the terminal to a usable state.

#### Scenario: Readable without color

- **WHEN** the frontend runs in a terminal that reports no or minimal color support
- **THEN** activity intensity and tier rarity remain distinguishable by glyph

#### Scenario: Selected scheme is subject to capability downsampling

- **WHEN** a colour scheme is active in a terminal that does not support truecolor
- **THEN** the scheme's colours are mapped onto the terminal's fallback ladder rather than emitted as unsupported escape codes

#### Scenario: NO_COLOR overrides the selected scheme

- **WHEN** `NO_COLOR` is set or the terminal reports no colour support
- **THEN** the frontend renders without colour regardless of which scheme is selected

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

The frontend SHALL NOT modify the contents of any registered workspace. Browsing, dashboard, season, and settings views SHALL be presentation-only with respect to workspace files. The frontend MAY persist application configuration to the shared configuration directory — including application settings, the registered-workspace list, and per-workspace presentation overrides (display name, palette colour, and disabled state) — which lives outside any registered workspace; doing so SHALL NOT constitute modifying a workspace. Registering, unregistering, or disabling a workspace changes only the application's record of which folders to observe and how to present them; it SHALL NOT create, modify, or delete any file inside the affected folder.

#### Scenario: Browsing does not alter workspace files

- **WHEN** the user navigates workspaces, reads artifacts, and views the dashboard and season screens
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

### Requirement: Settings Screen

The interactive frontend SHALL provide a Settings screen that presents the application settings the terminal frontend can act on. The screen SHALL present a set of toggle rows — each showing its current on/off state — an Appearance control for choosing the active colour scheme, and a Workspaces section listing the user-registered workspaces with controls to add, remove, rename, recolor, and enable/disable them. The first version's toggles SHALL include the gamification master switch and the Claude usage-quota opt-in. The user SHALL be able to flip each toggle, and the change SHALL be persisted immediately to the shared application settings without a separate save action. The Appearance control SHALL let the user choose among the available colour schemes; the choice SHALL be persisted to the terminal frontend's own configuration and SHALL take effect immediately. A setting changed from this screen SHALL take effect in the running frontend without requiring a restart. The behaviour of the Workspaces section is specified by the Workspace Management from the Terminal requirement.

#### Scenario: Settings screen lists actionable toggles

- **WHEN** the Settings screen is shown
- **THEN** a row is rendered for the gamification switch and for the Claude usage-quota opt-in
- **AND** each row shows whether that setting is currently on or off

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

#### Scenario: Toggling gamification updates the gamified surfaces

- **WHEN** the user toggles the gamification switch on the Settings screen
- **THEN** the gamified surfaces (Dashboard, Season, Garden) reflect the new state without restarting the frontend

#### Scenario: Toggling the quota opt-in updates the title-bar gauge

- **WHEN** the user disables the Claude usage-quota opt-in on the Settings screen
- **THEN** the title-bar quota gauge is cleared without restarting the frontend
- **AND** re-enabling it shows the gauge again once the quota poller next refreshes

### Requirement: Workspace Management from the Terminal

The interactive frontend SHALL allow the user to manage the registered-workspace set from the Settings screen: add a workspace by path, remove a user-registered workspace, set a workspace's display name and palette colour, and disable or re-enable a workspace. These operations SHALL go through the shared application service so the registry, the filesystem watcher, and the presentation store stay consistent, and their effects SHALL appear in the running frontend without a restart. The Settings workspace list SHALL present the user-registered workspaces only; auto-discovered worktrees SHALL NOT appear as manageable rows.

Every operation available on the focused workspace row SHALL be advertised in the frontend's key hints, so no control — the disable toggle included — is reachable only by prior knowledge. Because disabling removes a workspace's top-level row from the Browse tree, the Settings list SHALL remain its home: a disabled workspace SHALL stay listed there, SHALL be visibly marked as disabled, and SHALL be re-enabled from that same row, so the terminal frontend is a complete surface for the operation and does not require the desktop shell or the web UI to undo it.

Disabling SHALL be applied immediately without a confirmation step — it is reversible from the row that performed it — and SHALL be keyed the same way the shared service keys presentation overrides, so sibling worktrees of one repository share a single disabled state.

#### Scenario: Add a workspace by path

- **WHEN** the user invokes the add-workspace control and enters the path of a folder that contains an `openspec/` subdirectory
- **THEN** the folder is registered, a filesystem watcher is established for it, and it appears in the Settings workspace list and the Browse tree without a restart

#### Scenario: Invalid path is rejected with a message

- **WHEN** the user enters a path that does not exist, is not a directory, or lacks an `openspec/` subdirectory
- **THEN** the workspace is not added
- **AND** a message indicates why the folder is not a valid OpenSpec workspace
- **AND** the add prompt remains open for correction

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

### Requirement: Colour Scheme Selection

The interactive frontend SHALL support a set of named colour schemes and SHALL
let the user choose the active one. A scheme SHALL define the values for the
frontend's semantic colour slots (such as the accent, focused and dim borders,
secondary text, selection highlight, and error/warning/success indicators) and
for its data palettes (workspace tints and commit-graph lanes). The curated set
SHALL include a default scheme matching the desktop brand look, a high-contrast
scheme, a monochrome scheme, and a terminal-native scheme that defers to the
terminal's own ANSI palette instead of emitting imposed RGB. On first run, before
any selection is made, the frontend SHALL use the default scheme. The selected
scheme SHALL be applied to the running frontend immediately, without requiring a
restart. Choosing a scheme SHALL NOT change which distinctions are encoded by
glyph, so the interface remains legible regardless of the scheme.

#### Scenario: Default scheme on first run

- **WHEN** the frontend starts and no colour scheme has been chosen
- **THEN** it renders using the default scheme

#### Scenario: Selecting a scheme recolours the interface

- **WHEN** the user selects a different colour scheme
- **THEN** the accent, borders, secondary text and status indicators are redrawn in that scheme's colours without restarting the frontend

#### Scenario: Terminal-native scheme defers to the terminal palette

- **WHEN** the terminal-native scheme is active
- **THEN** the frontend paints with the terminal's own ANSI colours rather than imposing fixed RGB values

#### Scenario: Monochrome scheme uses no colour

- **WHEN** the monochrome scheme is active
- **THEN** the frontend conveys every distinction through glyph, weight and reverse-video rather than colour

