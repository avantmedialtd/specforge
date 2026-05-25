# Design: Preserve Tray Icon Template Flag Across Updates

## Context

`tauri::tray::TrayIcon` (Tauri 2.11, backed by `tray-icon` 0.23) exposes three relevant methods for changing the tray glyph:

- `set_icon(icon)` — replaces the underlying `NSImage`. On macOS, the new image's `isTemplate` flag defaults to `false`.
- `set_icon_as_template(bool)` — toggles the template flag on the *current* icon.
- `set_icon_with_as_template(icon, bool)` — atomic; replaces the icon **and** sets the template flag in one operation. Tauri's docstring explicitly warns: *"calling `set_icon` followed by `set_icon_as_template` causes a visible flicker"*.

The shell uses `set_icon` in three places, all of which currently lose the template flag.

## Goals / Non-Goals

**Goals:**
- Every icon update — initial, variant flip, scale-change re-rasterization — uses the template flag, so macOS continues to recolour the glyph from the menu-bar foreground colour.
- Avoid the flicker mode Tauri explicitly warns about.
- Make the invariant explicit in the spec so it's not silently re-broken.

**Non-Goals:**
- Changing the rasterizer, the variant predicate, or the badge logic.
- Reworking how the initial install sets the flag (`TrayIconBuilder::icon_as_template(true)` still works for the first icon).
- Adding tests that exercise NSImage's `isTemplate` directly — that's outside the Rust test scope and requires running on macOS with menu-bar inspection.

## Decisions

### 1. Use `set_icon_with_as_template`, not the two-call pattern

Calling `set_icon(...)` followed by `set_icon_as_template(true)` would also fix the bug functionally, but Tauri's own documentation flags it as flicker-inducing.

- **Alternative considered:** call the two separately for clarity.
- **Rationale:** the atomic API is the maintainer-blessed path. One call instead of two also makes call sites slightly tidier.

### 2. Leave `TrayIconBuilder::icon_as_template(true)` in place

The builder-level setting still correctly configures the initial icon. Removing it would be a no-op refactor.

- **Rationale:** smaller diff, no behavioural difference. The builder pattern stays consistent with how someone reading the code would expect the initial flag to be set.

### 3. Spec the invariant, not the API

The spec requirement talks about "the tray glyph SHALL be rendered as a template image on every set" rather than naming `set_icon_with_as_template` directly. The latter would couple the spec to a specific Tauri method name; the former captures the intent.

- **Rationale:** if Tauri renames or deprecates this method, the spec still holds.

## Risks / Trade-offs

- **Forgotten future call site** → If a new code path adds `tray.set_icon(...)` directly, the bug re-appears. Mitigation: the spec requirement is the authority; a code-review check or a clippy-style lint could enforce it later, but is overkill for the current call-site count (three).
- **Behaviour on non-macOS platforms** → `set_icon_with_as_template` is defined on all platforms; the template flag is a no-op outside macOS. Per Tauri source the flag is `#[allow(unused)]`-prefixed off-macOS, so behaviour is unchanged on Windows/Linux.
- **Test coverage** → The fix is not unit-testable without a real menu bar. Verification is manual: launch the app in macOS dark mode, confirm the glyph shows in the system's foreground colour, then trigger a variant flip and a scale change to confirm the flag survives both.

## Open Questions

None.
