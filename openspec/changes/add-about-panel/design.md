## Context

On macOS, Tauri installs a default application menu automatically when the app sets no menu of its own. SpecForge only ever constructs a *tray* menu (`tray.rs`, attached via the tray builder's `.menu()`), which is a separate surface — so the macOS app menu is Tauri's auto-default. That default already contains an "About SpecForge" item, but it opens the bare system panel populated only from the bundle's name and version.

The goal is to enrich that panel — add a tagline, copyright, repository link, and license — and to do so in a way that states SpecForge's identity against the OpenSpec format it reads. The catch is mechanical: `muda`/Tauri exposes the About item as a single `PredefinedMenuItem::about(manager, text, Some(AboutMetadata { .. }))`. There is no API to mutate one item inside the auto-default menu. To attach metadata, the app must build and install its own `Menu` — at which point it owns the *entire* menu and the auto-default (including Edit and Window) disappears.

All the content already exists on disk and is currently unused by the panel:

- `bundle.copyright` = `"© 2026 Avant Media Ltd"`
- `bundle.shortDescription` = `"SpecForge — menu-bar viewer for OpenSpec changes across workspaces"` (tagline source)
- `LICENSE` = MIT, holder "Avant Media LTD"
- workspace `version` = `0.1.0` (reachable at runtime via `app.package_info().version`)

One latent bug surfaced while gathering these: `bundle.homepage` is `https://github.com/avantmedia/specforge`, but the configured git remote is `https://github.com/avantmedialtd/specforge`. The About link must not ship the wrong URL.

## Goals / Non-Goals

**Goals:**

- The macOS About panel shows: product name "SpecForge", the live version, the copyright line, and a `credits` block carrying a tagline naming the OpenSpec format, the canonical repository URL (as text), and an MIT license line. (The native panel renders only `name`/`version`/`short_version`/`copyright`/`icon`/`credits`, so all the prose content rides in `credits` — see the dedicated decision below.)
- Replacing the auto-default menu does not regress any standard macOS behaviour the user relies on — specifically Cut/Copy/Paste/Select-All in text inputs and Minimize.
- The version is read at runtime so it tracks the bundle version with no second source of truth.
- The brand-vs-format discipline is preserved and, in fact, made explicit in the panel copy.

**Non-Goals:**

- Any Windows/Linux About surface. A custom `Menu` on those platforms renders as a visible window menu bar, which is wrong for a tray-resident app. The custom menu is `#[cfg(target_os = "macos")]`-gated; non-macOS platforms keep their current chrome and get no About item.
- An in-app (web-rendered) about pane, a standalone about window, or a tray-menu About item. The native macOS panel reached from the app menu is the chosen vessel.
- A new Tauri command or any frontend code. The panel is fully native; nothing crosses the IPC boundary.
- Reworking the tray menu (`tray.rs`). It stays Show / Quit.
- Adding a full menu vocabulary (View, Help, Format, etc.). Only the app, Edit, and Window submenus needed to avoid regressions are built.

## Decisions

### Decision: Own a custom macOS app menu rather than enrich the bundle

To put identity content into the panel, the metadata must be passed to `PredefinedMenuItem::about`. The auto-default menu offers no per-item patch point, so the app builds its own `Menu` in `run()` (via `app.set_menu(..)` in `setup`) and installs it on macOS only.

**Alternative considered:** leave the menu alone and only enrich bundle metadata (e.g. richer `Info.plist` keys). Rejected — the bundle drives the *bare* panel's name/version/copyright but offers no way to add the identity tagline, repository URL, or license text; only a custom About item's `credits` field carries that.

### Decision: Fold the tagline, repo URL, and license into `credits` — the only rich field macOS renders

The native macOS About panel is drawn by `NSApplication::orderFrontStandardAboutPanelWithOptions:`. As wired through muda 0.19.2, it reads **only** `name`, `version`, `short_version`, `copyright`, `icon`, and `credits` — and silently ignores `comments`, `website`, `website_label`, `license`, and `authors` (Tauri's own builder docs annotate each of those as "macOS: Unsupported"). Setting the tagline via `comments`, the repo via `website`, and the license via `license` would compile cleanly and then render *nothing*. So all the prose that must be visible is composed into a single multi-line `credits` string: the OpenSpec tagline, the repository URL, and the MIT license line. `name`/`version`/`copyright` stay in their own fields (those do render).

Because the whole module is `#[cfg(target_os = "macos")]`-gated, the macOS-unsupported fields are not set at all — leaving them would be dead, misleading code on the only platform this compiles for.

**Consequence:** `credits` renders as a plain `NSAttributedString`, so the repository URL appears as **text, not a clickable hyperlink**. A live link would require a custom (non-standard) About window, which is out of scope. The spec scenarios and verification tasks assert the URL is *present as text*, not that it is activatable.

**Alternative considered:** set the proper-named fields (`website`, `license`, `comments`) and accept that they only surface on a future Windows/Linux build. Rejected — this module is macOS-only by design (see Non-Goals), so those fields would never render anywhere, making them pure noise.

### Decision: Rebuild the Edit and Window submenus by hand

Installing a custom menu discards Tauri's auto-default, including the standard Edit and Window submenus. Those carry the system text-editing shortcuts (`Cmd-C/V/X/A`, Undo/Redo) and `Cmd-M` minimize, which the app's own inputs (the workspace-rename field in `SettingsView`, palette/name editing) depend on. The custom menu therefore reconstructs:

- **App submenu** ("SpecForge"): `about` (enriched) · separator · `services` · separator · `hide` / `hide_others` / `show_all` · separator · `quit`.
- **Edit submenu**: `undo` / `redo` · separator · `cut` / `copy` / `paste` / `select_all`.
- **Window submenu**: `minimize` / `maximize` · separator · `close_window`, mirroring Tauri's own default Window submenu. Crucially it is built with `Submenu::with_id_and_items(.., WINDOW_SUBMENU_ID, ..)`: Tauri only attaches the macOS Windows-menu role (`setWindowsMenu:` → Zoom, Bring All to Front, the live window list, Cmd-` cycling) when the submenu carries that magic id. A plain `SubmenuBuilder::new(handle, "Window")` gets an auto-generated id and the role is never applied. `Cmd-W` (`close_window`) is intercepted by the existing `CloseRequested` handler and hides the window, matching the traffic-light close button.

All of these are `PredefinedMenuItem` constructors already provided by Tauri — no manual key-equivalent wiring.

**Alternative considered:** ship only the app submenu with the About item. Rejected — it silently breaks paste/select-all in the rename field, a regression worse than the feature is worth.

### Decision: Read the version at runtime, hardcode the rest

`version` comes from `app.package_info().version.to_string()` so it can never disagree with the shipped bundle. The remaining *rendered* fields (`name`, `copyright`, and the composed `credits` block) are passed as literals to `AboutMetadataBuilder`; `comments`/`website`/`website_label`/`license`/`authors` are intentionally left unset because the native macOS panel ignores them (see the credits-folding decision above). The about predefined item takes all-or-nothing metadata — there is no "inherit the bundle value for the fields I didn't set" — and `package_info()` does not expose copyright or homepage, so those are literals too.

**Trade-off:** the copyright string is now duplicated between `bundle.copyright` and the menu literal. Accepted — it is one short string that changes once a year; centralising it (build-time env, generated constant) costs more than it saves. A code comment cross-references the bundle value so they are updated together.

### Decision: Display name "SpecForge", not the crate name

`package_info().name` is the Cargo crate name `specforge` (lowercase). The panel must read "SpecForge", so `name` is a literal. This also keeps the panel correct if the crate is ever renamed.

### Decision: Tagline harmonised with the existing bundle description

Rather than coin a new slogan, the tagline — carried as the first line of the `credits` block — echoes the established `bundle.shortDescription` framing ("a menu-bar viewer for OpenSpec changes across workspaces"). This keeps every product-description surface saying the same thing and embodies the `product-identity` rule: the line names the product (SpecForge) and the format (OpenSpec) in their correct senses.

### Decision: Fix `bundle.homepage` to the git-remote org in the same change

The repository URL in the About `credits` text and `bundle.homepage` should agree, and the panel must not show a possibly-dead URL. The git remote (`avantmedialtd/specforge`) is treated as canonical, and `bundle.homepage` is corrected from `avantmedia/specforge` to match. Bundling the one-line config fix with the feature avoids shipping a known-stale URL.

## Risks / Trade-offs

- **[Risk] Replacing the auto-default menu silently drops a standard item, regressing a shortcut.** → Mitigation: the rebuild list above is explicit, and verification exercises Paste and Select-All in the Settings rename field plus `Cmd-M` minimize before the change is considered done.
- **[Risk] The custom menu leaks onto Windows/Linux as a window menu bar.** → Mitigation: the install is `#[cfg(target_os = "macos")]`-gated; non-macOS builds are untouched.
- **[Risk] `package_info()` returns empty `authors`/`description` if the crate's Cargo metadata is sparse, making a "derive from package" approach yield blanks.** → Mitigation: only `version` is sourced from `package_info`; identity fields are literals, so sparse Cargo metadata cannot blank the panel.
- **[Trade-off] Copyright duplicated across bundle config and menu literal.** → Accepted; cross-referenced by comment.

## Open Questions

- **Canonical GitHub org** (`avantmedialtd` vs `avantmedia`). This design assumes the git remote is canonical and fixes `bundle.homepage` accordingly. If `avantmedia` is the intended public org, the fix direction flips. Resolve before landing — it determines the URL baked into a shipped artefact.
