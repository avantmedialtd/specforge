# Respect Config Order in the Sidebar

## Why

The tree pane (sidebar) renders its top-level rows — repository groups and flat
workspaces — in whatever array order the backend hands it, and that order is
random. Two `HashMap`s drive it: `WorkspaceRegistry.entries`
(`HashMap<PathBuf, RegistryEntry>`) and, inside `compute_views`, `entries_by_repo`
(`HashMap<RepoId, …>`). Rust seeds each `HashMap`'s hasher randomly per instance,
and both maps are rebuilt from scratch on every `get_workspace_views` call. Every
cache event — a change added, a file edited, a focus refresh, the `.git/index`
watcher — triggers the frontend's `refreshViews()`, which recomputes the views
with freshly-seeded maps and therefore a fresh random order. Because the frontend
keys each row by a stable id but renders rows in array order, React's keyed
reconciliation *moves* the DOM nodes: the sidebar visibly jumps around whenever
anything changes.

Crucially, **the on-disk file is already an ordered array.** `workspaces.json` is
`{ "workspaces": [ … ] }`, and `serde` round-trips a `Vec` in element order. The
order is destroyed only in memory (the `HashMap`) and then re-randomised on disk
because `WorkspaceRegistry::save` serialises `entries.values()` — `HashMap` order
again. So there is no on-disk *format* problem to migrate; there is an in-memory
ordering problem leaking into both the UI and the saved file.

That framing matters for the headline requirement: **migrate without ever losing a
registered workspace.** The safe way to "respect the config order" here is the
minimal one — preserve the file's array order in memory and stop re-scrambling it
on save — *not* a new on-disk format that would introduce upgrade/downgrade and
concurrent-write hazards.

## What Changes

- The registry's in-memory store becomes **order-preserving** (`IndexMap`), and
  `save()` emits user-registered entries in that order. The file format is
  **unchanged** (`{ "workspaces": [ … ] }`) — an old build and a new build read
  and write the same shape, so there is **no version field and no upgrade step**
  to get wrong.
- On load, the file's existing array order is **preserved as-is** (it stops
  jumping immediately and becomes canonical on the user's next add/remove). No
  alphabetical sort is imposed, so the order stays the user's and a future
  drag-to-reorder feature can build on it.
- The aggregated view returned by `get_workspace_views` emits top-level rows
  **deterministically in config order**: a repo group sits at the config position
  of its earliest user-registered worktree, flat workspaces sit at their own
  position, the two interleaved to match the config. Recomputing from an unchanged
  registry yields an identical order.
- **Data-preservation hardening** (the point of this change, per the no-data-loss
  requirement):
  - `load()` **never writes** — normalisation is in-memory; order persists on the
    next user-driven save. No eager startup write that could fail on a read-only
    dir or race a concurrent instance.
  - A corrupt `workspaces.json` is **never erased**. The core surfaces the load
    error (fail-closed), and — the part that actually bites today — the
    application shell **must stop silently downgrading that error to an empty
    registry** (`service.rs` currently does `load().unwrap_or_else(new)`, after
    which the next save overwrites the recoverable file with `{}`). The corrupt
    file is preserved (moved aside to a timestamped backup before starting fresh,
    or left untouched and not saved over).
  - Duplicate or non-canonical path spellings are **canonicalised (with a lexical
    fallback for missing folders) and deduped first-wins** on load — collapsed to
    one entry at its earliest position, never dropped to zero.
  - `save()` becomes **atomic** (write to a uniquely-named temp file in the same
    dir, fsync, then rename) so a crash or a concurrent worktree instance (which
    CLAUDE.md documents co-writing app state) can never truncate the registry.

## Capabilities

### Modified Capabilities
- `workspace-registry`: registration order becomes a persisted, honored property;
  the aggregated view's top-level ordering becomes deterministic and config-ordered;
  and the load/save path plus the shell bootstrap gain explicit data-preservation
  guarantees (no write-on-load, corrupt files never erased, dedup-not-drop, atomic
  save).

## Impact

- `crates/openspec-core/src/registry.rs` — `entries` becomes `IndexMap`; `load`
  preserves file order and canonicalises + dedupes first-wins without writing;
  `unregister` / `reconcile_repo` use order-preserving `shift_remove`; discovered
  entries appended deterministically; `save` emits in config order and writes
  atomically; `entries()` / `folders()` / `repos()` yield entries in config order;
  `list()` deliberately keeps its alphabetical sort.
- `crates/openspec-core/src/repo_view.rs` — `compute_views` walks the ordered
  entries once; `aggregate`'s input changes from two separate `(repos, flats)` vecs
  to a single ordered/interleaved sequence so rows follow config position;
  `build_repo_view` stays pure.
- `crates/openspec-app/src/service.rs` — the bootstrap stops collapsing a corrupt
  `workspaces.json` into an empty registry that a later save overwrites; preserve
  the file and surface the failure.
- `crates/openspec-core/Cargo.toml` — promote `indexmap` (already transitive at
  2.14.0) and `tempfile` (currently dev-only) to real dependencies for the
  `IndexMap` store and the atomic save.
- No frontend change required: `WorkspaceTree` already renders `views` in array
  order, so a deterministic backend order fixes the jump.
- `openspec/specs/workspace-registry/spec.md` — modified + added requirements.

## Out of Scope

- A drag-to-reorder UI. This change makes order *stable and honored*; a future
  change can make it *user-rearrangeable* and build on the now-stable order.
- A version/marker field or any on-disk format change — deliberately avoided
  (it would add upgrade/downgrade and concurrent-write hazards for no benefit, see
  `design.md`).
- Full multi-writer correctness (advisory locking / merge across concurrent
  registrations). Atomic save prevents *torn* writes; last-writer-wins between two
  simultaneous registrations is unchanged.
- The within-logical-change instance order (sorted most-recently-modified first)
  is intentional and unchanged.
- The alphabetical Settings list stays as-is (already stable; does not jump).
