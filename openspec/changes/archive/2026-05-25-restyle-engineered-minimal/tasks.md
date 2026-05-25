## 1. Tokens — establish the design-token layer

- [x] 1.1 In `src/App.css`, declare the full color token set (`--bg`, `--surface`, `--surface-2`, `--border`, `--border-strong`, `--text`, `--text-muted`, `--text-faint`, `--accent`, `--accent-hover`, `--accent-tint`, `--ok`, `--warn`) on `:root` with the light-scheme hex values from design.md §D3.
- [x] 1.2 Add a `@media (prefers-color-scheme: dark)` block on `:root` that overrides the neutral tokens (`--bg`, `--surface`, `--surface-2`, `--border`, `--border-strong`, `--text`, `--text-muted`, `--text-faint`) with the dark-scheme hex values. Accent and status tokens remain unchanged.
- [x] 1.3 Declare the type-size token set on `:root` (`--text-xs`, `--text-sm`, `--text-base`, `--text-md`, `--text-lg`, `--text-xl`, `--text-2xl`) and the type-family tokens (`--font-ui`, `--font-mono`) using the system fallback stacks for now (Inter/JetBrains land in §3).
- [x] 1.4 Declare the line-height tokens `--leading-tight: 1.4`, `--leading-prose: 1.65`, `--leading-code: 1.5` on `:root`.
- [x] 1.5 Declare the space tokens (`--space-1` through `--space-7`) and radii tokens (`--radius-sm`, `--radius`, `--radius-md`) on `:root`.
- [x] 1.6 Confirm `color-scheme: light dark` remains declared on `:root` so native scrollbars/form controls adapt.

## 2. CSS refactor — rewrite every rule to consume tokens

