## 1. Core: the preference and its event

Core first, as usual: the settings field and the event name are what both
transports and the frontend name, and they are the only new surface landing in a
mutation-gated crate.

- [ ] 1.1 Add a `DocumentWidth` enum to `crates/openspec-app/src/settings.rs` with variants `Compact`, `Default`, `Wide`, `Full`, `#[serde(rename_all = "camelCase")]`, `impl Default` returning `Default`, and `#[serde(other)]` on a fallback so an unrecognised stored value deserializes to the default rung (`document-width`: *An Unrecognised Reading Width Degrades to the Default*)
- [ ] 1.2 Add the `document_width: DocumentWidth` field to `AppSettings` with `#[serde(default)]`, and a doc comment recording that this is ONE application-wide value rather than per-document or per-window, citing the same reasoning `reader_window` states above it (`document-width`: *Reading Width Is a Selectable Preference*)
- [ ] 1.3 Add a settings test that a file whose `documentWidth` is an unrecognised string loads as `Default` **with the workspace registry and every other field intact** — the failure guarded against is a strict enum turning one unknown string into a total settings-load failure. `cargo mutants` generates whole-function replacements and will not emit a mutant that removes a serde attribute, so this assertion has to be written deliberately (`document-width`: *An Unrecognised Reading Width Degrades to the Default*)
- [ ] 1.4 Add `EVENT_DOCUMENT_WIDTH_CHANGED = "document-width-changed"` to `crates/openspec-app/src/events.rs`, with a doc comment stating — as the neighbouring names do — that it is NOT derived from a `CacheEvent` and is emitted directly by the setter in each transport, so no existing consumer of that stream in three frontends grows an arm that ignores it (`document-width`: *A Reading-Width Change Reaches Windows Already Open*)

## 2. Transport: the command pair in both hosts

All four registration sites, in the order `src/CLAUDE.md` lists them. Miss the
fourth and the command works in `bun tauri dev` and fails at runtime in the
browser with `unknown command`; neither `tsc` nor `cargo` catches it.

- [ ] 2.1 Add `get_document_width` and `set_document_width` to `crates/specforge/src/commands.rs`, taking `State<'_, SharedSettings>`, with the setter emitting `EVENT_DOCUMENT_WIDTH_CHANGED` carrying the new value — following `set_workspace_presentation`'s direct-emit shape
- [ ] 2.2 Register both in the `tauri::generate_handler![…]` list in `crates/specforge/src/lib.rs`
- [ ] 2.3 Add both match arms to `crates/specforge-web/src/dispatch.rs`, the setter pushing the same event name and payload onto the event sender, mirroring the `set_workspace_presentation` arm
- [ ] 2.4 Add the two wrappers to `src/api.ts` and the `DocumentWidth` union plus the `"document-width-changed"` event-name literal to `src/types.ts`, keeping the hand-mirrored Rust/TS shapes matched
- [ ] 2.5 Add an SSE test in `crates/specforge-web/src/sse.rs` that `document-width-changed` appears on the stream, alongside the existing `presentation_event_appears_on_stream` (`document-width`: *A Reading-Width Change Reaches Windows Already Open*)

## 3. Frontend: the preset ladder as data

The ladder is the whole contract and `src/` is outside the mutation gate — a diff
touching only `src/` and `crates/specforge*` short-circuits the Mutants job and
reports green in seconds without running. These tests are the ladder's only
automated coverage, the same reasoning `figureFloor.ts` and `figureZoom.ts` record
in their own headers.

- [ ] 3.1 Add `src/docWidth.ts`: the preset → `{ column, measure }` table for the four rungs, a `normalize` that folds any unrecognised value to `"default"`, and synchronous mirror read/write helpers, every export a total function of its arguments and touching no DOM beyond `localStorage` (`document-width`: *The Preset Ladder Moves Both Tiers Together*)
- [ ] 3.2 Add `src/docWidth.test.ts` asserting: each rung's exact pair; that the prose measure is bounded at every rung including `full`; that the three bounded rungs are monotonic in both tiers; and that an unrecognised value, `undefined`, and `null` all normalise to `"default"` (`document-width`: *The Preset Ladder Moves Both Tiers Together*, *An Unrecognised Reading Width Degrades to the Default*)
- [ ] 3.3 Cover the mirror helpers for the case where `localStorage` read or write throws — a private window or blocked site data — so a startup path cannot be broken by a storage exception (`document-width`: *The Reading Width Is In Effect On the First Paint*)

## 4. Frontend: tokenise the two-tier column

- [ ] 4.1 Define `--doc-column: 880px` and `--doc-measure: 74ch` on `:root` in `src/App.css`, so an unstamped body renders at the default rung rather than at nothing (`visual-identity`: *Markdown Body Adopts the Type System*)
- [ ] 4.2 Add the three `body[data-doc-width="…"]` blocks for `compact`, `wide` and `full`, per the ladder table; `full` sets `--doc-column: none` (`document-width`: *The Preset Ladder Moves Both Tiers Together*)
- [ ] 4.3 Replace the `880px` literal on `.markdown-view` with `var(--doc-column, 880px)`, and the second `880px` literal on `.detail-identity-inner` likewise — retiring the duplicated literal that kept the identity header aligned by hand (`document-width`: *The Reading Width Applies to Every Reading Surface*)
- [ ] 4.4 Replace the `74ch` on `.markdown-view p` and on the `:has()`-guarded `li`/`blockquote` rules with `var(--doc-measure, 74ch)`, leaving the guard's selector untouched (`visual-identity`: *Markdown Body Adopts the Type System*)
- [ ] 4.5 Comment the fallbacks as load-bearing rather than defensive: a `max-width` whose custom property does not resolve is invalid at computed-value time and computes to `none`, which would silently remove both tiers and produce full-bleed text with no error anywhere (`visual-identity`: *Markdown Body Adopts the Type System*)
- [ ] 4.6 Note in the same comment that `--doc-column` can hold the keyword `none`, so no rule may do arithmetic on it — `calc(var(--doc-column) / 2)` would be invalid at `full`

