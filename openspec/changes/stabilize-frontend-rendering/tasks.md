## 1. Baseline

- [ ] 1.1 Run `bun install && bun run build` in the worktree and record the `dist/assets` chunk table (name, raw and gzip size) for comparison in 7.3.
- [ ] 1.2 In a visible `bun tauri dev` window, take a React Profiler baseline of three actions: one live edit to an artifact holding a mermaid diagram, one `review-progress` hunk mark in a pull request of 20 or more files, and one no-op cache batch (touch a file outside any change). Record the commit counts and the components rendered per action.

## 2. Pure helpers (`src/`)

- [ ] 2.1 Add `src/structuralShare.ts` with `shareStructure(previous, next)` per design D3 (`spec-browser`: *Unchanged Workspace Views Keep Their Rows*).
- [ ] 2.2 Add `src/structuralShare.test.ts`. Cover these cases:
  - a fully equal value returns `previous` by identity;
  - one changed leaf in a nested array of objects keeps every sibling's identity and replaces only the path to the leaf;
  - an added key, a removed key, `undefined` versus a missing key, reordered array items, a length change, and `Object.is` primitives (`NaN`, `-0`) are all treated as changes;
  - `null` and primitives at the root.
- [ ] 2.3 Add `src/slotCache.ts` with `createSlotCache()` (`get(key, signature, build)`, `retain(keys)`) per design D5 (`diff-view`: *Host Slot Updates Re-render Only Their File*).
- [ ] 2.4 Add `src/slotCache.test.ts`. Cover these cases:
  - an equal signature returns the cached value without calling `build`;
  - a changed element or a changed length rebuilds;
  - keys are independent;
  - `retain` drops the keys of files that are gone.
- [ ] 2.5 Add `src/mathEngine.ts` with a memoized `loadMath()` (dynamic `rehype-katex` and `katex/dist/katex.min.css`) and `mayContainMath(content)` per design D7. Add `src/mathEngine.test.ts` for `mayContainMath`:
  - `$$` inline and display give true;
  - backtick and tilde `math` fences give true, including one with a space before the info string and one opened inside a list item or a blockquote;
  - single-dollar prose (`$50 and $60`), a `js` fence and a plain document give false.

## 3. Markdown rendering (`src/components/MarkdownView.tsx`, `src/pullRequestMarkdown.tsx`)

- [ ] 3.1 Hoist the `h1`–`h6`, `li`, `input`, `table` and `pre` overrides into module-level components in a frozen `WORKSPACE_COMPONENTS` (`spec-browser`: *Rendered Blocks Survive Document Re-renders*).
- [ ] 3.2 Replace the inline `a` override with a module-level `WorkspaceLink` that reads `MarkdownLinkContext`. The context carries `knownHeadingIds`, `onFragment`, `attemptOpen`, `reportFailure` and `web`. Provide it per render from `MarkdownViewImpl`, and keep classification against the current render's headings (`document-outline`: *Fragment Links Resolve Within the Document*).
- [ ] 3.3 Build `components` as `useMemo` over `WORKSPACE_COMPONENTS` merged with `pullRequestOverrides`, PR overrides last. Confirm that PR headings still carry no `id` (`pull-request-viewer`: *Pull-Request Content Is Untrusted*).
- [ ] 3.4 Move the failure toast into `LinkFailureToast`, which owns `failureCount` and the timer and exposes `report()` via a ref. Make `reportFailure` a stable callback that reads that ref. Keep `role="status"`, the restart-on-repeat key and `LINK_FAILURE_MS`.
- [ ] 3.5 Replace the static `rehype-katex` and KaTeX CSS imports with `mathEngine.ts`. While `mayContainMath(content)` holds and the engine is not loaded, render the last fully rendered content, or an empty container on first render, and re-render when `loadMath()` resolves (`spec-browser`: *Surfaces and Rendering Engines Load on Demand*). Update the `sameMarkdownViewProps` doc comment to say what is now stable.
- [ ] 3.6 Update the comment at the old `pre` arrow, which said fences remount with their siblings, so that no comment describes the remount as intended.
- [ ] 3.7 Extend `src/components/MarkdownView.test.tsx` and `src/pullRequestMarkdown.test.tsx` with these tests:
  - `WORKSPACE_COMPONENTS` is a frozen module constant. The merge used for `components` returns `WORKSPACE_COMPONENTS` itself when there are no pull-request overrides, and returns the same object for the same overrides object. Server renders cannot observe identity across renders, so extract the merge as a pure, exported helper and test that;
  - workspace-mode server markup still carries heading ids and fragment, dangling and external link classes;
  - PR-mode markup still has no heading `id` and no `<img>`;
  - a document with `$$x$$` rendered before `loadMath()` has resolved contains no raw `$$x$$` text, and after `await loadMath()` it renders typeset (KaTeX markup). Order these two cases explicitly, because the loaded flag is module state.

