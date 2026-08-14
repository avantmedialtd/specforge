## 1. Preflight

- [x] 1.1 Run `bun install` so `@types/bun` is present; confirm `bun run build` reaches `vite build` instead of failing at `tsc --noEmit` on the `bun:test` imports (a failed `tsc` short-circuits the `&&` and silently leaves `dist/` stale)
- [x] 1.2 Record the current desktop baseline. Captured as DOM measurements on the pre-change bundle under `body[data-platform="mac"]`: `.split-pane-left` / `.split-pane-far` `padding-top: 32px`; `.titlebar-drag-region` 32px tall with `pointer-events: auto`; `.pane-toggle` chevrons at `top: 36px`, 24×24. Mouse-drag confirmation in the native window is deferred to 7.4

## 2. Shell viewport sizing (`spec-browser`: *Master-Detail Layout*)

- [x] 2.1 In `src/App.css`, replace `height: 100vh` on `.app-shell` with the paired `height: 100%` then `height: 100dvh` declarations, per Decision 1 in `design.md`
- [x] 2.2 In `src/App.css`, apply the same paired declarations to `.split-pane`
- [x] 2.3 Confirm `html`, `body`, and `#root` retain `height: 100%` and `overflow: hidden` so the definite-height chain that `height: 100%` depends on stays intact

## 3. Platform flag gating (`visual-identity`: *macOS Hidden Inset Titlebar Layout*)

- [x] 3.1 In `src/main.tsx`, import `isTauri` from `./api` and change the guard to `isTauri() && /Mac/i.test(navigator.userAgent)`, keeping the assignment at module scope so `body[data-platform="mac"]` is still set before React mounts. Implemented via a new pure `usesMacTitlebarChrome(isTauriHost, userAgent)` in `src/platform.ts` so the two "Mac-like user-agent" scenarios in the delta are executable; covered by `src/platform.test.ts`
- [x] 3.2 Verify no other call site infers the platform from the user-agent (`grep -rn "userAgent" src/`); leave `src/App.tsx`'s existing `isTauri() && document.body.dataset.platform === "mac"` check as-is, since it is already correctly gated

## 4. Pointer-driven divider drag (`touch-input`: *Drag Interactions Accept Pointer Input*)

- [x] 4.1 In `src/components/SplitPane.tsx`, migrate `startLeftDrag` from `onMouseDown` + `document` mousemove/mouseup listeners to `onPointerDown` with `setPointerCapture`, handling `pointermove` / `pointerup` / `pointercancel` on the divider element
- [x] 4.2 Migrate `startFarDrag` the same way, preserving the inverted drag direction and the `onFarWidthChange` persistence call on gesture end (fire it on `pointercancel` too)
- [x] 4.3 Leave `maxLeftWidth`, `maxFarWidth`, `KEY_STEP`, and both `handle*KeyDown` handlers unchanged so pointer and keyboard resize through identical clamps
- [x] 4.4 Replace the `document.body.style.cursor` mutation with logic tied to the capture lifecycle so an interrupted gesture cannot strand the `col-resize` cursor — a `resizing` state flag drives a `.split-pane--resizing` class, cleared on both `pointerup` and `pointercancel`
- [x] 4.5 In `src/App.css`, add `touch-action: none` to `.split-pane-divider` so the browser does not claim the gesture as a pan

## 5. Touch discoverability (`touch-input`: *Essential Controls Are Discoverable Without Hover*)

- [x] 5.1 In `src/App.css`, add an `@media (hover: none)` block making `.row-favorite` visible at rest, leaving the existing `:hover` / `:focus` / `--active` rules untouched for hover-capable devices (`spec-browser`: *Change-Row Favorite Toggle*)
- [x] 5.2 In the same block, give `.pane-toggle` legible at-rest chrome — a `--surface` background and `--border` hairline matching the existing `.pane-restore-*` treatment — so the collapse chevrons read as controls without a hover highlight (`spec-browser`: *Side-Pane Visibility Toggles*)
- [x] 5.3 Confirm the reserved-slot geometry still holds: with `.row-favorite` visible at rest, no other row content shifts — verified by measurement, and structurally guaranteed since `opacity` never triggers reflow

## 6. Touch target sizing (`touch-input`: *Interactive Targets Meet a Minimum Size on Coarse Pointers*)

- [x] 6.1 In `src/App.css`, add an `@media (pointer: coarse)` block giving `.pane-toggle` a transparent centred `::after` overlay of at least 44×44px, without changing the button's rendered size
- [x] 6.2 Give `.split-pane-divider` a matching `::after` overlay, per Decision 5 — an overlay, not width or padding, so the 4px hairline and the `margin: 0 -2px` zero-width contribution are preserved and neither pane is displaced. 26px and **asymmetric**, growing into the detail pane rather than centred on the boundary (see 8.3 and the amendment to Decision 5)
- [x] 6.3 Give `.row-favorite` the same overlay treatment, checking it does not overlap the row's own click target in a way that would swallow row selection — bounded to the row height so it cannot overhang the rows above and below. Requests 44px wide; `.tree-row`'s `overflow: hidden` clips ~5px, for an effective ~39×28, above the 24×24 floor

## 8. Review fixes

Findings from `/code-review` on the implementation, all verified against a served build.

