## Context

The sidebar's top-level order is produced by `compute_views`
(`crates/openspec-core/src/repo_view.rs`) and consumed verbatim by the frontend
(`WorkspaceTree.tsx` maps `views` in array order, keyed by `repo:<id>` /
`flat:<uri>`). The order flows through non-deterministic `HashMap` hops:

```
workspaces.json            registry (memory)              compute_views
[ A, B, C ]  --load-->  HashMap<Path, Entry>  --entries()-->  HashMap<RepoId, Vec>
  Vec order              random per-instance     .values()      random per-instance
  (ordered!)             hasher seed             (fresh Vec)    hasher seed
                                                                      |
                                                                      v
                                                          Vec<WorkspaceView>  (random,
                                                                               re-rolled
                                                                               every call)
```

The file itself is **not** the problem: `ConfigFile { workspaces: Vec<WorkspaceFolder> }`
(`registry.rs:40-44`) is an ordered array and `serde` round-trips a `Vec` in order.
The randomness is injected entirely in memory — `entries` is a `HashMap`
(`registry.rs:53`) — and then written back to disk in random order because `save()`
iterates `self.entries.values()` (`registry.rs:303-317`). React reconciles the keyed
rows by id, so a reordered array *moves* DOM nodes rather than remounting them: the
visible "jump."

This change is therefore primarily an in-memory fix, with one overriding constraint
from the user: **never lose a registered workspace across the upgrade.** That
constraint reaches past `openspec-core` into the Tauri shell, which currently
defeats the core's fail-closed behavior (Decision 5).

## Goals / Non-Goals

**Goals:**
- Deterministic top-level sidebar order matching the config order of user-registered
  workspaces, stable across recomputations and restarts.
- Migrate **gracefully and losslessly** — no registered workspace is ever dropped,
  corrupted, or silently reset by the upgrade.
- No frontend changes (it already renders array order).

**Non-Goals:**
- A drag-to-reorder UI (future change builds on the now-stable order).
- A version/marker field or any on-disk format change (see Decision 6).
- Full multi-writer correctness — advisory locking / merge (see Decision 7).
- Changing the within-logical-change instance order (mtime-descending stays).
- Changing the alphabetical Settings list (already stable).

## Decisions

1. **Order-preserving registry via `IndexMap`.** Replace
   `entries: HashMap<PathBuf, RegistryEntry>` with
   `IndexMap<PathBuf, RegistryEntry>` (already in the tree at 2.14.0). Near drop-in:
   `get` / `get_mut` / `insert` / `contains_key` / `values` / `iter` carry over and
   iterate in insertion order. Removal uses `shift_remove` (order-preserving, O(n))
   not `swap_remove`; `unregister` (`registry.rs:160-198`) and `reconcile_repo`
   (`registry.rs:205-249`) must use it. N is a handful of workspaces, so O(n) is
   irrelevant.

2. **Config order = `workspaces.json` array order, preserved as-is.** `load`
   (`registry.rs:69-93`) inserts entries in file order into the `IndexMap`;
   `register` appends; `unregister` `shift_remove`s in place; `save` writes
   `entries.values().filter(UserRegistered)` which, with `IndexMap`, is already in
   config order. Discovered worktrees are derived *after* load and appended; they
   are never persisted, so they never perturb the array.

3. **Deterministic view assembly in config first-appearance order — `aggregate`
   signature changes.** Today `aggregate(repos, flats)` (`repo_view.rs:170-187`)
   structurally emits *all* repos then *all* flats, so it cannot interleave. Change
   it to accept a single ordered sequence — e.g. `Vec<ViewInput>` where
   `ViewInput = Repo(RepoSnapshot) | Flat(WorkspaceFolder, Vec<ChangeData>)` — and
   emit one `WorkspaceView` per element in order. `compute_views` builds that
   ordered vec by walking the now-ordered `entries()` once: a `repo_id` claims a
   slot the first time a user-registered entry for it is seen; a user-registered
   *flat* entry claims a slot at its position; the two interleave by slot. Per-repo
   worktree entries are still collected (into an `IndexMap<RepoId, Vec<&entry>>` or
   equivalent) and `RepoSnapshot.main_worktree` is still resolved via
   `git::worktree_list(...).find(is_main)` (`repo_view.rs:227-232`) — only the slot
   *position* comes from the earliest user-registered entry; the group's
   identity/name is unchanged. `build_repo_view` stays pure. Discovered entries
   without a `repo_id` are still skipped (today's `UserRegistered`-only flat filter,
   `repo_view.rs:209-217`).

4. **No write on load — the single most important data-safety rule.** `load()`
   currently never writes; a missing file means an empty registry and no file is
   created. The migration MUST preserve this. Normalisation (dedup, order) happens
   **in memory only**; the canonical order persists naturally on the next
   user-driven `save()` (register/unregister). Writing on load would (a) eagerly
   create `workspaces.json` on a brand-new install, (b) turn a read-only / locked /
   unparented config dir into a *startup failure* where load used to succeed, and
   (c) make every starting instance race to rewrite the file — maximally likely at
   startup, exactly when concurrent worktree dev instances overlap (CLAUDE.md).

