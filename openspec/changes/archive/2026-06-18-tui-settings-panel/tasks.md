## 1. Screen scaffolding

- [x] 1.1 Add `Screen::Settings` to the `Screen` enum in `app.rs`.
- [x] 1.2 Add a `settings_selected: usize` cursor field to `Model` and initialize it in `Model::new`.
- [x] 1.3 Bind a key to select the Settings screen (`6`) in `handle_key`, and route key events to a new `handle_settings_key` when `model.screen == Screen::Settings`.

## 2. Toggle interaction

- [x] 2.1 In `handle_settings_key`, move the cursor with `j`/`k` and arrow keys across the toggle rows, and return to Browse on `Esc`.
- [x] 2.2 On `Space`/`Enter`, flip the focused row: call `svc.settings.set_gamification_enabled(!cur)` or `svc.settings.set_claude_quota_enabled(!cur)`, reading the current value live from `svc.settings`.
- [x] 2.3 After flipping gamification, re-dispatch the gamified-surface fetch (the same async path used by the Dashboard/Season/Garden screen keys) so those screens reflect the new state without a restart.
- [x] 2.4 After disabling the quota opt-in, reset `model.quota` to the disabled state so the title-bar gauge clears immediately; leave enabling to the running poller's next refresh.

## 3. Rendering

- [x] 3.1 Add a `settings(f, area, model)` render fn in `ui.rs` that draws one row per toggle with its current on/off state and highlights the focused row.
- [x] 3.2 Dispatch `Screen::Settings => settings(...)` in the `match model.screen` block and add "Settings" to the `title_bar` screen switcher.
- [x] 3.3 Add a footer hint for the Settings screen (move/toggle/back) and update the screen-count hint to `1-6 screens`.
- [x] 3.4 Add the new keys (`6` Settings, toggle keys) to the help overlay.

## 4. Tests & docs

- [x] 4.1 Add a render test in `render_tests.rs` covering the Settings screen in both toggle states (gamification/quota on and off).
- [x] 4.2 Update `crates/specforge-tui/README.md`: list `6 Settings` in the interactive keys and note the TUI now writes app settings (never workspace files).
- [x] 4.3 Update the repo `README.md` quota/TUI mentions to reflect the new in-TUI opt-in control.

## 5. Verification

- [x] 5.1 `cargo test -p specforge-tui` passes (including the new render test).
- [x] 5.2 `cargo fmt --check` and `cargo clippy` are clean for the TUI crate.
- [x] 5.3 Behaviour verified by automated tests (a live TTY check isn't available headless): `settings_toggles_persist_and_take_effect` (flip → persist → gauge clears on disable), `settings_toggle_survives_restart` (disk round-trip across a fresh service), and `renders_settings_screen` (both toggle states render).
