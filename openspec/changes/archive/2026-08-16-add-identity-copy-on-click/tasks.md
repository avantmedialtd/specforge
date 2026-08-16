## 1. Reproduce the bug before fixing it

- [x] 1.1 Establish that the bug is reproducible in the browser loop. `src/main.tsx` sets `body[data-platform="mac"]` only inside Tauri, and that attribute is the ONLY gate on `.titlebar-drag-region`'s `pointer-events` — so forcing it in the served UI makes the hit test identical to the native window. This is why the previous change could ship the bug: without forcing it, neither the bug nor a fix reproduces
- [x] 1.2 Capture the failing probe: `document.elementFromPoint` over the centre of `.identity-name` returns `titlebar-drag-region`, name rect `y = 9…26` entirely inside the strip's `0…32`, strip `pointer-events: auto` / `z-index: 5` against the header's `z-index: 2`
- [x] 1.3 Confirm the symptom's mechanism in `src/App.tsx`: `handleTitlebarMouseDown` calls `startDragging()` on a click and `toggleMaximize()` on `event.detail === 2` — the reported "click expands the window"

## 2. Audit the collision class (`design.md` Context)

- [x] 2.1 Inventory every element overlapping the top 32px on macOS, to establish whether the header is a one-off or an instance. Result: **systemic**. Broken/degraded and NOT fixed here: `.file-browser-header` and its filter input and Refresh button, `.commit-detail-breadcrumb`, both `.split-pane-divider`s, and — once scrolled — Dashboard rows, Settings form controls, and markdown links. Structural cause: `.split-pane-right` takes no macOS top inset in the default sidebar-visible layout
- [x] 2.2 Record the out-of-scope decision in `proposal.md` so the next reader inherits the map instead of rediscovering it

## 3. Choose the clearance (`design.md` Decision 1)

- [x] 3.1 Evaluate raising `.detail-identity` to `z-index: 6`. **Rejected**: `.pane-restore-left`/`.pane-restore-far` are already `z-index: 6` and render earlier in tree order inside `.split-pane-right`, so the opaque bar would occlude them on every platform; it also steals the drag band, the trade `.pane-restore-*` refuses in writing, and would force an amendment to `visual-identity`
- [x] 3.2 Evaluate raising only `.identity-name`. **Rejected — it is a no-op**: `.detail-identity` is `position: sticky`, which creates a stacking context unconditionally, trapping any child `z-index` so it never competes with the strip
- [x] 3.3 Evaluate padding `.split-pane-right` or margining the bar. **Rejected**: container padding scrolls away, so the band becomes a live window onto scrolled prose (violating the background-spans-the-pane clause) and is invisible to `offsetHeight`, so anchors under-offset by 32px; the margin variant collapses through `.detail-pane`
- [x] 3.4 Adopt padding on the sticky bar itself, keeping `top: 0` — the only option where the bar's own background covers the band, the band stays draggable, and the offset is inside the measured height

## 4. Implement the clearance

- [x] 4.1 Add `body[data-platform="mac"] .split-pane-right > .detail-pane > .detail-identity { padding-top: var(--space-6) }` with the reasoning recorded inline
- [x] 4.2 Use the DIRECT-child combinator deliberately. Measured: the artifact header is `.split-pane-right > .detail-pane > .detail-identity` at top 0; the Archive reader nests one level deeper (`> .archive-view--reading >`) at top 121; the file browser's sits inside its preview column. Only the first is occluded, and a descendant selector would inset all three
- [x] 4.3 Verify the fix: `elementFromPoint` over the name returns `identity-name`; header height 36 → 68; name rect `y = 41…58`
- [x] 4.4 Verify window dragging is preserved: `elementFromPoint` in the `0…32` band still returns `titlebar-drag-region`, so `visual-identity`'s *Window draggable from the titlebar strip on macOS* scenario is satisfied with no exception carved out
- [x] 4.5 Verify the clearance holds at EVERY scroll position, not just scroll top — the failure mode that sinks the container-padding variant. Measured at scrollTop 0, 200, 900 and 4639 (bottom): name pinned at `y = 41…58`, hittable at all four
- [x] 4.6 Verify the scroll-anchor math still clears the taller bar: measured `headerH = 68`, header bottom 68, anchored heading top 84 — exactly the intended 16px. Correct by construction because the padding is inside the measured element

## 5. Copy on click (`design.md` Decisions 3–6)

- [x] 5.1 Add `src/clipboard.ts`. Select the strategy from what the origin exposes BEFORE any await, and never chain the two paths: `document.execCommand` is only permitted inside the originating user gesture, and awaiting a rejected `writeText` ends it, so "try async, fall back on failure" would degrade to no copy precisely where the fallback is needed
- [x] 5.2 Factor the strategy choice into a pure `clipboardStrategy(nav)` so it is testable without a DOM — the failure it guards is invisible on loopback, which is what gets developed against
- [x] 5.3 Cover it: async when `writeText` is callable; selection when `navigator.clipboard` is undefined (the non-loopback bind), when `clipboard` exists without `writeText`, when `writeText` is not callable, and when there is no navigator
- [x] 5.4 Add `src/components/CopyableIdentity.tsx`, shared by all three surfaces. Select the contents explicitly rather than relying on `user-select: all`, because a keyboard activation produces no selection of its own
- [x] 5.5 Keep `user-select: all`: the click copies AND selects, so the highlight confirms what was copied and a refused write degrades to one keystroke rather than to nothing
- [x] 5.6 Use a span with `role="button"`, `tabIndex={0}`, an accessible name and Enter/Space activation rather than a `<button>` — WebKit sets `user-select: none` on form controls in its UA stylesheet and the desktop app is a WKWebView, so a real button would risk the selection behaviour on precisely the host that cannot be checked from here. Space is `preventDefault`ed so it cannot scroll the pane
- [x] 5.7 Confirm with ink and a live region only — `--ok` on success, `--warn` on refusal, plus a visually-hidden `aria-live="polite"` announcement, reverting after 1.4s. No label swap or added glyph, because the identity shares a flex row with the branch chip and may wrap
- [x] 5.8 Clear a pending confirmation when the value changes, so it cannot outlive the artifact it described; clear the timer on unmount
- [x] 5.9 Render it from `DetailPane` (change name) and `FileBrowserView` (file path); the Archive reader inherits it through `DetailPane`
- [x] 5.10 Add the interactive CSS: `cursor: pointer`, hover/focus ink lift, the house `:focus-visible` ring (`box-shadow: var(--shadow-focus)`), the two confirmation colours, and a `prefers-reduced-motion` opt-out for the transition

