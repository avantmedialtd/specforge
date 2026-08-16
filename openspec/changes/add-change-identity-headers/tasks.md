## 1. Preflight

- [ ] 1.1 Run `bun install` then `bun run build` once in this worktree, so `dist/` exists — both Tauri's `generate_context!` and `specforge-web`'s `RustEmbed` need it at compile time, and its absence surfaces as an opaque proc-macro error rather than a missing bundle
- [ ] 1.2 Start the verification loop per the project's preferred path: a debug `specforge-serve` on a spare port with an isolated `HOME`, plus a scratch workspace registered through `POST /api/invoke`. Re-run `bun run build` before trusting any UI check — the debug build serves `dist/` from disk, so a stale bundle shows pre-change markup
- [ ] 1.3 Record the baseline: `DetailPane` renders `<MarkdownView>` as its root with no wrapper (`src/components/DetailPane.tsx:263-270`), `.markdown-view` is `max-width: 880px; margin: 0 auto` (`src/App.css:1165-1172`), and `.split-pane-right` is the scroll container (`src/App.css:422-424`). Confirm each against the running build before changing it

## 2. Branch resolution (`design.md` Decision 4)

- [ ] 2.1 Add a pure helper — new module under `src/`, not inlined in JSX — that takes a worktree path and the workspace views and returns the branch of the matching `ChangeInstance`, or `null` when no instance matches. Do **not** add a branch field to `ArtifactRenderTarget` (`src/types.ts:610-616`): targets also arrive from URL address resolution where no instance is in scope, so a field populated only on the tree-click path would drop the chip for link-opened artifacts
- [ ] 2.2 Cover the helper with tests: git instance on a named branch → branch; flat workspace path matching no instance → `null`; a worktree path that matches an instance whose `branch` is itself `null` (detached HEAD) → `null`. The mutation gate does not run on a frontend-only diff, so these tests are the only coverage
- [ ] 2.3 Thread the workspace views into `DetailPane` from `src/App.tsx`, where they are already in hand

## 3. Detail-pane header (`spec-browser`: *Change Identity Header in the Detail Pane*)

- [ ] 3.1 In `src/components/DetailPane.tsx`, wrap the returned `MarkdownView` in a container that renders the header above it. Leave the four early-return branches (no target, loading, error, `content == null`) rendering as they do today — the header names an artifact that is being shown, so it has nothing to name in those states
- [ ] 3.2 Render `target.changeId` as the header's identity, in full — no truncation, no ellipsis, no title-substitution. Verify against a change whose directory name exceeds the prose column's width that it wraps or overflows visibly rather than being silently clipped
- [ ] 3.3 Render the resolved branch as an outlined chip following the name, reusing the existing `.row-worktree` treatment. Render no chip when the branch resolves to `null`
- [ ] 3.4 Keep the chip a **sibling** of the name element, never a descendant — with `user-select: all` on the name, a nested chip would be swept into the selection and copied along with it (Decision 2)
- [ ] 3.5 Confirm the header is scoped to the artifact target only: the Dashboard, commit detail, file browser, Archive, and Settings views are separate branches in `src/App.tsx`'s center-pane switch and must be untouched

## 4. Sticky header and anchor compensation (`spec-browser`: *Section and Task Scroll Anchors*)

- [ ] 4.1 Make the header `position: sticky; top: 0` inside `.split-pane-right`, with an opaque background spanning the pane width so scrolled content does not show through
- [ ] 4.2 Publish the header's height as a CSS custom property, set in the same rule that sets its padding, so there is one place to change and the anchor effect never hard-codes a number that drifts
- [ ] 4.3 In `DetailPane`'s scroll-anchor effect (`src/components/DetailPane.tsx:214-218`), read that property and add it to the section offset — currently a bare `16`, which with a sticky header lands the anchored `h2` underneath it
- [ ] 4.4 In the same effect, correct the task centring: it currently centres within `scrollParent.clientHeight`, which overstates the visible box by the header's height. Centre within the effective box instead
- [ ] 4.5 Verify both anchors against a long artifact — click a section row and a task row and confirm each comes to rest fully visible below the header, not under it
- [ ] 4.6 Confirm `findScrollableAncestor` still resolves to `.split-pane-right` after the wrapper is introduced: it walks up from `containerRef` requiring `overflowY: auto|scroll` **and** `scrollHeight > clientHeight`, so a wrapper that accidentally scrolls would capture it and break every anchor

