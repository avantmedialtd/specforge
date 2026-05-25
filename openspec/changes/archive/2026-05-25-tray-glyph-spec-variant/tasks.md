## 1. Assets

- [x] 1.1 Confirm `crates/specforge/icons/tray-specs.svg` is present at the expected path; if only sitting untracked in the parent checkout, copy it into the worktree
- [x] 1.2 If `crates/specforge/icons/tray-icon.svg` has been updated in the parent checkout, bring that update into the worktree as well
- [x] 1.3 Verify both SVGs use `fill="currentColor"` on the visible path (required for the pure-black + alpha invariant)

## 2. Predicate on `WatcherManager`

- [x] 2.1 Add `WatcherManager::any_change_touches_specs(&self) -> bool` returning `self.snapshot().values().flatten().any(|c| !c.artifacts.specs.is_empty())`
- [x] 2.2 Add a unit test in `crates/openspec-core/tests/` (e.g. `watcher.rs` or a new `glyph_predicate.rs`) covering: empty cache → false, one workspace with no spec deltas → false, one workspace with one spec delta → true, two workspaces with the delta in the second → true

## 3. Rasterization layer

- [x] 3.1 In `crates/specforge/src/tray_icon.rs`, replace the single `pub const SVG` with `pub const SVG_DEFAULT` (existing `tray-icon.svg`) and `pub const SVG_SPECS` (new `tray-specs.svg`)
- [x] 3.2 Add `#[derive(Copy, Clone, Debug, PartialEq, Eq)] pub enum TrayGlyph { Default, Specs }` with discriminant values `0` and `1` (explicit `#[repr(u8)]`) so an `AtomicU8` can losslessly round-trip the value
- [x] 3.3 Add `TrayGlyph::svg(self) -> &'static [u8]` returning the matching const
- [x] 3.4 Change `rasterize_glyph(scale)` to `rasterize_glyph(variant: TrayGlyph, scale: f64)`; update the body to call `rasterize(variant.svg(), LOGICAL_SIZE, scale)`
- [x] 3.5 Extend the existing `rasterizes_at_multiple_scales` test to iterate both variants, asserting the pure-black + alpha invariant holds for `SVG_SPECS` too (the debug assertion in `rasterize` already enforces this — the test just needs to exercise both paths)

## 4. Shared variant state

- [x] 4.1 Add `pub struct TrayGlyphState(Arc<AtomicU8>)` (and `From<TrayGlyph>` / `TryFrom<u8>` helpers) co-located with `TrayGlyph` in `tray_icon.rs`
- [x] 4.2 Provide `TrayGlyphState::load() -> TrayGlyph` (using `Ordering::Relaxed`) and `TrayGlyphState::store(TrayGlyph)`
- [x] 4.3 Provide `TrayGlyphState::new(initial: TrayGlyph)` so callers can seed it from the cache rather than always defaulting to `Default`

## 5. Glyph updater task

- [x] 5.1 In `crates/specforge/src/tray.rs`, add `pub fn spawn_tray_glyph_updater(tray, app, watcher, state, initial_scale)` mirroring `spawn_badge_updater`'s structure. (Signature adapted from the sketch: an `AppHandle` parameter was added so the updater can query the main window's current scale at re-rasterize time. Without this, a scale change followed by a variant change would rasterize at the launch scale, not the current scale.)
- [x] 5.2 Inside the spawn: compute the initial variant from `watcher.any_change_touches_specs()`, write it to `state`, rasterize, and call `tray.set_icon(...)` once before subscribing
- [x] 5.3 Subscribe to `watcher.subscribe()`; on each event recompute the variant, and if it differs from the last stored variant, write the new value and `set_icon` the freshly-rasterized glyph
- [x] 5.4 Handle `RecvError::Lagged` by `continue`ing (no spurious flips); handle `RecvError::Closed` by returning, matching the badge updater

## 6. Wiring in `lib.rs`

- [x] 6.1 In `setup()`, construct `let glyph_state = TrayGlyphState::new(initial_variant_from_watcher)` and call `app.manage(glyph_state.clone())`. (Reordered relative to the sketch: this now happens *before* `install_tray` so the initial variant can be threaded into the very first rasterization, and *before* the window event handler is registered so the ordering invariant holds.)
- [x] 6.2 Pass the cloned `glyph_state` and `monitor_scale` into `spawn_tray_glyph_updater(...)`
- [x] 6.3 In `install_tray`, accept the initial variant so the *very first* `rasterize_glyph` call uses the correct SVG (avoids a one-frame flash); thread it from `setup()` through `install_tray`'s signature
- [x] 6.4 Update the `ScaleFactorChanged` handler in `lib.rs:93` to read the current variant from the managed `TrayGlyphState` via `app.state::<TrayGlyphState>()`, and pass it to `rasterize_glyph(variant, *scale_factor)`
- [x] 6.5 Comment-document the management ordering invariant: `TrayGlyphState` must be `manage()`-d before the window-event handler is registered

## 7. Verification

- [x] 7.1 `cargo test -p openspec-core` passes, including the new predicate test
- [x] 7.2 `cargo test -p specforge` passes (the rasterize test now exercises both variants)
- [x] 7.3 `bun run build` passes (no frontend regression)
- [ ] 7.4 Manual smoke test: launch with `bun tauri dev`, register a workspace with no spec deltas → confirm default glyph; create a change directory with `openspec/changes/<id>/specs/<cap>/spec.md` → confirm flip within the debounce window; archive that change → confirm revert
- [ ] 7.5 Manual smoke test: with the spec-activity glyph showing, drag the main window between two displays with different scale factors → confirm the glyph remains the spec-activity variant after the scale change (no flash back to default)
