# tray-indicator Specification

## Purpose

Defines the always-on operating-system tray presence of the application, the badge that summarises active OpenSpec changes across registered workspaces, and the desktop notifications dispatched on structural change events.
## Requirements
### Requirement: Tray Icon Presence

The application SHALL present an icon in the operating-system menu bar (macOS), system tray (Windows), or status notifier area (Linux) whenever the application process is running, independent of whether the main window is open or hidden. The icon SHALL be rasterized from a vector source at the active monitor's pixel density, and re-rasterized when the monitor's scale factor changes.

#### Scenario: Icon appears at startup

- **WHEN** the application launches
- **THEN** an icon is added to the operating-system tray area within one second of process start

#### Scenario: Icon persists across window state

- **WHEN** the user closes the main window while the application keeps running
- **THEN** the tray icon remains visible

#### Scenario: Icon disappears on quit

- **WHEN** the user issues an explicit quit command (e.g. Cmd-Q on macOS)
- **THEN** the tray icon is removed from the tray area

#### Scenario: Icon is rasterized at active display density

- **WHEN** the application launches on a monitor whose scale factor is `s`
- **THEN** the tray icon's rasterized pixel dimensions equal `logical_size * s` in each axis, rather than a fixed pre-rendered raster size

#### Scenario: Icon re-rasterizes on scale change

- **WHEN** the main window moves to a monitor with a different scale factor
- **THEN** the tray icon is re-rasterized at the new scale and applied to the existing tray handle, with no flicker or removal of the icon

### Requirement: Active-Change Badge

The tray icon SHALL display a badge whose value equals the total count of non-archived changes across all registered workspaces. A non-archived change is any directory directly under `openspec/changes/` whose immediate parent is not `openspec/changes/archive/`. The badge MUST be hidden when the count is zero.

#### Scenario: Badge reflects aggregate count

- **WHEN** the registered workspaces contain three non-archived changes in aggregate
- **THEN** the tray badge displays "3"

#### Scenario: Badge hidden when no active changes

- **WHEN** every registered workspace has zero non-archived changes
- **THEN** the tray badge is not displayed

#### Scenario: Badge decrements on archive

- **WHEN** an existing non-archived change is moved into the `openspec/changes/archive/` directory on disk
- **THEN** the badge value decreases by one within the watcher debounce window

#### Scenario: Badge increments on new change

- **WHEN** a new directory is created under `openspec/changes/` of a registered workspace
- **THEN** the badge value increases by one within the watcher debounce window

### Requirement: Click to Focus Main Window

Clicking the tray icon SHALL bring the main application window to the foreground, opening it if it is currently hidden.

#### Scenario: Click opens hidden window

- **WHEN** the main window is hidden
- **AND** the user clicks the tray icon
- **THEN** the main window is shown and given focus

#### Scenario: Click focuses backgrounded window

- **WHEN** the main window is open but behind other applications
- **AND** the user clicks the tray icon
- **THEN** the main window is raised to the foreground

### Requirement: Desktop Notification on New Change

The application SHALL display a desktop notification when a new change directory appears in any registered workspace's `openspec/changes/` directory.

#### Scenario: New change emits notification

- **WHEN** a new directory is created under `openspec/changes/` of a registered workspace
- **THEN** a desktop notification is dispatched identifying the workspace and the change identifier

### Requirement: Desktop Notification on Archive Transition

The application SHALL display a desktop notification when an existing change transitions between active (`openspec/changes/<id>/`) and archived (`openspec/changes/archive/<id>/`) locations.

#### Scenario: Change moved to archive

- **WHEN** a change directory is moved from `openspec/changes/<id>/` to `openspec/changes/archive/<id>/`
- **THEN** a desktop notification is dispatched indicating the change has been archived

### Requirement: No Notification on File Edit

The application SHALL NOT dispatch a desktop notification for ordinary file edits within an existing change directory. Only structural transitions (new change, archive move) trigger notifications.

#### Scenario: Editing tasks.md is silent

- **WHEN** a file inside an existing non-archived change directory is modified
- **THEN** no desktop notification is dispatched

### Requirement: Spec-Activity Glyph Variant

