## Context

`set_badge` in `crates/specforge/src/tray.rs` is the single funnel through which every badge update reaches the OS tray. Its macOS branch builds an `Option<String>` from `count.map(|n| n.to_string())` and forwards it to `tray.set_title(title.as_deref())`.

`tray.set_title` is a wrapper around `tray-icon`'s platform-specific implementation. On macOS, `tray-icon 0.23.1` defines `set_title_inner` as:

```rust
fn set_title_inner<S: AsRef<str>>(
    ns_status_item: &NSStatusItem,
    title: Option<S>,
    mtm: MainThreadMarker,
) {
    if let Some(title) = title {
        unsafe {
            if let Some(button) = ns_status_item.button(mtm) {
                button.setTitle(&NSString::from_str(title.as_ref()));
            }
        }
    }
}
```

The `if let Some(title) = title` early-returns on `None`, never calling `setTitle:`. Whatever NSString was attached previously remains attached.

So when `set_badge(Some(0))` runs, our internal filter (`count.filter(|&n| n > 0)`) collapses the count to `None`, the macOS branch hands `None` to `set_title`, and upstream silently ignores it. The user sees the previous count text continue to display.

The bug is latent in the same way the template-flag bug was: it only fires when the title goes from non-empty to "empty", which is exactly the moment we care most about visually.

## Goals / Non-Goals

**Goals:**

- Restore the implicit contract of `set_badge(Some(0))`: the menu-bar status item collapses back to icon-only width with no count text visible.
- Codify the workaround in the `tray-indicator` spec so a future contributor can't reach for `set_title(None)` and silently reintroduce the regression.
- Add a regression test that does not depend on rendering, only on observable arguments passed into the tray-icon layer.

**Non-Goals:**

- Patching `tray-icon` upstream. The maintainers may have intentional reasons for the `None` early-return (e.g. distinguishing "don't touch" from "clear"). Workaround at our call site is cheaper and reversible.
- Fixing `commands::unregister_workspace`'s silent removal path. That's a real bug with the same symptom, but a different code path; it warrants its own proposal.
- Fixing the `WorkspaceView::Flat` race in `diff_views`. Currently masked by the upstream `set_title(None)` bug; once this change lands the race becomes observable and can be addressed in a follow-up.
- Changing the badge tooltip behaviour. Tooltip never goes through the buggy code path because `set_tooltip(Some("SpecForge"))` always carries a non-`None` value.

## Decisions

### Decision 1: pass `Some("")` rather than `Some(" ")` or any other sentinel

`setTitle:@""` is the documented Cocoa way to clear an `NSStatusBarButton`'s title text. The button auto-collapses to its image width. A single-space (`" "`) would render as a one-character gap to the right of the icon — visually wrong, and dependent on the menu-bar font's whitespace advance.

Alternatives considered:

- **`tray.set_title(Some(""))`** — chosen. Round-trips to `setTitle:@""`, which is the standard clear-the-title call. Documented, predictable, future-proof.
- **`tray.set_title(Some(" "))`** — rejected. Leaves a visible gap.
- **Remove and re-create the tray icon** — rejected. Massive overkill, would also drop the menu, the icon, and any callbacks. Visible flicker.
- **Set the icon image to a wider transparent version** — rejected. The title text and the icon are two separate things; the title sits to the right of the icon, not on top of it.

### Decision 2: keep the `count.filter(|&n| n > 0)` short-circuit

The filter exists so `set_title` receives `None` when the count is zero or absent, even though the count was nominally `Some(0)`. After the fix, the same filter still routes `Some(0)` and `None` to the same "empty title" code path — just via `Some("")` instead of `None`. The filter is still useful because it keeps the tooltip text in sync (`tooltip = Some("SpecForge")` for the zero/absent case).

### Decision 3: test at the `set_badge` boundary, not at the menu-bar pixel level

Mocking the OS menu bar in a test is infeasible. Instead, the regression test injects an instrumented stand-in for `TrayIcon` that records `set_title` calls, then asserts that calling `set_badge(_, Some(0))` results in a `set_title(Some(""))` call rather than `set_title(None)`. This pins the workaround down to the exact API call that upstream cares about.

Implementation note: `TrayIcon` does not implement a trait we can dependency-inject against. Options:

- Refactor `set_badge` to accept a trait-bounded sink and run the test against that. Smallest surface change, recommended.
- Bypass `TrayIcon` entirely and test the title-string-from-count logic as a private helper. Acceptable fallback if trait extraction proves invasive.

Either approach is fine for the spec; the tasks artifact picks one.

## Risks / Trade-offs

- **Upstream may eventually fix `set_title(None)` to call `setTitle:nil`.** → If that happens, `Some("")` continues to work (it explicitly clears). The workaround is forward-compatible. No need to revert when upstream lands a fix.
- **Empty `NSString` vs `nil` — small semantic difference.** → `setTitle:@""` collapses the button to icon-only width. `setTitle:nil` does the same on modern macOS. Either is acceptable; only the former is reachable from `tray-icon 0.23.1`.
- **The fix only addresses the symptom on macOS.** → That's correct. The `cfg(target_os = "macos")` block is the only one affected; non-macOS platforms set tooltip text only and have no equivalent bug.
- **Test relies on the implementation detail "passes empty string to set_title".** → Acceptable trade-off because the alternative (assert on rendered pixels) is not implementable and because the spec explicitly codifies this implementation detail as a workaround.
