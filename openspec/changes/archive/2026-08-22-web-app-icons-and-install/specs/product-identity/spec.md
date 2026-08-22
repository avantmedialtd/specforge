## MODIFIED Requirements

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