## 6. Verify the copy

- [x] 6.1 Verify a real (trusted) click: `writeText` called with exactly `consolidate-e2e-auth-steps`, the element focused, the selection equal to the value
- [x] 6.2 Verify the confirmation state actually appears, via a MutationObserver across the click: class transitions `identity-name--copied` → idle, with no `--failed`
- [x] 6.3 Verify no layout shift: the name's width is identical before and after a copy (172px both)
- [x] 6.4 Verify the branch chip is excluded from both the copied value and the selection
- [x] 6.5 Verify the refusal path is honest: a synthetic click (no user activation) is refused by the browser, and the component reports `--failed`, announces the fallback, and leaves the value selected — rather than showing a false success
- [x] 6.6 Confirm the confirmation colours resolve. NOTE: an initial reading suggested they did not; that was a measurement artifact — this Chrome tab runs with `document.visibilityState === "hidden"`, which throttles CSS transitions, so `getComputedStyle` returned the frozen start colour. With the transition suppressed the value is exactly `--warn` / `--ok`
- [x] 6.7 Establish that one click performs one copy. Two `writeText` calls were observed and traced to the instrumentation — `writeText` had been wrapped twice across probe runs, both wrappers recording. There is exactly ONE call site (`src/clipboard.ts:69`), the served bundle is a production build (StrictMode's double-invocation is dev-only), and StrictMode never double-invokes event handlers regardless. **Reasoned, not re-verified**: a clean single-wrapper re-run was blocked by the browser harness ceasing to deliver synthetic clicks

## 7. Specs

- [x] 7.1 MODIFY `spec-browser` → *Change Identity Header in the Detail Pane*: retract the ban on an application clipboard write and on a keyboard binding, state the copy contract, its confirmation, its no-reflow constraint, its keyboard activation, and the non-secure-origin path; change the three selection-asserting scenarios to assert clipboard contents; add the macOS clearance, required to hold at every scroll position, with the drag region left intact; require any clearance to be inside the measured header height
- [x] 7.2 MODIFY `archive-browser` → *Read-Only Artifact Navigation*: it restates the banned-clipboard-write clause verbatim rather than delegating, so updating `spec-browser` alone would leave it asserting the opposite
- [x] 7.3 MODIFY `workspace-file-browser` → *File Browser Surface*: same duplication, same change
- [x] 7.4 Confirm `visual-identity` needs NO modification: its *Window draggable from the titlebar strip on macOS* scenario requires a press anywhere in the top 32px to enter drag mode, and the chosen fix preserves that. Verified by hit test

## 8. Verification

- [x] 8.1 `bun run build` — type-check plus bundle, clean
- [x] 8.2 `bun test` — 218 pass, 0 fail, including the 5 new `clipboard` tests
- [x] 8.3 `cargo test` — expected unaffected (no crate touched); run to prove rather than assume
- [x] 8.4 `openspec validate` the change and the full spec set

## 9. Outstanding

- [x] 9.1 **Assert the fix in a running Tauri window.** Done, on this worktree's own dev slot (`bun run wt:dev`, slot 1 → port 1430) so the user's instance on 1420 was untouched. Screenshots need TCC this session lacks, so the assertions were reported over HTTP from a temporary `index.html` probe carrying a marker — a stale bundle has no probe and reports nothing, so the check cannot produce a false pass. Probe reverted; `git status` clean. Measured in the native window with the REAL platform flag (`plat: "mac"`, `stripPE: "auto"` — the strip genuinely live):
    - `elementFromPoint` over the change name → `identity-name`, **`FIXED: true`**
    - the `0…32` band → `titlebar-drag-region`, **`dragOK: true`** — window dragging preserved
    - header height 67, name top 41 — clears the strip
    - `-webkit-user-select` computes to **`all`** — WKWebView honours it on the span, which was Decision 5's stated risk and the reason a real `<button>` was avoided
    - `role="button"`, `tabIndex 0`, selection text exactly `add-identity-copy-on-click` (no branch)
    - **`navigator.clipboard.writeText` is a function and `isSecureContext` is true** in the Tauri WebView — previously inferred, now measured, so the desktop app takes the async path and `execCommand` is only the non-secure-origin fallback
- [ ] 9.2 **The remaining titlebar-strip collisions** catalogued in task 2.1 are unfixed and deserve their own change: `.split-pane-right` taking no macOS top inset is the structural cause, and Settings' form controls and markdown links are the sharpest symptoms
- [ ] 9.3 **Pre-existing: section/task anchors do not fire for an already-open artifact** (carried over from the previous change's task 10.2, reproduced on stock `src/` at HEAD)
