# Detect Date-Prefixed Archive Directories

## Why

When the watcher re-parses a workspace and finds a change has left the active set, it decides *archived vs. deleted* with a single existence check (`crates/openspec-core/src/watcher.rs:511-512`):

```rust
let archive_path = workspace.uri.join("openspec/changes/archive").join(removed);
if archive_path.is_dir() {
    // emit ChangeArchived + record the archival achievement
}
```

`removed` is the change's logical id (e.g. `add-dark-mode`), but the archive tooling moves a change to `openspec/changes/archive/<YYYY-MM-DD>-<id>/` — a **date-prefixed** directory (the repo's own archive shows `archive/2026-06-02-two-line-change-rows/`). So `archive/<id>` essentially never exists, the `is_dir()` check fails, and an **archival is silently misclassified as a deletion**. The consequence: no `ChangeArchived` cache event is emitted, and the archival achievement is never recorded on the live path.

How visible this is depends on the workspace:

- **Git-backed workspaces are mostly shielded.** The desktop notification rides on `LogicalChangeArchived`, which the aggregator derives from the active-set diff (independent of this check), so users still get notified; and the activity-log archival is backfilled by `reconcile_lifecycle` from git history.
- **Flat (non-git) workspaces have no such backstop.** The `activity-log` capability's *Git Backfill of Historical Achievements* requirement explicitly states a non-git workspace "relies on live capture going forward" — but live capture is exactly what this bug breaks. So a flat workspace's archival **silently fires no notification and records no "shipped" achievement.**
- Across **all** workspaces, the raw `change-archived` Tauri event is effectively dead for the normal dated-archive flow. The tree still stays correct (the always-emitted `Updated` event drives a refetch), but anything keyed on `change-archived` specifically is starved.

The capability specs already mandate the correct behaviour (*"a re-parse shows a change moved to the archive → a change-archived achievement is recorded"*). This is an implementation bug; the fix makes the detection honour the archive tooling's naming convention, and pins that convention in the spec so it can't regress.

## What Changes

- **Archival detection becomes date-tolerant.** A removed active change `<id>` is classified as *archived* when a directory exists under `openspec/changes/archive/` whose name is either the bare `<id>` or the dated form `<YYYY-MM-DD>-<id>`. Implementation builds the set of archived logical ids once per batch — read the archive directory, strip an optional leading `YYYY-MM-DD-` from each entry — and tests membership, rather than stat-ing an exact `<id>` path per removed change.
- **Matching stays exact after the date strip.** The prefix is stripped only when the leading 11 characters form a valid `YYYY-MM-DD-`, and the remainder must equal `<id>` exactly — so an unrelated archive like `2026-06-04-x-foo` never matches id `foo`. (A naive `ends_with("-foo")` would; this avoids that.)
- **Downstream emission is unchanged.** On a positive match the watcher emits `ChangeArchived` and records the archival achievement exactly as it does today; a removal with no matching archive directory stays classified as a deletion and stays silent.
- **No new event types, IPC, or frontend changes.** Pure detection-logic correction in `openspec-core`.

## Capabilities

### Modified Capabilities

- `activity-log`: *Achievement Detection from Watcher Re-Parses* — the requirement is clarified to define the archived-vs-deleted classification and to require recognising the dated `archive/<YYYY-MM-DD>-<id>` directory name, with two new scenarios pinning the dated-archive and the no-archive-directory (deletion) cases.

## Impact

- **Spec:** one requirement modified in `openspec/specs/activity-log/spec.md` (clarifying prose + two added scenarios). No other capability's intent changes — `tray-indicator`'s *Desktop Notification on Archive Transition* is already correct in wording; this fix simply makes the implementation finally satisfy it for flat workspaces.
- **Code:** `crates/openspec-core/src/watcher.rs` — the archive-vs-delete branch in `handle_events` (the `archive_path.is_dir()` check at line 511) becomes a date-tolerant membership test, plus a small helper to strip a `YYYY-MM-DD-` prefix (watcher-local, or shared in `parser`). No type/IPC/boundary changes; no Tauri-shell or frontend changes.
- **Behaviour delta:** flat-workspace archival now fires a desktop notification and records a "shipped" achievement; the `change-archived` event fires for the normal dated-archive flow across all workspaces. Git-backed notifications already worked (via logical events) and are unaffected. Deletions remain silent, as before.
- **Risk:** low — restores intended behaviour with no contract change. The only new false-positive surface is an archive entry coincidentally matching `<YYYY-MM-DD>-<id>`, neutralised by the strict date-prefix-then-exact-id match.

## Out of Scope

- The broader **blind-watcher class** — filesystems where the native notify backend delivers no events at all (WSL/`\\wsl$`/DrvFs, network mounts, some Docker binds), with no poll-watcher fallback or refetch-on-focus recovery. That is a separate, larger reliability change.
- The `change_lifecycle` / `archive_change_name` git-history path, which keeps the dated directory name as the change name when reconstructing history — a reconcile-side naming concern independent of live watcher detection.
- Any change to *which* transitions notify (new-change and final-archive only) or to the `Updated`-never-notifies rule.
