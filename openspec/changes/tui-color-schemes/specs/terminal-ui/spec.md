## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: Settings Screen

The interactive frontend SHALL provide a Settings screen that presents the application settings the terminal frontend can act on. The screen SHALL present a set of toggle rows — each showing its current on/off state — an Appearance control for choosing the active colour scheme, and a Workspaces section listing the user-registered workspaces with controls to add, remove, rename, and recolor them. The first version's toggles SHALL include the gamification master switch and the Claude usage-quota opt-in. The user SHALL be able to flip each toggle, and the change SHALL be persisted immediately to the shared application settings without a separate save action. The Appearance control SHALL let the user choose among the available colour schemes; the choice SHALL be persisted to the terminal frontend's own configuration and SHALL take effect immediately. A setting changed from this screen SHALL take effect in the running frontend without requiring a restart. The behaviour of the Workspaces section is specified by the Workspace Management from the Terminal requirement.

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
