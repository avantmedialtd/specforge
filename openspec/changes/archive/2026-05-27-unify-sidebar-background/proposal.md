# Unify sidebar background across platforms

## Why

The sidebar's background is currently painted differently on macOS than on Windows/Linux: macOS gets `NSVisualEffectMaterial::Sidebar` vibrancy behind a transparent CSS surface, while Windows and Linux render a solid `var(--surface)` panel. The platform split costs a Rust dependency (`window-vibrancy`), three CSS overrides (`body`, `.split-pane`, `.split-pane-left`), a fault-tolerance branch in `lib.rs`, and a paragraph in CLAUDE.md — all for an effect that ties the sidebar's appearance to whatever wallpaper the user happens to have. We'd rather have a single, predictable sidebar background everywhere so screenshots, documentation, and the design tokens describe the actual rendered chrome.

## What Changes

- Drop the macOS `apply_vibrancy` call and the `window-vibrancy` crate dependency.
- Drop the three `body[data-platform="mac"]` background overrides in `src/App.css` so the sidebar paints `var(--surface)` on every platform.
- Keep the macOS hidden-inset titlebar, the 32px `--space-6` safe-area padding on `.split-pane-left`, the `titlebar-drag-region` element, and the `core:window:allow-start-dragging` capability — traffic lights still float over the top of the sidebar; only the vibrancy backdrop goes away.
- Trim the CLAUDE.md sections that describe sidebar vibrancy (the `lib.rs` bullet in "Rust workspace" and the comment block in "Window-lifecycle quirks" implicitly).
- **BREAKING** for users who specifically valued the wallpaper-blur look on macOS — there is no opt-in. The visual change is intentional and uniform.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `visual-identity`: the `macOS Sidebar Vibrancy and Hidden Inset Titlebar` requirement is split. The vibrancy contract is removed; the hidden-inset titlebar + safe-area padding + drag-region contract is preserved under a renamed, vibrancy-free requirement. The Windows/Linux scenario is removed because the sidebar background now renders uniformly via the existing `Design Token Layer` requirement.

## Impact

- `crates/specforge/Cargo.toml` — remove `window-vibrancy` dep (and its `[target.'cfg(target_os = "macos")'.dependencies]` block if it becomes empty).
- `crates/specforge/src/lib.rs` — remove the `apply_vibrancy` import and the macOS-gated call site.
- `src/App.css` — remove the `body[data-platform="mac"] { background: transparent }`, `body[data-platform="mac"] .split-pane { background: transparent }`, and the background line inside `body[data-platform="mac"] .split-pane-left` (keep the `padding-top: var(--space-6)`).
- `CLAUDE.md` — trim mentions of sidebar vibrancy under "Rust workspace > `lib.rs`" and the architectural notes.
- `openspec/specs/visual-identity/spec.md` — modified via this change's delta spec at archive time. The `Purpose` paragraph still references "sidebar vibrancy + hidden inset titlebar"; that stale phrasing is out of scope for this proposal and will be cleaned up the next time the spec is rewritten comprehensively.
- No frontend API change, no IPC surface change, no settings persistence change.
- Acceptance is visual: confirm the macOS dev build paints a solid `var(--surface)` sidebar (same as Windows/Linux) with traffic lights still floating over the top 32px.
