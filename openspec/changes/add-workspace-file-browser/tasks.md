# Tasks — Add Workspace File Browser

## 1. Core enumeration (openspec-core)

- [ ] 1.1 In `crates/openspec-core/src/git.rs`, add `markdown_files(worktree: &Path) -> Option<Vec<String>>` beside the other porcelain helpers: run `git_command(GitAnchor::Cwd(worktree), &["ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", ":(icase)*.md"])`, parse the NUL-delimited output, collect through a `BTreeSet<String>` (dedupes repeated unmerged-index entries and yields sorted output), and return `None` on spawn/exit failure following the neighbouring helpers' idiom. Paths come back repo-relative with forward slashes on every platform — return them as-is.
- [ ] 1.2 Add `crates/openspec-core/src/files.rs` (plus `pub mod files;` in `lib.rs`) with `walk_markdown_files(root: &Path) -> Vec<String>` for non-git roots: recursive walk that skips entries whose name starts with `.`, never follows directory symlinks (check `symlink_metadata`), skips the junk-name set `{node_modules, target, dist, build, out, vendor, __pycache__}`, caps depth at 16, keeps only files with a case-insensitive `.md` extension, and returns sorted forward-slash relative paths.
- [ ] 1.3 Tests: unit tests in `files.rs` (tempfile trees: junk and dot-dirs skipped, nested `.md` found, `README.MD` matched, forward-slash output, sorted) and an integration test `crates/openspec-core/tests/files.rs` that `git init`s a temp repo with a `.gitignore`d directory containing markdown, a tracked `.md`, an untracked `.md`, and a non-markdown file, asserting `markdown_files` includes the tracked + untracked markdown and excludes the ignored and non-markdown paths.

## 2. Service and command surface

- [ ] 2.1 In `crates/openspec-app/src/service.rs`, add `AppService::list_markdown_files(root: PathBuf) -> Result<Vec<String>, String>`: via `spawn_blocking`, use `git::git_common_dir(&root)` to pick the backend — git repo → `git::markdown_files` (mapping `None` to a readable error), otherwise `files::walk_markdown_files`. Add `AppService::read_workspace_file(root: PathBuf, rel_path: String) -> Result<String, String>`: reject absolute paths and any `..` component up front, join to `root`, canonicalise both sides with `paths::canonicalize`, require the resolved file to remain under the canonicalised root (rejects symlink escapes), require a case-insensitive `.md` extension, reject files over 5 MiB (metadata check before reading), then read to string — mirroring `read_artifact`'s guard structure.
- [ ] 2.2 In `crates/specforge/src/commands.rs`, add thin `#[tauri::command]` wrappers `list_markdown_files` and `read_workspace_file` delegating to `AppService`, and register both in the `generate_handler!` list in `crates/specforge/src/lib.rs`.
- [ ] 2.3 In `crates/specforge-web/src/dispatch.rs`, add both commands to the `/api/invoke` table with camelCase argument structs (`root`, `relPath`), mirroring the `read_artifact` entry.

## 3. Frontend

- [ ] 3.1 In `src/types.ts`, add `FilesRenderTarget` (`{ kind: "files"; root: string; label: string }`) and extend the `RenderTarget` union.
- [ ] 3.2 In `src/api.ts`, add `listMarkdownFiles(root)` and `readWorkspaceFile(root, relPath)` wrappers via `invokeLogged`.
- [ ] 3.3 In `src/App.tsx` `handleSelect`, replace the early-return for the `repo` and `workspace` cases: `repo` → look up the matching `RepoView` in `views` and set `centerTarget` to `{ kind: "files", root: view.mainWorktree, label: view.displayName ?? view.name }` (no-op if the view is missing); `workspace` → find the flat view by URI for its label and use the workspace URI as root; both fall through to the existing Settings/Archive dismissal. Add the center-pane render branch for `centerTarget?.kind === "files"` → `<FileBrowserView root={…} label={…} />`. The rail re-scoping via `repoIdForSelection` is already in place — leave it untouched.
- [ ] 3.4 Create `src/components/FileBrowserView.tsx`: fetch the listing on mount and when `root` changes; derive the folder tree from the flat path list in a `useMemo` (folders before files, case-insensitive order; folders exist only where files imply them); expansion state local, collapsed by default; filter input doing case-insensitive substring match on relative paths with ancestor folders revealed; selecting a file fetches via `readWorkspaceFile` and renders with `MarkdownView`; a refresh control re-runs the enumeration; `EmptyState` when the listing is empty; readable error states for enumeration and read failures that leave the tree usable.
- [ ] 3.5 Style the browser in `src/App.css` using existing tokens: two-column layout (tree column + preview), header row with the workspace label, filter input, and refresh control, consistent with ArchiveView's visual language.

## 4. Spec sync

- [ ] 4.1 Confirm the deltas at `openspec/changes/add-workspace-file-browser/specs/` (new `workspace-file-browser` capability, modified `spec-browser` requirements) match what was built; adjust if implementation diverged.

## 5. Verification

- [ ] 5.1 `cargo test` passes across the workspace, including the new `files` unit and integration tests.
- [ ] 5.2 `bun run build` passes (strict tsc + vite bundle).
- [ ] 5.3 Visual verification via `bun run wt:dev` on this worktree's slot: repo row click opens the browser rooted at the main worktree with the commit rail still re-scoping; gitignored directories (e.g. `target/`, `node_modules/`) absent from the tree; an untracked draft `.md` appears; the filter reveals a nested file; refresh picks up a newly created file; a read of a deleted-but-listed file shows a readable error with the tree still usable; a registered flat (non-git) workspace opens the browser via the walk backend; opening the browser dismisses an open Settings/Archive pane.
