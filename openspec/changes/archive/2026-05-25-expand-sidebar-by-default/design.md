# Design

## Context

`WorkspaceTree.tsx` currently models expansion state as `expanded: Set<string>` held in `useState`. A `useEffect` keyed on `views` seeds the set with top-level repo/workspace IDs on first sight; everything below stays collapsed until the user clicks. Toggles are local to the React component and are lost on unmount or restart.

The tree's IDs (built by the `repoId`/`logicalChangeId`/`instanceId`/`changeRowId`/`artifactNodeId`/`sectionNodeId`/`taskNodeId` helpers near the top of the file) are deterministic, hierarchical strings derived from stable identifiers — repo IDs, change names, worktree paths, capability names, and section/task indices. Because they're path-prefixed (`repo:X/lc:Y/inst:Z/...`), they survive across sessions as long as the underlying entities haven't been renamed.

`AppSettings` (in `crates/specforge/src/settings.rs`) is a small file-backed struct with one current field (`notifications_enabled`). The `SettingsStore` already exposes a `snapshot()` reader and a single typed setter; adding a second field follows the existing pattern with no architectural change.

The cache layer in `openspec-core` is unaffected — expansion state is pure UI concern and lives entirely above the IPC boundary.

## Goals / Non-Goals

**Goals:**

- Every collapsible tree node is open by default on first encounter.
- The user's collapses survive process restarts.
- Newly-arrived nodes (from `change-added` or `cache-updated` events after initial mount) appear expanded with zero extra effort.
- The implementation removes existing dead/stale code: the seeding `useEffect` and the misleading "Default to expanded" comment on the multi-instance logical-change branch.
- Persistence I/O is bounded and predictable: at most one settings write per user toggle, debounced to coalesce rapid clicks.

**Non-Goals:**

- No "collapse all" / "expand all" affordances in this change. They're a natural follow-up once the new default has been used in anger, but adding them here would inflate scope.
- No per-workspace or per-repo expansion scopes. The collapsed set is global; IDs already encode their parent context.
- No garbage collection of orphaned IDs (entries for workspaces unregistered or changes archived). At realistic scale (~100 bytes per ID, expected dozens of entries) the set stays small for years; sweeping adds code with no observable benefit yet.
- No change to the `openspec-core` crate, its event types, or its cache.
- No persistence of the divergence-label state, active-instance indicator, scroll position, or any other UI state that isn't expansion.

## Decisions

### Invert the state model: track collapsed IDs, not expanded IDs

Today's model defaults everything to closed and opts in to open. The proposal flips the default. Two ways to implement that flip:

**(A) Keep `expanded: Set<string>` and seed it with every ID on every view change.**
Requires recursively walking the view tree on every cache event to top up the set. Has to interact carefully with user-collapses: if I collapse a node and the watcher re-emits, the seeding effect mustn't blindly re-add it. Net result: a complex effect that has to remember "I've seen this ID before" — essentially reimplementing the inverted model with extra steps.

**(B) Replace with `collapsed: Set<string>`. A node is open iff its ID is absent.** — chosen.

Under (B) the seeding effect disappears entirely. New IDs from watcher events are open simply because they aren't in the set. The model matches what the user is actually expressing ("I want these specific things hidden"), and the set stays small (bounded by the user's clicks, not by the tree's size).

### Persist in `AppSettings`, not a separate file

Two options for where the collapsed IDs live on disk:

**(A)** New file (`tree-state.json` alongside `settings.json`).
**(B)** A new field on `AppSettings`. — chosen.

The collapsed set is small (kilobytes at most), changes on the same lifecycle as other app preferences, and is already needed at the same moment the settings load. Splitting it into its own file would mean a second `Mutex`-guarded store, a second IPC command pair, and a second file I/O path — all to avoid co-locating ~100 IDs with `notificationsEnabled` in the same JSON object. Co-location wins.

The new field is serialized with `#[serde(default)]` so existing `settings.json` files (without the field) load cleanly into the new struct.

