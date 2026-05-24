# Rename Product to SpecForge

## Why

The repository, GitHub remote, and intended brand are all "SpecForge", but every user-visible surface still reads "OpenSpec" — window title, tray menu items, tooltip, bundle product name, file dialog copy. The legacy name was "OpenSpec Tray", and traces of it remain in the crate name, npm package name, and bundle identifier. This conflates two different things: **OpenSpec** is an external spec format the app reads, while **SpecForge** is the product brand. Leaving the conflation in place misleads users, confuses search/discovery, and makes future docs awkward — especially since the app is explicitly not "just a tray" (the bootstrap proposal called it out: a real window with normal app chrome, not a popover).

## What Changes

- Rebrand every user-visible product string from "OpenSpec" or "OpenSpec Tray" to "SpecForge": Tauri `productName`, window title, tray menu entries ("Show SpecForge", "Quit SpecForge"), tray tooltip, bundle short/long descriptions, HTML `<title>`.
- **BREAKING (developer-facing, pre-release):** Rename the Tauri bundle identifier from `com.avantmedia.openspec-tray` to `com.avantmedia.specforge`. On macOS this changes the per-user data directory the app writes to, so any registered-workspace config from a prior install is abandoned. Acceptable at `0.1.0` with no released builds.
- Rename the app crate `crates/openspec-tray` → `crates/specforge`, its Cargo `name` from `openspec-tray` → `specforge`, and its lib `name` from `openspec_tray_lib` → `specforge_lib`. Update workspace member entry accordingly.
- Rename the npm package `name` from `openspec-tray` → `specforge`.
- Keep `crates/openspec-core` and its name unchanged — it's a reusable parser of the OpenSpec format and the name is correct.
- Keep every reference to the `openspec/` directory layout, `openspec/changes/`, `openspec/changes/archive/`, the `NotAnOpenSpecWorkspace` error, the "Choose an OpenSpec workspace folder" file-dialog title, and all settings copy describing "folders containing an `openspec/` directory". These describe the format the product reads — not the product itself.
- Update internal doc-comments that called the product "OpenSpec Tray" (e.g. `crates/openspec-core/src/lib.rs:1`) to "SpecForge".

## Capabilities

### New Capabilities

- `product-identity`: Codifies the distinction between the SpecForge product brand and the OpenSpec spec format, and the rule that user-visible product surfaces present "SpecForge" while references to the OpenSpec format retain that name.

### Modified Capabilities

(none — the rename does not change any requirement in `tray-indicator`, `spec-browser`, or `workspace-registry`. Those specs reference "OpenSpec" only in its format sense, which this change preserves.)

## Impact

- Code: `crates/openspec-tray/tauri.conf.json`, `crates/openspec-tray/src/tray.rs`, `crates/openspec-tray/src/lib.rs`, `crates/openspec-tray/Cargo.toml`, `crates/openspec-tray/src/main.rs`, `crates/openspec-core/src/lib.rs` (doc comment only), `Cargo.toml` (workspace members), `package.json`, `index.html`, `src/types.ts` (comment referencing the renamed crate path).
- Filesystem: `crates/openspec-tray/` directory renamed to `crates/specforge/`. All `git mv` to preserve history.
- macOS bundle identifier change implies abandoning the prior per-user config path; pre-release this is a no-op for anyone but the maintainer, who will need to re-register workspaces once after first launch of the renamed build.
- No new runtime dependencies. No frontend or Rust API contracts change. Tauri commands and event names are unaffected.
- Documentation / GitHub: repo URL and project description are already "SpecForge" — no remote changes required.
