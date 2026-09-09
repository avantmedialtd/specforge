## 1. Core: fix the wire shape

- [x] 1.1 In `crates/openspec-core/src/repo_view.rs`, add `rename_all_fields = "camelCase"` to `WorkspaceView`'s serde attribute, keeping the existing `tag = "kind"` and `rename_all` (those produce the `"kind":"flat"` / `"kind":"repo"` discriminant the TypeScript union matches on). Only `display_name` changes; `workspace`, `changes` and `color` are single-word, `disabled` stays `skip_serializing`, and the `Repo` newtype variant is untouched (`workspace-registry`: *A flat workspace's display name survives the boundary*).
- [x] 1.2 Add a regression test asserting the emitted key set of `WorkspaceView::Flat` — `displayName` present, `display_name` absent. **Verify it fails with the attribute reverted**, rather than assuming it bites: `openspec-core` is mutation-gated and a bare attribute edit is not a mutable line, so this test is the only thing covering the fix.
- [x] 1.3 In `crates/openspec-core/src/watcher.rs`, add the same attribute to `CacheEvent`, whose struct variants carry `change_id` / `repo_id` / `change_name` / `worktree_path`. Comment that this is trap-removal, not a bug fix: the shell translates each variant into an explicit payload, so this type's serialized form does not currently cross the IPC boundary.

## 2. Core: the recurrence guard

- [x] 2.1 Write a helper that walks a `serde_json::Value` recursively — descending into arrays and nested objects — and returns every object key containing `_`, with a path so a failure names the offending field rather than just reporting that one exists (`workspace-registry`: *Every key crossing the boundary is camelCase*).
- [x] 2.2 Add a test that serializes representative values of the types the frontend reads and asserts the helper finds nothing. Roots: `WorkspaceView` (**both** variants), `RegisteredWorkspace`, `DashboardData`, `CommitGraph`, `WorkspaceGarden`, and the quota states. Trace `crates/specforge/src/commands.rs` return types for the full set rather than trusting this list.
- [x] 2.3 Populate every `Option` in those fixtures with `Some` and exercise every enum variant. A `None` field is skipped or emitted as `null` without ever revealing its key, so a fixture full of defaults would let the guard pass while covering nothing. State this in the test's doc comment — it is the guard's real limit and the next person extending it needs to know.
- [x] 2.4 Prove the guard bites: temporarily revert 1.1 and confirm the guard test fails *and names* `display_name`. Restore afterwards.
- [x] 2.5 Prove it descends: temporarily introduce a snake_case key on a NESTED type (not a root) and confirm the guard catches it. Restore afterwards — this is the failure mode a shallow check would miss.

## 3. Documentation

- [x] 3.1 In `crates/CLAUDE.md`, extend the `types.rs` note that already warns about string-valued enums and `kebab-case` to cover this trap: on an enum, `rename_all` renames VARIANTS, and struct-variant fields need `rename_all_fields`. Name both bugs it has caused (`ArchiveScope::Repo`, `WorkspaceView::Flat`) so the warning reads as history rather than theory.

## 4. Verification

- [x] 4.1 `bun install && bun run build` once in this worktree before any `cargo` run — `dist/` is gitignored and both `generate_context!` and specforge-web's `RustEmbed` need it at compile time.
- [x] 4.2 `cargo fmt --check` and workspace `cargo clippy -- -D warnings`; both gate CI and neither is in CLAUDE.md's command table.
- [x] 4.3 `cargo test` (workspace), green before 4.4 — a red baseline makes every mutant report as caught.
- [x] 4.4 `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`. A survivor on the guard helper means 2.1's assertions are too weak — add the assertion rather than excluding the mutant, and never reach for `--baseline=skip`.
- [x] 4.5 `bun run build` — strict `tsc`, then the bundle. Expect no frontend change: `src/types.ts` already declares `displayName`, and the point of the fix is that Rust starts matching it.
- [x] 4.6 Manual smoke — start the app yourself, do not ask the user. Register a **flat** (non-git) folder containing `openspec/`, set a display name AND a tint on it in Settings, and confirm the tree row, the header label, the file-browser label and the reader title all show the new name. Before the fix the tint applies and the name does not, which is the tell.
- [x] 4.7 Confirm a repository row's display name still works, so the enum-wide attribute did not disturb the `Repo` variant.
- [x] 4.8 Verify through the browser path (`specforge-serve` plus `bun run dev`) as well as the Tauri shell — both serialize the same type, and the served web UI is where a wire-shape defect shows first.
