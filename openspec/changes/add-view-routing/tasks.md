## 1. Address Model and Pure Codec

- [ ] 1.1 Add `src/routing/address.ts` defining the `Address` union (home, settings, archive, files, artifact) and a `Scope` type distinguishing a flat workspace from a repo instance — identifiers only, no labels or payloads (`view-routing`: *Addressable Viewing State*)
- [ ] 1.2 Add `src/routing/codec.ts` with `encodeAddress(address): string` and `decodeAddress(path): Address | Unresolvable`, implementing the grammar in design.md and depending on no DOM, workspace data, or backend call (`view-routing`: *Address and URL Round-Trip Through a Pure Codec*)
- [ ] 1.3 Ensure `decodeAddress` returns the unresolvable outcome for any path not matching the grammar, never a partially-populated `Address` (`view-routing`: *Address and URL Round-Trip Through a Pure Codec*)
- [ ] 1.4 Add a `"test": "bun test"` script to `package.json` — no new dependency, Bun provides the runner
- [ ] 1.5 Add `src/routing/codec.test.ts` covering the round-trip invariant across every `Address` variant, and rejection of malformed paths

## 2. Slug Identity and Resolution

- [ ] 2.1 Add `src/routing/slug.ts` deriving a workspace or repo slug from its stable registered name, explicitly not from the `displayName` override, and never embedding an absolute filesystem path (`view-routing`: *Workspace Identity Is a Registry Slug*)
- [ ] 2.2 Implement shortest-unambiguous emission in `src/routing/slug.ts`: bare slug when unique, distinguishing suffix on collision, instance segment only when a logical change has more than one instance (`view-routing`: *Shortest Unambiguous Address*)
- [ ] 2.3 Add `src/routing/resolve.ts` mapping an `Address` plus the `WorkspaceView[]` list to one of three outcomes — resolved `RenderTarget`, ambiguous with candidates, or not found — attempting no artifact read or directory listing for an unresolved slug (`view-routing`: *Cold-Load Address Resolution*)
- [ ] 2.4 Add the reverse mapping `renderTargetToAddress(target, views)` in `src/routing/resolve.ts` so selections made by clicking emit the same addresses the codec parses
- [ ] 2.5 Add `src/routing/slug.test.ts` covering derivation stability across a `displayName` change, collision suffixing, single- versus multi-instance emission, and the ambiguous-resolution outcome

## 3. History Adapters

- [ ] 3.1 Add `src/routing/history.ts` defining the `History` interface (`current`, `push`, `replace`, `subscribe`) (`view-routing`: *Host-Detected History Adapter*)
- [ ] 3.2 Implement the browser adapter over `pushState` / `popstate` in `src/routing/history.ts`, reflecting the address in `window.location` (`view-routing`: *Host-Detected History Adapter*)
- [ ] 3.3 Implement the in-memory adapter (entry array plus index) in `src/routing/history.ts` for the desktop shell and tests (`view-routing`: *Host-Detected History Adapter*)
- [ ] 3.4 Select the adapter by host using the existing `isTauri()` / `isWeb()` helpers in `src/api.ts`
- [ ] 3.5 Add `src/routing/history.test.ts` exercising the in-memory adapter's back/forward semantics and subscription notifications

## 4. Shell Wiring

- [ ] 4.1 Add `src/hooks/useAddress.ts` owning the current `Address`, subscribing to the selected history adapter, and exposing `navigate(address, { replace })`
- [ ] 4.2 Rewrite `App.tsx` so `centerTarget`, `showSettings`, and `showArchive` derive from the resolved address rather than independent `useState` values
- [ ] 4.3 Convert `handleSelect`, `selectDashboard`, and the settings/archive footer toggles in `App.tsx` to publish addresses, leaving change disclosure rows and the Specs artifact node without an address (`view-routing`: *Addressable Viewing State*)
- [ ] 4.4 Replace the `handleOpenShip` deep-link and the `archiveSelection` state in `App.tsx` with the archive address carrying its workspace and archive directory
- [ ] 4.5 Remove `label` from `FilesRenderTarget` in `src/types.ts` and re-derive it from `views` in `App.tsx` when rendering `FileBrowserView`
- [ ] 4.6 Apply the history-entry discipline in `App.tsx` and `useAddress.ts`: push on view-changing activation, replace on canonicalisation, and no entry for disclosure toggles, focus traversal, scrolling, filter text, or graph paging (`view-routing`: *History Entry Discipline*)
- [ ] 4.7 Render the pending, ambiguous, and not-found resolution states, ensuring the home surface is not shown while resolution is outstanding (`view-routing`: *Cold-Load Address Resolution*, `dashboard`: *Dashboard Home Surface*)
- [ ] 4.8 Add a desktop-only back/forward keyboard handler in `App.tsx`, gated on `isTauri()` so the served web UI leaves the gestures to the browser (`view-routing`: *Desktop Back and Forward Gestures*)

## 5. Tree Reveal

- [ ] 5.1 Add a pure `addressToNodeId` mapping in `src/routing/` built from the existing compositional id helpers (`repoId`, `logicalChangeId`, `instanceId`, `artifactNodeId`) in `src/components/WorkspaceTree.tsx`, with tests
- [ ] 5.2 Add a transient forced-open set to `WorkspaceTree.tsx` layered above the persisted `collapsed` / `expanded` sets, never written to settings (`view-routing`: *Navigation Reveal Is Transient*, `spec-browser`: *Tree Expansion Has No First-Sight Auto-Expansion Effect*)
- [ ] 5.3 Expose a single imperative reveal entry point on `WorkspaceTree` that opens an addressed node's ancestors and marks it selected, rather than adding effects that react to view changes
- [ ] 5.4 Clear the transient set when the address moves elsewhere so revealed ancestors return to their persisted state (`view-routing`: *Navigation Reveal Is Transient*)

## 6. Served Deep Links

- [ ] 6.1 Verify `crates/specforge-web/src/assets.rs` already satisfies the pinned behaviour — unknown paths return the shell, asset paths are not shadowed, a missing bundle still reports the build hint — and change it only if a gap is found (`web-ui`: *Deep-Link Durability of the Served Bundle*)
- [ ] 6.2 Add coverage in `crates/specforge-web/tests/server.rs` for a deep address returning the shell and an asset path returning its own content type

## 7. Verification

- [ ] 7.1 Run `bun test` and confirm the codec, slug, history, and node-id suites pass
- [ ] 7.2 Run `bun run build` and confirm strict `tsc` (including `noUnusedLocals` / `noUnusedParameters`) and the bundle both succeed
- [ ] 7.3 Run `cargo test` and confirm the workspace is green, including the `specforge-web` server tests (run `bun run build` first — `dist/` must exist for the crate to compile)
- [ ] 7.4 Smoke the desktop shell with `bun run wt:dev`: select artifacts across a flat workspace and a multi-instance repo change, confirm back/forward gestures walk the history, confirm disclosure toggling and tree keyboard traversal create no entries, and confirm a reveal leaves collapse state unchanged after quitting and relaunching
- [ ] 7.5 Smoke the served UI with `specforge-serve`: copy a deep address, reload it, confirm it restores the same artifact with its tree node revealed, confirm the browser's native back/forward move exactly one entry, and confirm a stale address reports not found rather than falling back to the Dashboard