## 5. Frontend: apply the width across surfaces and windows

- [ ] 5.1 Stamp `document.body.dataset.docWidth` from the synchronous mirror in `src/main.tsx`, before `createRoot`, beside the existing `data-platform` and `data-surface` stamps and for the same stated reason — the attribute must be in effect from the first paint (`document-width`: *The Reading Width Is In Effect On the First Paint*)
- [ ] 5.2 Fetch the authoritative value on mount in `src/App.tsx` and reconcile the stamp and the mirror, so a width changed by another instance corrects itself (`document-width`: *The Reading Width Is In Effect On the First Paint*)
- [ ] 5.3 Do the same in `src/components/ReaderRoot.tsx` — it is a separate entry point with its own React root and does not pass through `App` (`document-width`: *The Reading Width Applies to Every Reading Surface*)
- [ ] 5.4 Subscribe both entry points to `document-width-changed` and re-stamp on receipt, so a reader window already open re-lays out without being reopened (`document-width`: *A Reading-Width Change Reaches Windows Already Open*)
- [ ] 5.5 Confirm no per-surface work is needed for the archive reader or the file browser preview — both nest `DocumentView`, which renders the single `.markdown-view`, so the body attribute reaches them already. Record the confirmation rather than adding code (`document-width`: *The Reading Width Applies to Every Reading Surface*)

## 6. Frontend: the Settings section

- [ ] 6.1 Add a `Reading width` section to `src/components/SettingsView.tsx` with the four-way picker, present in both hosts — do NOT place it behind the desktop-only gate that hides notifications and startup, since the width is pure CSS and works identically in a browser tab (`document-width`: *Reading Width Is a Selectable Preference*)
- [ ] 6.2 Render a sample — one paragraph of body prose and one fenced code well — inside a container carrying the rung under consideration, since Settings is a routed view that replaces the document and there is otherwise nothing to judge a rung against (`document-width`: *Reading Width Is a Selectable Preference*)
- [ ] 6.3 Scope the sample's tokens to its own container, never to `body`, so hovering or focusing a rung previews it without applying it to the reading surfaces (`document-width`: *Reading Width Is a Selectable Preference*)
- [ ] 6.4 Add the picker and sample styles to `src/App.css`, reusing the existing `settings-section` and `settings-field` vocabulary; the four-way picker is a new control shape and should be added to that vocabulary rather than styled ad hoc

## 7. Comments left inconsistent by the change

- [ ] 7.1 Update the doc comment in `src/components/figureFloor.ts` that cites the 880px column as fixed — `floorWidth` is a function of natural width and label size and is unchanged, but the prose around it now describes only the default rung, and `full` is precisely the case where less fitting is demanded of it
- [ ] 7.2 Re-read the `.markdown-view` comment about `overflow-wrap: break-word` ("what used to fit in ~104 characters now has ~78") and generalise it — the figure it quotes is the default rung's, and `compact` and `full` change it in both directions

## 8. Verification

- [ ] 8.1 `bun run build` — strict `tsc` plus the bundle; also required before `cargo test` in a fresh worktree, since both `generate_context!` and specforge-web's `RustEmbed` need `dist/` at compile time
- [ ] 8.2 `bun test` — the new `docWidth` tests among them; check the root test-file count still matches expectations given `bunfig.toml`'s `pathIgnorePatterns` for the site's Playwright specs
- [ ] 8.3 `cargo test` for the workspace, `cargo fmt --check`, and workspace clippy with `-D warnings` — the latter two gate CI without appearing in the command table
- [ ] 8.4 `cargo mutants --in-diff` against `origin/master`. Expect it to short-circuit or cover very little: the only gated lines are the settings enum and field. A green report here means "not run", not "covered" — task 1.3 and group 3 are what actually defend this change
- [ ] 8.5 Verify the rendering in the browser loop (`specforge-serve` + a scratch workspace registered via `POST /api/invoke`), asserting computed `max-width` on a paragraph and on a wide table at each of the four rungs, and confirming a wide mermaid diagram scales less at `full` than at `default` on a wide viewport (`document-width`: *The Widest Preset Fills the Surface and Still Bounds Prose*)
- [ ] 8.6 Verify the first-paint requirement specifically: cold-load a surface with a non-default width selected and confirm no reflow — the failure this guards is invisible in a static screenshot (`document-width`: *The Reading Width Is In Effect On the First Paint*)
- [ ] 8.7 Verify an open reader window re-lays out when the width is changed in the main window, which is the one behaviour the mirror alone does not deliver (`document-width`: *A Reading-Width Change Reaches Windows Already Open*)
- [ ] 8.8 Start `specforge-tui` against a settings file carrying a `documentWidth` and confirm it loads and renders unchanged (`document-width`: *The Terminal Frontend Does Not Apply the Reading Width*)
