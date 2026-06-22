## 1. Order-preserving registry (no format change)

- [x] 1.1 Promote `indexmap` and `tempfile` to real dependencies of `openspec-core` (`tempfile` is currently dev-only); switch `WorkspaceRegistry.entries` from `HashMap<PathBuf, RegistryEntry>` to `IndexMap<PathBuf, RegistryEntry>`. Leave `ConfigFile` as `{ workspaces: Vec<WorkspaceFolder> }` — no version field.
- [x] 1.2 `load`: build the `IndexMap` in `workspaces.json` array order; compute a dedup key per entry — canonicalised path when resolvable, else a lexically-normalised stored path (handles missing folders) — and dedupe first-wins (keep earliest position). Do not re-sort. Do not write.
- [x] 1.3 `register` appends; `unregister` and `reconcile_repo` use `shift_remove` (order-preserving); `derive_all_discovered` / `discover_and_collect` iterate deterministically (sorted by path) and append discovered entries after the user-registered block.
- [x] 1.4 `save`: emit `entries` in insertion order filtered to `UserRegistered` (the core fix), written **atomically** — `NamedTempFile::new_in(<target dir>)` (random name, no fixed-`.tmp` collision between concurrent instances), flush + fsync the temp, `persist`/rename over the target, fsync the parent dir; clean up the temp on failure; create the parent dir first; surface failures.
- [x] 1.5 Confirm `entries()`, `folders()`, and `repos()` yield entries in config order (iterate `entries` directly; `repos()` dedupes in first-seen order). **Leave `list()` alphabetical** (Settings list deliberately stays name-then-uri sorted; do not change it).

## 2. Data preservation across load + shell bootstrap

- [x] 2.1 Keep `load` write-free: missing file → empty registry, no file created; present empty file (`[]` or no `workspaces` key) → empty registry, file untouched.
- [x] 2.2 Keep the core loader fail-closed on corrupt JSON (parse error surfaces as `InvalidData`, file never overwritten).
- [x] 2.3 Fix the shell bootstrap in `crates/openspec-app/src/service.rs` (currently `WorkspaceRegistry::load(path).unwrap_or_else(|_| WorkspaceRegistry::new(path))`): on a load error, do NOT start with an empty registry that a later save writes over the corrupt file. Move the corrupt file aside to a distinct backup path before initialising empty (or run non-persisting), and surface the condition.
- [x] 2.4 Tests (core): load on missing path and on `{"workspaces":[]}` both yield `is_empty()` and leave the filesystem untouched; malformed JSON returns `Err` and leaves the file byte-identical; duplicate resolvable paths load to one entry at the earliest position; two spellings of a *missing* folder collapse via the fallback key (and two distinct missing paths stay separate); an arbitrary-order file loads in that same order (no re-sort); `save()` is atomic (no torn file; a partway failure leaves the prior file intact).
- [x] 2.5 Test (shell/app): a corrupt config file is preserved (moved aside / not overwritten) and the app does not silently persist an empty registry over it.
- [x] 2.6 Test: load a user-registered file A,B,C in a git repo with extra worktrees; after auto-discovery, `save()` writes exactly A,B,C in order with no discovered paths leaking in.

## 3. Deterministic config-ordered view assembly

- [x] 3.1 Change `aggregate`'s input from two separate `(repos, flats)` vecs to a single ordered, interleaved sequence (e.g. `Vec<ViewInput>` with `ViewInput = Repo(RepoSnapshot) | Flat(WorkspaceFolder, Vec<ChangeData>)`); emit one `WorkspaceView` per element in order. This is the load-bearing change that makes interleaving possible.
- [x] 3.2 Rework `compute_views` to build that ordered sequence by walking the ordered `entries()` once: a `repo_id` claims a slot on first sight of a user-registered entry for it; a user-registered flat claims a slot at its position; skip discovered flats (no `repo_id`). Keep `RepoSnapshot.main_worktree` resolution via `git::worktree_list(..).find(is_main)`; keep `build_repo_view` pure.
- [x] 3.3 Keep discovered worktrees from affecting a repo's top-level position; leave the instance mtime-descending sort unchanged.
- [x] 3.4 Tests: repeated `compute_views` yields an identical top-level order; interleaving — flat@0, repo@1, flat@2 → output flat, repo, flat; a repo is positioned by its earliest user-registered worktree; promotion appends deterministically and does not move the repo slot; discovering a worktree does not shift the top level.

## 4. Verify & polish

- [x] 4.1 Confirm in code that no frontend change is needed (`WorkspaceTree` renders `views` in array order).
- [~] 4.2 Manual spot-check: run the app and confirm the sidebar no longer jumps on edits/focus. The substance (recompute yields an identical top-level order) is proven by `compute_views_preserves_registration_order_deterministically`; the literal in-app visual confirmation needs a foreground session (no display/screen capture in this background run).
- [x] 4.3 `cargo fmt --all`, `cargo clippy --workspace`, `cargo test -p openspec-core` (and the app crate's tests for 2.5).
