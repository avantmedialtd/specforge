# Clear Tray Badge Title On Zero-Count Transition (macOS)

## Why

On macOS, when the active logical change count goes from a non-zero value to zero — for example by archiving the last active change — the menu-bar badge stays displaying the previous count. The tray glyph variant still flips correctly (`Specs` → `Default`), the in-memory cache and aggregated `last_views` both reflect zero, and the badge updater task wakes up and calls `set_badge(Some(0))`. The "1" simply never disappears from the menu bar.

Root cause is upstream. In `tray-icon 0.23.1`, `set_title_inner` (`src/platform_impl/macos/mod.rs:179-191`) early-returns when its title argument is `None`, so `NSStatusBarButton.setTitle:` is never invoked and the previous `NSString` remains attached to the button. Our `set_badge` (`crates/specforge/src/tray.rs:90-107`) deliberately passes `None` for the zero case, expecting it to clear the title; it silently doesn't.

This is the same shape as the `2026-05-25-preserve-tray-template-flag` change — upstream `tray-icon` has surprising defaults on macOS, and the fix is to be explicit at our call site rather than rely on intuitive `None` semantics.

## What Changes

- `set_badge` passes `Some("")` (empty string) to `tray.set_title` for the zero-count case, so `setTitle:@""` is actually invoked and the menu-bar item collapses back to icon-only.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `tray-indicator`: Adds a requirement codifying the explicit-empty-string invariant for the zero-count case, so a future contributor can't accidentally reach for `set_title(None)` again.

## Impact

- `crates/specforge/src/tray.rs`: one-line change in the macOS branch of `set_badge`.
- Regression test in the same crate verifying that `set_badge` reaches the macOS branch with an empty-string title (not `None`) when count is zero. Implementation-level test — observing the actual menu-bar pixels would require a UI test harness we don't have.
- No new dependencies. No IPC changes. No frontend changes.

### Out of scope (flagged as follow-up changes)

Two related bugs were surfaced during exploration but are deliberately not addressed here:

1. `commands::unregister_workspace` calls `watcher.remove_workspace(p)` without emitting any `CacheEvent`. Removing the last workspace from the UI leaves the badge updater, glyph updater, and aggregator all uninformed; `last_views` stays stale. Even after this proposal lands, that code path will still leave the badge displaying the pre-removal count.
2. `repo_view::diff_views` only indexes `WorkspaceView::Repo`. Flat (non-git) workspaces have no `LogicalChangeArchived` safety net, so a race between the badge updater and the aggregator on `last_views` can leave a stale read undetected. Currently masked by the upstream `set_title(None)` bug; will surface once this proposal lands.

Both deserve their own proposals.
