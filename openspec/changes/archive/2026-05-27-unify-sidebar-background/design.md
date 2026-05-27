## Context

The sidebar is the most visible chrome in the app and currently renders differently per platform. On macOS, `src/App.css` sets `body`, `.split-pane`, and `.split-pane-left` to `transparent`, and `crates/specforge/src/lib.rs` calls `apply_vibrancy(NSVisualEffectMaterial::Sidebar)` so the desktop wallpaper blurs through. On Windows and Linux, no vibrancy is applied and the sidebar paints `var(--surface)` directly. The macOS path requires the `window-vibrancy = "0.5"` crate (added inside a `[target.'cfg(target_os = "macos")'.dependencies]` block in `crates/specforge/Cargo.toml`), a try/log branch for vibrancy failures, and a `data-platform="mac"` body attribute that the CSS keys off.

The user-facing complaint is that the sidebar's color tracks the wallpaper on macOS but is a stable neutral on other platforms — screenshots, design tokens, and capability specs all describe an effect that may or may not be visible at any given moment. The capability spec `visual-identity` currently enshrines this split via the `macOS Sidebar Vibrancy and Hidden Inset Titlebar` requirement.

## Goals / Non-Goals

**Goals:**

- The sidebar renders the same background — `var(--surface)` — on macOS, Windows, and Linux, in both light and dark schemes.
- The `window-vibrancy` crate and the `apply_vibrancy` call site disappear from the workspace.
- The `body[data-platform="mac"]` background overrides disappear from `App.css`.
- The macOS hidden-inset titlebar with traffic lights overlaid at top-left of the sidebar continues to work, including the 32px safe-area padding and the IPC drag region.

**Non-Goals:**

- Changing the hidden-inset titlebar treatment on macOS or introducing a standard titlebar.
- Removing the 32px `--space-6` safe-area padding on `.split-pane-left`, the `titlebar-drag-region` element, or the `core:window:allow-start-dragging` capability.
- Introducing an in-app toggle to opt back into vibrancy.
- Re-tuning the `--surface` token value; the existing light (`#ffffff`) and dark (`#161a20`) values stand.
- Touching `Cargo.lock` directly — `cargo` regenerates it after the dep is removed.

## Decisions

**Decision 1: Remove `window-vibrancy` entirely rather than keep the dep behind a feature flag.**
The only call site is the unconditional `apply_vibrancy(...)` in `lib.rs`. There is no realistic future need that justifies keeping the dep around as dead weight. We can re-add it later if we ever want a vibrancy effect again. Removing the dep also removes the entire `[target.'cfg(target_os = "macos")'.dependencies]` block if `window-vibrancy` is its only entry, simplifying `Cargo.toml`.

**Decision 2: Unify on `var(--surface)`, not `var(--bg)`.**
Non-mac users already see `.split-pane-left { background: var(--surface) }`. Adopting the same value on macOS keeps the visual identity continuous with what every Windows/Linux screenshot in docs has shown to date. Switching to `var(--bg)` would change the sidebar/detail-pane separation on every platform, which is a larger design decision than this change is trying to make. Alternative considered: `var(--bg)`. Rejected — out of scope.

**Decision 3: Keep the `body[data-platform="mac"] .split-pane-left { padding-top: var(--space-6) }` rule.**
Vibrancy and traffic-light overlay are independent concerns. The hidden-inset titlebar is a Tauri window configuration that lives on regardless of vibrancy; the 32px padding exists so the first sidebar row isn't shadowed by the traffic lights. Removing only the `background: transparent` line from that rule keeps the padding intact.

**Decision 4: Replace the existing `macOS Sidebar Vibrancy and Hidden Inset Titlebar` requirement in `visual-identity` rather than split it into two changes.**
The titlebar and the vibrancy are described together in one requirement today. The spec delta REMOVEs that combined requirement (with reason + migration) and ADDs a focused `macOS Hidden Inset Titlebar Layout` requirement that covers the surviving behaviors (overlay traffic lights, 32px padding, drag region, ACL permission). This is cleaner than MODIFYing in place with a misleading heading.

**Decision 5: Leave `visual-identity`'s `Purpose` paragraph alone in this change.**
The Purpose still mentions "sidebar vibrancy + hidden inset titlebar". OpenSpec deltas operate on Requirements, not Purpose blocks, so Purpose drift is the normal cost of incremental changes. Cleaning it up belongs to the next change that touches `visual-identity` comprehensively, not here.

## Risks / Trade-offs

- **[Risk]** Users who specifically valued the vibrancy effect on macOS will perceive a regression. **Mitigation:** the trade-off is explicit in the proposal; the visual replacement (`var(--surface)`) is what every other platform already gets, so it is consistent rather than novel.
- **[Risk]** Removing the `[target.'cfg(target_os = "macos")'.dependencies]` block could break if another mac-only crate is added later and we forget the gating. **Mitigation:** trivial — `cargo` errors loudly if a missing platform-gated dep is referenced.
- **[Risk]** The `data-platform="mac"` attribute on `<body>` is now only consumed by the padding rule and the drag-region rule. It is not removed. **Mitigation:** none needed — the attribute remains useful for any future mac-specific styling and removing it is out of scope.
- **[Risk]** The `Purpose` paragraph in `visual-identity/spec.md` becomes stale (mentions vibrancy). **Mitigation:** documented as a known follow-up in proposal Impact. Reviewers checking the spec will see the requirements are authoritative.
