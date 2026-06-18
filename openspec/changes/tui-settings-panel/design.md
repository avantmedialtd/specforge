## Context

`specforge-tui` is a read-only OpenSpec browser built on the shared headless
`openspec_app::AppService`. The service exposes `pub settings:
Arc<SettingsStore>`, and the store already has the writers this change needs
(`set_gamification_enabled`, `set_claude_quota_enabled`). The TUI's `run_tui`
owns the `AppService` and today only *reads* those flags. So the settings panel
is a pure frontend addition: a new `Screen::Settings` plus key handling that
calls existing writers. No service method, IPC, or storage is added.

The TUI has five screens today (`Browse`, `Dashboard`, `Season`, `Garden`,
`History`), selected from a number-key switcher and rendered through a `match
model.screen` in `ui.rs`. The quota poller is spawned once at startup and
re-reads `claude_quota_enabled()` every tick.

## Goals / Non-Goals

**Goals:**
- A sixth screen, `Screen::Settings`, consistent with the existing screens
  (full pane, in the switcher, `Esc` back to Browse).
- Two toggle rows — gamification and Claude quota — each showing on/off state,
  flipped with the keyboard, persisted immediately.
- Toggles take effect live: the gamified surfaces and the title-bar gauge
  reflect a change without restarting the TUI.

**Non-Goals:**
- The `notifications_enabled` toggle (no-op in a terminal — the TUI fires no
  desktop notifications).
- Numeric settings (`claude_quota_refresh_secs`, `wsl_poll_interval_secs`) and
  the text/roster settings (`identity`, `people`). Deferred; they need stepper
  and text-entry affordances.
- Live cross-process propagation to a running desktop app.

## Decisions

- **Sixth screen, not a modal overlay.** `Screen::Settings` joins the existing
  enum and the number-key switcher (bind to `6`; the footer hint becomes
  `1-6 screens`). Chosen over a `?`-style overlay for consistency with the
  other screens and room to grow (identity/people later) without re-cramming a
  popup.
- **Cursor + flip interaction.** A `settings_selected: usize` cursor on `Model`
  (0..N rows); `j`/`k` (and arrows) move it, `Space`/`Enter` flips the focused
  row. Each row reads its current value live from `svc.settings` at render time,
  so the displayed state always matches what was just written.
- **Persist on flip, no save button.** Every `SettingsStore` setter writes
  `settings.json` synchronously, matching the desktop's immediate-persist model.
- **Gamification toggle must refetch.** `Dashboard`/`Season`/`Garden` read
  `gamification_enabled()` when their data is *built*, not at render. So after
  flipping gamification the handler must re-dispatch the same async fetch the
  `2`/`3`/`4` keys use, or the sibling screens stay stale until re-navigated.
  This is the one non-trivial wire-up in the change.
- **Quota toggle leans on the existing poller.** Enabling needs nothing — the
  always-running poller re-reads the flag each tick and emits `Msg::Quota` on
  its next refresh. Disabling additionally resets `model.quota` to the disabled
  state immediately so the title-bar gauge clears at once instead of lingering a
  tick.
- **Read-only stays true for workspaces.** The existing "Read-Only Operation"
  requirement is already scoped to *workspace* files; this change narrows it
  explicitly to permit writing app config (which lives in the shared config
  dir, outside any workspace). No workspace file is ever written.

## Risks / Trade-offs

- **Shared settings, process-local effect.** Each process holds its own
  in-memory `Mutex<AppSettings>`. A TUI toggle takes effect in the TUI
  immediately and in a running desktop app only on its *next launch* — not live.
  This is the same co-write trade-off already accepted for `activity.json` and
  window-state; the change documents it rather than implying live sync.
- **Concurrent writers.** A desktop app and the TUI can both write
  `settings.json`. Writes are whole-file and each setter persists a full
  snapshot, so the last writer wins per-write; there is no field-level merge.
  Acceptable for low-frequency manual toggles.
- **Forgetting the refetch** would make the gamification toggle look broken
  (flag flips, screens don't change until re-navigated). Called out above and
  covered by the "Toggling gamification updates the gamified surfaces"
  scenario.
