## ADDED Requirements

### Requirement: Zero-Count Badge Title Cleared At The OS Layer

When the active logical change count transitions to zero (or is initialised as zero), the macOS menu-bar title attached to the tray status item MUST be cleared by invoking the underlying `NSStatusBarButton.setTitle:` with an empty `NSString`. The application SHALL pass `Some("")` to `tray.set_title` for this case, not `None`.

This codifies a workaround for upstream `tray-icon` 0.23.x behaviour: `set_title_inner` in `tray-icon/src/platform_impl/macos/mod.rs` early-returns when given `None`, so `setTitle:` is never invoked and the previous title remains attached to the button. Passing `Some("")` reaches `setTitle:@""`, which collapses the status item back to icon-only width.

The application MUST NOT rely on the intuitive interpretation of `tray.set_title(None)` (i.e., "clear the title") for the zero-count case on macOS. Any future code path that drives the badge MUST funnel through a helper that explicitly substitutes the empty-string title for the no-count case.

#### Scenario: Last active change archived clears the menu-bar title

- **WHEN** the badge currently displays a non-zero count
- **AND** the last non-archived logical change across all tracked workspaces is moved into `openspec/changes/archive/`
- **THEN** within the watcher debounce window the menu-bar item's title is empty
- **AND** the status item collapses to icon-only width with no stale digit visible

#### Scenario: set_title called with empty string when count is zero

- **WHEN** `set_badge` is invoked with a count of `Some(0)` or `None`
- **THEN** the underlying `tray.set_title` call carries `Some("")`
- **AND** never carries `None`

#### Scenario: set_title called with the digit when count is non-zero

- **WHEN** `set_badge` is invoked with a count of `Some(n)` where `n` ≥ 1
- **THEN** the underlying `tray.set_title` call carries `Some(n.to_string())`
