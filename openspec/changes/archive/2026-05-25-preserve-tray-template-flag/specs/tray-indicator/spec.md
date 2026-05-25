## ADDED Requirements

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
