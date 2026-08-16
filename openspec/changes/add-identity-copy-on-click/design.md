## Context

Two problems, reported together and fixed together because the second makes the first unobservable.

**The header is occluded in the desktop app.** `.titlebar-drag-region` is a full-width, 32px, `z-index: 5` strip with `pointer-events: auto` under `body[data-platform="mac"]`. The identity header is `sticky; top: 0; z-index: 2`, 36px tall — so the change name lies entirely inside the strip. `handleTitlebarMouseDown` turns a click into `startDragging()` and a double-click into `toggleMaximize()`, which is precisely the reported symptom.

**The copy mechanism was designed under sidebar constraints that no longer apply.** `add-change-identity-headers` chose `user-select: all` and no clipboard write, because in the tree a copy control would have needed a keyboard chord (nested row controls are never focusable) and `Cmd+C` was taken. In the detail pane the identity can simply be a focusable control.

The occlusion is also why the first change shipped the bug: `src/main.tsx` sets `data-platform="mac"` only inside Tauri, so under `specforge-serve` — the project's preferred verification loop — the strip is inert and neither the bug nor a fix reproduces.

## Goals / Non-Goals

**Goals**

- One click on the identity puts exactly that value on the clipboard, in all three hosts.
- The identity is reachable and operable in the macOS desktop window.
- Window dragging by the titlebar strip is preserved unchanged.
- Confirmation that never reflows the row.
- The bug and the fix are both reproducible in the browser loop, not only in the native window.

**Non-Goals**

- Fixing the other elements the drag strip occludes. Real, catalogued in the proposal, systemically caused, and much larger.
- A general toast/notification system.
- Any change to the drag region, the sidebar, or the tree's keyboard model.

## Decisions

### Decision 1: Pad the sticky bar, don't raise it and don't pad the container

Four placements were evaluated adversarially. Three fail:

- **Raise `.detail-identity` to `z-index: 6`.** Works as a hit test — the stacking chain is clean and 6 beats 5 in the root stacking context. But it occludes `.pane-restore-left` / `.pane-restore-far`, which are *already* `z-index: 6` and rendered earlier in tree order inside `.split-pane-right`, so the opaque bar wins and the restore buttons vanish — on every platform, since the bar's z-index is not platform-gated. It also steals the drag band across the pane's full width, which is the exact trade `.pane-restore-*` refuses in writing ("a button under the strip would fight it for clicks"), and it would carve an exception out of `visual-identity`'s *Window draggable from the titlebar strip on macOS* scenario.
- **Raise only `.identity-name`.** A no-op. `.detail-identity` is `position: sticky`, which creates a stacking context unconditionally, so a child's `z-index` is trapped inside it and never competes with the strip. Removing the parent's `z-index` does not help — sticky creates the context on its own.
- **Pad `.split-pane-right` (or margin the bar).** A scroll container's `padding-top` is part of the scrollable area and scrolls away, so with the bar clamped at `top: 32px` the band above it becomes a live window onto scrolled prose — violating the requirement that the header's background span the pane so content cannot show through. The padding is also invisible to `offsetHeight`, so the anchor math under-offsets by 32px and every anchored section lands under the bar. The `margin-top` variant is worse: `.detail-pane` has no padding, border or overflow, so the margin collapses through it.

**Padding on the bar itself, keeping `top: 0`, has none of those failures — by construction, not by luck:**

- the bar's own opaque background now spans the strip band, so nothing can show above it at any scroll position;
- the band remains part of the drag region, so window dragging is untouched;
- the padding is *inside* the element, so `offsetHeight` grows from 36 to 68 and the scroll-anchor math absorbs it with no second value to maintain — the "measure, don't hard-code" decision from the previous change paying off unprompted.

Verified in the browser at scroll positions 0, 200, 900 and 4639 (the bottom): the name stays at y=41..58, `elementFromPoint` returns `.identity-name` at every one, and the 0–32 band still returns `.titlebar-drag-region`. Anchor clearance re-measured with the inset applied: header bottom 68, anchored heading top 84, exactly the intended 16px.

### Decision 2: Scope the clearance to the direct child of `.split-pane-right`

`.detail-identity` renders in three places and only one is flush with the window top. Measured:

