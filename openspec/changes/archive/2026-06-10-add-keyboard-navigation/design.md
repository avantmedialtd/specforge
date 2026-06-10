# Design: Make the spec browser keyboard-operable

## Context

The workspace tree renders recursively (`RepoNode`/`FlatWorkspaceNode` → … → `TaskNode`) into *nested* DOM: each parent row's children live in a wrapper `<div>` beneath it (e.g. `WorkspaceTree.tsx:563`), and every row is the shared `Row` primitive — a `<div onClick>` with a `stopPropagation` chevron `<span>` (`WorkspaceTree.tsx:285-304`). Three assets make keyboard support cheap:

- **Stable hierarchical node IDs** (`repo:X/lc:Y/inst:Z/change:C/artifact:K/…`, `WorkspaceTree.tsx:39-70`) that already persist in the collapse/expand override sets.
- **One disclosure mutation point**, `toggle(id, defaultOpen)` (`WorkspaceTree.tsx:164-172`), with persistence handled by existing debounced effects.
- **A finished focus visual**: `.tree-row:focus-visible` (App.css:517) implements the visual-identity spec's `--shadow-focus` recipe and is currently dead CSS.

The periphery is mostly healthy — graph-rail rows, archive rows/tabs, heatmap cells, ships rows, and palette swatches are already native `<button>`s. The gaps are the tree, the split-pane dividers (mouse-drag `<div role="separator">`), Escape handling, and focus-visible coverage on a handful of controls.

## Goals / Non-Goals

**Goals:**

- Full WAI-ARIA tree keyboard pattern on the workspace tree (roving tabindex, arrows, Home/End, Enter/Space, typeahead), driving the existing `onSelect`/`handleSelect` contract unchanged.
- Debounced follow-focus (~150 ms settle) for content rows; disclosure rows never touch the detail pane.
- Keyboard-resizable dividers, Escape dismissal of Settings/Archive, focus-visible sweep, swatch radio semantics + hit areas.
- No per-keystroke whole-tree re-renders.

**Non-Goals:**

- App-level accelerators (Cmd-1, Cmd-,) and Cmd-K quick-open — follow-up change.
- Keyboard support inside rendered markdown (link focus order etc.).
- Any Rust/IPC change; any change to the read-only contract.

## Decisions

### D1: DOM-driven roving focus, not a flatten-the-tree refactor

The arrow-key "visible row list" is obtained from the DOM (`querySelectorAll('[role="treeitem"]')` scoped to the tree container — document order equals visual order), not from a flattened data model. *Alternative considered:* refactoring the recursive renderer into a computed flat visible-row array (virtualization-ready, pure-data navigation). Rejected for this change: the recursive structure encodes per-node-type defaults and override-set logic that a parallel flattening walk would duplicate and let drift; flattening is a rendering-architecture change that can happen later without throwing away any of this work.

### D2: Focus lives in the DOM; React holds no focus state at all

Arrow keys call `.focus()` imperatively on the next row element; refs hold the current node ID and its ancestor chain. No component state tracks focus — every `Row` renders `tabIndex={-1}` and a post-render effect imperatively promotes exactly one row to `tabIndex={0}` (React never writes `-1` back over it because the vdom value is unchanged), so a focus move produces zero React renders. A single keydown listener on the tree container handles all keys — rows themselves get no individual key handlers. *Alternative:* `aria-activedescendant` (container keeps focus, rows are virtual). Rejected: roving tabindex gets native `:focus-visible` behavior for free — which is exactly the dead CSS we want to activate — and has broader screen-reader support in WKWebView/VoiceOver.

### D3: Selection reaches rows through a store; top-level nodes are memoized

Selection identity is distributed through a tiny external store (`useSyncExternalStore`) instead of a `selectedNodeId` prop threaded through every node component: each `Row` subscribes for its own boolean, so a selection change re-renders exactly the rows whose bit flipped. With the prop threading gone, `RepoNode`/`FlatWorkspaceNode` are `React.memo`-wrapped (their remaining props — view data, override sets, a `useCallback` toggle, a ref-stabilized `onSelect` — hold identity), so App-level selection re-renders skip the entire tree. Expansion toggles still re-render broadly (the override `Set` identity changes), which is a genuine layout change and matches today's cost.

### D4: Follow-focus is a debounced timer, gated to keyboard-originated focus

A 150 ms timer starts when focus lands on a content row; on expiry — re-checking that the row still holds focus and is not already selected — it dispatches the row's own click, so the keyboard path is byte-for-byte the pointer contract. Enter/Space fires immediately and cancels the timer. Grouping rows (workspace/repo/logicalChange/change, plus the Specs artifact row, whose click renders no content) and `aria-disabled` rows never start it; `instance` rows count as content rows (their click opens proposal.md).

Because rows are click-focusable (`tabIndex={-1}`), the timer is additionally gated on input modality: a `pointerDownRef` set in a capture-phase mousedown (cleared on focusin-consume, mouseup, and any keydown) suppresses the timer for pointer-induced focus — otherwise a chevron click would arm it and hijack the detail pane 150 ms after a pure disclosure gesture — and a `restoringRef` suppresses it around the roving effect's programmatic focus restore, so a watcher refresh can never navigate the pane.

