# product-identity Specification

## Purpose

Defines the distinction between the SpecForge product brand and the OpenSpec spec format, and the rules that keep them from collapsing into one another in user-visible copy, identifiers, and code. User-facing surfaces identify the application as "SpecForge"; references to the OpenSpec format the application reads retain that name.

## Requirements

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

### Requirement: Canonical Application Icon Source

The repository SHALL maintain a single canonical raster source for the application bundle icon at `crates/specforge/icons/app-icon.png`, authored as a 1024×1024 RGBA PNG. All derived bundle icons in the same directory — the `bundle.icon` PNG set (`32x32.png`, `64x64.png`, `128x128.png`, `128x128@2x.png`, `icon.png`), the platform-bundle binaries (`icon.icns`, `icon.ico`), the iOS Asset Catalog (`ios/AppIcon-*.png`), the Android adaptive icons (`android/mipmap-*/ic_launcher{,_round,_foreground}.png` and the `values/` / `mipmap-anydpi-v26/` XML), and the Windows Store assets (`Square*Logo.png`, `StoreLogo.png`) — SHALL be mechanically derivable from this source by running `bun tauri icon crates/specforge/icons/app-icon.png --ios-color "#1a1a1a"` from the repository root. The `--ios-color "#1a1a1a"` argument is part of the contract: iOS rejects transparent app icons, and `#1a1a1a` matches the near-black tone of the SpecForge anvil silhouette so the dark mark does not disappear against a default-white composite. The two tray glyph SVGs (`crates/specforge/icons/tray-icon.svg`, `crates/specforge/icons/tray-specs.svg`) are NOT derivatives — they are independently authored as black-only templates per the `tray-indicator` capability.

#### Scenario: Source file present at canonical path

- **WHEN** the repository is checked out at any commit on the default branch
- **THEN** `crates/specforge/icons/app-icon.png` exists
- **AND** the file is a 1024×1024 RGBA PNG

#### Scenario: Derivatives regenerable from source

- **WHEN** a developer runs `bun tauri icon crates/specforge/icons/app-icon.png --ios-color "#1a1a1a"` from the repository root
- **THEN** the command overwrites the derived icon files under `crates/specforge/icons/` with newly rasterized content from the source
- **AND** the source `crates/specforge/icons/app-icon.png` is left unchanged
- **AND** the tray vector sources `crates/specforge/icons/tray-icon.svg` and `crates/specforge/icons/tray-specs.svg` are left unchanged

#### Scenario: Bundle references derivatives, not the source

- **WHEN** `crates/specforge/tauri.conf.json` is loaded by the Tauri CLI at build time
- **THEN** the `bundle.icon` array references the derived files (at least `icons/32x32.png`, `icons/128x128.png`, `icons/128x128@2x.png`, `icons/icon.icns`, and `icons/icon.ico`)
- **AND** the raw `icons/app-icon.png` source does NOT appear in the `bundle.icon` array

#### Scenario: iOS composite uses the brand-locked background color

- **WHEN** the derived iOS Asset Catalog (`ios/AppIcon-*.png`) is inspected
- **THEN** transparent regions of the source `app-icon.png` resolve to `#1a1a1a` in every iOS icon variant
- **AND** no iOS variant exposes alpha transparency (iOS rejects transparent app icons)

### Requirement: macOS Icon Tile

The macOS bundle icon (`crates/specforge/icons/icon.icns`) and the bundle window-icon PNGs at the root of the `icons/` directory (`32x32.png`, `64x64.png`, `128x128.png`, `128x128@2x.png`, `icon.png`) SHALL present the SpecForge mark on an opaque rounded-rect tile, not on transparency. The tile is an 824×824 squircle (185 px corner radius, ≈22.4% of tile size, matching the macOS Big Sur+ convention) centered in a 1024×1024 transparent canvas, filled `#1a1a1a` to match the iOS composite color. The transparent source `app-icon.png` is scaled to 820×820 and composited over the tile; each shipped PNG is a resampled rendition of this 1024×1024 master.

`bun tauri icon` does NOT produce this composite on its own — its output for `.icns` and the bundle PNGs would be transparent-source-rasterized at every size, which on macOS Finder/Dock shows the bare anvil silhouette without a tile (and on Linux app launchers shows a transparent foreground). The tile composite is therefore produced as a post-processing step that overwrites the `tauri icon` output. The Windows `.ico`, the iOS Asset Catalog, the Android adaptive icons, and the Windows Store assets are explicitly excluded from this tile — those platforms either compose against their own backgrounds (iOS via `--ios-color`, Android via adaptive-icon `bg`) or handle transparency natively (Windows `.ico`).

#### Scenario: macOS icon presents an opaque tile

- **WHEN** the macOS `.icns` is decoded at the 512×512 rendition (the size macOS uses for Finder Quick Look)
- **THEN** the rendition has a square frame of opaque `#1a1a1a` pixels covering a centered 824/1024 = ~80.5% area
- **AND** the frame's corners are rounded with radius ~22.4% of the tile's edge length
- **AND** the outer ~10% margin on each side is fully transparent (alpha = 0)

#### Scenario: Bundle PNGs share the same tile

- **WHEN** any of `32x32.png`, `64x64.png`, `128x128.png`, `128x128@2x.png`, or `icon.png` is decoded
- **THEN** the rendition is a resized copy of the same 1024×1024 tile master used for `.icns`
- **AND** the tile geometry (squircle proportions, fill color, content scaling) is identical at every size

#### Scenario: Content fits inside the tile at every size

- **WHEN** the tiled `.icns` or any bundle PNG is decoded
- **THEN** the SpecForge mark (anvil + spec document) is contained within the squircle tile
- **AND** the content does not extend into the transparent margin

#### Scenario: Source remains transparent

- **WHEN** the canonical source `crates/specforge/icons/app-icon.png` is inspected
- **THEN** the file is RGBA with a fully transparent background (no opaque pixels outside the mark)
- **AND** the tile composite is not baked into the source — the source is reusable for the iOS, Android, and Windows derivatives that compose against their own per-platform backgrounds