- [x] 8.1 Drag could outlive the gesture two ways — a `setPointerCapture` failure leaving the release unreachable, and a pane hidden mid-drag unmounting its divider with no `pointerup`/`pointercancel`. Both stranded a shell-wide `col-resize` cursor and `user-select: none`. Added a per-gesture window-level `pointerup`/`pointercancel` safety net, an `onLostPointerCapture` handler, a settle-on-hide effect, and an unmount cleanup; settling is idempotent
- [x] 8.2 `.split-pane--resizing *` is specificity (0,1,0) and sat *before* `.tree-row` / `.pane-toggle` / `.row-favorite`'s `cursor: pointer`, so those won and the cursor flipped back mid-drag. Moved the drag rules to the end of the sheet; verified `.tree-row` now computes `col-resize` during a drag
- [x] 8.3 The centred divider band painted above the collapse chevron (divider `z-index: 1` vs chevron `auto`), covered the favorite star, and blocked the sidebar tree's scroll strip under `touch-action: none`. Made it asymmetric into the detail pane via `--left` / `--far` modifier classes; `elementFromPoint` now resolves chevron, tree, divider and star each to themselves
- [x] 8.4 `.pane-toggle` at-rest chrome used `var(--surface)` — the exact background both side panes paint, so the collapse chevrons were invisible. The rule had been copied from `.pane-restore-*`, which works only because those float over the detail pane's `--bg`. Now `var(--surface-3)`
- [x] 8.5 Centred 44×44 toggle overlays were clipped by `overflow: hidden` ancestors and the viewport edge (~38px effective). Re-anchored to the corner each toggle already hugs, growing inward
- [x] 8.6 The divider is `background: transparent` at rest and revealed only on hover, so on touch the handle this change made draggable had no visual location — the same defect the `hover: none` block fixes for the other controls. Given the hairline ink at rest
- [x] 8.7 `user-select: none` was unprefixed only; Safari shipped it unprefixed in 17, and this change targets iPadOS 16+. Added `-webkit-user-select`
- [x] 8.8 `resizing` was a single boolean, so ending one divider's drag stripped the cursor and selection guard from a second simultaneous drag. Now derived from both slots
- [x] 8.9 `DragState.latest` was written only on the far path, so a future `onLeftWidthChange` added by symmetry would have persisted the width the drag *started* at. Now maintained by both move handlers
- [x] 8.10 `.split-pane` re-asserted `100dvh` inside `.app-shell`, which is already `100dvh` — a second independent viewport claim that would overflow unrecoverably if anything were ever added above it in the shell. Changed to `height: 100%`
- [x] 8.11 The `visual-identity` delta forbade inferring the platform from the user-agent, but the implementation still uses a UA regex for the macOS-vs-Windows/Linux split inside Tauri. The real defect was only ever "UA decides whether we are in a native window"; reworded the requirement to bind the *host* determination, leaving the OS distinction free once the native host is established

## 7. Verification

- [x] 7.1 `cargo test` — every suite reports 0 failures; this change touches no Rust, and the run confirms it
- [x] 7.2 `bun run build` — strict `tsc` then the bundle, both clean
- [x] 7.3 `bun test` — 196 pass / 0 fail across 11 files, including the 8 new `src/platform.test.ts` cases
- [ ] 7.4 Desktop smoke via `bun run wt:dev` — **outstanding, needs a human at the machine.** Done so far: the app compiles and launches from this worktree with no runtime error. Still to confirm inside the native window: that the macOS traffic lights still clear the first sidebar row, that the top drag strip still moves the window, and that both dividers still drag with the mouse. Driving a native window's mouse is outside the automation available here. Indirect coverage meanwhile: `usesMacTitlebarChrome(true, macUA) === true` is unit-tested, the `body[data-platform="mac"]` CSS rules are untouched by this change, and the mouse drag path is verified in-browser in 7.5–7.7 through code that never branches on `pointerType`
- [x] 7.5 Browser smoke on a desktop Mac browser against a `specforge-serve` build: UA reports Mac yet `data-platform` is absent; both side panes at `padding-top: 0`; drag region `pointer-events: none`; `elementFromPoint` at the top of the detail pane returns the dashboard, not the drag strip; `(hover: none)` and `(pointer: coarse)` both false so the touch rules are inert — star still `opacity: 0` at rest, chevrons still transparent
- [x] 7.6 Tablet-geometry smoke at 1194×744 and 834×1104 (visible areas): `.app-shell` bottom equals the viewport height in both, and Archive, Settings, the Claude quota strip and the ChatGPT quota strip are all fully visible and inside the viewport in both orientations. No `height: 100vh` remains in the served stylesheet (`spec-browser`: *Master-Detail Layout*)
- [x] 7.7 Touch smoke via synthesized `pointerType: 'touch'` events: dragging the sidebar divider resized it by exactly the pointer delta (+120px); clamped at the maximum (615px, leaving the detail pane its 320px minimum) and at the minimum (180px + 1px border); `pointercancel` ended the gesture and cleared `.split-pane--resizing` with no stranded cursor; the collapse chevron hid the sidebar and the restore chevron brought it back with no keyboard; lifting the two media blocks out of their conditions gave `.pane-toggle::after` 44×44, `.split-pane-divider::after` 24px, `.row-favorite::after` 44×28, star `opacity: 1`, chevrons with `--surface` + hairline (`touch-input`: all three requirements)
- [x] 7.8 `openspec validate add-web-ui-touch-support --strict` reports the change valid after the Decision 5 amendment and the bounded-target rewrite of the third `touch-input` requirement
