# product-identity

## ADDED Requirements

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