## 4. Workspace views and links (`src/hooks/`, `src/components/WorkspaceTree.tsx`, roots)

- [ ] 4.1 In `src/hooks/useWorkspaces.ts`, apply `shareStructure` to `setViews` and `setWorkspaces` in both `refreshViews` and `refresh`. Keep the first-fetch `loading` flip in `finally` (`view-routing`: *Cold-Load Address Resolution*).
- [ ] 4.2 Add a stable `onViewsRead(listener)` emitter to `UseWorkspacesResult`, fired after every successful views read (from both `refreshViews` and `refresh`) whether or not anything changed (`pull-request-worktree-links`: *An Unchanged Links Snapshot Is Not Re-applied*).
- [ ] 4.3 Rework `src/hooks/usePullRequestLinks.ts` to take `onViewsRead`. Reads are triggered by it and by both provider-update events through `useCoalescedRefetch`, newest-reply-wins via a sequence number, and stored with `setLinks((prev) => shareStructure(prev, next))`. A failed read keeps the held snapshot. Update the two callers, `src/App.tsx` and `src/components/PullRequestWindowRoot.tsx`. `src/components/ReaderRoot.tsx` does not read the links and needs no change here.
- [ ] 4.4 Wrap `LogicalChangeRow` in `memo`. Audit the props of `RepoNode`, `FlatWorkspaceNode` and `LogicalChangeRow` in `WorkspaceTree.tsx` for per-render inline callbacks or objects, and stabilize each one found with `useCallback`/`useMemo` or a latest-ref.
- [ ] 4.5 Extract any pure piece of 4.2/4.3 (emitter, sequence guard) into a testable module. Add `bun test` coverage for these cases:
  - every `onViewsRead` fire leads to a links read even when views are identical;
  - an older reply arriving after a newer one is discarded;
  - an equal reply keeps the previous snapshot's identity.

## 5. Diff view slots (`src/components/DiffView.tsx`, `src/components/PullRequestView.tsx`)

- [ ] 5.1 Add the optional `fileSlots?: (file) => FileSlots | undefined` prop and the `FileSlots` type to `DiffView`. When it is given, derive `slotsByKey`, header extras and preambles from it, and reuse the previous `slotsByKey` `Map` when every entry is `Object.is`-equal. Pass `FileSection` the per-file `headerExtra`, `preamble` and `slots` values in place of render functions. Keep `renderFileHeaderExtra`, `renderFilePreamble` and `hunkSlots` working for other hosts (`diff-view`: *Host Slot Updates Re-render Only Their File*, *Diff View Hosts*).
- [ ] 5.2 Confirm that the `pruneShown` layout effect and the `folds` memo still fire on a per-file fold change and are skipped when `slotsByKey` is reused (`diff-view`: *Folded Hunks*).
- [ ] 5.3 Add the pure `fileSlotSignature(file, …)` to `src/pullRequestView.ts` and `buildFileSlots(file, …)` to `PullRequestView.tsx`, each with a comment naming the invariant and pointing at the other, and route them through a `useRef(createSlotCache())`. The signature covers every input listed in design D5: the file object, its progress entry, its hunk states, pending, hunk pending, including, failed, threads, linked image, the reference's four fields, `detail.row.url`, head and base. Add a small latest-ref hook (none exists in `src/hooks/` today) and call `markFile`, `markHunk` and `includeFile` through it so cached elements never hold stale handlers. Call `retain` with the detail's file keys when `detail` changes (`pull-request-viewer`: *Review Marks Re-render Only the Marked File*).
- [ ] 5.4 Pass `getReviewProgress` replies through `shareStructure` before `setProgress`, so unchanged per-file progress entries keep their identity across reloads and `review-progress-changed` notices.
- [ ] 5.5 Add `src/pullRequestView.test.ts` cases for `fileSlotSignature`:
  - it changes when, and only when, one of that file's inputs changes: the file object, progress entry, hunk states, pending, hunk pending, including, failed, threads, linked image, the reference, the page URL, head or base;
  - an equal progress reply passed through `shareStructure` leaves every file's signature equal;
  - another file's pending mark leaves this file's signature equal.
- [ ] 5.6 Confirm that the commit detail host (`src/components/CommitDetailView.tsx`) passes no slots and renders unchanged.

## 6. Commit graph (`src/hooks/useCommitGraph.ts`)

