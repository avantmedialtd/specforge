## 1. Preflight

- [ ] 1.1 Run `bun install` so `@types/bun` is present; confirm `bun run build` reaches `vite build` instead of failing at `tsc --noEmit` on the `bun:test` imports (a failed `tsc` short-circuits the `&&` and silently leaves `dist/` stale)
- [ ] 1.2 Record the current desktop baseline: launch `bun run wt:dev`, drag both dividers with the mouse, confirm the macOS traffic-light safe area and top drag strip behave as they do on `master`

## 2. Shell viewport sizing (`spec-browser`: *Master-Detail Layout*)

- [ ] 2.1 In `src/App.css`, replace `height: 100vh` on `.app-shell` with the paired `height: 100%` then `height: 100dvh` declarations, per Decision 1 in `design.md`
- [ ] 2.2 In `src/App.css`, apply the same paired declarations to `.split-pane`
- [ ] 2.3 Confirm `html`, `body`, and `#root` retain `height: 100%` and `overflow: hidden` so the definite-height chain that `height: 100%` depends on stays intact

## 3. Platform flag gating (`visual-identity`: *macOS Hidden Inset Titlebar Layout*)

- [ ] 3.1 In `src/main.tsx`, import `isTauri` from `./api` and change the guard to `isTauri() && /Mac/i.test(navigator.userAgent)`, keeping the assignment at module scope so `body[data-platform="mac"]` is still set before React mounts
- [ ] 3.2 Verify no other call site infers the platform from the user-agent (`grep -rn "userAgent" src/`); leave `src/App.tsx`'s existing `isTauri() && document.body.dataset.platform === "mac"` check as-is, since it is already correctly gated

## 4. Pointer-driven divider drag (`touch-input`: *Drag Interactions Accept Pointer Input*)

- [ ] 4.1 In `src/components/SplitPane.tsx`, migrate `startLeftDrag` from `onMouseDown` + `document` mousemove/mouseup listeners to `onPointerDown` with `setPointerCapture`, handling `pointermove` / `pointerup` / `pointercancel` on the divider element
- [ ] 4.2 Migrate `startFarDrag` the same way, preserving the inverted drag direction and the `onFarWidthChange` persistence call on gesture end (fire it on `pointercancel` too)
- [ ] 4.3 Leave `maxLeftWidth`, `maxFarWidth`, `KEY_STEP`, and both `handle*KeyDown` handlers unchanged so pointer and keyboard resize through identical clamps
- [ ] 4.4 Replace the `document.body.style.cursor` mutation with logic tied to the capture lifecycle so an interrupted gesture cannot strand the `col-resize` cursor
- [ ] 4.5 In `src/App.css`, add `touch-action: none` to `.split-pane-divider` so the browser does not claim the gesture as a pan

## 5. Touch discoverability (`touch-input`: *Essential Controls Are Discoverable Without Hover*)

- [ ] 5.1 In `src/App.css`, add an `@media (hover: none)` block making `.row-favorite` visible at rest, leaving the existing `:hover` / `:focus` / `--active` rules untouched for hover-capable devices (`spec-browser`: *Change-Row Favorite Toggle*)
- [ ] 5.2 In the same block, give `.pane-toggle` legible at-rest chrome — a `--surface` background and `--border` hairline matching the existing `.pane-restore-*` treatment — so the collapse chevrons read as controls without a hover highlight (`spec-browser`: *Side-Pane Visibility Toggles*)
- [ ] 5.3 Confirm the reserved-slot geometry still holds: with `.row-favorite` visible at rest, no other row content shifts (the slot is already reserved, so this should need no change — verify rather than assume)

## 6. Touch target sizing (`touch-input`: *Interactive Targets Meet a Minimum Size on Coarse Pointers*)

- [ ] 6.1 In `src/App.css`, add an `@media (pointer: coarse)` block giving `.pane-toggle` a transparent centred `::after` overlay of at least 44×44px, without changing the button's rendered size
- [ ] 6.2 Give `.split-pane-divider` a matching `::after` overlay at least 44px across, per Decision 5 — an overlay, not width or padding, so the 4px hairline and the `margin: 0 -2px` zero-width contribution are preserved and neither pane is displaced
- [ ] 6.3 Give `.row-favorite` the same overlay treatment, checking it does not overlap the row's own click target in a way that would swallow row selection

## 7. Verification

- [ ] 7.1 `cargo test` — expected to be unaffected; this change touches no Rust, and the run confirms it
- [ ] 7.2 `bun run build` — strict `tsc` (including `noUnusedLocals` / `noUnusedParameters`, which the removed mouse handlers could trip) then the bundle
- [ ] 7.3 `bun test` — the frontend unit suites still pass
- [ ] 7.4 Desktop smoke via `bun run wt:dev`: drag both dividers with the mouse and resize both with the keyboard; confirm the macOS traffic-light safe area and the top drag strip are unchanged against the 1.2 baseline (`visual-identity`: *macOS Hidden Inset Titlebar Layout*)
- [ ] 7.5 Browser smoke on a desktop Mac browser against a `specforge-serve` build: confirm no traffic-light padding, no drag region, that the full top edge of the detail pane accepts clicks, and that the favorite star and pane chevrons still hide at rest (hover-capable device — the touch rules must be inert)
- [ ] 7.6 Tablet-geometry smoke at 1194×834 and 834×1194: confirm Archive, Settings, and both quota strips are fully visible and activatable in each orientation (`spec-browser`: *Master-Detail Layout* — *Sidebar footer entrypoints stay reachable on a short viewport*)
- [ ] 7.7 Touch smoke with a coarse-pointer emulation or a real tablet: drag both dividers by touch through their clamps, hide and restore both panes using only the chevrons, and toggle a favorite — no keyboard, no hover (`touch-input`: all three requirements)
- [ ] 7.8 Run `openspec verify add-web-ui-touch-support` (or `/opsx:verify`) and confirm every delta scenario is satisfied before archiving
