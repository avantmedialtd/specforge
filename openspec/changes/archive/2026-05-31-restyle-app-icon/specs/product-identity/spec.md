# product-identity

> Direction resolved at the mock-compare gate (`tasks.md` §3), revised after a live review: **framed square** — the forge illustration shipped full-bleed with hard corners, exactly as supplied. The canonical source is a 1024×1024 opaque square and the macOS `.icns` / bundle PNGs are direct full-bleed rasterizations of it (no squircle post-process). This supersedes both the bare-mark `#1a1a1a` squircle of the prior `regenerate-brand-icons` contract and the rounded-squircle variant first shipped in this change.

## MODIFIED Requirements

### Requirement: Canonical Application Icon Source

The repository SHALL maintain a single canonical raster source for the application bundle icon at `crates/specforge/icons/app-icon.png`, authored as a 1024×1024 PNG. All derived bundle icons in the same directory — the `bundle.icon` PNG set (`32x32.png`, `64x64.png`, `128x128.png`, `128x128@2x.png`, `icon.png`), the platform-bundle binaries (`icon.icns`, `icon.ico`), the iOS Asset Catalog (`ios/AppIcon-*.png`), the Android adaptive icons (`android/mipmap-*/ic_launcher{,_round,_foreground}.png` and the `values/` / `mipmap-anydpi-v26/` XML), and the Windows Store assets (`Square*Logo.png`, `StoreLogo.png`) — SHALL be mechanically derivable from this source by running `bun tauri icon crates/specforge/icons/app-icon.png --ios-color "#1a1a1a"` from the repository root. The source MAY be an opaque, pre-composed tile: the forge illustration ships its own framed background, so unlike the prior transparent-mark source it does not require a per-platform background composite. Where the source is fully opaque the `--ios-color "#1a1a1a"` argument is a harmless no-op retained for command stability. The two tray glyph SVGs (`crates/specforge/icons/tray-icon.svg`, `crates/specforge/icons/tray-specs.svg`) are NOT derivatives — they are independently authored as black-only templates per the `tray-indicator` capability.

#### Scenario: Source file present at canonical path

- **WHEN** the repository is checked out at any commit on the default branch
- **THEN** `crates/specforge/icons/app-icon.png` exists
- **AND** the file is a 1024×1024 PNG

#### Scenario: Derivatives regenerable from source

- **WHEN** a developer runs `bun tauri icon crates/specforge/icons/app-icon.png --ios-color "#1a1a1a"` from the repository root
- **THEN** the command overwrites the derived icon files under `crates/specforge/icons/` with newly rasterized content from the source
- **AND** the source `crates/specforge/icons/app-icon.png` is left unchanged
- **AND** the tray vector sources `crates/specforge/icons/tray-icon.svg` and `crates/specforge/icons/tray-specs.svg` are left unchanged

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
