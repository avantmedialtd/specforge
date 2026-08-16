## 1. Preflight

- [x] 1.1 Run `bun install` then `bun run build` once in this worktree, so `dist/` exists — both Tauri's `generate_context!` and `specforge-web`'s `RustEmbed` need it at compile time, and its absence surfaces as an opaque proc-macro error rather than a missing bundle
- [x] 1.2 Start the verification loop per the project's preferred path: a debug `specforge-serve` on a spare port with an isolated `HOME`, plus a scratch workspace registered through `POST /api/invoke`. Re-run `bun run build` before trusting any UI check — the debug build serves `dist/` from disk, so a stale bundle shows pre-change markup. **Amended:** the background-session isolation guard refuses any command that sets `HOME`, and `config_dir()` (`crates/openspec-app/src/config.rs:31-32`) offers no env override, so state could not be isolated that way. Ran instead against the real config **read-only** — no workspace was registered, unregistered, or renamed; this worktree's own change was already visible in the tree because the repository is registered. Debug server on port 4399
- [x] 1.3 Record the baseline: `DetailPane` renders `<MarkdownView>` as its root with no wrapper (`src/components/DetailPane.tsx:263-270`), `.markdown-view` is `max-width: 880px; margin: 0 auto` (`src/App.css:1165-1172`), and `.split-pane-right` is the scroll container (`src/App.css:422-424`). Confirm each against the running build before changing it. All three confirmed; `.split-pane-right` measured at `scrollHeight 3093 / clientHeight 1987`

## 2. Branch resolution (`design.md` Decision 4)

- [x] 2.1 Add a pure helper — new module under `src/`, not inlined in JSX — that takes a worktree path and the workspace views and returns the branch of the matching `ChangeInstance`, or `null` when no instance matches. Do **not** add a branch field to `ArtifactRenderTarget` (`src/types.ts:610-616`): targets also arrive from URL address resolution where no instance is in scope, so a field populated only on the tree-click path would drop the chip for link-opened artifacts. Added `src/changeIdentity.ts` with `branchForWorktree`
- [x] 2.2 Cover the helper with tests: git instance on a named branch → branch; flat workspace path matching no instance → `null`; a worktree path that matches an instance whose `branch` is itself `null` (detached HEAD) → `null`. The mutation gate does not run on a frontend-only diff, so these tests are the only coverage. `src/changeIdentity.test.ts`, 17 tests including exact-vs-prefix path matching
- [x] 2.3 Thread the workspace views into `DetailPane` from `src/App.tsx`, where they are already in hand

## 3. Detail-pane header (`spec-browser`: *Change Identity Header in the Detail Pane*)

- [x] 3.1 In `src/components/DetailPane.tsx`, wrap the returned `MarkdownView` in a container that renders the header above it. Leave the four early-return branches (no target, loading, error, `content == null`) rendering as they do today — the header names an artifact that is being shown, so it has nothing to name in those states
- [x] 3.2 Render `target.changeId` as the header's identity, in full — no truncation, no ellipsis, no title-substitution. Verified in the running build: header shows `add-change-identity-headers` while the document's own `# H1` reads "Change Identity Headers in the Reading Surfaces", so the directory name is displayed and the title is not substituted for it. `overflow-wrap: anywhere` lets a long name wrap rather than clip
- [x] 3.3 Render the resolved branch as an outlined chip following the name, reusing the existing `.row-worktree` treatment. Render no chip when the branch resolves to `null`. Verified: chip reads `worktree-change-identity-headers`
- [x] 3.4 Keep the chip a **sibling** of the name element, never a descendant — with `user-select: all` on the name, a nested chip would be swept into the selection and copied along with it (Decision 2). Asserted in the DOM (`br.parentElement === name.parentElement && !name.contains(br)`) and behaviourally: a single click selects exactly `add-change-identity-headers`, with no branch text and no surrounding whitespace
- [x] 3.5 Confirm the header is scoped to the artifact target only: the Dashboard, commit detail, file browser, Archive, and Settings views are separate branches in `src/App.tsx`'s center-pane switch and must be untouched

## 4. Sticky header and anchor compensation (`spec-browser`: *Section and Task Scroll Anchors*)

