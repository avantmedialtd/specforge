# tray-indicator

## MODIFIED Requirements

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
