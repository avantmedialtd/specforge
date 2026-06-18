## 1. Screen scaffolding

- [ ] 1.1 Add `Screen::Settings` to the `Screen` enum in `app.rs`.
- [ ] 1.2 Add a `settings_selected: usize` cursor field to `Model` and initialize it in `Model::new`.
- [ ] 1.3 Bind a key to select the Settings screen (`6`) in `handle_key`, and route key events to a new `handle_settings_key` when `model.screen == Screen::Settings`.

## 2. Toggle interaction

- [ ] 2.1 In `handle_settings_key`, move the cursor with `j`/`k` and arrow keys across the toggle rows, and return to Browse on `Esc`.
- [ ] 2.2 On `Space`/`Enter`, flip the focused row: call `svc.settings.set_gamification_enabled(!cur)` or `svc.settings.set_claude_quota_enabled(!cur)`, reading the current value live from `svc.settings`.
- [ ] 2.3 After flipping gamification, re-dispatch the gamified-surface fetch (the same async path used by the Dashboard/Season/Garden screen keys) so those screens reflect the new state without a restart.
- [ ] 2.4 After disabling the quota opt-in, reset `model.quota` to the disabled state so the title-bar gauge clears immediately; leave enabling to the running poller's next refresh.

## 3. Rendering

- [ ] 3.1 Add a `settings(f, area, model)` render fn in `ui.rs` that draws one row per toggle with its current on/off state and highlights the focused row.
- [ ] 3.2 Dispatch `Screen::Settings => settings(...)` in the `match model.screen` block and add "Settings" to the `title_bar` screen switcher.
- [ ] 3.3 Add a footer hint for the Settings screen (move/toggle/back) and update the screen-count hint to `1-6 screens`.
- [ ] 3.4 Add the new keys (`6` Settings, toggle keys) to the help overlay.

## 4. Tests & docs

- [ ] 4.1 Add a render test in `render_tests.rs` covering the Settings screen in both toggle states (gamification/quota on and off).
- [ ] 4.2 Update `crates/specforge-tui/README.md`: list `6 Settings` in the interactive keys and note the TUI now writes app settings (never workspace files).
- [ ] 4.3 Update the repo `README.md` quota/TUI mentions to reflect the new in-TUI opt-in control.

## 5. Verification

- [ ] 5.1 `cargo test -p specforge-tui` passes (including the new render test).
- [ ] 5.2 `cargo fmt --check` and `cargo clippy` are clean for the TUI crate.
- [ ] 5.3 Manual check: toggling gamification swaps the gamified screens live; toggling the quota opt-in shows/clears the title-bar gauge; both survive a restart.