- [x] 4.1 Make the header `position: sticky; top: 0` inside `.split-pane-right`, with an opaque background spanning the pane width so scrolled content does not show through. Verified after a 1200px scroll: header pinned at pane top, name still readable
- [x] 4.2 ~~Publish the header's height as a CSS custom property~~ **Superseded — measure the element instead.** A published constant is wrong precisely when this change's own requirement bites: the name renders in full and wraps on a narrow pane, so the header's height varies at runtime. `DetailPane` holds a ref and reads `offsetHeight` inside the settled double-rAF. Same intent (no drifting hard-coded number), one fewer place to keep in sync. Recorded as an amendment to Decision 3
- [x] 4.3 In `DetailPane`'s scroll-anchor effect (`src/components/DetailPane.tsx:214-218`), add the measured header height to the section offset — currently a bare `16`, which with a sticky header lands the anchored `h2` underneath it
- [x] 4.4 In the same effect, correct the task centring: it currently centres within `scrollParent.clientHeight`, which overstates the visible box by the header's height. Centre within the effective box instead
- [x] 4.5 Verify both anchors against a long artifact. **Amended:** the anchor effect does not fire when a section row is clicked for an artifact that is already open — confirmed **pre-existing** by rebuilding stock `src/` from HEAD and reproducing identical zero-scroll behaviour with no header present, then restoring. Out of scope here; not introduced by this change. The offset fix was therefore verified directly instead, applying each formula to the live document: the old offset (`16`) leaves the anchored `h2` at y=16 against a header bottom of 36 — **20px obscured**; the new offset (`headerH + 16`) puts it at y=52 — **16px clearance**
- [x] 4.6 Confirm `findScrollableAncestor` still resolves to `.split-pane-right` after the wrapper is introduced: it walks up from `containerRef` requiring `overflowY: auto|scroll` **and** `scrollHeight > clientHeight`, so a wrapper that accidentally scrolls would capture it and break every anchor. Verified by replaying the exact walk in the live DOM: `.detail-pane` is `overflow: visible` and skipped; `.split-pane-right` is found

## 5. Archive reading header (`archive-browser`: *Read-Only Artifact Navigation*)

- [x] 5.1 Add a pure helper that strips the `archive/` prefix from a render target's `changeId`, with tests. `changeDirectoryName` in `src/changeIdentity.ts`; tests cover the prefixed and unprefixed forms, leading-only stripping, and exactly-one-prefix
- [x] 5.2 ~~In `src/components/ArchiveView.tsx`, add the dated directory name to the existing `.archive-header`~~ **Superseded — no ArchiveView change needed.** The Archive reader already renders through this same `DetailPane` (`ArchiveView.tsx:234`), so it inherits the identity header. Better than the plan: one implementation, and the identity sits above the artifact rather than in the toolbar. Verified showing `2026-07-04-expose-bitbucket-comment-resolution-filter` with no `archive/` prefix, alongside the untouched `.archive-reading-title`
- [x] 5.3 Render no branch chip here: an archived change has no live worktree. **This needed real work, not just omission.** An archived target's `workspace` is the registered worktree path, which routinely matches a live instance — so branch resolution would have labelled the archived change with its *host* worktree's branch. Added `isArchivedChangeId`, deriving archived-ness from the id itself rather than from a caller-passed flag, since both surfaces share the component. Verified: no chip rendered
- [x] 5.4 Verify with a real archived change that the displayed name carries no `archive/` prefix and matches the folder under `openspec/changes/archive/`

## 6. File-browser preview path (`workspace-file-browser`: *File Browser Surface*)

- [x] 6.1 In `src/components/FileBrowserView.tsx`, render `selectedPath` above the preview's `MarkdownView` (line 325). The value is already in state and currently displayed nowhere
- [x] 6.2 Leave the no-file-selected branch showing its existing `EmptyState` with no path rendered. Verified: zero `.detail-identity` elements in the preview column before a file is selected
- [x] 6.3 Verify with a file under `openspec/changes/<name>/` that the displayed path contains the change directory name. Verified: `openspec/changes/archive/2026-08-14-add-web-ui-touch-support/specs/spec-browser/spec.md`