| Surface | Chain | Identity top |
|---|---|---|
| Artifact | `.split-pane-right > .detail-pane > .detail-identity` | 0 — occluded |
| Archive reader | `… > .archive-view--reading > .detail-pane > …` | 121 — clear |
| File browser | inside `.file-browser-preview-col` | clear |

So the selector is `body[data-platform="mac"] .split-pane-right > .detail-pane > .detail-identity`. The direct-child combinator is load-bearing: the Archive reader nests the same wrapper one level deeper, and a descendant selector would give both a 32px inset that only one needs.

(Note `.detail-pane` is not a new class — `.archive-view--reading .detail-pane` has existed since the archive browser was implemented, matching nothing until the previous change introduced a wrapper with that name. The nesting above is why the archive reader inherits the identity header at all.)

### Decision 3: Choose the clipboard path up front; never chain the two

`document.execCommand("copy")` is only permitted while the browser still considers itself inside the triggering user gesture, and an `await` on a rejected `writeText` **ends that gesture**. So the intuitive "try the modern API, fall back on failure" degrades to no copy at all precisely where the fallback was meant to help. The strategy is therefore selected from what the origin exposes, before any await, and each path runs alone:

| Host | `navigator.clipboard` | Path |
|---|---|---|
| Tauri WKWebView | exposed | async |
| Browser on loopback | exposed (localhost is a secure context) | async |
| Browser on non-loopback `--bind`, plain HTTP | **`undefined`** (not a secure context) | selection + `execCommand` |

The strategy choice is factored out as a pure function so it is testable without a DOM — the failure it guards against is invisible on loopback, which is what gets developed against.

### Decision 4: Keep `user-select: all` alongside the copy

They are complementary rather than redundant. The click copies *and* selects: the highlight is instant, reflow-free confirmation of exactly what reached the clipboard, and if the write is refused the value is already selected for the platform's own shortcut, so a refusal costs one keystroke instead of everything. The selection is also what the `execCommand` path copies.

The component selects the contents explicitly rather than relying on `user-select: all` to have done it, because a keyboard activation produces no selection of its own.

### Decision 5: A span with button semantics, not a `<button>`

WebKit sets `user-select: none` on form controls in its UA stylesheet, and the desktop app runs in a WKWebView — so a real `<button>` would put the selection behaviour at risk on exactly the host that cannot be checked from this environment. The span carries the semantics explicitly instead: `role="button"`, `tabIndex={0}`, an accessible name, Enter/Space activation (with Space `preventDefault`ed so it cannot scroll the pane on the way), and the house `:focus-visible` ring.

This is only affordable because the identity is in the detail pane. In the tree it would have had to obey the roving-focus, single-Tab-stop model — the constraint that pushed the previous change away from a copy control in the first place.

### Decision 6: Confirm with ink and a live region, never with layout

The identity is a flex item that may wrap, sharing a row with the branch chip, so a swapped label or an added glyph reflows the row on every copy. Confirmation is a colour transition — `--ok` on success, `--warn` on refusal — plus a visually-hidden `aria-live="polite"` announcement, and it reverts after 1.4s. Verified: the element's width is identical before and after a copy. A pending confirmation is cleared when the pane moves to another artifact, so it can never outlive the value it described.

## Risks / Trade-offs

- **The macOS clearance is verified by simulation, not in the native window.** The browser reproduces the hit test faithfully — `data-platform="mac"` is the *only* gate on the strip's `pointer-events`, so forcing it in the served UI makes the occlusion identical — but the native window was not driven from this environment. What simulation cannot cover is Tauri's own `data-tauri-drag-region` delegation and WKWebView's `user-select` handling. Called out as an outstanding task rather than claimed.
- **32px of vertical space in the desktop app.** The bar goes 36px → 68px on macOS only. It buys the only clickable position available without taking the drag band.
- **`execCommand` is deprecated.** It is the fallback, not the primary, and only on origins that expose no alternative. If it is ever removed, that origin degrades to select-and-copy-yourself — which is exactly today's behaviour, so the floor never drops below what shipped.
- **`role="button"` on selectable text.** Assistive tech announces a button whose contents are also selectable. That matches what it does; the alternative — a separate icon button — adds a control to a bar deliberately kept to one line.
- **The audit found a systemic problem this change does not fix.** Leaving `.file-browser-filter`, `.commit-detail-breadcrumb`, the dividers, and scrolled Settings controls occluded is a deliberate scope decision, recorded in the proposal so the next reader finds the map rather than rediscovering it.