- [x] 2.1 Rewrite the `.split-pane`, `.split-pane-left`, `.split-pane-divider`, `.split-pane-right` rules to use `--border` and `--surface-2` (no inline rgba).
- [x] 2.2 Rewrite the tree rules: `.tree-row` (use `--space-*` for padding, `--radius`), `.tree-row:hover` (use `--surface-2`), `.tree-row.selected` (apply selection model from §4).
- [x] 2.3 Rewrite the chevron/icon/row-label rules to use `--text-faint` / `--text-muted` and `--text-sm`/`--text-xs`.
- [x] 2.4 Rewrite the row-meta family (`.row-count`, `.row-changeid`, `.row-branch`, `.row-progress`, `.row-mtime`, `.row-active-dot`, `.row-divergence*`) to use `--font-mono` and the new outlined-chip rules from §5.
- [x] 2.5 Rewrite the empty-state, detail-pane-status, detail-pane-error rules with tokens.
- [x] 2.6 Rewrite the `.markdown-view` rules: container width, `--text-lg` body with `--leading-prose`, headings stepped from `--text-xl`/`--text-2xl`, `--border` for `hr` and `blockquote`, inline `<code>` to the outlined-chip style (transparent bg, `--border` outline, `--radius-sm`).
- [x] 2.7 Rewrite the `.hljs-*` highlight colors to harmonize with the indigo + cool-neutral palette (no chrome changes — just retune hue/lightness so colors don't clash).
- [x] 2.8 Rewrite the settings rules (`.settings-view`, `.settings-header`, `.settings-section`, `.settings-help`, `.workspaces-list`, `.workspace-row`, `.btn-primary`, `.btn-remove`, `.settings-toggle-row`, `.settings-error`) so workspace rows share padding, dividers, and selection treatment with tree rows (uniform row grammar).
- [x] 2.9 Delete the legacy custom-property definitions `--row-hover`, `--row-selected`, the old `--text-muted` literal, `--divider`, `--divider-hover` from the top of `App.css`. Grep the codebase to confirm no remaining references.
- [x] 2.10 Grep `src/App.css` for `rgba(127, 127, 127` and `rgb(0, 122, 255)` and confirm zero hits.

## 3. Fonts — vendor Inter + JetBrains Mono locally

- [x] 3.1 Create `src/assets/fonts/` directory.
- [x] 3.2 Download Inter Variable woff2 (saved as `src/assets/fonts/InterVariable.woff2`, latin-subset from `@fontsource-variable/inter@5` via jsDelivr — 48KB).
- [x] 3.3 Download JetBrains Mono Variable woff2 (saved as `src/assets/fonts/JetBrainsMono-Variable.woff2`, latin-subset from `@fontsource-variable/jetbrains-mono@5` via jsDelivr — 40KB). Total payload ~88KB, within the 80-120KB design target.
- [x] 3.4 `src/fonts.css` declares both `@font-face` blocks with `font-display: swap` and `font-weight: 100 900`, imported from `src/main.tsx` before `App.css` so vite hashes the woff2 files at build.
- [x] 3.5 `--font-ui` and `--font-mono` updated in `App.css` to lead with `"Inter Variable"` and `"JetBrains Mono Variable"` respectively, with metric-compatible system fallbacks.
- [x] 3.6 `bun run build` succeeds in 2.61s. `dist/assets/InterVariable-Dx4kXJAl.woff2` (48KB) and `dist/assets/JetBrainsMono-Variable-B9CIFXIH.woff2` (40KB) both present with hashed names.
- [x] 3.7 Sanity-check the running app in `bun tauri dev`: confirmed visually — Inter + JetBrains Mono swap in without layout shift.

## 4. Selection model — 2px accent bar across all list surfaces

- [x] 4.1 In `App.css`, define the canonical `.tree-row.selected` (and any equivalent settings/list-row selector) with `border-left: 2px solid var(--accent)`, `background: var(--accent-tint)`, and adjusted left padding so content does not shift between selected and unselected states.
- [x] 4.2 Define the canonical hover state with `background: var(--surface-2)` only (no left bar, no accent tint).
- [x] 4.3 Add a `:focus-visible` rule on rows with `outline: 2px solid var(--accent); outline-offset: -2px;` for keyboard navigation.
- [x] 4.4 Confirm `WorkspaceTree.tsx` applies the `selected` class identically across logical-change rows, instance rows, and artifact rows (no per-level overrides). All `<Row>` instances use the same `tree-row${isSelected ? " selected" : ""}` className builder — confirmed.

## 5. Outlined chip badges + status dots

- [x] 5.1 Define a `.chip` base rule in `App.css`: `display: inline-flex`, `align-items: center`, `padding: 0 var(--space-1)`, `border: 1px solid currentColor`, `background: transparent`, `border-radius: var(--radius-sm)`, `font-family: var(--font-mono)`, `font-size: var(--text-xs)`, `text-transform: uppercase`, `letter-spacing: 0.05em`, `line-height: 1.5`.
- [x] 5.2 Define `.chip--warn` setting `color: var(--warn)` and `.chip--muted` setting `color: var(--text-muted)`.
- [x] 5.3 `SettingsView.tsx`'s `badge-missing` span is now `<span className="chip chip--warn">missing</span>`. The legacy `.badge-missing` rule never existed in App.css — it was a Tailwind-style "intended" class. The outlined-warn chip rule now governs the visual.
- [x] 5.4 `DivergenceChip` in `WorkspaceTree.tsx` now renders `<span className="chip chip--warn">diverged</span>` or `<span className="chip chip--muted">stale</span>`. The 4px collapsed `.status-dot` form is available in the design system but not yet needed — the chips render compactly enough at current row widths. Easy to swap to `<span className="status-dot status-dot--warn" />` later if rows shrink.
- [x] 5.5 Define `.status-dot` in CSS: `display: inline-block`, `width: 4px`, `height: 4px`, `border-radius: 999px`, `background: currentColor`, `margin: 0 var(--space-1)`.
- [x] 5.6 `WorkspaceTree.tsx` audit: `DivergenceChip` uses `chip`; active-row uses `status-dot status-dot--ok`. `SettingsView.tsx` audit: workspace `missing` indicator uses `chip chip--warn`. No bespoke pill styling remains.

## 6. Icon component set

- [x] 6.1 Created `src/components/icons.tsx` exporting `ChevronRight`, `ChevronDown`, `Settings`, `Close`, `DotFilled`, `DotOutline`, `Check`, `CheckSquare`, `Square` — all 24×24 viewBox, 1.5 stroke, `currentColor`, `width`/`height` props (default 14).
- [x] 6.2 `WorkspaceTree.tsx` chevron span now renders `<ChevronDown />` / `<ChevronRight />`.
- [x] 6.3 `WorkspaceTree.tsx` active-row `●` replaced with a CSS `.status-dot.status-dot--ok` (4px circle, no SVG needed for a simple dot). Artifact-absent `○` is now `<DotOutline />`.
- [x] 6.4 `App.tsx` settings-toggle button now renders `<SettingsIcon width={16} height={16} />`.
- [x] 6.5 `SettingsView.tsx` close button now renders `<Close width={14} height={14} />`.
- [x] 6.6 `DetailPane.tsx` audited — no placeholder glyphs present; the error chip is plain text inside `<code className="detail-pane-error">`.
- [x] 6.7 `grep -rnP '[▸▾●✕✓○☑☐⚙]' src/ --include='*.tsx' --include='*.ts'` returns zero hits.

## 7. macOS native chrome — vibrancy + hidden inset titlebar

- [x] 7.1 Added `window-vibrancy = "0.5"` to `crates/specforge/Cargo.toml` under `[target.'cfg(target_os = "macos")'.dependencies]`. Standalone crate chosen over `tauri-plugin-window-vibrancy` for tighter control and one less Tauri plugin dependency.
- [x] 7.2 `lib.rs` imports `apply_vibrancy` + `NSVisualEffectMaterial` under `#[cfg(target_os = "macos")]` and calls `apply_vibrancy(&main_window, NSVisualEffectMaterial::Sidebar, None, None)` inside the existing `if let Some(main_window) = …` block. Errors are logged via `eprintln!` and swallowed so an older-OS failure doesn't block launch.
- [x] 7.3 `tauri.conf.json` main-window config now has `"titleBarStyle": "Overlay"`. Tauri ignores this on Windows/Linux.
- [x] 7.4 `src/main.tsx` sets `document.body.dataset.platform = "mac"` before React mount. `App.css` rules: `body[data-platform="mac"]`, `body[data-platform="mac"] .split-pane`, and `body[data-platform="mac"] .split-pane-left` all set `background: transparent` so window-level vibrancy is visible through the sidebar while `.split-pane-right` keeps its `var(--bg)` opaque.
- [x] 7.5 `body[data-platform="mac"] .split-pane-left` rule adds `padding-top: var(--space-6)` for the 32px traffic-light safe-area drop zone.
- [x] 7.6 Smoke test on macOS: confirmed — sidebar shows desktop vibrancy, traffic lights float over the top-left of the sidebar, and dragging from the titlebar region moves the window. Drag required adding `core:window:allow-start-dragging` to `capabilities/default.json` since `core:default` doesn't include it; double-click on the strip toggles maximize via an explicit `getCurrentWindow().toggleMaximize()` call.

## 8. Markdown body typography

- [x] 8.1 In `App.css`, update `.markdown-view` to `font-family: var(--font-ui)`, `font-size: var(--text-lg)`, `line-height: var(--leading-prose)`, `max-width: 760px`.
- [x] 8.2 Update `.markdown-view h1` / `h2` / `h3` / `h4` to use the type-size tokens (e.g. h1 → `--text-2xl`, h2 → `--text-xl`, h3 → `--text-lg` with `font-weight: 600`); keep `border-bottom: 1px solid var(--border)` on h1/h2 for the hairline underline signature.
- [x] 8.3 Update `.markdown-view code` (inline) to use the outlined-chip treatment: `border: 1px solid var(--border)`, `background: transparent`, `border-radius: var(--radius-sm)`, `font-family: var(--font-mono)`, `font-size: 0.88em`.
- [x] 8.4 Update `.markdown-view pre` to `background: var(--surface-2)`, `border-radius: var(--radius)`, `font-family: var(--font-mono)`, `font-size: var(--text-md)`, `line-height: var(--leading-code)`.
- [x] 8.5 Confirm task-list-item checkboxes, blockquotes, tables, and `<hr>` adopt the new `--border` and spacing tokens.

## 9. Verification

- [x] 9.1 `cargo test --workspace` passes: 31 tests across registry, repo_monitor, self_write, watcher, and tray_icon. window-vibrancy compiles cleanly under `cfg(target_os = "macos")`.
- [x] 9.2 `bun run build` passes (typecheck + vite). 506 modules transformed; CSS bundle 11.92KB, JS bundle 498KB, two woff2 assets hashed.
- [x] 9.3 `bun tauri dev` launches; light/dark appearance toggle verified on macOS.
- [x] 9.4 Tree pane visual QA passed: chevron icons render, change IDs and branch names are mono and aligned, MISSING chip is outlined warn, selection shows the 2px accent bar.
- [x] 9.5 Detail pane visual QA passed: prose is Inter at 15px/1.65, headings have hairline underlines, inline code is an outlined chip.
- [x] 9.6 Settings pane visual QA passed: workspace rows share padding/dividers/selection with tree rows; Add Workspace button uses `--accent`.
- [x] 9.7 macOS sidebar vibrancy visible (confirmed by dragging a colorful window behind SpecForge); traffic lights float over the sidebar top-left.
- [x] 9.8 Tray badge and notifications unchanged: `cache-updated` / `change-added` / `change-archived` still fire and the tray title updates.
- [x] 9.9 Cross-platform: Windows/Linux not testable in this session. Code paths are macOS-gated (`body[data-platform="mac"]` CSS rules and `[target.'cfg(target_os = "macos")']` for window-vibrancy), so non-Mac builds inherit the same color/type/selection chrome on solid backgrounds with stock titlebars. Gap documented for the release checklist — needs a real build on Linux/Windows before shipping.
