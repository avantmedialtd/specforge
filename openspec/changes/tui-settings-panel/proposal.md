# Terminal UI Settings Screen

## Why

The terminal frontend (`specforge-tui`) renders surfaces that are governed by
opt-in settings it cannot change: the gamified Dashboard/Season/Garden screens
sit behind `gamification_enabled`, and the title-bar Claude usage-quota gauge
sits behind `claude_quota_enabled`. Today those switches can only be flipped in
the desktop app, so a terminal-only user — over SSH, in tmux, beside their
editor — can see a feature exists but has no way to turn it on or off without
leaving the TUI. The settings store that backs both switches is already
reachable from the TUI's `AppService`, so closing this gap is almost entirely a
frontend affordance.

## What Changes

- Add a sixth interactive screen, **Settings**, to `specforge-tui`, reachable
  from the existing screen switcher and returning to Browse on `Esc`.
- The screen presents the app settings the terminal frontend can act on as
  toggle rows. The first version ships **two toggles**: the gamification master
  switch and the Claude usage-quota opt-in.
- Flipping a toggle persists immediately to the shared application settings
  (`settings.json`) via the existing `SettingsStore` writers — no separate save
  action.
- Toggling a setting updates the affected surfaces **in the running TUI without
  a restart**: enabling/disabling gamification refetches the gamified screens;
  disabling the quota opt-in clears the title-bar gauge at once (enabling shows
  it on the poller's next refresh).
- Clarify the TUI's read-only guarantee: it remains read-only with respect to
  *workspace* files, while now writing *application config* (which lives outside
  any workspace).
- **Out of scope (first version):** the `notifications_enabled` toggle (a no-op
  in a terminal — the TUI fires no desktop notifications), the quota
  refresh-interval stepper, the Windows-only WSL poll-interval, identity/people
  roster editing, and live cross-process propagation to a running desktop app.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `terminal-ui`: adds a Settings screen to the interactive frontend's screen
  navigation, introduces the ability to toggle application settings from the
  terminal, and narrows the read-only requirement so persisting app config is
  explicitly permitted (workspace files remain untouched).

## Impact

- **`crates/specforge-tui`** — the only code touched:
  - `app.rs`: add `Screen::Settings`, a settings-row cursor on `Model`, a key to
    select the screen, and a `handle_settings_key` that flips the focused toggle
    via `svc.settings.set_gamification_enabled` / `set_claude_quota_enabled` and
    triggers the matching refetch / gauge-clear.
  - `ui.rs`: a `settings(...)` render fn, the screen in `title_bar`'s switcher,
    a footer hint, and the new keys in the help overlay.
  - `render_tests.rs`: a render test for the Settings screen in both toggle
    states.
- **No backend change.** `AppService.settings` is already a public
  `Arc<SettingsStore>` with the required writers; no new service method, IPC, or
  storage is introduced. The desktop shell (`crates/specforge`) is unaffected.
- **Docs:** `crates/specforge-tui/README.md` (interactive keys list and the
  read-only note) and the repo `README.md` quota/TUI mentions.