- [ ] 6.1 Route the initial load and every matching `graph-changed` notice through `useCoalescedRefetch`, created once at the hook's top level, with the load reading `repoId` and `limit` through refs. Apply only the reply whose sequence number is the newest, and only while the effect is current. Set `loading` true only when no graph is held (`commit-graph`: *Overlapping Graph Refreshes Settle on the Newest*).
- [ ] 6.2 Check `src/components/GraphRail.tsx`'s use of `loading` (`loading && !graph`) and confirm that nothing else keys off a refresh's `loading`.
- [ ] 6.3 Extract the sequence guard, shared with 4.5 where it fits, and add a `bun test` case: two overlapping loads resolving in reverse order apply the newer graph.

## 7. Load on demand (`src/main.tsx`, `src/App.tsx`, roots)

- [ ] 7.1 In `src/main.tsx`, import `installHeadPolicies` from `src/windowKind.ts` and keep its synchronous call before `createRoot().render`. Make all four roots `React.lazy` inside a `Suspense` whose fallback is `null`. Move `readerAddressPath` out of `ReaderRoot.tsx` into a shared module so that `PullRequestWindowRoot` does not import the reader root. Drop `PullRequestWindowRoot`'s re-export of `installHeadPolicies`, and reword the placement comments in `main.tsx` and at the top of `windowKind.ts`, which both say every root is imported statically (`spec-browser`: *Surfaces and Rendering Engines Load on Demand*; `pull-request-viewer`: *Pull-Request Window Permissions*).
- [ ] 7.2 In `src/App.tsx`, lazy-load `PullRequestAtAddress`, `CommitDetailView`, `SettingsView`, `FileBrowserView` and `ArchiveView` behind `Suspense` boundaries whose fallback is the detail pane's pending presentation, never the Dashboard. After the first commit, warm those imports and `loadMath()` from a `setTimeout`-scheduled callback, not rAF or `requestIdleCallback` (`spec-browser`: *Master-Detail Layout*).
- [ ] 7.3 Run `bun run build` and compare the chunk table with 1.1. Confirm that the main entry chunk shrank, that separate chunks exist for the PR/diff, commit detail, settings, file browser, archive and KaTeX, and that a reader window's entry graph excludes the dashboard, settings, rail and commit garden. Revert any split that saves nothing measurable.
- [ ] 7.4 Update `src/CLAUDE.md` to match: the *Pull requests ↔ worktrees* paragraph (`usePullRequestLinks(views)` re-reading "whenever `views` changes" becomes the `onViewsRead` signal), the *One diff renderer* paragraph (the `fileSlots` prop beside the render-function slots, and its identity contract), and a note that window roots, the non-default surfaces and the maths engine are lazy chunks.

## 8. Verification

- [ ] 8.1 `bun test` passes, including the new `structuralShare`, `slotCache`, `mathEngine`, links/graph sequencing and `fileSlotSignature` tests.
- [ ] 8.2 `bun run lint` (oxlint, type-aware) passes. Any `react-hooks/exhaustive-deps` directive sits directly above its deps array with the reason on the line above it.
- [ ] 8.3 `bun run build` passes (strict `tsc`, then bundle).
- [ ] 8.4 `cargo fmt --all -- --check` passes.
- [ ] 8.5 `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] 8.6 `cargo test` passes. `dist/` was rebuilt in 8.3, so `specforge-web`'s embed is current.
- [ ] 8.7 Run the mutants command from CLAUDE.md (`git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`). It reports nothing in scope, because no `openspec-core` or `openspec-app` lines change. Record that green here means "not run".
- [ ] 8.8 Smoke-test in a visible `bun tauri dev` (or `bun run wt:dev`) window. Do not use a Chrome MCP automation tab, where rAF, IntersectionObserver and transitions stall. Walk these scenarios:
  - **An edit elsewhere leaves an unchanged diagram drawn.** Keep a handle to the diagram's `<svg>` and check `isSameNode` after a refresh, and confirm that no `.mermaid-block--pending` appears.
  - **A failed link click does not re-render the document.**
  - **A scheme change still redraws the diagram.**
  - **Fragment links follow the headings of the current render.**
  - **A refresh that changes nothing re-renders no row.** Use the Profiler.
  - **One repository's change renders only its rows.**
  - **Marking one hunk leaves other files alone.** Use the Profiler on a PR of 20 or more files, and check that fold, show and hide still keep the reader's place.
  - **A mark from another window renders only that file.**
  - **A refresh does not flash the loading state** (graph rail).
  - **A reader window does not load the dashboard.** Use the devtools network panel.
  - **The pull-request window's policy precedes its code.** Check the head meta order against the first chunk request.
  - **Maths never flashes its source.** Do a cold open of a maths artifact.
- [ ] 8.9 Repeat the scenarios that matter most for the browser transport (diagram survives a refresh, hunk mark, maths) through `specforge-serve` + `bun run dev` with the tab focused. Compare Profiler commit counts with the baseline from 1.2.
