# Tasks

## 1. Rename the application crate directory

- [x] 1.1 `git mv crates/openspec-tray crates/specforge`
- [x] 1.2 Update root `Cargo.toml`: change `members = ["crates/openspec-core", "crates/openspec-tray"]` → `members = ["crates/openspec-core", "crates/specforge"]`
- [x] 1.3 Update `crates/specforge/Cargo.toml`: set `[package] name = "specforge"` and `[lib] name = "specforge_lib"`
- [x] 1.4 Update `crates/specforge/src/main.rs`: change `openspec_tray_lib::run()` → `specforge_lib::run()`
- [x] 1.5 Run `cargo check` from the repo root and confirm the workspace builds with no rename-related errors

## 2. Update Tauri product identity

- [x] 2.1 In `crates/specforge/tauri.conf.json`, set `productName` to `"SpecForge"`
- [x] 2.2 In the same file, set `identifier` to `"com.avantmedia.specforge"`
- [x] 2.3 In the same file, set the main window `title` to `"SpecForge"`
- [x] 2.4 In the same file, rewrite `bundle.shortDescription` and `bundle.longDescription` to begin with "SpecForge" rather than "OpenSpec"
- [x] 2.5 Confirm `bundle.homepage` already points at `https://github.com/avantmedia/specforge` (no change needed)

## 3. Update tray menu and tooltip copy

- [x] 3.1 In `crates/specforge/src/tray.rs`, change the Show menu item text from `"Show OpenSpec"` → `"Show SpecForge"`
- [x] 3.2 In the same file, change the Quit menu item text from `"Quit OpenSpec"` → `"Quit SpecForge"`
- [x] 3.3 In the same file, change the tray tooltip from `"OpenSpec"` → `"SpecForge"`
- [x] 3.4 In the same file, change the fallback tooltip/title string `"OpenSpec".to_string()` → `"SpecForge".to_string()`

## 4. Update frontend product identity

- [x] 4.1 In `package.json`, set `"name"` to `"specforge"`
- [x] 4.2 In `index.html`, set `<title>` to `SpecForge`
- [x] 4.3 In `src/types.ts`, update the comment that references `crates/openspec-tray/src/events.rs` to point at `crates/specforge/src/events.rs`

## 5. Update doc comments referencing the legacy product name

- [x] 5.1 In `crates/openspec-core/src/lib.rs`, change the module doc-comment "Headless core for the OpenSpec Tray application" → "Headless core for the SpecForge application"
- [x] 5.2 In the same file, change the line "UI concerns live in `openspec-tray`" → "UI concerns live in `specforge`"
- [x] 5.3 Grep for any remaining `OpenSpec Tray` / `openspec-tray` / `openspec_tray` matches outside `openspec/changes/archive/` and resolve each (rename to SpecForge equivalents, or leave only if the reference is to the archived bootstrap change history)

## 6. Verify nothing in the OpenSpec-format surface was touched

- [x] 6.1 Confirm `crates/openspec-core/src/registry.rs` still rejects folders with the message "not an OpenSpec workspace" and still keys validation on `canonical.join("openspec").is_dir()`
- [x] 6.2 Confirm `src/components/SettingsView.tsx` still uses the dialog title `"Choose an OpenSpec workspace folder"` and the help text mentioning `openspec/` directories
- [x] 6.3 Confirm `src/components/WorkspaceTree.tsx` still mentions `openspec/` directory in its empty state
- [x] 6.4 Confirm every `workspace_path.join("openspec")…` call site in `crates/openspec-core/src/` is unchanged
- [x] 6.5 Confirm the `NotAnOpenSpecWorkspace` error variant is unchanged

## 7. Build, run, and verify

- [x] 7.1 Run `cargo check --workspace` and confirm it passes
- [x] 7.2 Run `bun run build` (frontend `tsc --noEmit && vite build`) and confirm it passes
- [x] 7.3 Run `bun tauri dev`, confirm the window title bar reads "SpecForge", the tray tooltip reads "SpecForge", and the tray menu shows "Show SpecForge" / "Quit SpecForge"
- [x] 7.4 In the running app, open Settings, click the workspace picker, and confirm the dialog title still reads "Choose an OpenSpec workspace folder" (format reference preserved)
- [x] 7.5 Re-register the dev workspace once (since the bundle-id change abandoned the old per-user data) and confirm changes load correctly