5. **Corrupt file is never erased — fix the shell, not just the core.** `load`
   maps a parse error to `io::Error(InvalidData)` and propagates it
   (`registry.rs:73-74`) — correct, fail-closed. But the shell defeats it:
   `service.rs:102-103` does
   `WorkspaceRegistry::load(path).unwrap_or_else(|_| WorkspaceRegistry::new(path))`,
   so a corrupt file silently becomes an **empty** registry, and the next
   user-driven `save()` overwrites the recoverable file with `{}` — the exact data
   loss this change exists to prevent. Fix at the shell: on a `load()` error, do
   NOT start with an empty registry that will be saved over the corrupt file.
   Preferred: **move the corrupt file aside** to a timestamped backup
   (`workspaces.json.corrupt-<n>`) before initialising an empty registry, so the
   app still starts *and* the data is recoverable. (A corruption-triggered
   move-aside is a deliberate, data-preserving exception to "no write on load"; it
   relocates, never deletes.) Acceptable alternative: keep the file untouched and
   run in a non-persisting/degraded mode that refuses to save over it. Either way,
   surface the condition to the user.

6. **No version/marker field; the array IS the schema.** The reframe in Context
   means we do not need a `version` field to distinguish "legacy unordered" from
   "new ordered": switching to `IndexMap` + ordered `save()` makes the file
   self-describing and inherently ordered, read identically by old and new builds.
   A marker would *add* hazards: an older or concurrent build's `save()` drops the
   unknown field and re-scrambles, so the "one-time" migration re-runs forever in a
   version-mixed environment (CLAUDE.md's shared `app_config_dir`). Omitting it is
   strictly safer and loses nothing under the chosen "preserve order" behavior.

7. **Atomic `save()` (unique temp + fsync + rename).** Today `save()` is a
   non-atomic `fs::write` (`registry.rs:316`). A crash mid-write, or a concurrent
   worktree instance writing simultaneously, can truncate or corrupt the only copy
   of the registry — direct data loss. Promote `tempfile` from a dev-dependency to
   a real dependency and use `NamedTempFile::new_in(<same dir>)` (a **random** name,
   so two concurrent instances never collide on a fixed `.tmp`), write, fsync the
   temp file, `persist`/rename over the target, and fsync the parent dir so the
   rename is durable across a crash (otherwise a power-loss can leave a
   renamed-but-zero-length file on some filesystems — the very truncation we are
   preventing). The temp must live in the *same directory* as the target so the
   rename is atomic on one filesystem; clean up the temp on failure; create the
   parent dir first (as `save()` already does). Surface save failures rather than
   swallowing them. This satisfies the spec's "a save that fails partway leaves the
   previous config file intact."

8. **Promotion appends deterministically.** `register` on an existing *discovered*
   entry promotes it to user-registered in place (`registry.rs:117-133`). With
   `IndexMap`, the promoted entry keeps its discovery-time slot (after the loaded
   user-registered block, since discovery runs after load), so `save()`'s
   `UserRegistered` filter places it *after* the existing user-registered entries —
   i.e. promotion behaves like "append," which is consistent and deterministic, and
   stable across restart (it is then written into the array at that position). It
   does not change the repo group's slot, which is the earliest user-registered
   worktree.

9. **Lossless dedup, including missing folders.** Today `HashMap::insert` silently
   dedupes by key. With an `IndexMap`, compute a dedup key per entry on load:
   `paths::canonicalize(uri)` when the folder resolves (`paths.rs` is
   `dunce::canonicalize`, which **fails on a nonexistent path**), else a **lexical
   normalisation** of the stored path (e.g. strip a trailing separator) — because a
   registered-but-missing workspace is a supported state (existing "Missing
   Workspace Handling" requirement) and must still dedupe its spellings. Dedup
   collapses entries whose computed keys are *equal*, keeping the **earliest**
   occurrence's slot; it therefore never removes a genuinely distinct workspace and
   never reduces a duplicate to zero. The fallback uses the stored path, mirroring
   `unregister`'s `canonicalize(..).unwrap_or_else(|_| path.to_path_buf())`
   (`registry.rs:161`), with the added lexical normalisation so two spellings of a
   *missing* folder still collapse.

10. **Determinism scope.** The strong guarantee is over **top-level rows**: their
    order is deterministic regardless of hash-map seeding. Discovered entries are
    appended after the user-registered block; to keep even the internal `IndexMap`
    order reproducible (and avoid relying on `git worktree list` ordering),
    `derive_all_discovered` / `discover_and_collect` (`registry.rs:319-360`) iterate
    repos and worktrees in a deterministic order (sorted by path) rather than
    `HashSet` order. Worktrees are not a tree level (they flatten into mtime-sorted
    instances), so this is for reproducibility, not a user-visible guarantee.

11. **`list()` is intentionally left alphabetical.** The Settings list
    (`registry.rs:255-264`) keeps its `name`-then-`uri` sort. The sidebar (config
    order) and Settings (alphabetical) therefore differ until the first edit — an
    accepted trade-off (Risks). Do **not** "fix" `list()` to config order.

## Risks / Trade-offs

- **`shift_remove` is O(n).** Negligible: a handful of workspaces.
- **`aggregate` contract change.** Replacing `(repos, flats)` with one ordered
  sequence is load-bearing — it is the only change that makes interleaving possible;
  covered by a unit test asserting `flat@0, repo@1, flat@2` → `flat, repo, flat`.
- **First-launch order is arbitrary-but-stable.** A pre-existing scrambled file keeps
  its current order until the first edit; it no longer jumps, but it will not match
  the alphabetical Settings list until then. Accepted per the chosen behavior.
- **Corruption-triggered move-aside is a write.** It is a deliberate exception to
  "no write on load," justified because it relocates (never deletes) the data and is
  the graceful alternative to bricking startup. If even that write is undesirable in
  some environment, the degraded non-persisting mode is the fallback.
- **Atomic save is torn-write-safe, not multi-writer-correct.** It prevents a reader
  or a racing writer from seeing a half-written file; it does not merge two
  simultaneous registrations (last-writer-wins remains). No-write-on-load ensures
  the change does not worsen the existing race.
