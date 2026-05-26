# Design

## Context

The `tray-indicator` capability already maintains a numeric badge driven by `WatcherManager::total_active_logical_count()`. Its updater lives in `crates/specforge/src/tray.rs::spawn_badge_updater` (around line 126): it subscribes to the `WatcherManager` broadcast, recomputes the count on every `CacheEvent`, and reapplies it via `tray.set_title` on macOS (with a documented workaround for the `set_title(None)` bug in `tray-icon` 0.23.x — passing `Some("")` to actually clear the title).

The Dock is a separate macOS surface from the menu-bar tray. Tauri 2 exposes `Window::set_badge_count(Option<i64>)` which, on macOS, delegates through `tao` to `NSApp.dockTile.badgeLabel`. The CMD+Tab application switcher renders the same Dock tile, so a single call updates both visual surfaces.

The same `total_active_logical_count()` value drives both badges. They are never permitted to disagree.

## Goals / Non-Goals

**Goals:**

- The Dock tile (and therefore the CMD+Tab switcher) displays a numeric badge equal to the menu-bar tray badge at all times.
- The badge updates within the same debounce window as the tray badge — both consume the same `CacheEvent` stream rather than being polled independently.
- The implementation pattern is structurally symmetric to `spawn_badge_updater` so the two updaters read as siblings.
- The badge clears when the count returns to zero, with no stale digit remaining on the tile.
- Setting the Dock badge is funnelled through one helper so any future workaround lives in one place.

**Non-Goals:**

- Windows / Linux Dock-tile equivalents. Tauri's `set_badge_count` on those platforms uses different mechanisms (overlay icons on Windows, Unity launcher metadata on Linux) with different visual semantics. Out of scope here; tracked as a future change.
- A settings toggle for showing/hiding the Dock badge. The tray badge has no toggle; mirroring it unconditionally avoids an inconsistency and a new settings surface.
- Distinct numbers between Dock and tray. Both surfaces show `total_active_logical_count()`.
- Custom Dock-tile artwork. The Dock badge is the standard red-circle digit rendered by AppKit. No SVG, no overlay graphics.
- Glyph variants on the Dock tile. The spec-activity glyph distinction in the tray is a tray-specific affordance (custom-painted icon) and has no equivalent here.
- Replacing or unifying the tray badge logic. The tray and Dock updaters remain independent subscribers.

## Decisions

### Decision: Sibling capability `dock-indicator` rather than extending `tray-indicator`

The `tray-indicator` capability scopes itself to "operating-system tray presence" — menu bar on macOS, system tray on Windows, status notifier on Linux. The Dock is a different surface with different platform semantics: macOS-only, distinct visual model (red badge circle vs. menu-bar text), distinct lifecycle vis-à-vis main-window visibility. Mixing dock requirements into `tray-indicator` would muddy that scoping and force the tray spec to caveat itself with "on macOS only, also for the Dock".

Both capabilities consume the same `WatcherManager` API, but capability boundaries follow user-visible surfaces, not implementation reuse.

**Alternatives considered:**

- *Extend `tray-indicator` with a new "Dock Badge" requirement section.* Convenient (shared data source) but the resulting spec name no longer matches what the capability does. Rejected.

### Decision: Use Tauri's `Window::set_badge_count` rather than `objc2`-direct calls

A small spike confirmed `Window::set_badge_count(Some(3))` paints correctly on the current Tauri 2 pin in this repo. The known unresolved bug at [tauri#13905](https://github.com/tauri-apps/tauri/issues/13905) (`setBadgeCount` no-op on macOS in Tauri 2.7.0) does not reproduce — possibly different minor, possibly macOS-version-dependent. Using the Tauri API keeps the implementation simple and avoids pulling in `objc2` solely for this feature.

The `set_dock_badge` funnel (see next decision) reserves the option to swap to `objc2` later if a Tauri upgrade regresses the behaviour, with the swap localised to one function body.

### Decision: All Dock-badge updates route through a single `set_dock_badge` helper

The `tray-indicator` capability already encodes one upstream workaround in its setter (`set_title(Some(""))` to clear). The Dock badge has a directly analogous risk: `set_badge_count(None)` may or may not actually clear the tile, and the answer can shift with Tauri or macOS versions. Funnelling every update through one helper means:

- The "how to clear" answer lives in one place.
- A future regression can be patched without touching every call site.
- The `set_badge_count(Some(0)) → None` collapse is performed once, not duplicated.

This is a code-organisation discipline, not a spec requirement — the user-visible behaviour is captured by scenarios in the spec; the helper is documented here in design.

### Decision: Always on when count > 0; no settings toggle

The tray badge is unconditional. Adding a settings gate only for the Dock badge would create an inconsistency (users wondering why one toggle exists but not the other), plus an unfound discoverability cost. If the Dock badge proves too loud in practice, a single "show count badges" setting can be introduced later that governs both surfaces uniformly.

### Decision: macOS-only for v1, with a hard `cfg` gate

Tauri's `set_badge_count` is documented as "desktop" but does different things per platform — overlay icons on Windows, Unity launcher data on Linux. Treating it as a single cross-platform feature would force spec scenarios to caveat per platform.

Scoping to macOS keeps this change small and lets the spec describe a single coherent behaviour. The module file carries a file-level `#[cfg(target_os = "macos")]` so non-macOS builds neither compile nor reach this code. A Windows/Linux equivalent — likely with its own `windows-overlay-indicator` / `linux-launcher-indicator` capability — is left as future work.

## Open Questions

- **Does `set_badge_count(None)` actually clear the Dock tile on the current Tauri pin?** Will be answered during implementation verification (task 4.4). If it fails to clear, the workaround lives inside `set_dock_badge` — likely an `objc2` call to set the badge label to an empty `NSString`, mirroring the tray's `Some("")` strategy.
- **Quit-time cleanup.** When the user Cmd-Q's the app, macOS clears the Dock tile as the process exits. We don't expect to have to call `set_badge_count(None)` on the way down, but if a stale badge ever lingers, the same funnel helper is the place to deal with it.
