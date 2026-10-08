## 1. Registration gate and discovery (`openspec-core`)

- [x] 1.1 Change the registration gate in `crates/openspec-core/src/registry.rs`.
  - Replace `RegistrationError::NotAnOpenSpecWorkspace` with `NotOpenSpecOrGit(PathBuf)`. Its message is "neither an OpenSpec workspace nor a git repository (no `openspec/` subdirectory, and not inside a git working tree): {path}".
  - In `register`, resolve `git::git_common_dir` *before* the `openspec/` check.
  - A path with `openspec/` is registered as selected.
  - For a path without it, pick the non-prunable `git::worktree_list(repo_id)` entry whose canonical path is an ancestor of, or equal to, the canonical selected path. Register that worktree root, including when it is already registered or discovered (promotion is unchanged).
  - Reject when no worktree contains the path, or when `git` cannot be invoked.
  - (`workspace-registry`: *Manual Workspace Registration*)
- [x] 1.2 Relax the discovery filter in `discover_and_collect` (`registry.rs:394-428`) and `reconcile_repo` (`registry.rs:241-289`).
  - Skip the `join("openspec").is_dir()` check when the repository has at least one `UserRegistered` entry equal to one of its worktree roots, as listed by the same `worktree_list` call.
  - Keep the check when every user-registered entry of the repository lies below its worktree root.
  - Factor the predicate into one helper so both call sites share it.
  - (`workspace-registry`: *Worktree Auto-Discovery*)
- [x] 1.3 Extend `crates/openspec-core/tests/registry.rs` with a test for each case:
  - a git repository without OpenSpec registers;
  - `repo/docs/` registers `repo/`;
  - a subfolder containing `openspec/` registers as selected;
  - a non-git folder without OpenSpec is rejected, with a message containing both "OpenSpec" and "git repository";
  - a pre-OpenSpec sibling worktree is discovered at registration;
  - `reconcile_repo` adds a worktree without OpenSpec at runtime;
  - a subfolder-registered repository still skips a worktree without OpenSpec and never discovers its own worktree root.

  Update the existing rejection assertions in `tests/registry.rs:37-47` and `crates/openspec-app/tests/workspace_management.rs:43-66`.
- [x] 1.4 Re-check the tests whose comments rely on the old discovery predicate, at `crates/openspec-app/src/service.rs:3783` and `crates/openspec-core/src/repo_view.rs:1805`. Adjust fixtures that implicitly depended on a worktree *without* `openspec/` being ignored, and rewrite the comments.
- [x] 1.5 Fix registering an already-discovered worktree through `AppService::add_workspace`. *Added during implementation.* The fix makes the subfolder scenario reachable from every frontend.
  - **The bug:** promotion returned only newly discovered siblings, so `add_workspace` took `added.first()` as the registered folder. It picked the wrong one, or failed with "register returned no folders". That was a known defect, documented in a `service.rs` test, and the subfolder rule (registering `api/docs` while `api` is discovered) now routes through it.
  - **The fix:** add `WorkspaceRegistry::register_resolved`, which also names the registered or promoted folder. `register` keeps its contract, and `add_workspace` uses the new method.
  - **Tests:** the service test now promotes through `add_workspace`, and `workspace_management.rs` adds a promotion test.
- [x] 1.6 Apply the code-review fixes to the gate and discovery. *Added during implementation.*
  - Resolve the root with `git::worktree_toplevel` (`rev-parse --show-toplevel`, WSL-translated) instead of the deepest ancestor in `git worktree list`. That list names the git store as a submodule's or `--separate-git-dir` repository's main worktree.
  - Refuse a home-directory or filesystem-root repository without OpenSpec as `RegistrationError::TooBroad`.
  - Skip listed roots without a `.git` entry (`is_working_tree`) in `discover_and_collect` and `reconcile_repo`. This replaces the short-lived `WorktreeInfo::is_bare` flag, which has been removed.
  - Tests: `a_separate_git_dir_work_tree_registers_and_its_store_is_never_tracked` and `a_home_directory_or_filesystem_root_is_too_broad_without_openspec`.
  - (`workspace-registry`: *Manual Workspace Registration*, *Worktree Auto-Discovery*)

## 2. Derived OpenSpec presence (`openspec-core` → wire)

