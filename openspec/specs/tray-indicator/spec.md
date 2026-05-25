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

The tray icon SHALL display a badge whose value equals the count of non-archived *logical changes* across all tracked workspaces. A non-archived logical change is one whose `(repository_id, change_name)` tuple has at least one instance that is not under `openspec/changes/archive/`. For non-git workspaces (which have no repository identifier), each non-archived change directory directly under `openspec/changes/` contributes 1 to the count, as before. The badge MUST be hidden when the count is zero.

A logical change touched by multiple worktrees SHALL contribute 1 to the badge, not N — the badge counts distinct in-flight changes, not file copies.

#### Scenario: Multi-worktree change contributes 1 to the badge

- **WHEN** a repository has a logical change present in three worktrees, with at least one instance non-archived
- **THEN** the badge value includes 1 for that logical change, not 3

#### Scenario: Badge reflects mixed git and non-git workspaces

- **WHEN** a tracked git repository has two non-archived logical changes and a tracked non-git workspace has one non-archived change
- **THEN** the badge displays "3"

#### Scenario: Badge hidden when no active logical changes

- **WHEN** every tracked workspace has zero non-archived logical changes
- **THEN** the tray badge is not displayed

#### Scenario: Badge decrements only when the last active instance is archived

- **WHEN** one instance of a multi-instance logical change is archived
- **AND** at least one other instance of the same logical change is still active
- **THEN** the badge value does not change

#### Scenario: Badge decrements when the final active instance is archived

- **WHEN** the last non-archived instance of a logical change is archived
- **THEN** the badge value decreases by one within the watcher debounce window

#### Scenario: Badge increments on a brand-new logical change

- **WHEN** a change directory with a new name (not present in any other worktree of the repository) is created in a tracked worktree
- **THEN** the badge value increases by one within the watcher debounce window

#### Scenario: Badge does not increment when a new instance joins an existing logical change

- **WHEN** a new worktree appears that contains a change whose `(repository_id, change_name)` tuple already had at least one active instance
- **THEN** the badge value does not change

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

The application SHALL display a desktop notification when a logical change first appears in a repository — that is, when a `(repository_id, change_name)` tuple has its first instance added in any tracked worktree. The notification SHALL NOT fire when an additional instance of an already-tracked logical change appears (for example, a Claude harness worktree opens and contains a copy of an existing change).

#### Scenario: First instance of a new logical change emits notification

- **WHEN** a change directory with a name not present in any other worktree of its repository is created in a tracked worktree
- **THEN** a desktop notification is dispatched identifying the repository and the change name

#### Scenario: Additional instance of an existing logical change is silent

- **WHEN** a worktree appears (or is created with `git worktree add`) and contains a change whose name already exists in another tracked worktree of the same repository
- **THEN** no desktop notification is dispatched for the appearance of that instance

### Requirement: Desktop Notification on Archive Transition

The application SHALL display a desktop notification when a logical change transitions from active to archived — that is, when the last non-archived instance of a `(repository_id, change_name)` tuple is moved into `openspec/changes/archive/`. Per-instance archive moves that leave at least one other instance still active SHALL NOT trigger a notification.

#### Scenario: Final-instance archive emits notification

- **WHEN** the last non-archived instance of a logical change is moved into the archive directory of its worktree
- **THEN** a desktop notification is dispatched indicating the logical change has been archived

#### Scenario: Non-final-instance archive is silent

- **WHEN** one instance of a multi-instance logical change is moved into the archive directory of its worktree
- **AND** at least one other instance of the same logical change is still active
- **THEN** no desktop notification is dispatched

### Requirement: No Notification on File Edit

The application SHALL NOT dispatch a desktop notification for ordinary file edits within an existing change instance, nor for the appearance or disappearance of an additional instance of an already-known logical change. Only logical-level transitions (new logical change, final-instance archive) trigger notifications.

#### Scenario: Editing tasks.md in any instance is silent

- **WHEN** a file inside any non-archived change instance directory is modified
- **THEN** no desktop notification is dispatched

#### Scenario: Discovered worktree appearing is silent (for existing logical changes)

- **WHEN** a new worktree is auto-discovered and its OpenSpec content is parsed
- **AND** every change in that worktree is part of a logical change that already had an instance elsewhere
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

### Requirement: Zero-Count Badge Title Cleared At The OS Layer

When the active logical change count transitions to zero (or is initialised as zero), the macOS menu-bar title attached to the tray status item MUST be cleared by invoking the underlying `NSStatusBarButton.setTitle:` with an empty `NSString`. The application SHALL pass `Some("")` to `tray.set_title` for this case, not `None`.

This codifies a workaround for upstream `tray-icon` 0.23.x behaviour: `set_title_inner` in `tray-icon/src/platform_impl/macos/mod.rs` early-returns when given `None`, so `setTitle:` is never invoked and the previous title remains attached to the button. Passing `Some("")` reaches `setTitle:@""`, which collapses the status item back to icon-only width.

The application MUST NOT rely on the intuitive interpretation of `tray.set_title(None)` (i.e., "clear the title") for the zero-count case on macOS. Any future code path that drives the badge MUST funnel through a helper that explicitly substitutes the empty-string title for the no-count case.

#### Scenario: Last active change archived clears the menu-bar title

- **WHEN** the badge currently displays a non-zero count
- **AND** the last non-archived logical change across all tracked workspaces is moved into `openspec/changes/archive/`
- **THEN** within the watcher debounce window the menu-bar item's title is empty
- **AND** the status item collapses to icon-only width with no stale digit visible

#### Scenario: set_title called with empty string when count is zero

- **WHEN** `set_badge` is invoked with a count of `Some(0)` or `None`
- **THEN** the underlying `tray.set_title` call carries `Some("")`
- **AND** never carries `None`

#### Scenario: set_title called with the digit when count is non-zero

- **WHEN** `set_badge` is invoked with a count of `Some(n)` where `n` ≥ 1
- **THEN** the underlying `tray.set_title` call carries `Some(n.to_string())`