### Full-set replace IPC, not deltas

The persistence command takes the entire ID array on every toggle:

```rust
set_collapsed_tree_node_ids(ids: Vec<String>) -> Result<(), ...>
```

A delta API (`add`, `remove`) would shrink each IPC payload from ~1KB to ~50 bytes, but at the cost of: two commands instead of one, a server-side mutation of the set that has to be order-tolerant for racing toggles, and a frontend wrapper to translate Set mutations into delta calls. Full-set replace keeps the frontend a single line per toggle and removes any concurrency questions on the Rust side — every write is just "the new truth is this exact array."

A single 150ms debounce on the frontend coalesces rapid open-close-open clicks into one write.

### Hydration ordering and first-paint behaviour

On mount, the frontend reads `AppSettings` via the existing `get_settings` flow and seeds the `collapsed` state. Until hydration completes the tree renders with `collapsed = ∅` (everything open).

That brief mismatch between first paint and hydrated state is acceptable because:
- Settings load is local file I/O, well under one frame at usual sizes.
- The "wrong" state during that gap (everything expanded) is the new default anyway — only nodes the user previously collapsed will visibly snap shut once hydration completes. Most users have collapsed nothing.
- Adding a loading gate (blank tree until settings load) would introduce a visible flash on every cold start for zero correctness gain.

### Toggle handler shape

```ts
const toggle = (id: string) =>
  setCollapsed((prev) => {
    const next = new Set(prev)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    return next
  })
// + a debounced effect that persists `collapsed` after it settles
```

Identical structure to today's `toggle`, just operating on the opposite-polarity set. Every existing `isOpen = expanded.has(id)` call site flips to `isOpen = !collapsed.has(id)`.

### Top-level seeding effect: deleted, not adapted

The current `useEffect` at `WorkspaceTree.tsx:76-88` is the *only* place that auto-opens anything. Under the inverted model it has no job — top-level rows are open because they're not in the collapsed set, same as everything else. Removing it is required, not optional; leaving it in place wouldn't break anything but would be dead code.

The stale comment at `WorkspaceTree.tsx:316` ("Default to expanded — … Promotion (singleton → multi) is handled by useEffect below.") refers to a `useEffect` that never existed. Under the inverted model it becomes literally true ("default to expanded") and the misleading second sentence should be removed.

## Risks / Trade-offs

- **Risk**: First launch after upgrade shows users a dramatically denser sidebar than they're used to.
  → Mitigation: this is the desired outcome of the change; the proposal exists precisely because the current default hides too much. No migration to do — there was no persisted state to convert.

- **Risk**: A user with 30 changes × 5 sections × ~10 tasks renders ~1500 rows in a fully-expanded tree, causing scroll/perf annoyance.
  → Mitigation: the user can collapse what they don't care about, and those collapses now stick. A "collapse all changes" affordance is the natural follow-up if this turns out to bite real users.

- **Risk**: Orphaned IDs accumulate in `collapsed_tree_node_ids` when workspaces are unregistered or changes archived.
  → Mitigation: accepted in this change. At realistic usage the set stays in the dozens of entries; even pathological cases (thousands of orphans) would still be on the order of low hundreds of KB in `settings.json`. Sweeping is a cheap follow-up if the file ever gets noticeably large.

- **Risk**: A node ID format change (e.g., changing how worktree paths get encoded) would invalidate persisted IDs, leaving expand-state stuck.
  → Mitigation: the existing ID scheme is already used as React keys and survived the multi-worktree refactor; treating it as a stable on-disk format is acceptable. If ever changed, the persisted set becomes harmless garbage — the worst case is "some user collapses are forgotten on next launch."

- **Trade-off**: Full-set replace writes the whole JSON file on every toggle. At < 100 collapses, the file is a couple of KB — a non-issue. If the set ever grew to thousands of entries the cost would still be O(few-KB write), debounced, on the user's local SSD.
