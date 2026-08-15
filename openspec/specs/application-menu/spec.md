# application-menu Specification

## Purpose

Defines the macOS application menu SpecForge installs in the system menu bar in place of Tauri's auto-generated default: the SpecForge submenu whose About item opens the native About panel from an `AboutMetadata` value (product name, runtime-read package version, copyright, and a credits block carrying the OpenSpec tagline, the canonical repository URL, and the MIT license line), the rebuilt Edit and Window submenus that keep the standard editing shortcuts, Minimize, and the system Windows-menu role working once the default is discarded, and a View submenu whose Cmd+B and Cmd+Alt+B items emit pane-toggle events to the webview. The menu is macOS-only; Windows and Linux install none, so those platforms have no View submenu. It does not own the tray icon's own context menu, nor the pane-visibility behaviour the View items trigger.

## Requirements
### Requirement: Custom macOS Application Menu

On macOS, the application SHALL install its own application menu rather than relying on Tauri's auto-generated default menu, so that the About item can carry enriched metadata. The custom menu SHALL be installed only on macOS; on other platforms the application SHALL NOT install a custom menu (a custom `Menu` would render as a window menu bar, which is inappropriate for a tray-resident application).

The custom menu SHALL contain at least three submenus: an application ("SpecForge") submenu, an Edit submenu, and a Window submenu.

#### Scenario: macOS installs a custom application menu

- **WHEN** the application launches on macOS
- **THEN** the menu bar's application submenu is the application's own menu, not Tauri's unmodified auto-default
- **AND** the application submenu's first item is "About SpecForge"

#### Scenario: Non-macOS platforms install no custom menu

- **WHEN** the application launches on Windows or Linux
- **THEN** the application installs no custom menu
- **AND** no application-provided window menu bar appears

### Requirement: Enriched About Panel

The "About SpecForge" application-menu item SHALL open the native macOS About panel populated from an `AboutMetadata` value. Because the native macOS panel renders only `name`, `version`, `short_version`, `copyright`, `icon`, and `credits`, the metadata SHALL set the product name "SpecForge", the application version, a copyright line, and a `credits` block. The `credits` block SHALL contain a tagline naming the OpenSpec format the application reads, the canonical repository URL, and an MIT license indication. The version SHALL be read at runtime from the application's package version so it cannot drift from the shipped bundle version. The `AboutMetadata` fields the native macOS panel does not render (`comments`, `website`, `website_label`, `license`, `authors`) SHALL NOT be relied upon to surface content on macOS.

#### Scenario: About panel shows enriched metadata

- **WHEN** the user selects "About SpecForge" from the application menu on macOS
- **THEN** the native About panel opens
- **AND** it displays the name "SpecForge", the application version, and the copyright line
- **AND** its credits text contains the OpenSpec tagline, the repository URL, and the MIT license line

#### Scenario: Version is read at runtime

- **WHEN** the About panel renders the version
- **THEN** the value equals the application's package version (e.g. `0.1.0`) read at runtime, not a separately hardcoded string

#### Scenario: Repository URL is canonical

- **WHEN** the About panel's credits text is shown
- **THEN** it contains the canonical SpecForge repository URL as text (the native panel does not render it as a clickable link)
- **AND** that URL agrees with `bundle.homepage` in the Tauri configuration

### Requirement: Standard Edit and Window Items Preserved

Because installing a custom menu replaces Tauri's auto-default menu, the custom menu SHALL reconstruct the standard editing and window items so that system shortcuts continue to work. The Edit submenu SHALL include Undo, Redo, Cut, Copy, Paste, and Select All. The Window submenu SHALL include Minimize, and SHALL be registered as the macOS Windows menu (via the framework's window-submenu id) so the system attaches the standard Windows-menu role.

#### Scenario: Text-editing shortcuts work in app inputs

- **WHEN** a text input is focused in the application (for example the workspace-rename field in Settings) on macOS
- **THEN** Cut, Copy, Paste, and Select All operate via their standard keyboard shortcuts
- **AND** Undo and Redo are available

#### Scenario: Minimize shortcut works

- **WHEN** the main window is focused on macOS
- **THEN** the standard Minimize shortcut minimizes the window

#### Scenario: Window submenu carries the macOS Windows-menu role

- **WHEN** the application menu is installed on macOS
- **THEN** the Window submenu is registered as the system Windows menu
- **AND** the system-managed Windows-menu behaviour (Zoom, Bring All to Front, the open-window list) is attached rather than lost by replacing the auto-default menu

### Requirement: View Submenu Pane Toggles

On macOS, the custom application menu SHALL include a View submenu containing two items: "Toggle Sidebar" with the Cmd+B accelerator, and "Toggle Commit Rail" with the Cmd+Alt+B accelerator. Activating an item SHALL toggle the visibility of the corresponding pane in the main window (see the *Side-Pane Visibility Toggles* requirement in the `spec-browser` capability), showing the main window first if it is hidden.

The menu items SHALL reach the frontend by emitting an event to the webview; the event name SHALL be defined once on the Rust side and mirrored in the frontend's shared types, consistent with the existing cache-event bridge.

A single keypress of an accelerator SHALL produce exactly one toggle: the native menu accelerator and any in-webview handling of the same key combination SHALL NOT both fire for one keypress.

Because the custom menu is macOS-only (see the *Custom macOS Application Menu* requirement), no View submenu exists on Windows or Linux; those platforms rely on the keyboard bindings and on-screen affordances specified in the `spec-browser` capability.

#### Scenario: View menu toggles the sidebar

- **WHEN** the user selects View → Toggle Sidebar on macOS
- **THEN** the main window's sidebar visibility flips
- **AND** the commit rail's visibility is unchanged

#### Scenario: View menu toggles the commit rail

- **WHEN** the user selects View → Toggle Commit Rail on macOS
- **THEN** the main window's commit-rail visibility flips
- **AND** the sidebar's visibility is unchanged

#### Scenario: Accelerator fires exactly one toggle

- **WHEN** the user presses Cmd+B once with the main window focused on macOS
- **THEN** the sidebar's visibility flips exactly once

