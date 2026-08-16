## 1. Shared Identifier-Chip Vocabulary

_No `openspec-core` or `specforge` work precedes these: the change is frontend-only (proposal → Impact), so the usual core → shell → frontend order starts at the frontend._

- [ ] 1.1 In `src/App.css`, add the shared verbatim-identifier chip class carrying the appearance block `.row-worktree` and `.identity-branch` hold identically today — `var(--font-mono)`, `var(--text-2xs)`, `line-height: 1.5`, `color: var(--text-muted)`, `border: var(--border-width) solid var(--border-strong)`, `var(--radius-sm)`, `padding: 0 var(--space-1)`. Keep `border-color` explicit on the base so the untinted chip renders exactly as it does today (design.md D3).
- [ ] 1.2 In `src/App.css`, add the eight palette tint modifiers on the shared class, one `color:` declaration each against `--ws-text-indigo|blue|teal|green|amber|orange|rose|purple`, with the border tracking the ink via `currentColor` so text and border cannot disagree (design.md D3).
- [ ] 1.3 In `src/App.css`, reduce `.row-worktree` to its layout-only properties (`flex: 0 1 auto`, `min-width: 0`, `overflow: hidden`, `text-overflow: ellipsis`, `white-space: nowrap`) and delete the eight `.row-worktree--<colour>` rules now served by 1.2. Reduce `.identity-branch` to `flex: 0 0 auto` (design.md D1).
- [ ] 1.4 Leave `.chip` / `.chip--warn` / `.chip--muted` and `.row-branch` untouched — `.chip` is the uppercase status-badge vocabulary and would corrupt a case-sensitive branch name, and `.row-branch` uses the weaker `--border` deliberately (design.md D2; proposal → Impact).
- [ ] 1.5 In `src/components/WorkspaceTree.tsx`, emit the shared class alongside `row-worktree` on the tree's branch chip and move the tint modifier onto the shared class, leaving the rendered result unchanged (design.md D1).

## 2. Resolving the Owning Workspace's Colour

- [ ] 2.1 In `src/changeIdentity.ts`, widen `branchForWorktree` so the single existing `views → kind:"repo" → active → instances` walk returns the matched instance's `branch` together with its `RepoView.color`, resolved from one match rather than two traversals (design.md D4). Update the doc comment to say the colour is the owning workspace's.
- [ ] 2.2 Update `src/changeIdentity.test.ts`: a matched worktree yields both branch and colour; a repo with `color: null` yields a null colour rather than a derived one (`spec-browser`: *Change Identity Header in the Detail Pane* — "The branch chip stays neutral when no palette colour is configured"); an unmatched worktree path yields neither.
- [ ] 2.3 Update every existing `branchForWorktree` call site for the new return shape — `tsc` runs with `noUnusedLocals`/`noUnusedParameters`, so a missed site fails `bun run build` rather than failing silently.

## 3. Tinting the Header Chip

- [ ] 3.1 In `src/components/DetailPane.tsx`, thread the resolved palette colour into `ChangeIdentityHeader` as a `PaletteColor | null` prop beside the existing `branch`, from the same lookup (design.md D4).
- [ ] 3.2 In `ChangeIdentityHeader`, emit the tint modifier alongside the chip's classes when a colour is resolved, and the base class alone when it is null (`spec-browser`: *Change Identity Header in the Detail Pane*).
- [ ] 3.3 Confirm the archived path needs no colour branch of its own: `isArchivedChangeId` already suppresses the chip entirely, so no chip exists to tint and an archived change cannot be painted in a live workspace's colour (`spec-browser`: *Change Identity Header in the Detail Pane* — "An archived change shows no branch chip"; design.md D4).
- [ ] 3.4 Keep the chip a sibling of `CopyableIdentity`, never a child: `.identity-name` carries `user-select: all`, so a nested chip would be swept into the copied value (`spec-browser`: *Change Identity Header in the Detail Pane* — "The copied value excludes the branch").

## 4. Verification

- [ ] 4.1 `bun install && bun run build` in this worktree — required once before `cargo test` can compile at all, since `dist/` is gitignored and both `generate_context!` and specforge-web's `RustEmbed` need it.
- [ ] 4.2 `cargo test` — expected green and unchanged; the change touches no Rust, so this proves the frontend-only scope claim rather than testing new behaviour. The mutation gate does not apply (nothing in `openspec-core` / `openspec-app` is touched).
- [ ] 4.3 `bun run build` — strict `tsc` must pass, catching any `branchForWorktree` call site missed in 2.3.
- [ ] 4.4 Visual smoke via the browser loop (`specforge-serve` against the freshly built `dist/`, which the debug build reads from disk per request — rebuild before trusting it). The native shell is not under test here, so `bun tauri dev` is not needed. Walk the scenarios: a change in a workspace **with** a palette colour shows a tinted header chip; one **without** shows today's neutral chip; the tree chip and header chip for the same branch render identically; an archived change and a flat-workspace change each still show the name alone.
- [ ] 4.5 Confirm the tree renders unchanged by 1.3/1.5 — compare a tinted tree chip and an untinted one against master, since the tree is refactored for a defect that was never in it (design.md → Risks).
- [ ] 4.6 Confirm the contrast claim rather than assuming it: `.detail-identity` sets `background: var(--surface)`, the same token `--ws-text-*` was tuned against, in both light and dark scheme (design.md → Risks).
- [ ] 4.7 Confirm a click on the change name still copies the name alone, with no chip text in the clipboard or the selection, now that the chip's classes have changed (`spec-browser`: *Change Identity Header in the Detail Pane* — "The copied value excludes the branch").
