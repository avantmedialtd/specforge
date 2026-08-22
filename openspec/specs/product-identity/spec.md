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

The repository SHALL maintain a single canonical raster source for the application bundle icon at `crates/specforge/icons/app-icon.png`, authored as a 1024×1024 PNG. All derived bundle icons in the same directory — the `bundle.icon` PNG set (`32x32.png`, `64x64.png`, `128x128.png`, `128x128@2x.png`, `icon.png`), the platform-bundle binaries (`icon.icns`, `icon.ico`), the iOS Asset Catalog (`ios/AppIcon-*.png`), the Android adaptive icons (`android/mipmap-*/ic_launcher{,_round,_foreground}.png` and the `values/` / `mipmap-anydpi-v26/` XML), and the Windows Store assets (`Square*Logo.png`, `StoreLogo.png`) — SHALL be mechanically derivable from this source by running `bun tauri icon crates/specforge/icons/app-icon.png --ios-color "#1a1a1a"` from the repository root. The source MAY be an opaque, pre-composed tile: the forge illustration ships its own framed background, so unlike the prior transparent-mark source it does not require a per-platform background composite. Where the source is fully opaque the `--ios-color "#1a1a1a"` argument is a harmless no-op retained for command stability.

The same canonical source SHALL additionally yield the **web icon raster set** consumed by the served browser bundle — the Apple touch icon and the manifest icons, including the maskable variant, committed under `public/` — see the *Served Document Declares an Icon Set* requirement in the `web-app-install` capability. That set is NOT produced by `bun tauri icon`, which emits no web sizes and knows nothing of the manifest; it SHALL instead be regenerable by its own documented repository script. Running either generator SHALL leave the canonical source unchanged and SHALL NOT overwrite the other generator's outputs.

Three icon files in the repository are **authored, not derived**, and no regeneration run SHALL overwrite them: the two tray glyph SVGs (`crates/specforge/icons/tray-icon.svg`, `crates/specforge/icons/tray-specs.svg`), independently authored as black-only templates per the `tray-indicator` capability, and the web glyph (`public/favicon.svg`), independently authored for the sizes at which the illustration is not legible per the *Small Sizes Use an Authored Glyph, Not the Illustration* requirement in the `web-app-install` capability.

#### Scenario: Source file present at canonical path

- **WHEN** the repository is checked out at any commit on the default branch
- **THEN** `crates/specforge/icons/app-icon.png` exists
- **AND** the file is a 1024×1024 PNG

#### Scenario: Derivatives regenerable from source

- **WHEN** a developer runs `bun tauri icon crates/specforge/icons/app-icon.png --ios-color "#1a1a1a"` from the repository root
- **THEN** the command overwrites the derived icon files under `crates/specforge/icons/` with newly rasterized content from the source
- **AND** the source `crates/specforge/icons/app-icon.png` is left unchanged
- **AND** the tray vector sources `crates/specforge/icons/tray-icon.svg` and `crates/specforge/icons/tray-specs.svg` are left unchanged

#### Scenario: Web icon set regenerable from the same source

- **WHEN** a developer runs the repository's web icon generation script
- **THEN** the web icon raster set under `public/` is rewritten from `crates/specforge/icons/app-icon.png`
- **AND** the source `crates/specforge/icons/app-icon.png` is left unchanged
- **AND** no file under `crates/specforge/icons/` is modified

#### Scenario: Authored icon sources survive every regeneration

- **WHEN** either the bundle icon command or the web icon generation script is run
- **THEN** `crates/specforge/icons/tray-icon.svg`, `crates/specforge/icons/tray-specs.svg`, and `public/favicon.svg` are all left unchanged

#### Scenario: Bundle references derivatives, not the source

- **WHEN** `crates/specforge/tauri.conf.json` is loaded by the Tauri CLI at build time
- **THEN** the `bundle.icon` array references the derived files (at least `icons/32x32.png`, `icons/128x128.png`, `icons/128x128@2x.png`, `icons/icon.icns`, and `icons/icon.ico`)
- **AND** the raw `icons/app-icon.png` source does NOT appear in the `bundle.icon` array

#### Scenario: Opaque source yields opaque iOS variants

- **WHEN** the derived iOS Asset Catalog (`ios/AppIcon-*.png`) is inspected
- **THEN** no iOS variant exposes alpha transparency (iOS rejects transparent app icons)
- **AND** because the source is an opaque tile, the iOS variants present the framed forge mark directly

### Requirement: macOS Icon Tile

The macOS bundle icon (`crates/specforge/icons/icon.icns`) and the bundle window-icon PNGs at the root of the `icons/` directory (`32x32.png`, `64x64.png`, `128x128.png`, `128x128@2x.png`, `icon.png`) SHALL present the forge illustration as a **full-bleed opaque square** that fills the icon canvas edge-to-edge — no rounded corners, no transparent margin, and no separate `#1a1a1a` tile. Because the canonical source is itself an opaque square, the macOS renditions are direct rasterizations of `app-icon.png`: `icon.icns` is packed via `iconutil` from the ten canonical macOS sizes and the bundle PNG set is re-rendered at the corresponding sizes, all from the square source. The Windows `.ico`, the iOS Asset Catalog, the Android adaptive icons, and the Windows Store assets likewise derive full-bleed from the same square source and rely on their own platform masking or backgrounds.

#### Scenario: macOS icon is a full-bleed opaque square

- **WHEN** the macOS `.icns` is decoded at the 512×512 rendition
- **THEN** it is opaque edge-to-edge with square (un-rounded) corners (corner alpha = 255)
- **AND** there is no transparent margin and no flat `#1a1a1a` tile

#### Scenario: Bundle PNGs are square rasterizations of the source

- **WHEN** any of `32x32.png`, `64x64.png`, `128x128.png`, `128x128@2x.png`, or `icon.png` is decoded
- **THEN** the rendition is a full-bleed square rasterization of `app-icon.png` at that size
- **AND** its corners are opaque (no rounding)

#### Scenario: Mark remains recognizable at small sizes

- **WHEN** the bundle icon is rendered at 32×32
- **THEN** the forge mark is recognizable as a dark anvil-and-hammer icon
- **AND** a dedicated simplified `32x32.png` MAY be shipped if the full illustration does not survive downscaling (this change ships the full illustration at all sizes — no simplified variant)

### Requirement: About Panel States Product and Format

The macOS About panel SHALL be a governed brand surface. It SHALL identify the application as "SpecForge", display the application version and copyright, and its credits text SHALL name the **OpenSpec** format the application reads — keeping both names in their correct senses per the brand-vs-format distinction (SpecForge is the product; OpenSpec is the format). The tagline line carried in the credits text SHALL be consistent with the bundle short description rather than introducing a divergent product description.

#### Scenario: About panel identifies the product as SpecForge

- **WHEN** the user opens the macOS About panel
- **THEN** the panel displays the product name as "SpecForge"
- **AND** it displays the application version and the copyright line

#### Scenario: About panel names the OpenSpec format

- **WHEN** the About panel's credits text is shown
- **THEN** it contains a tagline that refers to the **OpenSpec** format the application reads
- **AND** it uses "OpenSpec" in its format sense, not as a product name

#### Scenario: Tagline consistent with the bundle description

- **WHEN** the tagline line in the About panel's credits text and the bundle short description are compared
- **THEN** they describe the product consistently (the tagline does not introduce a divergent product description)