- [x] 2.1 Add `has_open_spec: bool` to `RepoView` and to `WorkspaceView::Flat` in `crates/openspec-core/src/repo_view.rs`.
  - Compute it in the warm and the cold (disabled) aggregation paths: any tracked worktree's `<path>/openspec` is a directory for a repository, the folder itself for a flat workspace.
  - It uses `stat` only and spawns no git process.
  - The field serialises as `hasOpenSpec` through the existing `rename_all` / `rename_all_fields`; confirm `rename_all_fields` reaches the struct variant.
  - (`workspace-registry`: *OpenSpec Presence Is Derived on Every Aggregation*)
- [x] 2.2 Mirror `hasOpenSpec: boolean` on `RepoView` and on the `flat` variant of `WorkspaceView` in `src/types.ts`.
- [x] 2.3 Extend `crates/openspec-app/tests/wire_shape.rs` fixtures with a repository and a flat row carrying the new field, so a snake_case `has_open_spec` fails the guard.
- [x] 2.4 Add unit tests in `repo_view.rs`:
  - OpenSpec only on a feature worktree makes the repository `true`;
  - no worktree with OpenSpec makes it `false` with zero active and zero archived changes;
  - a cold row computes the flag without a git process (reuse the existing cold-aggregation spawn-counting harness);
  - a flat folder follows its own `openspec/`.
- [x] 2.5 Add a test in `crates/openspec-app/tests/workspace_management.rs`: registering a git repository without OpenSpec leaves `workspaces.json` as `{ uri, name }` entries only. Also assert its row reports `hasOpenSpec: false`, contributes zero to the badge count, and appears in `workspace_views`.

## 3. Watcher waits for `openspec/` (`openspec-core`)

- [x] 3.1 Change `add_workspace` in `crates/openspec-core/src/watcher.rs`.
  - Choose the watch root and mode when arming: `<ws>/openspec` recursive when it exists, `<ws>` non-recursive when it does not.
  - `build_native_debouncer` and `build_poll_debouncer` take the `RecursiveMode` instead of hard-coding `Recursive`.
  - Record the mode in `WatcherEntry`.
  - (`workspace-registry`: *Filesystem Watching of Registered Workspaces*)
- [x] 3.2 In awaiting mode, make `handle_events` recognise a batch containing `<ws>/openspec` that is now a directory.
  - Re-arm through the idempotent `add_workspace`: rebuild a `WatcherManager` from the upgraded `Arc<Inner>`.
  - Then run the same re-parse → recompute → `Updated` + diff-events path a relevant `openspec/changes/` batch runs. One emission carries the new changes and the flipped `hasOpenSpec`.
  - Ignore every other awaiting-mode event.
- [x] 3.3 Add tests in `crates/openspec-core/tests/watcher.rs`.
  - Creating `openspec/changes/<id>/proposal.md` under a workspace armed without `openspec/` yields `Updated` and the change in the cache.
  - Writes into a nested directory of an awaiting workspace cause no re-parse.
  - Make both deterministic, following the `recompute_gate` pattern rather than wall-clock margins. Assert the invariant rather than a timeout, so slow CI catches still pass.
- [x] 3.4 Compile-check the Windows poll path: `cargo check --target x86_64-pc-windows-msvc -p openspec-core --all-targets` and the matching clippy run with `-D warnings`.
- [x] 3.5 Update the `registry.rs` and `watcher.rs` bullets in `crates/CLAUDE.md`: registration accepts a git working tree and registers its root, and the watcher waits on the root while `openspec/` is absent.

- [x] 3.6 Apply the code-review fixes to the re-arm. *Added during implementation.*
  - The awaiting task calls `arm(…, Arm::IfStillTracked, false)` itself and retries on the next batch if that fails. The detached `openspec_appeared` task is gone.
  - Every new task waits on an install oneshot, then replays first when `needs_replay` says so. The replay therefore never runs beside the workspace's own batches, and never for a workspace removed meanwhile.
  - `add_workspace` records whether its parse saw `openspec/`, so a folder that appears before `arm` is replayed too.
  - `handle_events` writes the cache only while the workspace still has an entry (`WorkspaceCache::contains`).
  - The watcher lock is released before a replaced entry is dropped.
  - Unit tests in `watcher.rs`: the `needs_replay` truth table; a re-arm for a removed workspace installs nothing; a batch for a removed workspace writes and announces nothing; arming over a parse that missed `openspec/` replays it.

## 4. Frontends

- [x] 4.1 Update the copy in `src/components/settings/WorkspacesGroup.tsx`.
  - Picker title: "Choose an OpenSpec workspace or git repository folder".
  - The help text at `:96-99` and the web path-input placeholder or label name both accepted forms.
  - (`product-identity`: *OpenSpec Format References Preserved*; `web-ui`: *Web-Flavoured Workspace Registration*)
