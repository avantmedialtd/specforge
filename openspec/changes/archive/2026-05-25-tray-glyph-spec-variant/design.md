# Design: Tray Glyph Variant for Active Spec Changes

## Context

The Tauri shell currently embeds a single tray glyph as a compile-time constant in `crates/specforge/src/tray_icon.rs`. The function `rasterize_glyph(scale)` always renders that constant at the active monitor's pixel density. Two places trigger rasterization: the initial `install_tray` call (lib.rs setup), and the `ScaleFactorChanged` window event (lib.rs:93), both of which feed the result back via `tray.set_icon(...)`.

The watcher (`crates/openspec-core/src/watcher.rs`) already broadcasts `CacheEvent::{Updated, ChangeAdded, ChangeArchived}` whenever the on-disk state of a registered workspace changes. The existing badge updater (`tray::spawn_badge_updater`) subscribes to this stream and refreshes the badge count on every event. The cache already contains, per change, an `ArtifactStatus.specs: Vec<String>` listing capability subdirectories that have a `spec.md` under that change. This is the exact signal needed for the variant predicate, and no additional file-system coverage is required to pick it up.

A second SVG asset (`tray-specs.svg`) has been authored alongside the existing `tray-icon.svg`. Both share viewBox, fill conventions, and template-safety properties (pure black + alpha after rasterization), so both will pass the existing `assert_template_safe` debug guard.

## Goals / Non-Goals

**Goals:**
- Show the `tray-specs.svg` glyph whenever any non-archived change in any registered workspace has a non-empty `ArtifactStatus.specs`.
- Show the default `tray-icon.svg` glyph in every other situation, including the "we don't know yet" boot window.
- Preserve the existing tray badge, click-to-focus behaviour, and scale-factor re-rasterization across variant changes.
- Keep the change reversible — the variant swap is a thin layer on top of the existing rasterization path, removable by deleting one task and one shared state slot.

**Non-Goals:**
- Differentiating proposal-only, design-only, or tasks-only changes — only spec activity is surfaced.
- Surfacing *which* workspace or *which* change is touching specs (the tray icon has no room for that detail).
- Adding new `CacheEvent` variants or expanding the watcher's coverage.
- Changing the badge, IPC surface, or any frontend behaviour.
- Animating or cross-fading between glyphs — the swap is instantaneous.

## Decisions

### 1. Predicate is a snapshot fold, not a stored counter

`any_change_touches_specs()` walks `WatcherManager::snapshot()` and returns `true` iff any `ChangeData` has `!artifacts.specs.is_empty()`.

- **Alternative considered:** maintain a running `spec_active_count` on `WorkspaceCache`, decremented/incremented at insert time, with the variant derived from `count > 0`.
- **Rationale:** the snapshot fold is O(total active changes) and runs only on debounced `CacheEvent`s (200ms minimum cadence). Realistic working-set sizes (tens of changes per workspace, single-digit workspaces) make the cost negligible. A stored counter would couple cache mutation to a derived quantity for no measurable benefit and would have to be kept correct across every cache code path.

### 2. Variant lives in a dedicated `TrayGlyph` enum

Add `enum TrayGlyph { Default, Specs }` to `tray_icon.rs`. `rasterize_glyph` becomes `rasterize_glyph(variant: TrayGlyph, scale: f64)`. The const `SVG` splits into `SVG_DEFAULT` and `SVG_SPECS`.

- **Alternative considered:** a `bool` flag.
- **Rationale:** the enum is self-documenting at call sites, extends cleanly if a third variant emerges (e.g. "design-only activity"), and pairs naturally with shared state stored as `Arc<AtomicU8>` whose discriminant maps 1:1 to the enum.

### 3. Shared state is `Arc<AtomicU8>` managed on `AppHandle`

A new `TrayGlyphState(Arc<AtomicU8>)` newtype is created in `setup()`, stored via `app.manage()`, and read by both the glyph updater (writer) and the `ScaleFactorChanged` handler (reader-only).

- **Alternative considered:** `Arc<Mutex<TrayGlyph>>`, or a `tokio::sync::watch::channel` from updater to scale handler.
- **Rationale:** the scale-change handler is on a window-event callback that should never block; `AtomicU8::load(Relaxed)` is lock-free and adequate (the variant is single-writer, multi-reader, with no compound invariants). A `watch` channel works too but is heavier than warranted for a 1-bit state.

### 4. Glyph updater is a sibling task to `spawn_badge_updater`

`spawn_tray_glyph_updater(tray, watcher, glyph_state)` lives in `tray.rs`, mirrors `spawn_badge_updater`'s structure (initial set → subscribe loop → recompute on every `CacheEvent`), and is spawned from `lib.rs` right after `spawn_badge_updater`.

- **Alternative considered:** extend `spawn_badge_updater` to also flip the glyph.
- **Rationale:** badge and glyph are orthogonal — badge is a count, glyph is a categorical signal. Keeping them separate makes either one easy to remove or replace, mirrors the existing notification-dispatcher pattern, and avoids tangling the tasks' error and lag-recovery paths.

### 5. Initial set in updater, before subscribing

The updater computes the initial variant from the cache *before* calling `subscribe()`. This is the same pattern `spawn_badge_updater` uses for the initial badge count.

- **Rationale:** at app launch the cache is populated synchronously in `lib.rs:47` before the tray is installed, so a "first paint" with the wrong variant is avoidable. Users who relaunch with spec-touching changes already in flight see the `Specs` glyph from the first frame.

### 6. Pure-black + alpha invariant carries over

`tray-specs.svg` is structurally identical to `tray-icon.svg`: same `viewBox`, `fill="none"` on background, `fill="currentColor"` on the foreground path. The existing `assert_template_safe` debug check therefore covers both. The existing `rasterizes_at_multiple_scales` test will get a sibling that exercises `SVG_SPECS`.

## Risks / Trade-offs

- **Wrong variant after scale change** → The `ScaleFactorChanged` handler must read the shared variant state, not assume `Default`. Mitigation: a unit-ish integration test asserts the right variant is re-rasterized after a simulated scale change while in `Specs` state. Without this test the bug is silent (the icon still appears, just briefly as `Default` until the next `CacheEvent`).
- **Flicker on bulk spec edits** → The existing 200ms debouncer smooths file-level activity. Beyond that, `set_icon` is itself fast; the only flicker risk is a user editing then deleting a spec file inside one debounce window, which would no-op anyway.
- **Variant state managed after window-event handler registers** → `setup()` must `app.manage(TrayGlyphState::new())` *before* `main_window.on_window_event(...)` is wired. Otherwise the scale handler's first invocation would race the state and fall back to default. Mitigation: explicit ordering in `lib.rs`, called out by a comment.
- **Stale variant on transient parse error** → When `parse_all_changes` returns `Err`, the cache entry is left untouched (mirrors badge behaviour). The variant therefore reflects the *last known good* state, not "unknown". This is a deliberate trade-off: a flickering glyph on transient I/O errors would be worse than briefly stale state.

## Open Questions

None — interpretation (any active change with spec deltas in any workspace) and fail-safe direction (default on unknown) are settled.
