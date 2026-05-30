# Add an Enriched macOS About Panel

## Why

SpecForge has no dedicated "about" surface. On macOS, Tauri auto-installs a default application menu whenever the app sets no menu of its own — and the only menu the app builds today is the *tray* menu (`tray.rs`: Show / Quit). So there is already an **"About SpecForge"** item in the macOS app menu, but it opens the bare system panel: app name and version from the bundle, and nothing else. No tagline, no copyright, no link, and — most importantly — no statement of what SpecForge actually is.

That last gap matters here more than in most apps. The whole codebase is disciplined about a single distinction: **SpecForge** is the product, **OpenSpec** is the format it reads (`product-identity` spec). The About panel is the one natural place to *say* that out loud — "SpecForge, a menu-bar viewer for OpenSpec workspaces" — turning an empty system box into the canonical expression of the brand-vs-format line.

The values needed are already on disk (`bundle.copyright`, `bundle.shortDescription`, the MIT `LICENSE`, the package version); they are simply not wired into the panel. This change wires them in.

## What Changes

- Install a **custom macOS application menu** so the About item can carry enriched metadata. Tauri's auto-default menu cannot be patched item-by-item, so the app must own the menu — which means the standard **Edit** (Undo/Redo, Cut/Copy/Paste, Select-All) and **Window** (Minimize/Zoom/Close) submenus have to be rebuilt by hand, or text-editing shortcuts stop working in the app's inputs (e.g. the workspace-rename field in Settings). The Window submenu must be built with the framework's `WINDOW_SUBMENU_ID` so macOS still attaches the standard Windows-menu role (Zoom, Bring All to Front, the open-window list).
- Populate the About item with `AboutMetadata`. **The native macOS panel renders only `name`, `version`, `short_version`, `copyright`, `icon`, and `credits`** — it ignores `comments`, `website`, `license`, and `authors` — so all prose content rides in `credits`:
  - **name** — `"SpecForge"` (literal display name, not the lowercase crate name).
  - **version** — read at runtime from `app.package_info().version` (`0.1.0`), so it never drifts from the bundle.
  - **copyright** — `"© 2026 Avant Media Ltd"`, mirroring the existing `bundle.copyright`.
  - **credits** — a multi-line block carrying the tagline (naming the product *and* the OpenSpec format, harmonised with `bundle.shortDescription`), the canonical repository URL, and an `MIT License` line. Rendered as plain text, so the URL is visible but not a clickable link.
  - **icon** — omitted; the macOS native panel already uses the bundle icon.
  - `comments`/`website`/`license`/`authors` are **not set** — the macOS panel ignores them, and the module is macOS-only, so setting them would be dead, misleading code.
- **Fix the stale repository URL.** `bundle.homepage` currently reads `https://github.com/avantmedia/specforge`, but the configured git remote is `https://github.com/avantmedialtd/specforge`. The repository URL in the `credits` text SHALL use the canonical (git-remote) URL, and `bundle.homepage` SHALL be corrected to match so the two agree.
- Scope the custom menu to **macOS only** (`#[cfg(target_os = "macos")]`). On Windows/Linux a custom `Menu` becomes a visible *window* menu bar, which is unwanted for a tray-resident app; those platforms keep their current chrome and gain no About item.

## Capabilities

### New Capabilities

- `application-menu`: The app owns a custom macOS application menu containing an enriched About item that opens the native About panel, while preserving the standard Edit and Window items so system text-editing and window shortcuts keep working.

### Modified Capabilities

- `product-identity`: Adds a requirement that the About panel content states the SpecForge product identity (name, version, copyright) and names the OpenSpec format it reads — making the panel a governed brand surface, consistent with the existing brand-vs-format rules.

## Impact

- Code: `crates/specforge/src/lib.rs` (build + install the menu in `run()`), a new menu-construction helper (e.g. `crates/specforge/src/menu.rs`), and `crates/specforge/tauri.conf.json` (correct `bundle.homepage`).
- The custom menu replaces Tauri's macOS auto-default; the Edit and Window submenus must be rebuilt or `Cmd-C`/`Cmd-V`/`Cmd-M` regress in app inputs. This is the main implementation cost and the primary verification target.
- No change to `openspec-core`, the IPC contract, Tauri command names, or event names. No frontend code changes (the panel is fully native; the version comes from `package_info`, not a new command).
- No new runtime dependencies — `AboutMetadata`, `PredefinedMenuItem`, and the menu builders are already part of the `tauri` crate the shell depends on.
- Platform scope: macOS only. Windows/Linux are explicitly out of scope and keep their existing chrome (no About item there).

## Open Questions

- **Canonical repo org.** The git remote (`avantmedialtd/specforge`) and `bundle.homepage` (`avantmedia/specforge`) disagree. This change assumes the git-remote org is canonical and corrects `bundle.homepage` to match. If `avantmedia` is the intended public org, flip the direction (fix the remote / confirm the redirect) before landing.