- [x] 4.2 Change the tree empty state in `src/components/WorkspaceTree.tsx:682` to "Add an OpenSpec workspace or a git repository from settings.", and the dashboard empty state in `src/components/DashboardView.tsx:505` to "Register an OpenSpec workspace or a git repository from Settings to see your progress here."
- [x] 4.3 Render the marker in `src/components/WorkspaceTree.tsx`, for both Repo rows (`:946-994`) and flat rows (`:1247-1266`).
  - When `hasOpenSpec` is false, render a muted "no OpenSpec" marker in place of the count badge, with `aria-label="No OpenSpec folder"`.
  - Use existing colour tokens and match the badge's footprint so the row height does not change.
  - Keep the leaf, selection and file-browser behaviour.
  - (`spec-browser`: *Workspace Tree Hierarchy*)
- [x] 4.4 Update `crates/specforge-tui`.
  - The add-prompt label asks for "an OpenSpec workspace or a git repository" (`app.rs:840-847`).
  - The Browse tree renders a `hasOpenSpec: false` top-level row in the scheme's dimmed style, with `no OpenSpec` in place of the count.
  - Selecting it shows the one-line explanation in the detail pane, with no artifact tabs.
  - Update the error fixture at `render_tests.rs:645` and add render tests for the dimmed row and the detail line.
  - (`terminal-ui`: *Workspace Management from the Terminal*, *Rows Without OpenSpec in the Browse Tree*)

## 5. Documentation

- [x] 5.1 Update `README.md:95` to describe both accepted folder kinds and the new rejection message.
- [x] 5.2 Update the site docs: `site/pages/docs/workspaces/+Page.tsx` (intro and rejection copy), `site/pages/docs/troubleshooting/+Page.tsx` (the `not-a-workspace` section heading and message) and `site/pages/docs/+Page.tsx:44`. A master push touching `site/**` publishes the live site, so these land only together with the release that ships the behaviour.

## 6. Verification

- [x] 6.1 Run `bun install && bun run build` once in the worktree. Then run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test`.
- [x] 6.2 Run `bun run build` (strict tsc) and `bun test`.
- [x] 6.3 Run `bun run site:build` and `bun run site:test`.
- [x] 6.4 Mutation-test the diff: `git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`. Kill or justify every survivor in `registry.rs`, `repo_view.rs` and `watcher.rs`.
  - *Done 2026-10-08,* split per the macOS `document_watch` baseline note.
  - **`openspec-core`**, run against every test target except `document_watch`: 67 mutants, 53 caught, 13 unviable, 0 timeouts, 1 missed.
    - The miss is `build_poll_debouncer`, a `#[cfg(target_os = "windows")]` body that is compiled out on macOS and Linux. It is excluded in `.cargo/mutants.toml` with that reason, beside `token_from_keychain`.
    - The Windows target is clippy-checked instead.
  - **`openspec-app`:** 1 mutant, unviable (`RegisteredWorkspace` has no `Default`).
  - An earlier partial run surfaced a `reconcile_repo` survivor, `&&`→`||` on the prunable/bare filter. The `is_working_tree` rework removed that line.
- [x] 6.5 Do a manual smoke test with the `specforge-web` debug build, isolated state, driven through `POST /api/invoke` and the browser. Walk these scenarios:
  - register a scratch git repository without OpenSpec: its row shows the "no OpenSpec" marker, and clicking it opens the file browser;
  - register `<scratch>/docs/`: the root is registered;
  - register a non-git folder: the rejection names both forms;
  - run `openspec init` and `openspec new change demo` in the scratch repository: the row flips to a count of 1 with `demo` beneath it, without a restart;
  - add a second worktree on a branch without OpenSpec in an OpenSpec repository: it appears in `worktrees`, and a pull request on its branch links to it.
  - *Done 2026-10-08:* every scenario was walked except the live pull-request link, which needs real pull requests and tokens. Its prerequisite, the pre-OpenSpec worktree appearing in `worktrees`, was verified. The matching rule is unchanged and unit-tested. The `openspec init` step was simulated by moving an `openspec/changes/demo` tree into place.
- [x] 6.6 Run `specforge-tui` against the same state. Confirm the dimmed `no OpenSpec` row, its detail line and the add-prompt copy.
  - *Done 2026-10-08,* through `render_tests::a_repository_without_openspec_is_listed_dimmed_and_explained`, not an interactive TTY. The background job has no terminal. The test drives the real `ui::view` against a real `AppService` and git repository, and asserts the drawn DIM modifier, the label, the detail line and the prompt copy.