## 7. Selection treatment (`design.md` Decision 2)

- [x] 7.1 Add a shared class carrying `user-select: all` plus the monospace dense-tier treatment, and apply it to all three identity strings — detail-pane change name, archive directory name, file-browser path. `.identity-name`, with the `-webkit-` prefix for WebKit
- [x] 7.2 Verify in the served UI that one click on each selects the **whole** token, and that the platform copy gesture yields exactly that string with no surrounding whitespace, chip text, or punctuation. Verified on the detail pane: one click → `add-change-identity-headers` exactly, `trimmed === raw`, one range. Computed `user-select: all` confirmed on all three surfaces
- [x] 7.3 Verify the detail-pane case specifically excludes the branch chip from the selection (the failure mode Decision 2 calls out)
- [x] 7.4 Confirm nothing sets `user-select: none` on an ancestor of the new headers. `.tree-row` does (`src/App.css:553`) but is not an ancestor of the center pane; `.split-pane--resizing` does too, but only for the duration of a divider drag, which is correct

## 8. Header placement (`design.md` Decision 5)

- [x] 8.1 Give the header the same `max-width` and centring as `.markdown-view` and the same horizontal padding. **Amended:** implemented as two elements — an outer `.detail-identity` carrying the sticky position and a full-bleed opaque background, and an inner `.detail-identity-inner` carrying the column geometry. A single element cannot do both, since `max-width` would clip the background to the column. Verified: outer 1306px against a 1318px pane, inner 944px exactly matching `.markdown-view`'s 944px
- [x] 8.2 Check the header at a narrow center pane and at a wide one, confirming the identity tracks the prose column at both

## 9. Verification

- [x] 9.1 `bun run build` — type-check plus bundle, clean
- [x] 9.2 Run the frontend test suite; confirm the new tests are present and passing. **213 pass, 0 fail** across 12 files, including the 17 new `changeIdentity` tests
- [x] 9.3 `cargo test` — expected to be unaffected, since no crate is touched; run it to prove that rather than assume it. **Exit 0**
- [x] 9.4 Walk all three surfaces in the served UI: an artifact of a git change (name + branch chip), an archived change (name, no chip), and a file in the browser (full path). All three walked and asserted in the DOM
- [x] 9.5 Verify the copy path where the hosts differ: the served UI over loopback, and over a non-loopback `--bind` on plain HTTP where `navigator.clipboard` is undefined. Ran a second server bound to `192.168.1.32:4400` and confirmed there: `isSecureContext === false`, `navigator.clipboard === undefined`, and the identity still rendering with computed `user-select: all`. The click-to-select gesture itself was proven on loopback; re-running it on the LAN origin returned no selection, but so did a control click on loopback in the same browser session — a harness focus artifact, not an origin difference. `user-select` is presentational CSS with no secure-context gating, and `grep -rn "clipboard\|execCommand" src/` finds **no call at all** (only a CSS comment), so no code path here can vary by origin
- [ ] 9.6 Confirm in the Tauri window that the header renders and the selection copies there too. **Not done — see section 10.** The desktop shell renders the same `dist/` bundle and the same DOM, and `-webkit-user-select: all` is present for WebKit, but this was not asserted in a running Tauri window
- [x] 9.7 Re-read the three delta specs against the running build and confirm every scenario is satisfied, including the negative ones — no header on non-artifact targets, no chip on flat or archived changes, no path when no file is selected

## 10. Outstanding

- [ ] 10.1 **Assert the header in a running Tauri window.** Not verified from this session: driving the native window needs screen-capture access the background session does not have, and the Chrome automation used for every other check cannot reach a `WKWebView`. Risk is low — no Rust changed, `cargo test` passes, the bundle is byte-identical to the one verified in the browser, and the `-webkit-` prefixed property is in the stylesheet — but it is unverified, not verified-by-inference
- [ ] 10.2 **Pre-existing: section/task anchors do not fire for an already-open artifact.** Reproduced on stock `src/` at HEAD with no header present, so it predates this change and is out of scope here. Worth its own change — clicking a section row of the artifact you are already reading silently does nothing
