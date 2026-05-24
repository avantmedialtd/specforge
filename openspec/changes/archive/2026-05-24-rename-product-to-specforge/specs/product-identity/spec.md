## ADDED Requirements

### Requirement: Product Brand Name

The application SHALL identify itself as "SpecForge" in every user-visible product surface, including the OS application name, main window title, system tray menu items, system tray tooltip, bundle descriptions, and the HTML document title of the embedded frontend.

#### Scenario: macOS application name

- **WHEN** the application is installed on macOS
- **THEN** the macOS Finder, Dock, and About dialog display the application name as "SpecForge"

#### Scenario: Main window title

- **WHEN** the main application window is open
- **THEN** the window's title bar reads "SpecForge"

#### Scenario: Tray menu entries

- **WHEN** the user opens the system tray menu
- **THEN** the menu contains an entry labelled "Show SpecForge" that brings the main window forward
- **AND** the menu contains an entry labelled "Quit SpecForge" that exits the application

#### Scenario: Tray tooltip

- **WHEN** the user hovers over the tray icon and no per-workspace tooltip has been set
- **THEN** the tooltip text reads "SpecForge"

### Requirement: OpenSpec Format References Preserved

The application SHALL retain the "OpenSpec" name in every string, identifier, path segment, and error message that refers to the **OpenSpec spec format** as opposed to the SpecForge product. This includes filesystem path segments (`openspec/`, `openspec/changes/`, `openspec/changes/archive/`), workspace validation errors, file-dialog prompts asking the user to select an OpenSpec workspace folder, and any settings copy describing the format the application reads.

#### Scenario: Workspace folder selection dialog

- **WHEN** the user opens the workspace folder picker from settings
- **THEN** the dialog title refers to selecting an OpenSpec workspace folder

#### Scenario: Invalid workspace rejection

- **WHEN** the user selects a folder that does not contain an `openspec/` subdirectory
- **THEN** the application emits an error identifying the folder as not being an OpenSpec workspace
- **AND** the error message uses the term "OpenSpec" to describe the required format

#### Scenario: Filesystem layout references

- **WHEN** the application reads or watches workspace contents
- **THEN** it joins paths using the literal segments `openspec`, `changes`, and `archive` as defined by the OpenSpec format

### Requirement: Application Bundle Identifier

The Tauri bundle identifier SHALL be `com.avantmedia.specforge`. The application SHALL NOT ship with the legacy identifier `com.avantmedia.openspec-tray` in any built artefact.

#### Scenario: Bundle identifier in built application

- **WHEN** the application is built for any target platform
- **THEN** the produced bundle declares its identifier as `com.avantmedia.specforge`

### Requirement: Application Crate and Package Names

The Tauri application crate SHALL be named `specforge` and live at `crates/specforge/`. Its library target SHALL be named `specforge_lib`. The frontend npm package declared in the root `package.json` SHALL be named `specforge`. The headless OpenSpec-format parser crate `openspec-core` SHALL retain its current name and location.

#### Scenario: Cargo workspace membership

- **WHEN** the Cargo workspace is resolved
- **THEN** it contains a member at `crates/specforge` whose package name is `specforge`
- **AND** it contains a member at `crates/openspec-core` whose package name is `openspec-core`

#### Scenario: Application library symbol

- **WHEN** the Tauri entry point invokes the application library
- **THEN** it calls into a crate named `specforge_lib`

#### Scenario: Frontend package name

- **WHEN** `package.json` is read by tooling
- **THEN** its `name` field is `specforge`