## 5. Archive reading header (`archive-browser`: *Read-Only Artifact Navigation*)

- [ ] 5.1 Add a pure helper that strips the `archive/` prefix from a render target's `changeId`, with tests: a prefixed archive id → the bare dated directory name; an unprefixed active change id → unchanged. `src/components/ArchiveView.tsx:170` is where the prefix is applied
- [ ] 5.2 In `src/components/ArchiveView.tsx`, add the dated directory name to the existing `.archive-header`, alongside the title it already renders at line 186. Show the directory name, not the undated change id — the directory is what exists on disk
- [ ] 5.3 Render no branch chip here: an archived change has no live worktree
- [ ] 5.4 Verify with a real archived change that the displayed name carries no `archive/` prefix and matches the folder under `openspec/changes/archive/`

## 6. File-browser preview path (`workspace-file-browser`: *File Browser Surface*)

- [ ] 6.1 In `src/components/FileBrowserView.tsx`, render `selectedPath` above the preview's `MarkdownView` (line 322). The value is already in state and currently displayed nowhere
- [ ] 6.2 Leave the no-file-selected branch showing its existing `EmptyState` with no path rendered
- [ ] 6.3 Verify with a file under `openspec/changes/<name>/` that the displayed path contains the change directory name

## 7. Selection treatment (`design.md` Decision 2)

- [ ] 7.1 Add a shared class carrying `user-select: all` plus the monospace dense-tier treatment, and apply it to all three identity strings — detail-pane change name, archive directory name, file-browser path
- [ ] 7.2 Verify in the served UI that one click on each selects the **whole** token, and that the platform copy gesture yields exactly that string with no surrounding whitespace, chip text, or punctuation
- [ ] 7.3 Verify the detail-pane case specifically excludes the branch chip from the selection (the failure mode Decision 2 calls out)
- [ ] 7.4 Confirm nothing sets `user-select: none` on an ancestor of the new headers. `.tree-row` does (`src/App.css:553`) but is not an ancestor of the center pane; `.split-pane--resizing` does too, but only for the duration of a divider drag, which is correct — a drag should not sweep a selection across the panes

## 8. Header placement (`design.md` Decision 5)

- [ ] 8.1 Give the header the same `max-width` and centring as `.markdown-view` (`src/App.css:1165-1172`) and the same horizontal padding, so the identity sits directly above the document's first line rather than floating left of the prose column on a wide window
- [ ] 8.2 Check the header at a narrow center pane (drag to `minRightWidth = 320`) and at a wide one, confirming the identity tracks the prose column at both

## 9. Verification

- [ ] 9.1 `bun run build` — type-check plus bundle, clean
- [ ] 9.2 Run the frontend test suite; confirm the new branch-resolution and prefix-strip tests are present and passing
- [ ] 9.3 `cargo test` — expected to be unaffected, since no crate is touched; run it to prove that rather than assume it
- [ ] 9.4 Walk all three surfaces in the served UI: select an artifact of a git change (name + branch chip), an artifact of a flat-workspace change (name, no chip), an archived change, and a file in the browser
- [ ] 9.5 Verify the copy path in the two hosts that differ: the served UI over loopback, and the served UI over a non-loopback `--bind` on plain HTTP where `navigator.clipboard` is undefined. Both must copy, because neither goes through a clipboard API
- [ ] 9.6 Confirm in the Tauri window (`bun tauri dev`) that the header renders and the selection copies there too
- [ ] 9.7 Re-read the three delta specs against the running build and confirm every scenario is satisfied, including the negative ones — no header on non-artifact targets, no chip on flat or archived changes, no path when no file is selected