The tray icon SHALL render in one of two visual variants — a default variant and a *spec-activity* variant — selected from the cache of registered workspaces. The spec-activity variant SHALL be shown whenever any non-archived change in any registered workspace has at least one capability spec delta (i.e. its `ArtifactStatus.specs` is non-empty). The default variant SHALL be shown in every other case, including when no workspaces are registered, when the cache has not yet been populated, when no active change has spec deltas, and when the cache cannot be re-evaluated due to a transient error (in which case the most recently determined variant is retained until the next successful evaluation).

The variant selection SHALL be recomputed on every `CacheEvent` from the watcher, using the same broadcast stream that drives the active-change badge. The variant SHALL persist across monitor scale-factor changes — when re-rasterization is triggered by a scale change, the currently-selected variant (not the default) SHALL be re-rasterized.

#### Scenario: Variant flips to spec-activity when a spec delta appears

- **WHEN** a registered workspace has no active changes touching specs
- **AND** a new change directory appears whose `ArtifactStatus.specs` is non-empty, or an existing change directory gains a non-empty `ArtifactStatus.specs`
- **THEN** the tray icon flips to the spec-activity variant within the watcher debounce window

#### Scenario: Variant reverts to default when the last spec delta disappears

- **WHEN** the tray icon is currently showing the spec-activity variant
- **AND** every change in every registered workspace becomes one with an empty `ArtifactStatus.specs`, whether by file deletion, archival, or workspace removal
- **THEN** the tray icon reverts to the default variant within the watcher debounce window

#### Scenario: Any-workspace aggregation

- **WHEN** two registered workspaces are present, one with no spec activity and one with at least one active change whose `ArtifactStatus.specs` is non-empty
- **THEN** the tray icon shows the spec-activity variant

#### Scenario: Default variant on empty registry

- **WHEN** no workspaces are registered
- **THEN** the tray icon shows the default variant

#### Scenario: Default variant before first cache populate

- **WHEN** the application has just launched and the cache for the registered workspaces has not yet been populated
- **THEN** the tray icon shows the default variant

#### Scenario: Stale variant retained on transient parse error

- **WHEN** the tray icon is currently showing the spec-activity variant
- **AND** a filesystem event triggers a re-parse that returns an error
- **THEN** the cache entry is left unchanged
- **AND** the tray icon continues to show the spec-activity variant

#### Scenario: Initial variant reflects pre-existing state at startup

- **WHEN** the application launches with at least one registered workspace whose cache contains an active change with a non-empty `ArtifactStatus.specs`
- **THEN** the tray icon's first painted frame uses the spec-activity variant, not the default

#### Scenario: Current variant survives scale-factor change

- **WHEN** the tray icon is currently showing the spec-activity variant
- **AND** the main window moves to a monitor with a different scale factor
- **THEN** the spec-activity variant is re-rasterized at the new scale and applied to the existing tray handle
- **AND** the tray icon does not briefly revert to the default variant

### Requirement: Template Rendering Preserved Across Icon Updates

The tray icon SHALL be rendered as a template image — recolourable from the menu-bar's foreground colour — on every icon update, not only on the initial install. Updates include, but are not limited to: the initial seed performed by the glyph updater after install, the variant flip in response to a cache event, and the re-rasterization performed in response to a monitor scale-factor change.

The application MUST NOT use any update pattern that hands the operating system a tray icon whose template-image attribute defaults to "off" (e.g., a plain `set_icon` call on macOS that drops `NSImage.isTemplate`).

#### Scenario: Glyph remains template-rendered after the updater's initial set

- **WHEN** the application launches in macOS dark mode
- **AND** the glyph updater performs its initial `set_icon` call immediately after the tray is installed
- **THEN** the tray icon is rendered in the menu-bar's foreground colour (white on a dark menu bar), not as the literal pure-black bitmap

#### Scenario: Glyph remains template-rendered after a variant flip

- **WHEN** the tray is currently showing the default glyph
- **AND** a `CacheEvent` causes the glyph updater to swap to the spec-activity glyph
- **THEN** the new glyph is rendered as a template image and continues to recolour with the menu-bar appearance

#### Scenario: Glyph remains template-rendered after a scale-factor change

- **WHEN** the tray is currently showing any glyph variant
- **AND** the main window moves to a monitor with a different scale factor, triggering re-rasterization
- **THEN** the re-rasterized glyph is rendered as a template image and continues to recolour with the menu-bar appearance

#### Scenario: Dark-mode menu bar shows the glyph in white

- **WHEN** the operating system is in dark mode (menu bar foreground colour is white)
- **AND** the application's tray icon is visible
- **THEN** the glyph appears in white, not black