### D5: Focus fallback walks a captured ancestor chain

Node IDs embed filesystem paths, so string-trimming an ID at `/` cannot recover ancestry. Instead, when a row receives focus its ancestor chain (root → self, as node IDs) is captured by scanning back through document order for strictly decreasing `aria-level`s — while all the elements still exist. When a views refresh removes the focused row, the post-render effect walks that chain backwards to the nearest surviving ancestor (matching by `data-node-id` dataset compare, never CSS-selector interpolation) and restores focus if it was lost to the document body. Fallback of last resort: the first visible row.

### D6: Typeahead over visible rows' text labels

Single-character buffer (no multi-char accumulation in v1): match `textContent` of the row's primary label, case-insensitive, starting after the current row and wrapping. Rows whose labels are ReactNodes (styled spans) are matched via their rendered text. *Alternative:* multi-character accumulating buffer with timeout (full APG behavior) — deferred; Cmd-K quick-open will own "type a name" properly.

### D7: Dividers — arrow-key resize on the existing separators

`SplitPane`'s dividers get `tabIndex={0}` and a keydown handler: ArrowLeft/ArrowRight adjust the stored width by 16 px (Shift: 64 px), clamped by the same `minRightWidth`/minimum logic as drag; `aria-valuenow/min/max` derive from the same numbers. No pointer-behavior change.

The clamp is directional: a keypress applies only when the clamped result actually moves in the pressed direction. At narrow windows the live maximum can sit *below* the current width (default 900 px window: max sidebar = 320 < initial 340), and a naive clamp would make a "grow" press snap the pane smaller. `aria-valuemax` is floored at the current width so `valuenow > valuemax` (invalid ARIA) can't render; live re-clamping on window resize is deliberately left to the planned SplitPane-hardening change.

### D8: Escape at the App level

One `keydown` listener (added in `App.tsx`) handles Escape: if Settings or Archive is open, close it. Local consumers win by `stopPropagation` — the workspace-rename input is the only control that binds Escape today (abandon edit + blur), and it did **not** previously stop propagation, so this change adds `e.stopPropagation()` to that branch. No focus-context bookkeeping is needed.

### D9: Swatches implement the full radio-group contract

`role="radiogroup"` already exists (SettingsView.tsx); each swatch button changes `aria-pressed` → `role="radio"` + `aria-checked`, and gets a ≥24 px hit area via an invisible `::after` extension without changing the 18 px visual. The role swap carries its keyboard contract with it — a radio group is one tab stop with arrow movement, not nine tab stops: the checked swatch (the "No tint" radio when no colour is set, so exactly one always) is the group's only `tabIndex={0}`, and ArrowLeft/Right/Up/Down on the group move focus and select with wrap.

### D10: Memoization must not freeze wall-clock labels

`formatRelativeTime` reads `Date.now()` at render, and pre-change the labels were incidentally refreshed by every App-level re-render — which D3's memoization deliberately stops. The mtime label is therefore a self-ticking `RelativeTime` leaf component owning a 60 s interval, re-rendering only itself, preserving the memo win at pre-change freshness.

### D11: Escape-abandon in the rename input must actually abandon

The workspace-rename input's Escape branch resets the draft and blurs — but `blur()` dispatches synchronously before React flushes the reset, so the commit-on-blur closure still sees the abandoned draft and would persist it (pre-existing latent bug, promoted to load-bearing by the Escape feature). An `abandoningRef` set before the blur tells the blur handler to stand down for that one event.

## Risks / Trade-offs

- **[DOM-order dependency]** D1 assumes `querySelectorAll` order equals visual order — true for the current nested rendering; a future CSS reordering (e.g. `order`) would silently break traversal. → Mitigation: comment the invariant at the query site; the future flatten refactor removes it entirely.
- **[Memo correctness]** `React.memo` with object-ish props (`label: ReactNode`) can miss updates if parents pass fresh nodes each render. → Mitigation: memoize at the node-component level (props are views data + booleans), not just `Row`; spot-check with React DevTools highlighting during review.
- **[VoiceOver tree support in WKWebView]** ARIA tree semantics render acceptably in Safari/VoiceOver but quirks exist (level announcement). → Mitigation: explicit `aria-level` on every row rather than relying on `role="group"` nesting inference; manual VoiceOver pass as a task.
- **[Follow-focus vs. slow disks]** 150 ms settle still allows a fast arrow sequence to queue a couple of reads. → Acceptable: reads are local-file IPC, and the debounce cancels strictly on every move; Enter remains the deterministic path.
- **[Escape conflicts]** Future modal-ish surfaces (e.g. season recap overlay) may also want Escape. → Convention established here: innermost surface binds locally and stops propagation; the App-level handler is the outermost fallback.

## Open Questions

None blocking. (Whether grouping-row *pointer* clicks should also toggle expansion — the review's "row-click consistency" item — is intentionally left out; this change only adds the keyboard path. A separate change can align pointer behavior.)
