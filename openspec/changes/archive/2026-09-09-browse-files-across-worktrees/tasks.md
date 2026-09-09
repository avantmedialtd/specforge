## 1. Core: the pooled listing

- [x] 1.1 In `crates/openspec-core`, add the shapes the union returns: a row carrying the root-relative path plus the copies that hold it, each copy naming its worktree. `#[serde(rename_all = "camelCase")]` on every struct; if any is an enum with a struct variant, `rename_all_fields` too — `ArchiveScope` shipped broken without it and no gate caught it.
- [x] 1.2 Implement the pure grouping function: per-worktree path lists in, de-duplicated rows out, keyed on the root-relative path, with a deterministic total order (`workspace-file-browser`: *Union Markdown Listing Across a Repository's Worktrees*).
- [x] 1.3 Adversarial tests for 1.2 that the mutation gate cannot pass vacuously: a path in exactly one worktree, the same path in three, two worktrees whose path sets are disjoint, and a tie in the sort resolved by the stable tie-break. Assert exact row order and exact copy order.
- [x] 1.4 Decide how divergence is determined and write down why. Measure first: size+mtime is nearly free but reports false differences after a checkout; a content hash of 661 files × N worktrees is exact but may dominate the listing. Whatever is chosen, the tree must render from paths alone before divergence is known (`workspace-file-browser`: *The tree renders before divergence is known*).
- [x] 1.5 Tests for 1.4: identical copies are unmarked, differing copies are marked, and a single-copy row is never marked.

## 2. App service: the repository-scoped enumeration

- [x] 2.1 In `crates/openspec-app/src/service.rs`, add the repo-scoped listing: resolve the repository's tracked worktrees from the registry (user-registered **and** discovered), run the existing per-worktree enumeration on each, and group via 1.2.
- [x] 2.2 Authorize by repository identifier through the existing `ensure_registered_repo`, with a test that an unregistered identifier is refused and nothing enumerated (`workspace-file-browser`: *An unregistered repository is refused*).
- [x] 2.3 Degrade per worktree: one unreadable worktree contributes an empty list, never an error for the whole repository. Test it with a deterministic failure — a regular file where the worktree directory should be gives `ENOTDIR` without needing a permissions trick (`workspace-file-browser`: *One unreadable worktree does not blank the listing*).
- [x] 2.4 Add a test pinning that a registry-**discovered** worktree is an acceptable browse root for both enumeration and read. The union depends on it, and `ensure_browse_root` already allows it — pin it so a future tightening cannot silently empty the union.
- [x] 2.5 Run the fan-out off the async runtime as the other filesystem-walking operations do, and confirm by test that nothing it does runs during watcher aggregation.

## 3. IPC: register the new command in all four places

- [x] 3.1 `#[tauri::command]` handler in `crates/specforge/src/commands.rs` — deserialize args, call `AppService`, nothing more.
- [x] 3.2 Add it to `tauri::generate_handler![…]` in `crates/specforge/src/lib.rs`.
- [x] 3.3 Add the `/api/invoke` match arm in `crates/specforge-web/src/dispatch.rs`. Skipping this compiles, passes `tsc`, and fails only at runtime in the browser.
- [x] 3.4 Wrapper in `src/api.ts`.
- [x] 3.5 Hand-mirror the new types into `src/types.ts`, and add a wire-shape test asserting the literal JSON `api.ts` sends and reads — in both directions. `cargo test` builds values in Rust and `tsc` checks the mirror against itself, so nothing else can see a mismatch.

## 4. Frontend: union tree, divergence marker, copy selector

- [x] 4.1 In `src/components/FileBrowserView.tsx`, fetch the repo-scoped union for a Repo group and derive the folder tree from the pooled path list (`workspace-file-browser`: *File Browser Surface*).
- [x] 4.2 Mark rows whose copies differ. The marker states only that they differ — no ranking, no "newer" (`workspace-file-browser`: *A file differing between worktrees is marked*).
- [x] 4.3 Add the per-file copy control: a chooser when the file has several copies, a plain label when it has one. Label by workspace display name, falling back to the worktree basename.
- [x] 4.4 Hold the selected copy in state **separate** from the listing scope, so switching it re-points only the preview and does not refetch the listing or collapse the tree. The archive browser's `selectedUri`/`activeCopy` split is the precedent, and the reason: its scope `onChange` clears the open item.
- [x] 4.5 Open the main worktree's copy first when it holds the file, else the first copy that does. Pin the choice at open time rather than tracking `copies[0]`, so a refresh that reorders copies cannot silently re-point the preview.
- [x] 4.6 Move the document watch with the selected copy (`workspace-file-browser`: *Switching copy moves the watch*).
- [x] 4.7 Point the relative-link resolution base and containment root at the selected copy's worktree, not the repository (`workspace-file-browser`: *Preview Link Handling*). This is a containment boundary — verify the refusal case, not just the success case.
- [x] 4.8 Widen the refresh control to re-run every tracked worktree's enumeration.
- [x] 4.9 Replace the four `view.mainWorktree` browse-root sites: `src/App.tsx:101` and `src/routing/resolve.ts:221` / `:369` / `:437`.
- [x] 4.10 Ensure switching copy forms no Address and no history entry (`view-routing`: *A file address carries no worktree segment*, *Switching copy forms no address*).
- [x] 4.11 Styles for the copy control and the divergence marker in `src/App.css`, following the archive view's copy-row treatment.

## 5. Verification

- [x] 5.1 `bun install && bun run build` once in this worktree before any `cargo` run — `dist/` is gitignored and both `generate_context!` and specforge-web's `RustEmbed` need it at compile time.
- [x] 5.2 `cargo fmt --check` and workspace `cargo clippy -- -D warnings`; both gate CI and neither is in CLAUDE.md's command table.
- [x] 5.3 `cargo test` (workspace), green before 5.4 — a red baseline makes every mutant report as caught.
- [x] 5.4 `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`. Survivors on the dedup key, the copy ordering, or the divergence predicate mean 1.3/1.5 are too weak — add the assertion, never exclude the mutant, never `--baseline=skip`.
- [x] 5.5 `bun run build` (strict `tsc`) and `bun test`.
- [x] 5.6 Manual smoke — start the app yourself, do not ask the user. With two worktrees of one repository: a file in only the feature worktree appears in the repo's tree; a file in both shows a copy chooser; a file differing between them is marked; switching copy re-renders without collapsing the tree or changing the URL.
- [x] 5.7 Smoke the containment boundary: preview a file from the feature worktree containing a relative link whose target exists only there, confirm it opens; then switch to the main worktree's copy and confirm the same link is refused rather than opening a different file.
- [x] 5.8 Confirm a flat workspace still browses exactly as before — one root, no copy control.
- [x] 5.9 Verify through the browser path (`specforge-serve` plus `bun run dev`) as well as the Tauri shell, so a missing `dispatch.rs` arm cannot slip through.
